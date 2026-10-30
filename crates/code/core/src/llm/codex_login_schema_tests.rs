use super::*;
use crate::tools::{builtin::read::ReadTool, Tool};

#[test]
fn responses_keeps_optional_read_arguments_without_weakening_local_validation() {
    let read = ReadTool;
    let client = CodexLoginClient::from_parts(
        "synthetic".into(),
        String::new(),
        "gpt-test",
        "schema-test",
        None,
    );
    let body = client.build_body(
        &[Message::user("read the work order")],
        None,
        &[read.definition()],
        true,
    );
    assert_eq!(
        body["tools"][0]["strict"], false,
        "Responses must not promote mutually exclusive optional arguments to required fields"
    );

    let validator = jsonschema::validator_for(&read.parameters()).unwrap();
    assert!(validator.is_valid(&json!({"file_path": ".wiki-task.md"})));
    assert!(validator.is_valid(&json!({"files": [{"path": ".wiki-task.md"}]})));
    assert!(!validator
        .is_valid(&json!({"file_path": ".wiki-task.md", "files": [{"path": ".wiki-task.md"}]})));
}
