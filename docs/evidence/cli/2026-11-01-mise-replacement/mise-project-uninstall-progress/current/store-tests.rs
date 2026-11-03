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
async fn project_removal_selects_damaged_versions_without_weakening_execution_or_identity_checks() {
    let temporary = tempfile::tempdir().unwrap();
    let store = ToolStore::open(temporary.path().join("工具 with spaces")).unwrap();
    for version in ["1.0.0", "1.5.0", "2.0.0"] {
        install_version(&store, temporary.path(), version).await;
    }
    let mut tool = crate::ResolvedTool {
        name: "uv".into(),
        spec: "github:astral-sh/uv".into(),
        version: Some("1".into()),
        executable: "uv".into(),
    };
    let receipt = store
        .installed_receipt("uv", Some("1.5.0"))
        .unwrap()
        .unwrap();
    let paths = store.installation_paths(&receipt).unwrap();
    fs::remove_file(paths.executable()).unwrap();
    assert!(store.installed_for(&tool).is_err());
    assert_eq!(
        store
            .uninstall_candidate_for(&tool)
            .unwrap()
            .unwrap()
            .version,
        "1.5.0"
    );
    assert_eq!(
        fs::read_to_string(store.root_path().join("current/uv")).unwrap(),
        "2.0.0\n"
    );
    tool.spec = "github:other/uv".into();
    assert!(store.uninstall_candidate_for(&tool).unwrap().is_none());
    tool.spec = "github:astral-sh/uv".into();
    fs::write(
        store.root_path().join("receipts/uv/1.5.0.json"),
        b"invalid receipt",
    )
    .unwrap();
    assert!(store.uninstall_candidate_for(&tool).is_err());
}

#[tokio::test]
async fn global_uninstall_selection_includes_old_versions_without_pointers_and_missing_executables()
{
    let temporary = tempfile::tempdir().unwrap();
    let store = ToolStore::open(temporary.path().join("工具 with spaces")).unwrap();
    assert!(store.all_uninstall_candidates().unwrap().is_empty());
    install_version(&store, temporary.path(), "1.0.0").await;
    install_version(&store, temporary.path(), "2.0.0").await;
    store.remove_version_sync("uv", "2.0.0").unwrap();
    let payload = payload(temporary.path(), "fd", b"fd fixture");
    store
        .apply_local(LocalApply {
            name: "fd".into(),
            spec: "github:sharkdp/fd".into(),
            version: "10.0.0".into(),
            executable_name: "fd".into(),
            payload,
            sha256: None,
            require_checksum: false,
            companions: Vec::new(),
        })
        .await
        .unwrap();
    fs::remove_file(store.root_path().join("installs/fd/10.0.0/fd")).unwrap();
    let selected = store.all_uninstall_candidates().unwrap();
    assert_eq!(
        selected
            .iter()
            .map(|receipt| (receipt.name.as_str(), receipt.version.as_str()))
            .collect::<Vec<_>>(),
        [("fd", "10.0.0"), ("uv", "1.0.0")]
    );
    assert!(!store.root_path().join("current/uv").exists());
    assert!(store.root_path().join("installs/uv/1.0.0/uv").is_file());
    assert!(store.root_path().join("installs/fd/10.0.0").is_dir());
}

#[tokio::test]
async fn global_uninstall_selection_refuses_malformed_identities_and_uses_a_shared_entry_budget() {
    let temporary = tempfile::tempdir().unwrap();
    let store = ToolStore::open(temporary.path().join("tools")).unwrap();
    install_version(&store, temporary.path(), "1.0.0").await;
    install_version(&store, temporary.path(), "2.0.0").await;
    let mut remaining = 1;
    assert!(matches!(
        store.version_records("uv", VersionRecordCheck::Identity, &mut remaining),
        Err(ToolAcquireError::VersionListLimit)
    ));
    let receipt = store.root_path().join("receipts/uv/1.0.0.json");
    fs::write(receipt, b"not JSON").unwrap();
    assert!(store.all_uninstall_candidates().is_err());
    assert!(store.root_path().join("installs/uv/1.0.0/uv").is_file());
    assert!(store.root_path().join("installs/uv/2.0.0/uv").is_file());
}

#[cfg(unix)]
#[test]
fn global_uninstall_selection_refuses_a_linked_tool_directory() {
    use std::os::unix::fs::symlink;
    let temporary = tempfile::tempdir().unwrap();
    let store = ToolStore::open(temporary.path().join("tools")).unwrap();
    let outside = temporary.path().join("outside");
    fs::create_dir(&outside).unwrap();
    fs::create_dir(store.root_path().join("installs")).unwrap();
    symlink(&outside, store.root_path().join("installs/uv")).unwrap();
    assert!(matches!(
        store.all_uninstall_candidates(),
        Err(ToolAcquireError::LinkRefused { .. })
    ));
    assert!(outside.is_dir());
}

#[tokio::test]
async fn installation_manifest_rejects_invalid_membership_and_oversized_records() {
    let temporary = tempfile::tempdir().unwrap();
    let store = ToolStore::open(temporary.path().join("tools")).unwrap();
    install_version(&store, temporary.path(), "1.0.0").await;
    let files = store.installation_files("uv", "1.0.0").unwrap().unwrap();
    assert_eq!(
        files,
        BTreeMap::from([("uv".into(), sha256(b"fixture executable"))])
    );
    let path = store.root_path().join("receipts/uv/1.0.0.files.json");
    for body in [
        serde_json::json!({"schema":MANIFEST_SCHEMA,"files":{"../outside":sha256(b"fixture executable")}}).to_string(),
        serde_json::json!({"schema":"unknown","files":{"uv":sha256(b"fixture executable")}}).to_string(),
        " ".repeat(MAX_MANIFEST_BYTES as usize + 1),
    ] {
        fs::write(&path, body).unwrap();
        assert!(store.installation_files("uv", "1.0.0").is_err());
    }
}

#[tokio::test]
async fn latest_selection_uses_stable_versions_without_reading_the_current_pointer() {
    let temporary = tempfile::tempdir().unwrap();
    let store = ToolStore::open(temporary.path()).unwrap();
    for version in ["1.2.0", "1.20.0", "2.0.0-rc.1"] {
        install_version(&store, temporary.path(), version).await;
    }
    fs::write(store.root_path().join("current/uv"), "invalid/../pointer").unwrap();
    let selected = store
        .installed_for(&crate::ResolvedTool {
            name: "uv".into(),
            spec: "github:astral-sh/uv".into(),
            version: None,
            executable: "uv".into(),
        })
        .unwrap()
        .unwrap();
    assert_eq!(selected.version, "1.20.0");
    assert_eq!(
        fs::read_to_string(store.root_path().join("current/uv")).unwrap(),
        "invalid/../pointer"
    );
}

#[test]
fn cached_version_metadata_rejects_a_different_source_identity() {
    let temporary = tempfile::tempdir().unwrap();
    let store = ToolStore::open(temporary.path()).unwrap();
    let spec = parse_tool_spec("npm:fixture").unwrap();
    let mut source = crate::StaticSource::new();
    source.insert_versions(
        spec.raw(),
        VersionListing::new(vec![("1.0.0".into(), false)], BTreeMap::new()).unwrap(),
    );
    store.remote_versions_with(&spec, Some(&source)).unwrap();
    let path = store.version_cache_path(&spec).unwrap();
    let mut record: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    record["spec"] = serde_json::Value::String("npm:other".into());
    fs::write(&path, serde_json::to_vec(&record).unwrap()).unwrap();
    assert!(matches!(
        store.remote_versions_with(&spec, None),
        Err(ToolAcquireError::InvalidSpec(_))
    ));
}

#[tokio::test]
async fn latest_installed_ignores_remote_aliases_and_preserves_provider_prerelease_flags() {
    let temporary = tempfile::tempdir().unwrap();
    let store = ToolStore::open(temporary.path().join("tools")).unwrap();
    for version in ["1.0.0", "2.0.0", "3.0.0"] {
        install_version(&store, temporary.path(), version).await;
    }
    let spec = parse_tool_spec("github:astral-sh/uv").unwrap();
    let mut source = crate::StaticSource::new();
    source.insert_versions(
        spec.raw(),
        VersionListing::new(
            vec![
                ("1.0.0".into(), false),
                ("2.0.0".into(), false),
                ("3.0.0".into(), true),
                ("4.0.0".into(), false),
            ],
            BTreeMap::from([("latest".into(), "4.0.0".into())]),
        )
        .unwrap(),
    );
    store.remote_versions_with(&spec, Some(&source)).unwrap();
    let pointer = store.root_path().join("current/uv");
    let before = fs::read(&pointer).unwrap();
    let tool = crate::ResolvedTool {
        name: "uv".into(),
        spec: spec.raw().into(),
        version: None,
        executable: "uv".into(),
    };
    assert!(store.installed_for(&tool).unwrap().is_none());
    assert_eq!(
        store.latest_installed_for(&tool).unwrap().unwrap().version,
        "2.0.0"
    );
    assert_eq!(fs::read(pointer).unwrap(), before);
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
async fn exact_uninstall_preserves_other_versions_and_clears_only_a_removed_current() {
    let temporary = tempfile::tempdir().unwrap();
    let store = ToolStore::open(temporary.path().join("工具 store with spaces")).unwrap();
    for version in ["1.0.0", "2.0.0", "3.0.0"] {
        install_version(&store, temporary.path(), version).await;
    }
    let current = store.installed_receipt("uv", None).unwrap().unwrap();
    store.remove_version("uv", "2.0.0").await.unwrap();
    assert_eq!(store.installed_receipt("uv", None).unwrap(), Some(current));
    assert_eq!(
        store
            .installed_versions("uv")
            .unwrap()
            .iter()
            .map(|receipt| receipt.version.as_str())
            .collect::<Vec<_>>(),
        ["1.0.0", "3.0.0"]
    );
    for suffix in ["", ".files", ".archive"] {
        assert!(!store
            .root_path()
            .join("receipts/uv")
            .join(format!("2.0.0{suffix}.json"))
            .exists());
    }
    assert!(store.shim_dir().join(shim_name("uv", false)).is_file());
    store.remove_version("uv", "3.0.0").await.unwrap();
    assert!(store.list_sync().unwrap().is_empty());
    assert!(store.installed_receipt("uv", None).unwrap().is_none());
    assert!(store
        .installed_receipt("uv", Some("1.0.0"))
        .unwrap()
        .is_some());
    assert!(!store.shim_dir().join(shim_name("uv", false)).exists());
    store.remove_version("uv", "3.0.0").await.unwrap();
    for version in ["", "../1.0.0", "1.0.0/../../outside"] {
        assert!(store.remove_version("uv", version).await.is_err());
    }
    assert!(store
        .installed_receipt("uv", Some("1.0.0"))
        .unwrap()
        .is_some());
}

#[tokio::test]
async fn uninstall_selection_prefers_exact_labels_and_retains_ambiguous_prefix_candidates() {
    let temporary = tempfile::tempdir().unwrap();
    let store = ToolStore::open(temporary.path().join("tools")).unwrap();
    for version in ["1.2", "1.2.3", "1.2.9", "1.20.0", "ref-main", "system"] {
        install_version(&store, temporary.path(), version).await;
    }
    let spec = parse_tool_spec("github:astral-sh/uv").unwrap();
    let mut source = crate::StaticSource::new();
    source.insert_versions(
        spec.raw(),
        VersionListing::new(
            ["1.2", "1.2.3", "1.2.9", "1.20.0"]
                .into_iter()
                .map(|version| (version.into(), false))
                .collect(),
            BTreeMap::from([("stable".into(), "1.2.9".into())]),
        )
        .unwrap(),
    );
    store.remote_versions_with(&spec, Some(&source)).unwrap();
    for (request, expected) in [
        ("1.2", vec!["1.2"]),
        ("1", vec!["1.2", "1.2.3", "1.2.9", "1.20.0"]),
        ("prefix:1.2", vec!["1.2.9"]),
        ("stable", vec!["1.2.9"]),
        ("latest", vec!["1.20.0"]),
        ("ref-main", vec!["ref-main"]),
        ("system", vec!["system"]),
        ("9", vec![]),
    ] {
        let receipts = store.uninstall_candidates("uv", Some(request)).unwrap();
        assert_eq!(
            receipts
                .iter()
                .map(|receipt| receipt.version.as_str())
                .collect::<Vec<_>>(),
            expected
        );
    }
    assert_eq!(store.uninstall_candidates("uv", None).unwrap().len(), 6);
    assert_eq!(store.installed_versions("uv").unwrap().len(), 6);
    assert!(matches!(
        store.uninstall_candidates("uv", Some("branch:missing")),
        Err(ToolAcquireError::UnsupportedVersionRequest(_))
    ));
}

#[tokio::test]
async fn uninstall_operator_does_not_fall_back_when_the_provider_version_is_not_installed() {
    let temporary = tempfile::tempdir().unwrap();
    let store = ToolStore::open(temporary.path().join("tools")).unwrap();
    install_version(&store, temporary.path(), "1.2.3").await;
    assert!(matches!(
        store.uninstall_candidates("uv", Some("prefix:1.2")),
        Err(ToolAcquireError::VersionMetadataMissing { .. })
    ));
    let spec = parse_tool_spec("github:astral-sh/uv").unwrap();
    let mut source = crate::StaticSource::new();
    source.insert_versions(
        spec.raw(),
        VersionListing::new(
            vec![("1.2.3".into(), false), ("1.2.9".into(), false)],
            BTreeMap::new(),
        )
        .unwrap(),
    );
    store.remote_versions_with(&spec, Some(&source)).unwrap();
    assert!(store
        .uninstall_candidates("uv", Some("prefix:1.2"))
        .unwrap()
        .is_empty());
    assert!(store
        .installed_receipt("uv", Some("1.2.3"))
        .unwrap()
        .is_some());
}

#[tokio::test]
async fn removal_selection_admits_missing_executables_but_refuses_foreign_receipts() {
    let temporary = tempfile::tempdir().unwrap();
    let store = ToolStore::open(temporary.path().join("tools")).unwrap();
    install_version(&store, temporary.path(), "1.0.0").await;
    fs::remove_file(store.root_path().join("installs/uv/1.0.0/uv")).unwrap();
    assert!(store.installed_versions("uv").is_err());
    let selected = store.uninstall_candidates("uv", Some("1")).unwrap();
    assert_eq!(selected.len(), 1);
    assert_eq!(selected.first().unwrap().version, "1.0.0");
    let archive = store.root_path().join("receipts/uv/1.0.0.archive.json");
    fs::write(&archive, b"{}").unwrap();
    assert!(store.uninstall_candidates("uv", None).is_err());
    assert!(store.uninstall_candidates("uv", Some("1.0.0")).is_err());
    fs::remove_file(archive).unwrap();
    let receipt = store.root_path().join("receipts/uv/1.0.0.json");
    let mut foreign = selected.into_iter().next().unwrap();
    foreign.name = "node".into();
    fs::write(&receipt, serde_json::to_vec(&foreign).unwrap()).unwrap();
    assert!(matches!(
        store.uninstall_candidates("uv", None),
        Err(ToolAcquireError::ReceiptDisagrees { .. })
    ));
    assert!(store.root_path().join("installs/uv/1.0.0").is_dir());
}

#[tokio::test]
async fn exact_uninstall_cleans_identified_records_when_payloads_are_missing() {
    let temporary = tempfile::tempdir().unwrap();
    let store = ToolStore::open(temporary.path().join("tools")).unwrap();
    for version in ["1.0.0", "2.0.0"] {
        install_version(&store, temporary.path(), version).await;
    }
    fs::remove_dir_all(store.root_path().join("installs/uv/1.0.0")).unwrap();
    fs::remove_file(store.root_path().join("installs/uv/2.0.0/uv")).unwrap();
    for version in ["1.0.0", "2.0.0"] {
        store.remove_version_sync("uv", version).unwrap();
        assert!(!store
            .root_path()
            .join("receipts/uv")
            .join(format!("{version}.json"))
            .exists());
        assert!(!store
            .root_path()
            .join("receipts/uv")
            .join(format!("{version}.files.json"))
            .exists());
        assert!(!store.root_path().join("installs/uv").join(version).exists());
    }
    assert!(store.list_sync().unwrap().is_empty());
    assert!(!store.shim_dir().join(shim_name("uv", false)).exists());
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
