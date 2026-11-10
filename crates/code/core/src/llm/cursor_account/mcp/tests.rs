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

#[test]
fn rejections_identify_the_failed_check_without_payloads() {
    let base = json!({"providerIdentifier":"aster", "toolName":"read", "args":{}});
    for (field, value, reason) in [
        ("toolName", json!(null), "missing_tool_name"),
        (
            "providerIdentifier",
            json!("synthetic-secret"),
            "provider_mismatch",
        ),
        (
            "serverIdentifier",
            json!("synthetic-secret"),
            "server_mismatch",
        ),
        ("smartModeApprovalOnly", json!(true), "approval_only"),
        ("toolName", json!("synthetic-secret"), "tool_unavailable"),
    ] {
        let mut args = base.clone();
        args[field] = value;
        let request = json!({"execServerMessage":{"mcpArgs":args}});
        let error = tool_call(&request, &tools()).unwrap_err();
        assert!(error.to_string().ends_with(reason), "{error}");
        assert!(!error.to_string().contains("synthetic-secret"));
        assert!(error
            .downcast_ref::<super::super::super::NonRetryableLlmError>()
            .is_some());
    }
}

#[tokio::test]
async fn malformed_arguments_can_be_corrected_without_execution() {
    use crate::agent::tests::MockLlmClient;
    use crate::agent::{AgentConfig, AgentLoop};
    use crate::llm::types::ContentBlock;
    use crate::tools::{ToolContext, ToolExecutor};
    use std::sync::Arc;

    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("input.txt"), "verified bytes\n").unwrap();
    let mut response = MockLlmClient::text_response("");
    let request = json!({"execServerMessage":{"mcpArgs":{
        "providerIdentifier":"aster", "toolName":"read", "args":"synthetic-secret"
    }}});
    response.message.content = vec![tool_call(&request, &tools()).unwrap().unwrap()];
    assert!(!serde_json::to_string(&response.message)
        .unwrap()
        .contains("synthetic-secret"));
    let client = Arc::new(MockLlmClient::new(vec![
        response,
        MockLlmClient::tool_call_response("fixed", "read", json!({"file_path":"input.txt"})),
        MockLlmClient::text_response("Verified"),
    ]));
    let executor = Arc::new(ToolExecutor::new(root.path().display().to_string()));
    let config = AgentConfig {
        tools: tools(),
        planning_mode: crate::PlanningMode::Disabled,
        continuation_enabled: false,
        permission_checker: Some(Arc::new(
            crate::permissions::PermissionPolicy::new().allow("read(*)"),
        )),
        ..AgentConfig::default()
    };
    let engine = AgentLoop::new(
        client,
        executor,
        ToolContext::new(root.path().to_path_buf()),
        config,
    );
    let result = engine.execute(&[], "Read input.txt", None).await.unwrap();
    let results: Vec<_> = result
        .messages
        .iter()
        .flat_map(|message| &message.content)
        .filter_map(|block| match block {
            ContentBlock::ToolResult {
                content, is_error, ..
            } => Some((content, is_error)),
            _ => None,
        })
        .collect();
    assert_eq!(results.len(), 2);
    assert_eq!(*results[0].1, Some(true));
    assert!(results[0].0.as_text().contains("arguments_not_object"));
    assert_eq!(*results[1].1, Some(false), "{}", results[1].0.as_text());
    assert!(results[1].0.as_text().contains("verified bytes"));
}
