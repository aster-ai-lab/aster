#![cfg(test)]
#![cfg(unix)]

use super::*;
use std::io::Write;

fn record(path: &str, content: &str) -> MutatedPathRecord {
    MutatedPathRecord {
        path: path.into(),
        content_digest: sha256::digest(content),
    }
}

#[test]
fn every_mutation_must_have_matching_content_evidence() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("one"), "one").unwrap();
    std::fs::write(root.path().join("two"), "two").unwrap();
    let attestor = DiskContentAttestor::new(root.path());
    let paths = [record("one", "one"), record("two", "two")];
    assert!(attestor.attest("effect", &paths).is_some());
    let missing = MutatedPathRecord {
        path: "two".into(),
        content_digest: String::new(),
    };
    assert!(attestor
        .attest("effect", &[paths[0].clone(), missing])
        .is_none());
    std::fs::write(root.path().join("two"), "tampered after verification").unwrap();
    assert!(attestor.attest("effect", &paths).is_none());
    assert!(attestor.attest("", &paths).is_none());
}

#[test]
fn path_boundaries_reject_absolute_parent_symlink_and_special_files() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("secret"), "secret").unwrap();
    std::fs::write(root.path().join("safe..name"), "safe").unwrap();
    symlink(outside.path(), root.path().join("escape")).unwrap();
    symlink(root.path().join("safe..name"), root.path().join("link")).unwrap();
    let attestor = DiskContentAttestor::new(root.path());
    for path in [
        "../secret",
        "/etc/hosts",
        "escape/secret",
        "link",
        ".",
        "missing",
        "",
    ] {
        assert!(
            attestor
                .attest("effect", &[record(path, "secret")])
                .is_none(),
            "accepted {path}"
        );
    }
    assert!(attestor
        .attest("effect", &[record("./safe..name", "safe")])
        .is_some());
}

#[test]
fn large_file_is_hashed_with_the_shared_streaming_observer() {
    use sha2::{Digest, Sha256};
    let root = tempfile::tempdir().unwrap();
    let mut file = std::fs::File::create(root.path().join("large")).unwrap();
    let buffer = [b'x'; 64 * 1024];
    let mut hash = Sha256::new();
    for _ in 0..1024 {
        file.write_all(&buffer).unwrap();
        hash.update(buffer);
    }
    drop(file);
    let paths = [MutatedPathRecord {
        path: "large".into(),
        content_digest: format!("{:x}", hash.finalize()),
    }];
    assert!(DiskContentAttestor::new(root.path())
        .attest("effect", &paths)
        .is_some());
}

fn observed_ledger(root: &std::path::Path, paths: &[&str]) -> crate::harness_loop::MutationLedger {
    use a3s_effect::{ToolExecutionOutcome, ToolExecutionResultV1, ToolInvocationIdentityV1};
    let effects = crate::porcelain::observe_path_effects(
        root,
        crate::porcelain::PathDelta {
            paths: paths.iter().map(|path| (*path).to_owned()).collect(),
            incomplete: false,
            budget_exceeded: false,
            status_budget: false,
        },
    );
    let receipt = ToolExecutionResultV1 {
        schema: "a3s.code.tool-result.v1".into(),
        identity: ToolInvocationIdentityV1 {
            turn: 1,
            cycle: 0,
            tool_call_id: "candidate-write".into(),
            batch_index: 0,
            admission: None,
        },
        prepared_digest: format!("sha256:{}", "a".repeat(64)),
        outcome: ToolExecutionOutcome::Succeeded,
        exit_code: Some(0),
        output: serde_json::Value::Null,
        effects,
    };
    receipt.validate().unwrap();
    let mut ledger = crate::harness_loop::MutationLedger::default();
    ledger.observe_tool_receipt("write", &receipt);
    ledger
}

fn attested(
    root: &std::path::Path,
    ledger: &crate::harness_loop::MutationLedger,
) -> Vec<VerificationReport> {
    let attestor = Arc::new(DiskContentAttestor::new(root)) as Arc<dyn CompletionAttestor>;
    let mut reports = Vec::new();
    merge_attested_report(Some(&attestor), ledger, &mut reports);
    reports
}

#[test]
fn new_candidate_directories_bind_verification_without_shell_and_reject_late_changes() {
    use crate::harness_loop::{decide_with_observations, CompletionGate, CompletionTerminal};
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("concepts")).unwrap();
    let page = root.path().join("concepts/overview.md");
    std::fs::write(&page, "Cited candidate").unwrap();
    let ledger = observed_ledger(root.path(), &["concepts", "concepts/overview.md"]);
    let reports = attested(root.path(), &ledger);
    assert!(matches!(
        decide_with_observations(&ledger, &reports, &[], false, &[]),
        CompletionGate::Allow(CompletionTerminal::Verified { .. })
    ));
    std::fs::write(&page, "Changed after the receipt").unwrap();
    assert!(attested(root.path(), &ledger).is_empty());
    std::fs::remove_file(&page).unwrap();
    assert!(attested(root.path(), &ledger).is_empty());
    std::fs::write(&page, "Cited candidate").unwrap();
    assert_eq!(attested(root.path(), &ledger).len(), 1);
    let deletion = observed_ledger(
        root.path(),
        &["concepts", "concepts/overview.md", "deleted"],
    );
    std::fs::create_dir(root.path().join("deleted")).unwrap();
    assert!(attested(root.path(), &deletion).is_empty());
}

#[test]
fn typed_directory_attestation_does_not_follow_a_replaced_parent() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("concepts")).unwrap();
    std::fs::write(root.path().join("concepts/page.md"), "Same bytes").unwrap();
    std::fs::write(outside.path().join("page.md"), "Same bytes").unwrap();
    let ledger = observed_ledger(root.path(), &["concepts", "concepts/page.md"]);
    std::fs::remove_dir_all(root.path().join("concepts")).unwrap();
    std::os::unix::fs::symlink(outside.path(), root.path().join("concepts")).unwrap();
    assert!(attested(root.path(), &ledger).is_empty());
}
