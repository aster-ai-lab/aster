use super::*;
use a3s_effect::{ToolExecutionOutcome, ToolFileIdentityV1, ToolFileKind, ToolPathChangeV1};

fn file(bytes: &[u8], mode: u32) -> ToolFileIdentityV1 {
    ToolFileIdentityV1 {
        kind: ToolFileKind::File,
        content_digest: Some(format!("sha256:{}", sha256::digest(bytes))),
        mode: Some(mode),
    }
}

fn receipt(
    id: &str,
    before: Option<ToolFileIdentityV1>,
    after: ToolFileIdentityV1,
) -> a3s_effect::ToolExecutionResultV1 {
    a3s_effect::ToolExecutionResultV1 {
        schema: a3s_effect::TOOL_RESULT_SCHEMA.into(),
        identity: a3s_effect::ToolInvocationIdentityV1 {
            turn: 1,
            cycle: 0,
            tool_call_id: id.into(),
            batch_index: 0,
            admission: None,
        },
        prepared_digest: format!("sha256:{}", "a".repeat(64)),
        outcome: ToolExecutionOutcome::Succeeded,
        exit_code: Some(0),
        output: serde_json::json!("actual receipt"),
        effects: a3s_effect::ToolEffectObservationV1::complete(vec![ToolPathChangeV1 {
            path: "proof.txt".into(),
            before,
            after,
        }]),
    }
}

#[test]
fn unchanged_receipt_does_not_create_mutation_or_failed_effect_debt() {
    for failed in [false, true] {
        let state = file(b"same bytes", 0o644);
        let mut result = receipt("same-state", Some(state.clone()), state);
        if failed {
            result.outcome = ToolExecutionOutcome::Failed;
            result.exit_code = Some(1);
        }
        let mut ledger = MutationLedger::default();
        ledger.observe_tool_receipt("bash", &result);
        assert!(ledger.is_empty());
        assert!(!ledger.has_failed_effects());
        assert_eq!(ledger.turn_progress.receipts_applied, 1);
    }
}

#[test]
fn unchanged_receipt_preserves_prior_mutation_and_its_verification_debt() {
    let before = file(b"before", 0o644);
    let after = file(b"after", 0o644);
    let mut ledger = MutationLedger::default();
    ledger.observe_tool_receipt("write", &receipt("write", Some(before), after.clone()));
    let digest = ledger.digest().to_owned();
    ledger.observe_tool_receipt("bash", &receipt("later-read", Some(after.clone()), after));
    assert!(!ledger.is_empty());
    assert_eq!(ledger.digest(), digest);
    assert!(matches!(
        decide_completion(&ledger, &[], &[], false),
        CompletionGate::Incomplete { .. }
    ));
}

#[test]
fn unchanged_receipt_filter_keeps_unknown_preimages_bytes_and_mode_changes() {
    let original = file(b"before", 0o644);
    for result in [
        receipt("unknown-before", None, original.clone()),
        receipt(
            "different-bytes",
            Some(original.clone()),
            file(b"after", 0o644),
        ),
        receipt(
            "different-mode",
            Some(original.clone()),
            file(b"before", 0o600),
        ),
    ] {
        let mut ledger = MutationLedger::default();
        ledger.observe_tool_receipt("bash", &result);
        assert!(!ledger.is_empty());
    }
}

#[test]
fn unchanged_receipt_does_not_forgive_unknown_invocation_outcomes() {
    let state = file(b"same", 0o644);
    let mut result = receipt("unknown", Some(state.clone()), state);
    result.outcome = ToolExecutionOutcome::Unknown;
    result.exit_code = None;
    let mut ledger = MutationLedger::default();
    ledger.observe_tool_receipt("bash", &result);
    assert!(!ledger.unresolved_tool_effects.is_empty());
    assert!(matches!(
        decide_completion(&ledger, &[], &[], false),
        CompletionGate::Incomplete { .. }
    ));
}

#[test]
fn unchanged_receipt_background_and_nested_paths_share_the_same_rule() {
    let state = file(b"same", 0o644);
    let result = receipt("background", Some(state.clone()), state.clone());
    let mut ledger = MutationLedger::default();
    ledger.observe_background_receipt("background:terminal:exit", &result.effects);
    assert!(ledger.is_empty());
    ledger.observe_receipt_path("task", "proof.txt", Some(&state), &state);
    assert!(ledger.is_empty());
}

#[test]
fn unchanged_receipt_still_reconciles_a_prior_created_path_that_is_absent_again() {
    let absent = ToolFileIdentityV1 {
        kind: ToolFileKind::Missing,
        content_digest: None,
        mode: None,
    };
    let mut ledger = MutationLedger::default();
    ledger.observe_tool_receipt(
        "write",
        &receipt("created", Some(absent.clone()), file(b"temporary", 0o644)),
    );
    assert!(!ledger.is_empty());
    ledger.observe_tool_receipt(
        "bash",
        &receipt("absent-again", Some(absent.clone()), absent),
    );
    assert!(ledger.is_empty());
}

#[test]
fn unchanged_receipt_does_not_treat_unknown_or_unread_bytes_as_unchanged() {
    for kind in [ToolFileKind::Unknown, ToolFileKind::File] {
        let state = ToolFileIdentityV1 {
            kind,
            content_digest: None,
            mode: None,
        };
        let mut result = receipt("unread-state", Some(state.clone()), state);
        result.effects.completeness = a3s_effect::ToolObservationCompleteness::Incomplete;
        result.effects.reason = Some("file identity could not be read".into());
        result.validate().unwrap();
        let mut ledger = MutationLedger::default();
        ledger.observe_tool_receipt("bash", &result);
        assert!(!ledger.is_empty());
        assert!(matches!(
            decide_completion(&ledger, &[], &[], false),
            CompletionGate::Incomplete { .. }
        ));
    }
}
