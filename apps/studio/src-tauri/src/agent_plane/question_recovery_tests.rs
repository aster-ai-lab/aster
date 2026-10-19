#![cfg(test)]

use super::tests::{harness, stored_line, until_done};
use super::*;

async fn requested(events: &mut broadcast::Receiver<Value>) -> Value {
    loop {
        let event = tokio::time::timeout(Duration::from_secs(10), events.recv())
            .await
            .expect("question deadline")
            .unwrap();
        let chunk = &event["payload"]["chunk"];
        if chunk["type"] == "data-form" && chunk["data"]["state"] == "form-requested" {
            return chunk.clone();
        }
    }
}

#[tokio::test]
async fn an_expired_question_closes_its_persisted_card_and_refuses_a_late_answer() {
    let h = harness("question-expiry", "default").expect("node fixture");
    let mut events = h.hub.subscribe();
    h.turns
        .open(
            &h.session_id,
            &super::tests::message("native-question-choice"),
        )
        .unwrap();
    let request = requested(&mut events).await;
    let binding: FormBinding = serde_json::from_value(request["data"]["binding"].clone()).unwrap();
    let buffered = h.turns.attach(&h.session_id);
    tokio::time::pause();
    tokio::time::advance(Duration::from_secs(1800)).await;
    tokio::task::yield_now().await;
    tokio::time::resume();
    let seen = until_done(&mut events, |_| {}).await;
    h.turns.close_test_runtimes().await;
    if let Some(output) = std::env::var_os("A3S_STUDIO_QUESTION_RECOVERY_OUTPUT") {
        let path = PathBuf::from(output);
        assert!(path.is_absolute() && path.parent().unwrap().join(".owned").is_file());
        let requested: Vec<_> = buffered["bufferedChunks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|payload| payload["chunk"].clone())
            .collect();
        let closed: Vec<_> = seen
            .iter()
            .filter_map(|event| {
                let chunk = &event["payload"]["chunk"];
                (chunk["type"] == "data-form" && chunk["data"]["state"] == "closed")
                    .then(|| chunk.clone())
            })
            .collect();
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .unwrap();
        file.write_all(&serde_json::to_vec_pretty(&json!({"requested":requested,"closed":closed,"binding":binding,"source":"actual native 1800-second question expiry with advanced test clock"})).unwrap()).unwrap();
    }
    assert_eq!(seen.last().unwrap()["channel"], "ai.stream.error");
    assert!(
        seen.last().unwrap()["payload"]["error"]["message"]
            .as_str()
            .unwrap()
            .contains("expired")
    );
    assert!(
        seen.iter()
            .any(|event| event["payload"]["chunk"]["data"]["state"] == "closed")
    );
    let reopened = PlaneStore::open(Some(h.dir.join("state")));
    let rows = reopened.messages(&h.session_id);
    let card = rows
        .last()
        .unwrap()
        .parts()
        .iter()
        .find(|part| part["type"] == "data-form")
        .unwrap();
    assert_eq!(card["data"]["state"], "closed");
    assert_eq!(card["data"]["spec"], request["data"]["spec"]);
    assert_eq!(card["data"]["binding"], json!(binding));
    assert!(card["data"].get("values").is_none());
    assert!(!h.turns.submit_form(
        binding,
        serde_json::from_value(json!({"choice":"question-1"})).unwrap()
    ));
}

#[tokio::test]
async fn reattaching_a_waiting_question_keeps_its_owner_and_accepts_one_explicit_answer() {
    let h = harness("question-reattach", "default").expect("node fixture");
    let mut events = h.hub.subscribe();
    h.turns
        .open(
            &h.session_id,
            &super::tests::message("native-question-choice"),
        )
        .unwrap();
    let request = requested(&mut events).await;
    drop(events);
    h.turns.settle_orphan_pending(&h.session_id);
    let attached = h.turns.attach(&h.session_id);
    let restored = attached["bufferedChunks"]
        .as_array()
        .unwrap()
        .iter()
        .find_map(|payload| (payload["chunk"]["type"] == "data-form").then_some(&payload["chunk"]))
        .unwrap();
    assert_eq!(restored, &request);
    assert_eq!(
        h.store
            .messages(&h.session_id)
            .last()
            .unwrap()
            .parts()
            .iter()
            .find(|part| part["type"] == "data-form")
            .unwrap()["data"],
        request["data"]
    );
    let binding: FormBinding = serde_json::from_value(restored["data"]["binding"].clone()).unwrap();
    let mut events = h.hub.subscribe();
    let answer = serde_json::from_value(json!({"choice":"question-1"})).unwrap();
    assert!(h.turns.submit_form(binding.clone(), answer));
    let seen = until_done(&mut events, |_| {}).await;
    h.turns.close_test_runtimes().await;
    assert_eq!(seen.last().unwrap()["payload"]["status"], "success");
    let rows = h.store.messages(&h.session_id);
    let card = rows
        .last()
        .unwrap()
        .parts()
        .iter()
        .find(|part| part["type"] == "data-form")
        .unwrap();
    assert_eq!(card["data"]["state"], "answered");
    assert_eq!(card["data"]["values"]["choice"], "question-1");
    assert!(
        rows.last()
            .unwrap()
            .searchable_text
            .contains("selected B.docx")
    );
    assert!(!h.turns.submit_form(
        binding,
        serde_json::from_value(json!({"choice":"question-0"})).unwrap()
    ));
}

#[test]
fn process_recovery_closes_only_unanswered_cards_and_preserves_their_source() {
    for status in ["pending", "paused", "error", "success"] {
        let h = harness(&format!("question-process-recovery-{status}"), "default")
            .expect("node fixture");
        let owner = FormOwner {
            session_id: h.session_id.clone(),
            execution_id: "model".into(),
            attempt_id: 1,
            anchor_message_id: "assistant".into(),
            acp_session_id: "acp".into(),
            prompt_id: "prompt".into(),
            run: None,
        };
        let question = Question {
            question: "选择文档".into(),
            choices: vec![],
            allow_free_text: true,
        };
        let (pending, spec) = h
            .turns
            .forms
            .register_question(owner.clone(), question.clone())
            .unwrap();
        let (answered, _) = h.turns.forms.register_question(owner, question).unwrap();
        let mut stream = TurnStream::new("assistant");
        stream.question_request(&pending.binding, &spec);
        stream.question_request(&answered.binding, &spec);
        let values = serde_json::from_value(json!({"answer":"B.docx"})).unwrap();
        stream.form_answered(&answered.binding, &values);
        let mut row = stored_line(&h.session_id, "assistant", status, "已保留的正文");
        row.data = json!({"parts":stream.parts});
        h.store.save_message(row).unwrap();
        let (restarted, store, _) = super::tests::plane_fixture(&h.dir);
        restarted.settle_orphan_pending(&h.session_id);
        let rows = store.messages(&h.session_id);
        let row = rows.last().unwrap();
        assert_eq!(
            row.status,
            if status == "pending" { "error" } else { status }
        );
        assert!(row.searchable_text.contains("已保留的正文"));
        let cards: Vec<_> = row
            .parts()
            .iter()
            .filter(|part| part["type"] == "data-form")
            .collect();
        assert_eq!(cards[0]["data"]["state"], "closed");
        assert_eq!(cards[0]["data"]["spec"], spec);
        assert_eq!(cards[0]["data"]["binding"], json!(pending.binding));
        assert_eq!(cards[1]["data"]["state"], "answered");
        assert_eq!(cards[1]["data"]["values"], json!(values));
        assert!(!restarted.submit_form(pending.binding.clone(), values));
        restarted.settle_orphan_pending(&h.session_id);
        assert_eq!(store.messages(&h.session_id).last().unwrap().data, row.data);
        assert_eq!(
            store.messages(&h.session_id).last().unwrap().updated_at,
            row.updated_at
        );
    }
}
