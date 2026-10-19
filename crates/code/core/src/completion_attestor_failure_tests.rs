#![cfg(test)]

use super::*;
use crate::harness_loop::{decide_with_observations, CompletionGate, MutationLedger};
use a3s_effect::{
    ToolEffectObservationV1, ToolExecutionOutcome, ToolExecutionResultV1, ToolFileIdentityV1,
    ToolFileKind, ToolInvocationIdentityV1, ToolPathChangeV1,
};

fn receipt(
    turn: u64,
    outcome: ToolExecutionOutcome,
    changes: &[(&str, Option<&str>)],
) -> ToolExecutionResultV1 {
    let result = ToolExecutionResultV1 {
        schema: "a3s.code.tool-result.v1".into(),
        identity: ToolInvocationIdentityV1 {
            turn,
            cycle: 0,
            tool_call_id: format!("fixture-{turn}"),
            batch_index: 0,
            admission: None,
        },
        prepared_digest: format!("sha256:{}", "a".repeat(64)),
        outcome,
        exit_code: Some(if outcome == ToolExecutionOutcome::Succeeded {
            0
        } else {
            1
        }),
        output: serde_json::Value::Null,
        effects: ToolEffectObservationV1::complete(
            changes
                .iter()
                .map(|(path, contents)| ToolPathChangeV1 {
                    path: (*path).into(),
                    before: None,
                    after: ToolFileIdentityV1 {
                        kind: if contents.is_some() {
                            ToolFileKind::File
                        } else {
                            ToolFileKind::Missing
                        },
                        content_digest: contents.map(|contents| {
                            format!("sha256:{}", sha256::digest(contents.as_bytes()))
                        }),
                        mode: None,
                    },
                })
                .collect(),
        ),
    };
    result.validate().unwrap();
    result
}

fn automatic(ledger: &MutationLedger, root: &std::path::Path) -> Vec<VerificationReport> {
    let attestor = Arc::new(DiskContentAttestor::new(root)) as Arc<dyn CompletionAttestor>;
    let mut reports = Vec::new();
    merge_attested_report(Some(&attestor), ledger, &mut reports);
    reports
}

#[test]
fn failed_complete_effects_change_the_digest_and_refuse_automatic_byte_certification() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("out.txt"), "saved").unwrap();
    let mut succeeded = MutationLedger::default();
    succeeded.observe_tool_receipt(
        "arbitrary-tool",
        &receipt(
            1,
            ToolExecutionOutcome::Succeeded,
            &[("out.txt", Some("saved"))],
        ),
    );
    assert_eq!(automatic(&succeeded, root.path()).len(), 1);
    let old_report = automatic(&succeeded, root.path());
    let mut failed = MutationLedger::default();
    failed.observe_tool_receipt(
        "arbitrary-tool",
        &receipt(
            1,
            ToolExecutionOutcome::Failed,
            &[("out.txt", Some("saved"))],
        ),
    );
    assert!(failed.has_failed_effects());
    assert_ne!(
        failed.digest(),
        succeeded.digest(),
        "same bytes do not erase the typed failed invocation"
    );
    assert!(automatic(&failed, root.path()).is_empty());
    assert!(matches!(
        decide_with_observations(&failed, &old_report, &[], false, &[]),
        CompletionGate::Incomplete { .. }
    ));
}

#[test]
fn successful_recovery_requires_each_exact_path_to_have_a_new_after_state() {
    let root = tempfile::tempdir().unwrap();
    let mut ledger = MutationLedger::default();
    let failed = receipt(
        1,
        ToolExecutionOutcome::Failed,
        &[
            ("left.txt", Some("old-left")),
            ("right.txt", Some("old-right")),
        ],
    );
    ledger.observe_tool_receipt("compound", &failed);
    ledger.observe_tool_receipt("read", &receipt(2, ToolExecutionOutcome::Succeeded, &[]));
    ledger.observe_tool_receipt(
        "other-writer",
        &receipt(
            3,
            ToolExecutionOutcome::Succeeded,
            &[("other.txt", Some("unrelated"))],
        ),
    );
    ledger.observe_tool_receipt(
        "same-bytes",
        &receipt(
            4,
            ToolExecutionOutcome::Succeeded,
            &[("left.txt", Some("old-left"))],
        ),
    );
    assert!(ledger.has_failed_effects());
    ledger.observe_tool_receipt(
        "recovered",
        &receipt(
            5,
            ToolExecutionOutcome::Succeeded,
            &[("left.txt", Some("new-left"))],
        ),
    );
    assert!(
        ledger.has_failed_effects(),
        "one successful path cannot clear a different failed path"
    );
    ledger.observe_tool_receipt(
        "recovered",
        &receipt(
            6,
            ToolExecutionOutcome::Succeeded,
            &[("right.txt", Some("new-right"))],
        ),
    );
    assert!(!ledger.has_failed_effects());
    for (path, contents) in [
        ("left.txt", "new-left"),
        ("right.txt", "new-right"),
        ("other.txt", "unrelated"),
    ] {
        std::fs::write(root.path().join(path), contents).unwrap();
    }
    assert_eq!(automatic(&ledger, root.path()).len(), 1);
    let (keys, _) = ledger.checkpoint_receipt_keys();
    assert!(
        keys.contains(&failed.identity.key().unwrap()),
        "recovery must retain the original failure receipt identity"
    );
}

#[test]
fn expected_read_failure_and_successful_compound_output_do_not_create_failed_effects() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("out.txt"), "recovered").unwrap();
    let mut ledger = MutationLedger::default();
    ledger.observe_tool_receipt(
        "expected-failure",
        &receipt(1, ToolExecutionOutcome::Failed, &[]),
    );
    ledger.observe_tool_receipt(
        "compound-with-recovered-child",
        &receipt(
            2,
            ToolExecutionOutcome::Succeeded,
            &[("out.txt", Some("recovered"))],
        ),
    );
    assert!(!ledger.has_failed_effects());
    assert_eq!(automatic(&ledger, root.path()).len(), 1);
}

#[test]
fn serialization_original_replay_and_cache_restore_cannot_erase_failed_effects() {
    let mut original = MutationLedger::default();
    let failed = receipt(1, ToolExecutionOutcome::Failed, &[("out.txt", Some("old"))]);
    original.observe_tool_receipt("compound", &failed);
    let serialized = serde_json::to_value(&original).unwrap();
    let decoded: MutationLedger = serde_json::from_value(serialized.clone()).unwrap();
    assert!(decoded.has_failed_effects());
    assert_eq!(decoded.digest(), original.digest());
    assert!(decoded.checkpoint_cache_covered_by(&original));
    let mut erased = serialized;
    erased
        .as_object_mut()
        .unwrap()
        .remove("failed_effect_paths");
    let erased: MutationLedger = serde_json::from_value(erased).unwrap();
    assert!(!erased.checkpoint_cache_covered_by(&original));
    original.restore_checkpoint_cache(&erased, true);
    assert!(original.has_failed_effects());
    let mut replayed = MutationLedger::default();
    replayed.restore_checkpoint_cache(&decoded, false);
    assert!(replayed.has_failed_effects());
    replayed.observe_tool_receipt("compound", &failed);
    assert!(replayed.has_failed_effects());
    assert_eq!(replayed.digest(), decoded.digest());
}

#[test]
fn recomputing_a_positive_cache_digest_cannot_erase_a_failed_original_prefix() {
    let mut original = MutationLedger::default();
    original.observe_tool_receipt(
        "compound",
        &receipt(
            1,
            ToolExecutionOutcome::Failed,
            &[("out.txt", Some("same"))],
        ),
    );
    let mut positive = MutationLedger::default();
    positive.observe_tool_receipt(
        "compound",
        &receipt(
            1,
            ToolExecutionOutcome::Succeeded,
            &[("out.txt", Some("same"))],
        ),
    );
    let mut forged = serde_json::to_value(&original).unwrap();
    forged
        .as_object_mut()
        .unwrap()
        .remove("failed_effect_paths");
    forged["digest"] = serde_json::json!(positive.digest());
    let forged: MutationLedger = serde_json::from_value(forged).unwrap();
    assert!(!forged.checkpoint_cache_covered_by(&original));
    original.restore_checkpoint_cache(&forged, false);
    assert!(original.has_failed_effects());
}

#[test]
fn failed_deletion_stays_negative_but_current_digest_formal_verification_can_recover() {
    let root = tempfile::tempdir().unwrap();
    let mut ledger = MutationLedger::default();
    ledger.observe_tool_receipt(
        "compound",
        &receipt(1, ToolExecutionOutcome::Failed, &[("deleted.txt", None)]),
    );
    assert!(ledger.has_failed_effects());
    assert!(automatic(&ledger, root.path()).is_empty());
    let report = VerificationReport::new(
        "explicit-recovery",
        vec![VerificationCheck::required(
            "reconciled-delete",
            "explicit_validation",
            "Required validation reconciled the original failed deletion",
        )
        .with_status(VerificationStatus::Passed)],
    )
    .with_effect_digest(ledger.digest());
    assert!(matches!(
        decide_with_observations(&ledger, &[report], &[], false, &[]),
        CompletionGate::Allow(_)
    ));
    ledger.observe_tool_receipt(
        "recreate",
        &receipt(
            2,
            ToolExecutionOutcome::Succeeded,
            &[("deleted.txt", Some("new"))],
        ),
    );
    assert!(!ledger.has_failed_effects());
    std::fs::write(root.path().join("deleted.txt"), "new").unwrap();
    assert_eq!(automatic(&ledger, root.path()).len(), 1);
}
