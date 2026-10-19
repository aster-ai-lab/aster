#![cfg(test)]

//! Original fact receipts retain child evidence even when their wrapper fails.

use super::*;
use serde_json::{json, Value};

struct ControlledFailedCheck;

#[async_trait]
impl crate::tools::Tool for ControlledFailedCheck {
    fn name(&self) -> &str {
        "controlled_failed_check"
    }
    fn description(&self) -> &str {
        "Controlled unsuccessful check"
    }
    fn parameters(&self) -> Value {
        json!({"type":"object"})
    }
    async fn execute(
        &self,
        _: &Value,
        _: &crate::tools::ToolContext,
    ) -> anyhow::Result<crate::tools::ToolOutput> {
        Ok(crate::tools::ToolOutput::error("controlled check failed")
            .with_metadata(json!({"source":"controlled-check"})))
    }
}

async fn failed_program_receipt(source: &str, writes: bool) -> a3s_effect::ToolExecutionResultV1 {
    let workspace = tempfile::tempdir().unwrap();
    std::fs::write(workspace.path().join("input.txt"), "controlled input\n").unwrap();
    let agent = crate::Agent::from_config(test_config()).await.unwrap();
    let session = session_for(
        &agent,
        workspace.path(),
        "program-failed-original-receipt",
        Arc::new(ScriptedClient {
            seen: Arc::new(std::sync::Mutex::new(Vec::new())),
            responses: std::sync::Mutex::new(vec![
                tool_response(
                    "program-failed-parent",
                    "program",
                    json!({"type":"script","source":source,
                        "allowed_tools":["read","write","controlled_failed_check"]}),
                ),
                text_response("controlled terminal response"),
            ]),
        }),
        &crate::SessionOptions::new()
            .with_permission_policy(crate::permissions::PermissionPolicy::new().allow("*"))
            .with_continuation(false),
    );
    session
        .register_dynamic_tool(Arc::new(ControlledFailedCheck))
        .unwrap();
    // Delivery may refuse the failed operation; the original receipt must still
    // contain its actual error and all already observed child results.
    let _ = session.send("Run the controlled orchestration", None).await;
    let facts = read_workspace_facts(
        workspace.path(),
        &crate::fact_control::thread_for_session(session.session_id()),
    )
    .unwrap();
    let receipts = facts
        .into_iter()
        .filter(|fact| fact.kind == "tool.result")
        .map(|fact| {
            serde_json::from_value::<a3s_effect::ToolExecutionResultV1>(
                fact.payload["receipt"].clone(),
            )
            .unwrap()
        })
        .filter(|receipt| receipt.identity.tool_call_id == "program-failed-parent")
        .collect::<Vec<_>>();
    session.close().await;
    assert_eq!(receipts.len(), 1, "one original wrapper receipt");
    if writes {
        assert_eq!(
            std::fs::read(workspace.path().join("saved.txt")).unwrap(),
            b"saved-marker\n"
        );
    }
    let receipt = receipts.into_iter().next().unwrap();
    receipt.validate().unwrap();
    assert_eq!(receipt.outcome, a3s_effect::ToolExecutionOutcome::Failed);
    assert_eq!(receipt.exit_code, Some(1));
    let output: Value = serde_json::from_str(receipt.output.as_str().unwrap()).unwrap();
    assert_eq!(output["is_error"], true);
    assert_eq!(output["outcome"], "failed");
    assert_eq!(output["metadata"]["program"]["success"], false);
    assert_eq!(output["metadata"]["program"]["tool_calls_complete"], true);
    receipt
}

fn calls(receipt: &a3s_effect::ToolExecutionResultV1) -> Vec<Value> {
    let output: Value = serde_json::from_str(receipt.output.as_str().unwrap()).unwrap();
    output["metadata"]["program"]["tool_calls"]
        .as_array()
        .unwrap()
        .clone()
}

#[tokio::test]
async fn read_then_throw_keeps_a_failed_parent_and_the_actual_nested_read() {
    let receipt = failed_program_receipt(
        "async function run(ctx) { await ctx.readFile('input.txt'); throw new Error('controlled JS failure'); }",
        false,
    ).await;
    let calls = calls(&receipt);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0]["tool_name"], "read");
    assert_eq!(calls[0]["success"], true);
    assert_eq!(calls[0]["exit_code"], 0);
    assert_eq!(calls[0]["metadata"]["range"]["returned_lines"], 1);
    assert!(calls[0]["output_bytes"].as_u64().unwrap() > 0);
}

#[tokio::test]
async fn write_then_failed_check_and_throw_keeps_both_original_child_results() {
    let receipt = failed_program_receipt(
        "async function run(ctx) { await ctx.tool('write', {file_path:'saved.txt',content:'saved-marker\\n'}); const check = await ctx.tool('controlled_failed_check', {}); if (check.exitCode !== 0) throw new Error('controlled check rejection'); }",
        true,
    ).await;
    let calls = calls(&receipt);
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0]["tool_name"], "write");
    assert_eq!(calls[0]["success"], true);
    assert_eq!(calls[0]["exit_code"], 0);
    assert_eq!(calls[1]["tool_name"], "controlled_failed_check");
    assert_eq!(calls[1]["success"], false);
    assert_ne!(calls[1]["exit_code"], 0);
    assert_eq!(calls[1]["metadata"]["source"], "controlled-check");
    assert!(calls
        .iter()
        .all(|call| call["duration_ms"].as_u64().is_some()));
}
