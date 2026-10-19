#![cfg(all(test, unix))]

use super::*;
use crate::tools::{Tool, ToolCapabilities, ToolContext, ToolOutput};
use a3s_effect::{ToolExecutionOutcome, ToolExecutionResultV1, ToolObservationCompleteness};
use serde_json::{json, Value};

struct VerifyBytes(Arc<AtomicUsize>);

#[async_trait]
impl Tool for VerifyBytes {
    fn name(&self) -> &str {
        "verify_bytes"
    }
    fn description(&self) -> &str {
        "Observe the current controlled file bytes"
    }
    fn parameters(&self) -> Value {
        json!({"type":"object","properties":{},"additionalProperties":false})
    }
    fn capabilities(&self, _: &Value) -> ToolCapabilities {
        ToolCapabilities::read_only_paginated(1)
    }
    async fn execute(&self, _: &Value, context: &ToolContext) -> anyhow::Result<ToolOutput> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(ToolOutput::success(std::fs::read_to_string(
            context.workspace.join("state.txt"),
        )?))
    }
}

async fn original_revalidation(change_bytes: bool) -> (Vec<ToolExecutionResultV1>, usize) {
    let workspace = tempfile::tempdir().unwrap();
    std::fs::write(workspace.path().join("state.txt"), "version-0").unwrap();
    let mut responses = Vec::new();
    for index in 0..4 {
        if index > 0 {
            responses.push(tool_response(
                &format!("write-{index}"), "write",
                json!({"file_path":"state.txt","content":format!("version-{}", if change_bytes { index } else { 0 })}),
            ));
        }
        responses.push(tool_response(
            &format!("verify-{index}"),
            "verify_bytes",
            json!({}),
        ));
    }
    responses.extend([text_response("done"), text_response("[]")]);
    let agent = crate::Agent::from_config(test_config()).await.unwrap();
    let session = session_for(
        &agent,
        workspace.path(),
        "original-file-revalidation",
        Arc::new(ScriptedClient {
            seen: Default::default(),
            responses: std::sync::Mutex::new(responses),
        }),
        &crate::SessionOptions::new()
            .with_permission_policy(crate::permissions::PermissionPolicy::new().allow("*"))
            .with_duplicate_tool_call_threshold(3)
            .with_auto_delegation_enabled(false)
            .with_continuation(false),
    );
    let checks = Arc::new(AtomicUsize::new(0));
    session
        .register_dynamic_tool(Arc::new(VerifyBytes(checks.clone())))
        .unwrap();
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(15),
        session.send("Observe the current file after each controlled write", None),
    )
    .await;
    let facts = read_workspace_facts(workspace.path(), "original-file-revalidation");
    session.close().await;
    agent.close().await;
    assert!(result.is_ok(), "controlled original Run must settle");
    let receipts = facts
        .unwrap()
        .into_iter()
        .filter(|fact| fact.kind == "tool.result")
        .map(|fact| serde_json::from_value(fact.payload["receipt"].clone()).unwrap())
        .collect();
    (receipts, checks.load(Ordering::SeqCst))
}

#[tokio::test]
async fn original_write_byte_changes_permit_the_fourth_identical_verification() {
    let (receipts, checks) = original_revalidation(true).await;
    let writes: Vec<_> = receipts
        .iter()
        .filter(|receipt| receipt.identity.tool_call_id.starts_with("write-"))
        .collect();
    assert_eq!(writes.len(), 3);
    for receipt in writes {
        assert_eq!(receipt.outcome, ToolExecutionOutcome::Succeeded);
        assert_eq!(
            receipt.effects.completeness,
            ToolObservationCompleteness::Complete
        );
        let change = receipt
            .effects
            .changes
            .iter()
            .find(|change| change.path == "state.txt")
            .unwrap();
        assert_ne!(
            change.before.as_ref().unwrap().content_digest,
            change.after.content_digest
        );
    }
    assert_eq!(
        checks, 4,
        "original producer effects must reach the duplicate gate"
    );
    assert!(receipts
        .iter()
        .filter(|receipt| receipt.identity.tool_call_id.starts_with("verify-"))
        .all(|receipt| receipt.outcome == ToolExecutionOutcome::Succeeded));
}

#[tokio::test]
async fn original_same_byte_rewrites_keep_the_fourth_verification_parked() {
    let (receipts, checks) = original_revalidation(false).await;
    for receipt in receipts
        .iter()
        .filter(|receipt| receipt.identity.tool_call_id.starts_with("write-"))
    {
        assert_eq!(receipt.outcome, ToolExecutionOutcome::Succeeded);
        for change in &receipt.effects.changes {
            assert_eq!(
                change.before.as_ref().unwrap().content_digest,
                change.after.content_digest
            );
        }
    }
    assert_eq!(checks, 3);
    let fourth = receipts
        .iter()
        .find(|receipt| receipt.identity.tool_call_id == "verify-3")
        .unwrap();
    assert_eq!(fourth.outcome, ToolExecutionOutcome::Denied);
}
