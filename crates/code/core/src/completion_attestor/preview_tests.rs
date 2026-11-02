use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

struct FinalOnlyAttestor(Arc<AtomicUsize>);

impl CompletionAttestor for FinalOnlyAttestor {
    fn attest(&self, _: &str, _: &[MutatedPathRecord]) -> Option<VerificationReport> {
        self.0.fetch_add(1, Ordering::SeqCst);
        None
    }
}

#[test]
fn preview_does_not_consume_a_legacy_hosts_final_attestation() {
    let calls = Arc::new(AtomicUsize::new(0));
    let attestor: Arc<dyn CompletionAttestor> = Arc::new(FinalOnlyAttestor(calls.clone()));
    let mut ledger = MutationLedger::default();
    ledger.observe_tool(
        "write",
        0,
        Some(&serde_json::json!({"file_path":"result.txt", "after":"original"})),
    );
    let mut reports = Vec::new();
    merge_preview_report(Some(&attestor), &ledger, &mut reports);
    merge_preview_report(Some(&attestor), &ledger, &mut reports);
    assert!(reports.is_empty());
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    merge_attested_report(Some(&attestor), &ledger, &mut reports);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn disk_preview_rechecks_the_current_bytes() {
    let workspace = tempfile::tempdir().unwrap();
    let path = "result 甲.txt";
    std::fs::write(workspace.path().join(path), "original").unwrap();
    let mut ledger = MutationLedger::default();
    ledger.observe_tool(
        "write",
        0,
        Some(&serde_json::json!({"file_path":path, "after":"original"})),
    );
    let attestor: Arc<dyn CompletionAttestor> =
        Arc::new(DiskContentAttestor::new(workspace.path()));
    let mut reports = Vec::new();
    merge_preview_report(Some(&attestor), &ledger, &mut reports);
    assert_eq!(reports.len(), 1);
    assert!(matches!(
        crate::harness_loop::decide_with_observations(&ledger, &reports, &[], false, &[]),
        crate::harness_loop::CompletionGate::Allow(
            crate::harness_loop::CompletionTerminal::Verified { .. }
        )
    ));
    std::fs::write(workspace.path().join(path), "changed").unwrap();
    let mut current_reports = Vec::new();
    merge_preview_report(Some(&attestor), &ledger, &mut current_reports);
    assert!(current_reports.is_empty());
}
