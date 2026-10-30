#![cfg(test)]

use super::*;
use serde_json::json;

#[test]
fn observations_retain_sizes_and_prefix_hashes_without_prompt_or_secret_text() {
    let system = format!("{}private-prompt-sentinel", "a".repeat(17000));
    let original = json!({"messages":[{"role":"system","content":system},{"role":"user","content":"private-user-sentinel"},{"role":"assistant","reasoning_content":"private-thought-sentinel"},{"role":"tool","content":"private-tool-output"}],"tools":[{"type":"function","function":{"name":"private-schema-sentinel"}}]});
    let before = RequestSummary::capture(1, &original);
    assert!(before.shape_complete);
    assert_eq!(before.messages, 4);
    assert_eq!(before.tools, 1);
    assert_eq!(before.role_bytes.len(), 4);
    assert_eq!(before.serialized_request_bytes, original.to_string().len());
    let encoded = serde_json::to_string(&before).unwrap();
    assert!(!encoded.contains("private-"));
    assert!(!encoded.contains("tokens"), "bytes are not token estimates");
    let mut changed = original.clone();
    changed["messages"][0]["content"] = format!("{system} changed").into();
    let after = RequestSummary::capture(2, &changed);
    assert_ne!(before.system_sha256, after.system_sha256);
    assert_eq!(
        before.system_prefix_sha256[&16384],
        after.system_prefix_sha256[&16384]
    );
    assert_eq!(before.tools_sha256, after.tools_sha256);
    changed["tools"][0]["function"]["name"] = "changed-tool".into();
    assert_ne!(
        before.tools_sha256,
        RequestSummary::capture(3, &changed).tools_sha256
    );
    assert_eq!(
        original["messages"][0]["content"], system,
        "observation must not alter transport bytes"
    );
}

#[test]
fn unknown_roles_and_missing_messages_are_explicitly_incomplete_and_never_exported() {
    for name in ["system", "developer", "user", "assistant", "tool"] {
        assert_eq!(Role::parse(name).unwrap().as_str(), name);
    }
    assert!(Role::parse("private-role-sentinel").is_err());
    let observation = RequestSummary::capture(
        1,
        &json!({"messages":[{"role":"private-role-sentinel","content":"private-body"}]}),
    );
    assert!(!observation.shape_complete);
    assert!(observation.role_bytes.is_empty());
    assert!(!serde_json::to_string(&observation)
        .unwrap()
        .contains("private-"));
    let missing = RequestSummary::capture(1, &json!({}));
    assert!(!missing.shape_complete);
    assert_eq!(missing.message_bytes, 0);
    assert_eq!(missing.tool_schema_bytes, 0);
}
