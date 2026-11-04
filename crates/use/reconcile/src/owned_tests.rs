use std::fs;

use crate::{
    files_sha256, PackageFile, PackageSpec, Payload, ReconcileError, ReconcileStore, SurfaceKind,
    SurfaceSpec,
};

fn spec() -> PackageSpec {
    let files = vec![PackageFile {
        path: "toolchain.acl".into(),
        bytes: b"source declaration".to_vec(),
    }];
    PackageSpec {
        package_id: "aster/toolchain-fixture".into(),
        surfaces: vec![SurfaceSpec {
            kind: SurfaceKind::Plugin,
            id: "toolchain".into(),
            entry: Some("toolchain.acl".into()),
            sha256: files_sha256(&files).unwrap(),
            payload: Payload::Files(files),
        }],
    }
}

#[test]
fn failed_owned_preparation_keeps_the_previous_runtime_and_cleans_the_candidate() {
    let root = tempfile::Builder::new()
        .prefix("owned runtime 空格 ")
        .tempdir()
        .unwrap();
    let store = ReconcileStore::open(root.path()).unwrap();
    let first = store
        .apply_with_owned_sync(&spec(), |path| {
            fs::write(path.join("runtime"), b"working version")?;
            Ok("a".repeat(64))
        })
        .unwrap();
    let mut failed_directory = None;
    let error = store
        .apply_with_owned_sync(&spec(), |path| {
            failed_directory = Some(path.to_path_buf());
            fs::write(path.join("partial"), b"downloaded bytes")?;
            Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "fixture checksum failure",
            ))
        })
        .unwrap_err();
    assert!(
        matches!(error, ReconcileError::Io { source, .. } if source.kind()==std::io::ErrorKind::InvalidData)
    );
    assert_eq!(
        store
            .current_sync("aster/toolchain-fixture")
            .unwrap()
            .unwrap(),
        first
    );
    assert_eq!(
        fs::read(first.owned_directory().join("runtime")).unwrap(),
        b"working version"
    );
    assert!(!failed_directory.unwrap().exists());
}

#[test]
fn runtime_receipt_and_lifetime_follow_the_exact_publication_lease() {
    let root = tempfile::tempdir().unwrap();
    let store = ReconcileStore::open(root.path()).unwrap();
    let first = store
        .apply_with_owned_sync(&spec(), |path| {
            fs::write(path.join("runtime"), b"old")?;
            Ok("a".repeat(64))
        })
        .unwrap();
    let lease = store.acquire_sync(&first.cursor()).unwrap();
    let second = store
        .apply_with_owned_sync(&spec(), |path| {
            fs::write(path.join("runtime"), b"new")?;
            Ok("b".repeat(64))
        })
        .unwrap();
    assert_ne!(first.cursor(), second.cursor());
    assert_eq!(
        first.owned_digest(),
        Some(format!("sha256:{}", "a".repeat(64)).as_str())
    );
    assert_eq!(
        store
            .current_sync("aster/toolchain-fixture")
            .unwrap()
            .unwrap()
            .owned_digest(),
        Some(format!("sha256:{}", "b".repeat(64)).as_str())
    );
    store.withdraw_sync("aster/toolchain-fixture").unwrap();
    assert!(!second.owned_directory().exists());
    assert_eq!(
        fs::read(first.owned_directory().join("runtime")).unwrap(),
        b"old"
    );
    lease.release_sync().unwrap();
    assert!(!first.owned_directory().exists());
}

#[test]
fn ordinary_publications_keep_their_existing_schema_and_cursor_reuse() {
    let root = tempfile::tempdir().unwrap();
    let store = ReconcileStore::open(root.path()).unwrap();
    let first = store.apply_sync(&spec()).unwrap();
    assert!(first.owned_digest().is_none());
    assert!(!first.owned_directory().exists());
    assert_eq!(store.apply_sync(&spec()).unwrap().cursor(), first.cursor());
    assert!(matches!(
        store.apply_with_owned_sync(&spec(), |_| Ok("not a digest".into())),
        Err(ReconcileError::DigestMismatch)
    ));
    assert_eq!(
        store
            .current_sync("aster/toolchain-fixture")
            .unwrap()
            .unwrap()
            .cursor(),
        first.cursor()
    );
}
