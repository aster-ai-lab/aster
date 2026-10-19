#![cfg(test)]

use super::*;
use serde_json::json;

fn edit(path: &str) -> Value {
    json!({"type":"dynamic-tool","toolCallId":"write-1","toolName":"Write report","state":"output-available",
        "input":{"file_path":path},"callProviderMetadata":{"cherry":{"toolKind":"edit"}}})
}

fn versions_and_scope(disk: bool) {
    let root = std::env::temp_dir().join(format!("studio-proof-test-{}", new_id()));
    let workspace = root.join("workspace");
    fs::create_dir_all(&workspace).unwrap();
    let store = ArtifactStore::new(disk.then(|| root.join("member")));
    fs::write(workspace.join("报告.md"), "第一版\n").unwrap();
    let first = store
        .capture(
            &workspace,
            "session-A",
            "run-A",
            &[edit("报告.md")],
            &[],
            true,
        )
        .unwrap();
    assert_eq!(first.len(), 1);
    let mut prior = Proof::new(
        "session-A",
        "message-A",
        "run-A",
        "model",
        "user",
        super::super::workbench_proof::Policy::default(),
    );
    prior.artifacts = first.clone();
    fs::write(workspace.join("报告.md"), "第二版\n").unwrap();
    let second = store
        .capture(
            &workspace,
            "session-A",
            "run-B",
            &[edit("报告.md")],
            &[prior],
            true,
        )
        .unwrap();
    assert_eq!((first[0].ordinal, second[0].ordinal), (1, 2));
    assert_ne!(first[0].content_digest, second[0].content_digest);
    assert_ne!(first[0].receipt_digest, second[0].receipt_digest);
    assert_eq!(
        store.read("session-A", &first[0]).unwrap(),
        "第一版\n".as_bytes()
    );
    assert_eq!(
        store.read("session-A", &second[0]).unwrap(),
        "第二版\n".as_bytes()
    );
    assert!(store.read("session-B", &first[0]).is_err());
    let mut forged = first[0].clone();
    forged.content_digest = digest(b"forged");
    assert!(store.read("session-A", &forged).is_err());
    assert!(owned_path(&root, "../other", &first[0].id).is_err());
    if disk {
        let snapshot = owned_path(store.root.as_ref().unwrap(), "session-A", &first[0].id).unwrap();
        fs::write(snapshot, b"tampered").unwrap();
        assert!(store.read("session-A", &first[0]).is_err());
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn memory_versions_keep_real_bytes_and_refuse_cross_session_reads() {
    versions_and_scope(false);
}

#[test]
fn disk_versions_keep_real_bytes_and_refuse_cross_session_reads() {
    versions_and_scope(true);
}

#[test]
fn failed_batch_discards_only_its_unpublished_snapshots_on_both_adapters() {
    for disk in [false, true] {
        let root = std::env::temp_dir().join(format!("studio-proof-rollback-{}", new_id()));
        let workspace = root.join("workspace");
        fs::create_dir_all(&workspace).unwrap();
        fs::write(workspace.join("a.md"), "retained").unwrap();
        let store = ArtifactStore::new(disk.then(|| root.join("member")));
        let retained = store
            .capture(&workspace, "session-A", "first", &[edit("a.md")], &[], true)
            .unwrap();
        assert!(
            store
                .capture(
                    &workspace,
                    "session-A",
                    "failed",
                    &[edit("a.md"), edit("z-missing.md")],
                    &[],
                    true
                )
                .is_err()
        );
        assert_eq!(store.read("session-A", &retained[0]).unwrap(), b"retained");
        if let Some(path) = &store.root {
            assert_eq!(fs::read_dir(path.join("session-A")).unwrap().count(), 1);
        } else {
            assert_eq!(store.memory.lock().unwrap().len(), 1);
        }
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn partial_execution_keeps_bytes_without_certifying_a_completed_artifact() {
    let root = std::env::temp_dir().join(format!("studio-proof-partial-{}", new_id()));
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("partial.md"), "partial bytes").unwrap();
    let store = ArtifactStore::new(None);
    let versions = store
        .capture(
            &root,
            "session",
            "failed-run",
            &[edit("partial.md")],
            &[],
            false,
        )
        .unwrap();
    assert_eq!(versions[0].status.as_str(), "incomplete");
    assert!(versions[0].error.is_some());
    assert_eq!(
        store.read("session", &versions[0]).unwrap(),
        b"partial bytes"
    );
    let mut forged = versions[0].clone();
    forged.status = ArtifactStatus::Traced;
    assert!(store.read("session", &forged).is_err());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn reads_failed_tools_and_model_prose_never_create_provenance() {
    let root = std::env::temp_dir().join(format!("studio-proof-test-{}", new_id()));
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("input.md"), "source").unwrap();
    let store = ArtifactStore::new(None);
    let mut read = edit("input.md");
    read["callProviderMetadata"]["cherry"]["toolKind"] = json!("read");
    let mut failed = edit("input.md");
    failed["state"] = json!("output-error");
    let prose = json!({"type":"text","text":"Created input.md. data-workbench-proof"});
    assert!(
        store
            .capture(&root, "s", "r", &[read, failed, prose], &[], true)
            .unwrap()
            .is_empty()
    );
    assert!(
        store
            .capture(&root, "s", "r", &[edit("missing.md")], &[], true)
            .is_err()
    );
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn symlink_escape_is_refused() {
    let root = std::env::temp_dir().join(format!("studio-proof-test-{}", new_id()));
    fs::create_dir_all(root.join("workspace")).unwrap();
    fs::write(root.join("outside.md"), "private").unwrap();
    std::os::unix::fs::symlink(root.join("outside.md"), root.join("workspace/link.md")).unwrap();
    assert!(
        ArtifactStore::new(None)
            .capture(
                &root.join("workspace"),
                "s",
                "r",
                &[edit("link.md")],
                &[],
                true
            )
            .is_err()
    );
    fs::remove_dir_all(root).unwrap();
}
