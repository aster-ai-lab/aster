#![cfg(test)]

use super::*;

struct Fixture {
    turns: Arc<TurnService>,
    root: PathBuf,
    execution: Execution,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn fixture() -> Fixture {
    let root = std::env::temp_dir().join(format!("a3s-form-owner-{}", new_id()));
    let (turns, _, _) = super::tests::plane_fixture(&root);
    let (abort, _) = watch::channel(false);
    let (steer_tx, _) = mpsc::unbounded_channel();
    turns.lock_live().insert(
        "session".into(),
        LiveTurn {
            acp_session: None,
            attempt_id: 1,
            execution_id: "model".into(),
            owner: "assistant".into(),
            status: "streaming".into(),
            observed: None,
            buffered: vec![],
            abort,
            approvals: HashMap::new(),
            steer_tx,
        },
    );
    Fixture {
        turns,
        root,
        execution: Execution {
            session_id: "session".into(),
            topic_id: "agent-session:session".into(),
            execution_id: "model".into(),
            attempt_id: 1,
            anchor_id: "assistant".into(),
        },
    }
}

#[tokio::test]
async fn a_form_is_bound_to_the_live_execution_and_is_revoked_synchronously_on_abort() {
    let fixture = fixture();
    let mut form = fixture
        .turns
        .register_form(&fixture.execution, "acp", "prompt", "source")
        .unwrap();
    assert_eq!(form.binding.owner.session_id, "session");
    fixture.turns.abort("session");
    assert!(
        !fixture
            .turns
            .submit_form(form.binding.clone(), FormValues::new())
    );
    assert!(form.receive().await.is_err());
    assert!(
        fixture
            .turns
            .register_form(&fixture.execution, "acp", "prompt", "source")
            .is_none()
    );
}

#[tokio::test]
async fn old_submission_and_cleanup_do_not_affect_the_new_execution() {
    let fixture = fixture();
    let old = fixture
        .turns
        .register_form(&fixture.execution, "acp", "prompt", "source")
        .unwrap();
    fixture.turns.lock_live().get_mut("session").unwrap().owner = "replacement".into();
    let mut next_execution = fixture.execution.clone();
    next_execution.anchor_id = "replacement".into();
    next_execution.attempt_id = 2;
    fixture
        .turns
        .lock_live()
        .get_mut("session")
        .unwrap()
        .attempt_id = 2;
    let mut next = fixture
        .turns
        .register_form(&next_execution, "acp", "next-prompt", "source")
        .unwrap();
    assert!(
        !fixture
            .turns
            .submit_form(old.binding.clone(), FormValues::new())
    );
    drop(old);
    let values = serde_json::from_value(json!({"accepted":false})).unwrap();
    assert!(fixture.turns.submit_form(next.binding.clone(), values));
    assert_eq!(
        next.receive().await.unwrap().get("accepted"),
        Some(&json!(false))
    );
    assert!(
        !fixture
            .turns
            .submit_form(next.binding.clone(), FormValues::new())
    );
}

#[test]
fn only_native_prompt_correlated_observations_are_included_in_form_ownership() {
    let fixture = fixture();
    let mut observer = super::super::execution_observation::Observer::new("acp", "prompt");
    let mut packet = json!({"protocolVersion":1,"sessionId":"acp","promptId":"prompt", "coreSessionId":"core", "runId":"run", "sequence":1,"kind":"run-opened","turn":null,"status":null});
    observer.accept(&packet).unwrap();
    packet["sequence"] = json!(2);
    packet["kind"] = json!("model-turn-opened");
    packet["turn"] = json!(3);
    fixture
        .turns
        .lock_live()
        .get_mut("session")
        .unwrap()
        .observed = observer.accept(&packet).unwrap();
    let form = fixture
        .turns
        .register_form(&fixture.execution, "acp", "prompt", "source")
        .unwrap();
    let run = form.binding.owner.run.as_ref().unwrap();
    assert_eq!(
        (&run.core_session_id, &run.run_id, run.model_turn),
        (&"core".to_string(), &"run".to_string(), Some(3))
    );
    assert!(
        fixture
            .turns
            .register_form(&fixture.execution, "acp", "foreign-prompt", "source")
            .is_none()
    );
}

#[tokio::test]
async fn waiting_on_a_form_retains_the_active_execution_in_status_and_reconnect() {
    let fixture = fixture();
    let mut events = fixture.turns.hub.subscribe();
    fixture
        .turns
        .status(&fixture.execution, "awaiting-form", true);
    let status = tokio::time::timeout(Duration::from_secs(1), events.recv())
        .await
        .expect("owned status deadline")
        .unwrap();
    assert_eq!(
        status["payload"]["status"]["activeExecutions"][0],
        fixture.execution.active()
    );
    assert!(status["payload"]["status"].get("lastCompletedAt").is_none());
}

#[tokio::test]
#[ignore = "requires explicit A3S_CLOUD_TEST_AGENT_FORM_OUTPUT for the native/renderer campaign"]
async fn native_owned_form_contract_built_for_sdk_and_renderer() {
    let output = std::env::var("A3S_CLOUD_TEST_AGENT_FORM_OUTPUT").expect("explicit output path");
    let fixture = fixture();
    let mut observer = super::super::execution_observation::Observer::new("acp", "prompt");
    fixture.turns.lock_live().get_mut("session").unwrap().observed = observer.accept(&json!({
        "protocolVersion":1,"sessionId":"acp","promptId":"prompt", "coreSessionId":"core", "runId":"run",
        "sequence":1,"kind":"run-opened","turn":null,"status":null
    })).unwrap();
    let source = "root = Card(\"Details\", [count, ready, topic, submit])\ncount = Input(\"count\", \"Count\", 0, \"number\")\nready = Checkbox(\"ready\", \"Ready\", false)\ntopic = Input(\"topic\", \"Topic\", \"Work\")\nsubmit = Button(\"submit\", \"Continue\")";
    let mut form = fixture
        .turns
        .register_form(&fixture.execution, "acp", "prompt", source)
        .unwrap();
    let mut stream = TurnStream::new(&fixture.execution.anchor_id);
    let requested = stream.form_request(&form.binding, source);
    let submission: super::super::forms::FormSubmission = serde_json::from_value(json!({
        "binding":form.binding,"values":{"count":0,"ready":false,"topic":"Work"}
    }))
    .unwrap();
    assert!(
        fixture
            .turns
            .submit_form(submission.binding, submission.values)
    );
    let values = form.receive().await.unwrap();
    let answered = stream.form_answered(&form.binding, &values);
    let envelope = |chunk: &Value| {
        let mut payload = fixture.execution.envelope();
        payload.insert("chunk".into(), chunk.clone());
        Value::Object(payload)
    };
    let artifact = json!({
        "requested":requested,"answered":answered,"parts":stream.parts,
        "requestEnvelope":envelope(requested.iter().find(|chunk| chunk["type"]=="data-form").unwrap()),
        "answerEnvelope":envelope(&answered[0]),
        "submission":{"binding":form.binding,"values":values},
        "receipt":{"protocolVersion":1,"requestId":form.binding.request_id,"ok":true}
    });
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .expect("new artifact");
    use std::io::Write;
    file.write_all(&serde_json::to_vec_pretty(&artifact).unwrap())
        .unwrap();
}

#[test]
fn native_alignment_form_ownership_checks_execution_and_attempt_even_with_the_same_anchor() {
    let fixture = fixture();
    let form = fixture
        .turns
        .register_form(&fixture.execution, "acp", "prompt", "source")
        .unwrap();
    fixture
        .turns
        .lock_live()
        .get_mut("session")
        .unwrap()
        .execution_id = "replacement".into();
    assert!(
        !fixture
            .turns
            .submit_form(form.binding.clone(), FormValues::new())
    );
    assert!(
        fixture
            .turns
            .register_form(&fixture.execution, "acp", "prompt", "source")
            .is_none()
    );
    fixture
        .turns
        .lock_live()
        .get_mut("session")
        .unwrap()
        .execution_id = fixture.execution.execution_id.clone();
    fixture
        .turns
        .lock_live()
        .get_mut("session")
        .unwrap()
        .attempt_id = 2;
    assert!(
        !fixture
            .turns
            .submit_form(form.binding.clone(), FormValues::new())
    );
    assert!(
        fixture
            .turns
            .register_form(&fixture.execution, "acp", "prompt", "source")
            .is_none()
    );
}
