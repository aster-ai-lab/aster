#![cfg(test)]
use super::*;

#[test]
fn body_compaction_preserves_lifecycle_identity_and_uncertainty() {
    let metadata = serde_json::json!({"body":"x".repeat(5101), "workspace_child":"child-1",
        "source_run":"source-1", "run":"run-1", "command_started":true,
        "observation_incomplete":true});
    let compacted = compact_child_metadata(Some(metadata.clone())).unwrap();
    for key in [
        "workspace_child",
        "source_run",
        "run",
        "command_started",
        "observation_incomplete",
    ] {
        assert_eq!(compacted[key], metadata[key]);
    }
    assert!(serde_json::to_vec(&compacted).unwrap().len() <= 4096);
}

#[test]
fn oversized_identity_is_an_explicit_bounded_evidence_gap() {
    let compacted = compact_child_metadata(Some(serde_json::json!({
        "workspace_child":"x".repeat(5000), "command_started":true
    })))
    .unwrap();
    assert_eq!(compacted["evidence_incomplete"], true);
    assert_eq!(
        compacted["evidence_error"]["kind"],
        "batch_lifecycle_metadata_budget"
    );
    assert!(serde_json::to_vec(&compacted).unwrap().len() <= 4096);
}
