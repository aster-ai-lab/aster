#![cfg(unix)]

use super::*;
use crate::agent::{AgentConfig, AgentLoop};
use crate::harness_loop::{CompletionGate, CompletionTerminal, MutationLedger};
use crate::tools::builtin::bash::BashTool;
use crate::tools::{Tool, ToolContext, ToolExecutor};
use aster_effect::{ToolExecutionOutcome, ToolExecutionResultV1, ToolInvocationIdentityV1};
use serde_json::json;
use std::path::Path;
use std::sync::Arc;

const PATHS: [&str; 2] = ["retired 値.py", "old notes.md"];
const CHECK: &str = "test ! -e 'retired 値.py' && test ! -e 'old notes.md'";

fn removed_files(root: &Path) -> MutationLedger {
    let mut ledger = MutationLedger::default();
    for (index, path) in PATHS.into_iter().enumerate() {
        std::fs::write(root.join(path), "original bytes").unwrap();
        std::fs::remove_file(root.join(path)).unwrap();
        let result = ToolExecutionResultV1 {
            schema: aster_effect::TOOL_RESULT_SCHEMA.into(),
            identity: ToolInvocationIdentityV1 {
                turn: 1,
                cycle: 0,
                tool_call_id: format!("remove-{index}"),
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
        ledger.observe_tool_receipt("bash", &result);
    }
    assert!(!ledger.is_empty());
    ledger
}

async fn check(
    root: &Path,
    ledger: &mut MutationLedger,
    command: &str,
    recreate: bool,
    contracts: Vec<VerificationContractV1>,
) -> Vec<VerificationReport> {
    let context = ToolContext::new(root.to_path_buf());
    let engine = AgentLoop::new(
        Arc::new(crate::agent::tests::MockLlmClient::new(Vec::new())),
        Arc::new(ToolExecutor::new(root.to_string_lossy().into_owned())),
        context.clone(),
        AgentConfig {
            verification_contracts: contracts,
            ..Default::default()
        },
    );
    let result = BashTool
        .execute(&json!({"command":command}), &context)
        .await
        .unwrap();
    assert!(
        result.success,
        "the real absence check must succeed: {result:?}"
    );
    if recreate {
        std::fs::write(root.join(PATHS[0]), "recreated after check").unwrap();
    }
    let mut output = result.content;
    let mut metadata = result.metadata;
    let mut reports = Vec::new();
    engine
        .record_prepared_tool_verification(
            &crate::llm::ToolCall {
                id: "check".into(),
                name: "bash".into(),
                args: json!({"command":command}),
            },
            0,
            &mut output,
            &mut metadata,
            ledger,
            &mut reports,
        )
        .await;
    let mut replay = Vec::new();
    AgentLoop::collect_verification_report(&mut replay, &metadata);
    assert_eq!(replay, reports);
    reports
}

#[tokio::test]
async fn deletion_check_binds_host_report_and_completes() {
    let root = tempfile::tempdir().unwrap();
    for path in [".aster", "meta"] {
        std::fs::create_dir(root.path().join(path)).unwrap();
    }
    let mut ledger = removed_files(root.path());
    let reports = check(root.path(), &mut ledger, CHECK, false, Vec::new()).await;
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].checks.len(), PATHS.len());
    assert_eq!(reports[0].effect_digest.as_deref(), Some(ledger.digest()));
    assert!(matches!(
        crate::harness_loop::decide_completion(&ledger, &reports, &[], false),
        CompletionGate::Allow(CompletionTerminal::Verified { .. })
    ));
    for path in [".aster", "meta"] {
        assert!(root.path().join(path).is_dir());
    }
}

#[tokio::test]
async fn deletion_check_refuses_partial_stale_mixed_and_declared_suite_cases() {
    for mode in ["partial", "stale", "mixed", "contract"] {
        let root = tempfile::tempdir().unwrap();
        let mut ledger = removed_files(root.path());
        if mode == "mixed" {
            std::fs::write(root.path().join("new.py"), "new bytes").unwrap();
            ledger.observe_tool(
                "write",
                0,
                Some(&json!({"file_path":"new.py","after":"new bytes"})),
            );
        }
        let contracts = if mode == "contract" {
            vec![VerificationContractV1::new("host-suite", "python -m unittest", PATHS).unwrap()]
        } else {
            Vec::new()
        };
        let command = if mode == "partial" {
            "test ! -e 'retired 値.py'"
        } else {
            CHECK
        };
        let reports = check(
            root.path(),
            &mut ledger,
            command,
            mode == "stale",
            contracts,
        )
        .await;
        assert!(reports.is_empty(), "must refuse {mode}: {reports:?}");
        assert!(matches!(
            crate::harness_loop::decide_completion(&ledger, &reports, &[], false),
            CompletionGate::Incomplete { .. }
        ));
    }
}
