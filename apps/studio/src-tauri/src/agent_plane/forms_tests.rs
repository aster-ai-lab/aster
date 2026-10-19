#![cfg(test)]

use super::*;

fn owner(session: &str) -> FormOwner {
    FormOwner {
        session_id: session.into(),
        execution_id: "model".into(),
        attempt_id: 1,
        anchor_message_id: "assistant".into(),
        acp_session_id: "acp".into(),
        prompt_id: "prompt".into(),
        run: None,
    }
}

fn answers(value: Value) -> FormValues {
    serde_json::from_value(value).expect("answers")
}

#[tokio::test]
async fn invalid_question_answers_do_not_consume_the_pending_request() {
    let registry = Arc::new(FormRegistry::default());
    let question = Question {
        question: "选哪份文档？".into(),
        allow_free_text: false,
        choices: vec![super::super::question::Choice {
            id: "question-1".into(),
            label: "B.docx".into(),
        }],
    };
    let (mut form, _) = registry.register_question(owner("one"), question).unwrap();
    for invalid in [
        json!({}),
        json!({"choice":"allow-once"}),
        json!({"answer":"B.docx"}),
    ] {
        assert!(!registry.resolve(&form.binding, answers(invalid)));
    }
    assert!(registry.resolve(&form.binding, answers(json!({"choice":"question-1"}))));
    assert_eq!(form.receive().await.unwrap()["choice"], "question-1");
}

#[tokio::test]
async fn a_repeated_source_does_not_close_another_waiter() {
    let registry = Arc::new(FormRegistry::default());
    let mut first = registry
        .register(owner("one"), "root = Card()")
        .expect("first");
    let mut second = registry
        .register(owner("two"), "root = Card()")
        .expect("second");
    assert_ne!(first.binding.request_id, second.binding.request_id);
    assert_eq!(first.binding.revision, second.binding.revision);
    assert!(matches!(
        first.receiver.try_recv(),
        Err(tokio::sync::oneshot::error::TryRecvError::Empty)
    ));
    assert!(registry.resolve(&first.binding, answers(json!({"choice":"one"}))));
    assert!(registry.resolve(&second.binding, answers(json!({"choice":"two"}))));
    assert_eq!(
        first.receive().await.unwrap(),
        answers(json!({"choice":"one"}))
    );
    assert_eq!(
        second.receive().await.unwrap(),
        answers(json!({"choice":"two"}))
    );
    assert!(!registry.resolve(&second.binding, answers(json!({"choice":"late"}))));
}

#[tokio::test]
async fn a_host_identity_collision_is_refused_without_overwriting_the_waiter() {
    let registry = Arc::new(FormRegistry::default());
    let mut first = registry.register(owner("one"), "source").unwrap();
    assert!(registry.register_binding(first.binding.clone()).is_none());
    assert!(registry.resolve(&first.binding, answers(json!({"ready":false}))));
    assert_eq!(
        first.receive().await.unwrap(),
        answers(json!({"ready":false}))
    );
}

#[tokio::test]
async fn altered_owners_and_content_versions_cannot_answer_or_withdraw_a_form() {
    let registry = Arc::new(FormRegistry::default());
    let mut form = registry.register(owner("one"), "source").unwrap();
    let original = form.binding.clone();
    let mut variants = Vec::new();
    let mut changed = original.clone();
    changed.owner.session_id = "two".into();
    variants.push(changed);
    let mut changed = original.clone();
    changed.owner.anchor_message_id = "new".into();
    variants.push(changed);
    let mut changed = original.clone();
    changed.owner.attempt_id = 2;
    variants.push(changed);
    let mut changed = original.clone();
    changed.owner.execution_id = "new-model".into();
    variants.push(changed);
    let mut changed = original.clone();
    changed.owner.prompt_id = "new-prompt".into();
    variants.push(changed);
    let mut changed = original.clone();
    changed.owner.acp_session_id = "new-acp".into();
    variants.push(changed);
    let mut changed = original.clone();
    changed.owner.run = Some(FormRun {
        core_session_id: "core".into(),
        run_id: "run".into(),
        model_turn: Some(1),
    });
    variants.push(changed);
    let mut changed = original.clone();
    changed.revision = format!("sha256:{}", "a".repeat(64));
    variants.push(changed);
    for changed in variants {
        assert!(!registry.resolve(&changed, answers(json!({"value":1}))));
        registry.withdraw(&changed);
    }
    assert!(registry.resolve(&original, answers(json!({"value":0}))));
    assert_eq!(form.receive().await.unwrap(), answers(json!({"value":0})));
}

#[tokio::test]
async fn stale_lease_cleanup_cannot_delete_a_replacement_or_another_plane() {
    let registry = Arc::new(FormRegistry::default());
    let other = Arc::new(FormRegistry::default());
    let old = registry.register(owner("one"), "source").unwrap();
    let binding = old.binding.clone();
    registry.withdraw(&binding);
    let mut replacement = binding.clone();
    replacement.owner.prompt_id = "next-prompt".into();
    let mut next = registry.register_binding(replacement).unwrap();
    drop(old);
    assert!(!registry.resolve(&binding, FormValues::new()));
    assert!(!other.resolve(&next.binding, FormValues::new()));
    assert!(registry.resolve(&next.binding, answers(json!({"value":"next"}))));
    assert_eq!(
        next.receive().await.unwrap(),
        answers(json!({"value":"next"}))
    );
}

#[tokio::test]
async fn cancellation_closes_the_request_and_frees_only_that_sessions_capacity() {
    let registry = Arc::new(FormRegistry::default());
    let mut cancelled = registry.register(owner("one"), "source").unwrap();
    let mut healthy = registry.register(owner("two"), "source").unwrap();
    registry.withdraw_session("one");
    assert!(cancelled.receive().await.is_err());
    assert!(!registry.resolve(&cancelled.binding, FormValues::new()));
    assert!(registry.resolve(&healthy.binding, FormValues::new()));
    assert_eq!(healthy.receive().await.unwrap(), FormValues::new());
    let leases: Vec<_> = (0..MAX_PENDING)
        .map(|_| registry.register(owner("one"), "source").unwrap())
        .collect();
    assert!(registry.register(owner("two"), "source").is_none());
    drop(leases);
    assert!(registry.register(owner("two"), "source").is_some());
}

#[tokio::test]
async fn racing_submissions_resolve_exactly_once() {
    let registry = Arc::new(FormRegistry::default());
    let mut form = registry.register(owner("one"), "source").unwrap();
    let barrier = Arc::new(std::sync::Barrier::new(2));
    let handles: Vec<_> = [1, 2]
        .into_iter()
        .map(|value| {
            let registry = Arc::clone(&registry);
            let binding = form.binding.clone();
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                (
                    value,
                    registry.resolve(&binding, answers(json!({"winner":value}))),
                )
            })
        })
        .collect();
    let results: Vec<_> = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .collect();
    let winners: Vec<_> = results.iter().filter(|(_, accepted)| *accepted).collect();
    assert_eq!(winners.len(), 1);
    assert_eq!(
        form.receive().await.unwrap(),
        answers(json!({"winner":winners[0].0}))
    );
}

#[tokio::test]
async fn malformed_or_oversized_answers_do_not_consume_the_pending_request() {
    let registry = Arc::new(FormRegistry::default());
    let mut form = registry.register(owner("one"), "source").unwrap();
    for value in [
        json!({"bad":null}),
        json!({"bad":[]}),
        json!({"bad":{}}),
        json!({"bad":"界".repeat(1366)}),
        json!({"bad":9007199254740992_u64}),
        json!({"":true}),
    ] {
        assert!(!registry.resolve(&form.binding, answers(value)));
    }
    let too_many: FormValues = (0..65)
        .map(|n| (format!("field-{n}"), json!(true)))
        .collect();
    assert!(!registry.resolve(&form.binding, too_many));
    let too_large: FormValues = (0..17)
        .map(|n| (format!("field-{n}"), json!("a".repeat(4096))))
        .collect();
    assert!(!registry.resolve(&form.binding, too_large));
    assert!(registry.resolve(
        &form.binding,
        answers(json!({"count":0,"ready":false,"topic":"Work"}))
    ));
    assert_eq!(
        form.receive().await.unwrap(),
        answers(json!({"count":0,"ready":false,"topic":"Work"}))
    );
}

#[test]
fn submit_dto_refuses_unknown_fields_and_missing_ownership() {
    let registry = Arc::new(FormRegistry::default());
    let form = registry.register(owner("one"), "source").unwrap();
    let mut body = json!({"binding":form.binding,"values":{}});
    assert!(serde_json::from_value::<FormSubmission>(body.clone()).is_ok());
    body["unexpected"] = json!(true);
    assert!(serde_json::from_value::<FormSubmission>(body).is_err());
    assert!(serde_json::from_value::<FormSubmission>(json!({"values":{}})).is_err());
    let mut body = json!({"binding":form.binding,"values":{}});
    body["binding"]["owner"]["authority"] = json!("all");
    assert!(serde_json::from_value::<FormSubmission>(body).is_err());
    let mut body = json!({"binding":form.binding,"values":{}});
    body["binding"]["owner"]["run"] = Value::Null;
    assert!(serde_json::from_value::<FormSubmission>(body.clone()).is_err());
    body["binding"]["owner"]["run"] = json!({"coreSessionId":"core","runId":"run"});
    assert!(serde_json::from_value::<FormSubmission>(body.clone()).is_err());
    body["binding"]["owner"]["run"]["modelTurn"] = Value::Null;
    assert!(serde_json::from_value::<FormSubmission>(body).is_ok());
}
