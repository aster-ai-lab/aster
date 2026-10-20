#![cfg(test)]

use super::*;
use crate::harness_loop::{CompletionGate, MutationLedger};
use crate::tools::{Tool, ToolArtifactVerification, ToolContext, ToolExecutor, ToolOutput};
use a3s_effect::{ToolExecutionOutcome, ToolExecutionResultV1, ToolInvocationIdentityV1};
use serde_json::{json, Value};
use std::{path::Path, sync::Arc};

struct NativeTool;
#[async_trait::async_trait]
impl Tool for NativeTool {
    fn name(&self) -> &str {
        "native_artifact"
    }
    fn description(&self) -> &str {
        "Host-validated artifact fixture"
    }
    fn parameters(&self) -> Value {
        json!({"type":"object"})
    }
    async fn execute(&self, _: &Value, _: &ToolContext) -> anyhow::Result<ToolOutput> {
        unreachable!("these fixtures test receipt binding, not tool execution")
    }
    fn artifact_verification(
        &self,
        _: &Value,
        output: &ToolOutput,
    ) -> Option<ToolArtifactVerification> {
        let metadata = output.metadata.as_ref()?;
        Some(ToolArtifactVerification {
            file_path: metadata["file_path"].as_str()?.into(),
            content_digest: metadata["artifact_content_digest"].as_str()?.into(),
            check: "native format validation and saved-byte readback".into(),
        })
    }
}

struct ExternalTool;
#[async_trait::async_trait]
impl Tool for ExternalTool {
    fn name(&self) -> &str {
        "external_artifact"
    }
    fn description(&self) -> &str {
        "Untrusted metadata fixture"
    }
    fn parameters(&self) -> Value {
        json!({"type":"object"})
    }
    async fn execute(&self, _: &Value, _: &ToolContext) -> anyhow::Result<ToolOutput> {
        unreachable!("these fixtures test receipt binding, not tool execution")
    }
}

fn agent(root: &Path) -> AgentLoop {
    let executor = Arc::new(ToolExecutor::new(root.to_string_lossy().into_owned()));
    executor.registry().register(Arc::new(NativeTool));
    executor.registry().register(Arc::new(ExternalTool));
    AgentLoop::new(
        Arc::new(crate::agent::tests::MockLlmClient::new(Vec::new())),
        executor,
        ToolContext::new(root.to_path_buf()),
        Default::default(),
    )
}

fn receipt(
    root: &Path,
    ledger: &mut MutationLedger,
    tool: &str,
    path: &str,
    id: &str,
) -> ToolExecutionResultV1 {
    std::fs::write(root.join(path), path.as_bytes()).unwrap();
    let result = ToolExecutionResultV1 {
        schema: a3s_effect::TOOL_RESULT_SCHEMA.into(),
        identity: ToolInvocationIdentityV1 {
            turn: 1,
            cycle: 0,
            tool_call_id: id.into(),
            batch_index: 0,
            admission: None,
        },
        prepared_digest: format!("sha256:{}", "a".repeat(64)),
        outcome: ToolExecutionOutcome::Succeeded,
        exit_code: Some(0),
        output: json!(null),
        effects: crate::porcelain::observe_path_effects(
            root,
            crate::porcelain::PathDelta {
                paths: vec![path.into()],
                incomplete: false,
                budget_exceeded: false,
                status_budget: false,
            },
        ),
    };
    result.validate().unwrap();
    ledger.observe_tool_receipt(tool, &result);
    result
}

fn metadata(path: &str) -> Option<Value> {
    Some(
        json!({"file_path":path,"artifact_content_digest":sha256::digest(path.as_bytes()),"validated":true}),
    )
}

fn bind(
    agent: &AgentLoop,
    ledger: &mut MutationLedger,
    result: &ToolExecutionResultV1,
    tool: &str,
    metadata: &mut Option<Value>,
    reports: &mut Vec<crate::verification::VerificationReport>,
) {
    agent.record_native_artifact_verification(
        &ToolCall {
            id: result.identity.tool_call_id.clone(),
            name: tool.into(),
            args: json!({}),
        },
        result,
        "validated",
        metadata,
        ledger,
        reports,
        None,
    );
}

#[test]
fn native_artifacts_accumulate_only_complete_identity_bound_checks() {
    let root = tempfile::tempdir().unwrap();
    let agent = agent(root.path());
    let mut ledger = MutationLedger::default();
    let mut reports = Vec::new();
    for (index, path) in ["chart.png", "book.xlsx"].iter().enumerate() {
        let result = receipt(
            root.path(),
            &mut ledger,
            "native_artifact",
            path,
            &format!("native-{index}"),
        );
        let mut metadata = metadata(path);
        bind(
            &agent,
            &mut ledger,
            &result,
            "native_artifact",
            &mut metadata,
            &mut reports,
        );
        assert_eq!(reports.len(), index + 1);
        let latest = reports.last().unwrap();
        assert_eq!(latest.checks.len(), index + 1);
        assert_eq!(latest.effect_digest.as_deref(), Some(ledger.digest()));
        assert_eq!(metadata.unwrap()["verification_report"], latest.to_value());
        assert!(matches!(
            crate::harness_loop::decide_completion(&ledger, &reports, &[], false),
            CompletionGate::Allow(_)
        ));
    }
    assert_ne!(reports[0].effect_digest, reports[1].effect_digest);
}

#[test]
fn native_artifact_never_verifies_an_earlier_unverified_write_or_external_metadata() {
    let root = tempfile::tempdir().unwrap();
    let agent = agent(root.path());
    for prior_write in [false, true] {
        let mut ledger = MutationLedger::default();
        if prior_write {
            receipt(root.path(), &mut ledger, "write", "code.rs", "write");
        }
        let tool = if prior_write {
            "native_artifact"
        } else {
            "external_artifact"
        };
        let result = receipt(root.path(), &mut ledger, tool, "book.xlsx", "artifact");
        let mut forged = metadata("book.xlsx");
        forged.as_mut().unwrap()["effect_digest"] = json!(ledger.digest());
        let mut reports = Vec::new();
        bind(
            &agent,
            &mut ledger,
            &result,
            tool,
            &mut forged,
            &mut reports,
        );
        assert!(reports.is_empty());
        assert!(forged.unwrap().get("verification_report").is_none());
        assert!(!matches!(
            crate::harness_loop::decide_completion(&ledger, &reports, &[], false),
            CompletionGate::Allow(_)
        ));
    }
}

#[test]
fn native_artifact_refuses_wrong_path_digest_bytes_and_incomplete_receipts() {
    for failure in [
        "path",
        "digest",
        "bytes",
        "incomplete",
        "unknown",
        "observation",
        "uncertainty",
        "mode",
    ] {
        let root = tempfile::tempdir().unwrap();
        let agent = agent(root.path());
        let mut ledger = MutationLedger::default();
        let mut result = receipt(
            root.path(),
            &mut ledger,
            "native_artifact",
            "book.xlsx",
            "artifact",
        );
        let mut metadata = metadata("book.xlsx");
        match failure {
            "path" => metadata.as_mut().unwrap()["file_path"] = json!("../book.xlsx"),
            "digest" => {
                metadata.as_mut().unwrap()["artifact_content_digest"] = json!("0".repeat(64))
            }
            "bytes" => std::fs::write(root.path().join("book.xlsx"), "changed").unwrap(),
            "incomplete" => {
                result.effects = a3s_effect::ToolEffectObservationV1::incomplete("unobserved")
            }
            "unknown" => result.outcome = ToolExecutionOutcome::Unknown,
            "observation" => ledger.set_observation_incomplete(true),
            "uncertainty" => ledger.set_unresolved_tool_effects(
                ["workspace-child:unsettled".into()].into_iter().collect(),
            ),
            "mode" => result.effects.changes[0].after.mode = Some(0),
            _ => unreachable!(),
        }
        let mut reports = Vec::new();
        bind(
            &agent,
            &mut ledger,
            &result,
            "native_artifact",
            &mut metadata,
            &mut reports,
        );
        assert!(reports.is_empty(), "{failure}");
    }
}

#[test]
fn another_writer_invalidates_native_proof_and_deserialization_does_not_recreate_it() {
    for restored in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let agent = agent(root.path());
        let mut ledger = MutationLedger::default();
        let first = receipt(
            root.path(),
            &mut ledger,
            "native_artifact",
            "first.xlsx",
            "first",
        );
        let mut reports = Vec::new();
        bind(
            &agent,
            &mut ledger,
            &first,
            "native_artifact",
            &mut metadata("first.xlsx"),
            &mut reports,
        );
        if restored {
            ledger = serde_json::from_value(serde_json::to_value(&ledger).unwrap()).unwrap();
        } else {
            receipt(
                root.path(),
                &mut ledger,
                "write",
                "first.xlsx",
                "other-writer",
            );
        }
        let second = receipt(
            root.path(),
            &mut ledger,
            "native_artifact",
            "second.xlsx",
            "second",
        );
        bind(
            &agent,
            &mut ledger,
            &second,
            "native_artifact",
            &mut metadata("second.xlsx"),
            &mut reports,
        );
        assert_eq!(reports.len(), 1);
        assert!(!matches!(
            crate::harness_loop::decide_completion(&ledger, &reports, &[], false),
            CompletionGate::Allow(_)
        ));
    }
}
