#![cfg(test)]

use super::*;
use a3s_effect::{ToolFileIdentityV1, ToolFileKind};

#[cfg(unix)]
fn checked_scope(root: &std::path::Path) -> MutationLedger {
    std::fs::write(root.join("checked.txt"), b"actual checked bytes").unwrap();
    let mut scope = MutationLedger::default();
    scope.observe_nested_artifact_effects(
        "write",
        &crate::porcelain::observe_path_effects(
            root,
            crate::porcelain::PathDelta {
                paths: vec!["checked.txt".into()],
                incomplete: false,
                budget_exceeded: false,
                status_budget: false,
            },
        ),
    );
    let started = scope.begin_nested_mutation().unwrap();
    let epoch = scope.finish_nested_mutation(started).unwrap();
    // Private host-result binding fixture; actual governed Bash wiring is
    // independently exercised by the fact/program sibling tests.
    scope.record_nested_shell_verification("test -f checked.txt", epoch, root);
    assert!(scope.nested_shell_verification_report(root).is_some());
    scope
}

#[cfg(unix)]
fn parent_receipt(root: &std::path::Path) -> a3s_effect::ToolExecutionResultV1 {
    a3s_effect::ToolExecutionResultV1 {
        schema: a3s_effect::TOOL_RESULT_SCHEMA.into(),
        identity: a3s_effect::ToolInvocationIdentityV1 {
            turn: 1,
            cycle: 0,
            tool_call_id: "actual-parent".into(),
            batch_index: 0,
            admission: None,
        },
        prepared_digest: format!("sha256:{}", "a".repeat(64)),
        outcome: a3s_effect::ToolExecutionOutcome::Succeeded,
        exit_code: Some(0),
        output: serde_json::Value::Null,
        effects: crate::porcelain::observe_path_effects(
            root,
            crate::porcelain::PathDelta {
                paths: vec!["checked.txt".into()],
                incomplete: false,
                budget_exceeded: false,
                status_budget: false,
            },
        ),
    }
}

#[cfg(unix)]
fn bind_parent(
    root: &std::path::Path,
    scope: &MutationLedger,
    receipt: &a3s_effect::ToolExecutionResultV1,
    id: &str,
) -> Vec<crate::verification::VerificationReport> {
    let engine = crate::agent::AgentLoop::new(
        std::sync::Arc::new(crate::agent::tests::MockLlmClient::new(vec![])),
        std::sync::Arc::new(crate::tools::ToolExecutor::new(root.display().to_string())),
        crate::tools::ToolContext::new(root.to_path_buf()),
        Default::default(),
    );
    let mut ledger = MutationLedger::default();
    ledger.observe_tool_receipt("program", receipt);
    let mut reports = Vec::new();
    engine.record_native_artifact_verification(
        &crate::llm::ToolCall {
            id: id.into(),
            name: "program".into(),
            args: serde_json::json!({}),
        },
        receipt,
        "",
        &mut None,
        &mut ledger,
        &mut reports,
        Some(scope),
    );
    reports
}

#[test]
#[cfg(unix)]
fn shell_proof_requires_a_complete_successful_exact_parent_and_current_effect_identity() {
    for case in [
        "valid",
        "failed",
        "incomplete",
        "other-parent",
        "later-bytes",
        "unobserved-change",
    ] {
        let root = tempfile::tempdir().unwrap();
        let scope = checked_scope(root.path());
        let mut receipt = parent_receipt(root.path());
        match case {
            "failed" => {
                receipt.outcome = a3s_effect::ToolExecutionOutcome::Failed;
                receipt.exit_code = Some(1);
            }
            "incomplete" => {
                receipt.effects.completeness = a3s_effect::ToolObservationCompleteness::Incomplete;
                receipt.effects.reason = Some("controlled incomplete parent observation".into());
            }
            "later-bytes" => {
                std::fs::write(root.path().join("checked.txt"), b"later unverified bytes").unwrap()
            }
            "unobserved-change" => {
                std::fs::write(
                    root.path().join("unobserved.txt"),
                    b"not in the checked child boundary",
                )
                .unwrap();
                receipt.effects = crate::porcelain::observe_path_effects(
                    root.path(),
                    crate::porcelain::PathDelta {
                        paths: vec!["checked.txt".into(), "unobserved.txt".into()],
                        incomplete: false,
                        budget_exceeded: false,
                        status_budget: false,
                    },
                );
            }
            _ => {}
        }
        receipt.validate().unwrap();
        let reports = bind_parent(
            root.path(),
            &scope,
            &receipt,
            if case == "other-parent" {
                "foreign-parent"
            } else {
                "actual-parent"
            },
        );
        assert_eq!(reports.len(), usize::from(case == "valid"), "{case}");
    }
}

#[test]
#[cfg(unix)]
fn serialized_or_forged_checkpoint_fields_cannot_restore_private_shell_authority() {
    let root = tempfile::tempdir().unwrap();
    let scope = checked_scope(root.path());
    let mut wire = serde_json::to_value(&scope).unwrap();
    for field in [
        "nested_shell_verifications",
        "nested_mutation_epoch",
        "active_nested_mutations",
    ] {
        assert!(wire.get(field).is_none());
    }
    wire["nested_shell_verifications"] = serde_json::json!({"checked.txt":{
        "identity": scope.records[0].after_identity,
        "command":"test -f checked.txt", "scope_digest":scope.digest(), "epoch":scope.nested_mutation_epoch,
    }});
    wire["nested_mutation_epoch"] = serde_json::json!(scope.nested_mutation_epoch);
    wire["active_nested_mutations"] = serde_json::json!(0);
    let restored: MutationLedger = serde_json::from_value(wire).unwrap();
    assert_eq!(restored.digest(), scope.digest());
    assert!(restored
        .nested_shell_verification_report(root.path())
        .is_none());
}

#[test]
#[cfg(unix)]
fn shell_proof_authority_is_bounded_by_existing_command_and_effect_limits() {
    let root = tempfile::tempdir().unwrap();
    for command in [
        "x".repeat(a3s_effect::MAX_TOOL_EFFECT_CANONICAL_BYTES + 1),
        std::iter::repeat_n("test -f checked.txt", a3s_effect::MAX_TOOL_EFFECT_PATHS + 1)
            .collect::<Vec<_>>()
            .join(";"),
    ] {
        let mut scope = checked_scope(root.path());
        let started = scope.begin_nested_mutation().unwrap();
        let epoch = scope.finish_nested_mutation(started).unwrap();
        scope.record_nested_shell_verification(&command, epoch, root.path());
        assert!(scope
            .nested_shell_verification_report(root.path())
            .is_none());
    }
}

#[test]
#[cfg(unix)]
fn a_closed_wrapper_window_is_rechecked_after_a_sibling_restores_the_same_digest() {
    let root = tempfile::tempdir().unwrap();
    let child = checked_scope(root.path());
    std::fs::write(root.path().join("other.txt"), b"original").unwrap();
    let mut parent = MutationLedger::default();
    parent.observe_nested_artifact_effects(
        "seed",
        &crate::porcelain::observe_path_effects(
            root.path(),
            crate::porcelain::PathDelta {
                paths: vec!["checked.txt".into(), "other.txt".into()],
                incomplete: false,
                budget_exceeded: false,
                status_budget: false,
            },
        ),
    );
    let started = parent.begin_nested_mutation().unwrap();
    let window = parent.finish_nested_mutation(started).unwrap();
    assert!(parent.nested_mutation_window_is_current(window));
    let original_digest = parent.digest().to_owned();
    let sibling = parent.begin_nested_mutation().unwrap();
    for bytes in [b"modified".as_slice(), b"original".as_slice()] {
        std::fs::write(root.path().join("other.txt"), bytes).unwrap();
        parent.observe_nested_artifact_effects(
            "sibling",
            &crate::porcelain::observe_path_effects(
                root.path(),
                crate::porcelain::PathDelta {
                    paths: vec!["other.txt".into()],
                    incomplete: false,
                    budget_exceeded: false,
                    status_budget: false,
                },
            ),
        );
    }
    parent.finish_nested_mutation(sibling).unwrap();
    assert_eq!(parent.digest(), original_digest);
    assert!(!parent.nested_mutation_window_is_current(window));
    let effects = parent_receipt(root.path()).effects;
    let allow_shell = parent.nested_mutation_window_is_current(window);
    parent.absorb_nested_artifact_verifications(&child, "wrapper", &effects, allow_shell);
    assert!(parent
        .nested_shell_verification_report(root.path())
        .is_none());
}

#[test]
fn shell_gate_advice_uses_a_present_exact_file_instead_of_a_deleted_suffix_alias() {
    let mut ledger = MutationLedger::default();
    ledger.observe_receipt_path(
        "program",
        ".scratch/output/results.json",
        &ToolFileIdentityV1 {
            kind: ToolFileKind::Missing,
            content_digest: None,
            mode: None,
        },
    );
    ledger.observe_receipt_path(
        "program",
        "output",
        &ToolFileIdentityV1 {
            kind: ToolFileKind::Directory,
            content_digest: None,
            mode: Some(0o40700),
        },
    );
    ledger.observe_receipt_path(
        "program",
        "output/results.json",
        &ToolFileIdentityV1 {
            kind: ToolFileKind::File,
            content_digest: Some(format!("sha256:{}", sha256::digest(b"actual results"))),
            mode: Some(0o100600),
        },
    );
    let CompletionGate::Incomplete { message } = decide_completion(&ledger, &[], &[], false) else {
        panic!("unverified mutations must remain incomplete");
    };
    let next = message.split(" Next step:").nth(1).unwrap();
    assert!(next.contains("test -f output/results.json"), "{next}");
    assert!(!next.contains(".scratch/"), "{next}");
}
