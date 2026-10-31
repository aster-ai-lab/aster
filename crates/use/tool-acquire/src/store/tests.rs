use super::*;

fn payload(directory: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let path = directory.join(name);
    fs::write(&path, bytes).unwrap();
    path
}

fn sha256(bytes: &[u8]) -> String {
    hex_encode(&Sha256::digest(bytes))
}

async fn install_version(store: &ToolStore, directory: &Path, version: &str) {
    store
        .apply_local(LocalApply {
            name: "uv".into(),
            spec: "github:astral-sh/uv".into(),
            version: version.into(),
            executable_name: "uv".into(),
            payload: payload(directory, &format!("uv-{version}"), b"fixture executable"),
            sha256: None,
            require_checksum: false,
            companions: Vec::new(),
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn version_receipts_do_not_depend_on_the_current_pointer() {
    let temporary = tempfile::tempdir().unwrap();
    let store = ToolStore::open(temporary.path().join("工具 with spaces")).unwrap();
    install_version(&store, temporary.path(), "1.0.0").await;
    install_version(&store, temporary.path(), "2.0.0").await;
    assert_eq!(
        store
            .installed_receipt("uv", None)
            .unwrap()
            .unwrap()
            .version,
        "2.0.0"
    );
    fs::write(store.root_path().join("current/uv"), "invalid/version").unwrap();

    for version in ["1.0.0", "2.0.0"] {
        let receipt = store
            .installed_receipt("uv", Some(version))
            .unwrap()
            .unwrap();
        assert_eq!(receipt.version, version);
        assert_eq!(receipt.spec, "github:astral-sh/uv");
    }
    assert!(store.installed_receipt("uv", None).is_err());
    assert!(store
        .installed_receipt("uv", Some("3.0.0"))
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn legacy_current_receipt_is_accepted_only_for_its_exact_version() {
    let temporary = tempfile::tempdir().unwrap();
    let store = ToolStore::open(temporary.path()).unwrap();
    install_version(&store, temporary.path(), "1.0.0").await;
    fs::remove_file(temporary.path().join("receipts/uv/1.0.0.json")).unwrap();
    assert_eq!(
        store
            .installed_receipt("uv", Some("1.0.0"))
            .unwrap()
            .unwrap()
            .version,
        "1.0.0"
    );

    install_version(&store, temporary.path(), "2.0.0").await;

    assert!(store
        .installed_receipt("uv", Some("1.0.0"))
        .unwrap()
        .is_none());
    assert_eq!(
        store
            .installed_receipt("uv", Some("2.0.0"))
            .unwrap()
            .unwrap()
            .version,
        "2.0.0"
    );
}

#[tokio::test]
async fn mismatched_and_oversized_version_receipts_are_refused() {
    let temporary = tempfile::tempdir().unwrap();
    let store = ToolStore::open(temporary.path()).unwrap();
    install_version(&store, temporary.path(), "1.0.0").await;
    let record = temporary.path().join("receipts/uv/1.0.0.json");
    let mut receipt = store
        .installed_receipt("uv", Some("1.0.0"))
        .unwrap()
        .unwrap();
    receipt.version = "2.0.0".into();
    fs::write(&record, serde_json::to_vec(&receipt).unwrap()).unwrap();
    assert!(matches!(
        store.installed_receipt("uv", Some("1.0.0")),
        Err(ToolAcquireError::ReceiptDisagrees { .. })
    ));

    fs::write(&record, vec![b' '; 64 * 1024 + 1]).unwrap();

    assert!(matches!(
        store.installed_receipt("uv", Some("1.0.0")),
        Err(ToolAcquireError::Io { .. })
    ));
}

#[tokio::test]
async fn uninstall_removes_all_versions_and_their_receipts() {
    let temporary = tempfile::tempdir().unwrap();
    let store = ToolStore::open(temporary.path()).unwrap();
    install_version(&store, temporary.path(), "1.0.0").await;
    install_version(&store, temporary.path(), "2.0.0").await;

    store.remove("uv").await.unwrap();

    assert!(!temporary.path().join("installs/uv").exists());
    assert!(!temporary.path().join("receipts/uv").exists());
    assert!(store
        .installed_receipt("uv", Some("1.0.0"))
        .unwrap()
        .is_none());
    assert!(store
        .installed_receipt("uv", Some("2.0.0"))
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn checksum_failure_keeps_the_previous_current_version() {
    let temporary = tempfile::tempdir().unwrap();
    let store = ToolStore::open(temporary.path()).unwrap();
    let good = b"#!/bin/sh\necho uv\n";
    let good_path = payload(temporary.path(), "uv-good", good);
    store
        .apply_local(LocalApply {
            name: "uv".to_string(),
            spec: "uv".to_string(),
            version: "1.0.0".to_string(),
            executable_name: "uv".to_string(),
            payload: good_path,
            sha256: Some(sha256(good)),
            require_checksum: true,
            companions: Vec::new(),
        })
        .await
        .unwrap();

    let bad = b"not-the-published-bytes";
    let bad_path = payload(temporary.path(), "uv-bad", bad);
    let error = store
        .apply_local(LocalApply {
            name: "uv".to_string(),
            spec: "uv".to_string(),
            version: "2.0.0".to_string(),
            executable_name: "uv".to_string(),
            payload: bad_path,
            sha256: Some(sha256(good)),
            require_checksum: true,
            companions: Vec::new(),
        })
        .await
        .unwrap_err();
    assert_eq!(error.code(), "use.tool_acquire.checksum_mismatch");
    assert_eq!(
        fs::read_to_string(temporary.path().join("current/uv")).unwrap(),
        "1.0.0\n"
    );
    assert!(!temporary.path().join("installs/uv/2.0.0").exists());
    assert!(temporary.path().join("installs/uv/1.0.0/uv").is_file());
    let failure: ToolFailureRecord = serde_json::from_slice(
        &fs::read(temporary.path().join("receipts/uv.failure.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(failure.schema, crate::FAILURE_SCHEMA);
    assert_eq!(failure.version, "2.0.0");
    let listed = store.list().await.unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].version, "1.0.0");
}

#[tokio::test]
async fn first_checksum_failure_leaves_no_current_version() {
    let temporary = tempfile::tempdir().unwrap();
    let store = ToolStore::open(temporary.path()).unwrap();
    let bytes = b"tool";
    let error = store
        .apply_local(LocalApply {
            name: "rg".to_string(),
            spec: "rg".to_string(),
            version: "14.1.1".to_string(),
            executable_name: "rg".to_string(),
            payload: payload(temporary.path(), "rg-payload", bytes),
            sha256: Some("ab".repeat(32)),
            require_checksum: true,
            companions: Vec::new(),
        })
        .await
        .unwrap_err();
    assert_eq!(error.code(), "use.tool_acquire.checksum_mismatch");
    assert!(!temporary.path().join("current/rg").exists());
    assert!(store.list().await.unwrap().is_empty());
}

#[cfg(unix)]
#[tokio::test]
async fn remove_refuses_a_symlinked_install_directory() {
    let temporary = tempfile::tempdir().unwrap();
    let store = ToolStore::open(temporary.path()).unwrap();
    let outside = tempfile::tempdir().unwrap();
    let kept = outside.path().join("kept.txt");
    fs::write(&kept, b"keep").unwrap();
    fs::create_dir_all(temporary.path().join("installs")).unwrap();
    std::os::unix::fs::symlink(outside.path(), temporary.path().join("installs/uv")).unwrap();

    let error = store.remove("uv").await.unwrap_err();
    assert_eq!(error.code(), "use.tool_acquire.link_refused");
    assert_eq!(fs::read(&kept).unwrap(), b"keep");
    assert!(temporary
        .path()
        .join("installs/uv")
        .symlink_metadata()
        .unwrap()
        .file_type()
        .is_symlink());
}

#[tokio::test]
async fn install_dir_follows_current_pointer() {
    let temporary = tempfile::tempdir().unwrap();
    let store = ToolStore::open(temporary.path()).unwrap();
    store
        .apply_local(LocalApply {
            name: "uv".to_string(),
            spec: "github:astral-sh/uv".to_string(),
            version: "0.6.0".to_string(),
            executable_name: "uv".to_string(),
            payload: payload(temporary.path(), "uv", b"#!/bin/sh\necho uv\n"),
            sha256: None,
            require_checksum: false,
            companions: Vec::new(),
        })
        .await
        .unwrap();
    let directory = store.install_dir("uv").unwrap().unwrap();
    assert_eq!(directory, store.root_path().join("installs/uv/0.6.0"));
    assert!(directory.join("uv").is_file());
    assert_eq!(store.shim_dir(), store.root_path().join("shims"));
}
