#![cfg(test)]
use super::*;

#[test]
fn owned_form_remains_bound_through_stream_checkpoint_and_answer() {
    let mut stream = TurnStream::new("m1");
    let source = r#"root = Card("Details", [])"#;
    let binding = super::super::forms::FormBinding {
        protocol_version: 1,
        request_id: "a".repeat(64),
        revision: format!("sha256:{}", "b".repeat(64)),
        owner: super::super::forms::FormOwner {
            session_id: "session".into(),
            execution_id: "model".into(),
            attempt_id: 1,
            anchor_message_id: "m1".into(),
            acp_session_id: "acp".into(),
            prompt_id: "prompt".into(),
            run: None,
        },
    };
    let chunks = stream.form_request(&binding, source);
    let request = chunks
        .iter()
        .find(|chunk| chunk["type"] == "data-form")
        .unwrap();
    assert_eq!(request["data"]["format"], "openui");
    assert_eq!(request["data"]["spec"], source);
    assert_eq!(request["data"]["binding"], json!(binding));
    let part = stream
        .parts
        .iter()
        .find(|part| part["type"] == "data-form")
        .unwrap();
    assert_eq!(part["data"]["format"], "openui");
    assert_eq!(part["data"]["spec"], source);
    let values = serde_json::from_value(json!({"days":3,"accepted":false})).unwrap();
    let mut stale = binding.clone();
    stale.owner.prompt_id = "old-prompt".into();
    assert!(stream.form_answered(&stale, &values).is_empty());
    assert_eq!(
        stream
            .parts
            .iter()
            .find(|part| part["type"] == "data-form")
            .unwrap()["data"]["state"],
        "form-requested"
    );
    let answered = stream.form_answered(&binding, &values);
    assert_eq!(answered[0]["data"]["binding"], json!(binding));
    let part = stream
        .parts
        .iter()
        .find(|part| part["type"] == "data-form")
        .unwrap();
    assert_eq!(part["data"]["format"], "openui");
    assert_eq!(part["data"]["state"], "answered");
    assert_eq!(part["data"]["values"], json!(values));
    assert_eq!(part["data"]["binding"], json!(binding));
}
