#![cfg(test)]

use super::*;

fn tools() -> Vec<ToolDefinition> {
    vec![ToolDefinition {
        name: "read".into(),
        description: "Read a workspace file".into(),
        parameters: json!({"type":"object","properties":{"file_path":{"type":"string"}}}),
    }]
}

#[test]
fn native_catalog_preserves_exact_names_and_schemas() {
    let tools = tools();
    let catalog = definitions(&tools);
    assert_eq!(catalog[0]["providerIdentifier"], "aster");
    assert_eq!(catalog[0]["toolName"], tools[0].name);
    assert_eq!(
        serde_json::from_str::<Value>(catalog[0]["inputSchemaJson"].as_str().unwrap()).unwrap(),
        tools[0].parameters
    );
    let value = json!({"execServerMessage":{"mcpStateExecArgs":{"serverIdentifiers":["aster"]}}});
    let reply: Value =
        serde_json::from_slice(&state_reply(&value, &tools).unwrap().unwrap()).unwrap();
    assert_eq!(reply["execClientMessage"]["id"], 0);
    assert_eq!(
        reply["execClientMessage"]["mcpStateExecResult"]["success"]["servers"][0]["tools"],
        json!(catalog)
    );
}

#[test]
fn native_calls_use_the_exact_provider_and_tool_name_not_the_transformed_display_name() {
    let value = json!({"execServerMessage":{"mcpArgs":{
        "name":"aster-read", "providerIdentifier":"aster", "toolName":"read",
        "serverIdentifier":"aster", "args":{"file_path":"space dir/你好.rs"}
    }}});
    let Some(ContentBlock::ToolUse { name, input, .. }) = tool_call(&value, &tools()).unwrap()
    else {
        panic!("missing governed call")
    };
    assert_eq!(name, "read");
    assert_eq!(input["file_path"], "space dir/你好.rs");
    let empty = json!({"execServerMessage":{"mcpArgs":{
        "providerIdentifier":"aster", "toolName":"read"
    }}});
    let Some(ContentBlock::ToolUse { input, .. }) = tool_call(&empty, &tools()).unwrap() else {
        panic!("missing empty-argument call")
    };
    assert_eq!(input, json!({}));
}

#[test]
fn foreign_or_malformed_native_requests_fail_without_echoing_payloads() {
    for (provider, name, input) in [
        ("foreign", "read", json!({})),
        ("aster", "Read", json!({})),
        ("aster", "shell", json!({"command":"synthetic-secret"})),
        ("aster", "read", json!("synthetic-secret")),
    ] {
        let value = json!({"execServerMessage":{"mcpArgs":{
            "providerIdentifier":provider, "toolName":name, "args":input
        }}});
        let error = tool_call(&value, &tools()).unwrap_err();
        assert!(error
            .downcast_ref::<super::super::super::NonRetryableLlmError>()
            .is_some());
        assert!(!error.to_string().contains("synthetic-secret"));
    }
    for value in [
        json!({"execServerMessage":{"mcpArgs":{"providerIdentifier":"aster","toolName":"read","smartModeApprovalOnly":true}}}),
        json!({"execServerMessage":{"mcpArgs":{"providerIdentifier":"aster","toolName":"read"},"shellArgs":{}}}),
    ] {
        assert!(tool_call(&value, &tools()).is_err());
    }
    let value = json!({"execServerMessage":{"mcpStateExecArgs":{"serverIdentifiers":["foreign"]}}});
    assert!(state_reply(&value, &tools()).is_err());
    assert!(
        tool_call(&json!({"execServerMessage":{"shellArgs":{}}}), &tools())
            .unwrap()
            .is_none()
    );
}
