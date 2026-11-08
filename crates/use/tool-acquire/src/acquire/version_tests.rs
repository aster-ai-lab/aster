use super::*;
use crate::{StaticSource, ToolPayload};
use sha2::{Digest, Sha256};

#[test]
fn version_requests_fetch_the_resolved_payload_and_keep_its_identity() {
    let temporary = tempfile::tempdir().unwrap();
    let store = crate::ToolStore::open(temporary.path()).unwrap();
    let mut source = StaticSource::new();
    for version in ["1.2.3", "1.2.9", "2.78.9", "2.79.0", "3.2.1"] {
        let bytes = format!("#!/bin/sh\nprintf '%s\\n' '{version}'\n").into_bytes();
        source.insert(
            "github:fixture/tool",
            ToolPayload::new(version, store::hex_encode(&Sha256::digest(&bytes)), bytes),
        );
    }
    for (request, expected) in [
        ("prefix:1.2", "1.2.9"),
        ("sub-0.0.1:2.79.0", "2.78.9"),
        ("sub-1:latest", "2.79.0"),
    ] {
        let receipt = store
            .acquire_requested_with(
                AcquireRequest {
                    name: "fixture".into(),
                    spec: "github:fixture/tool".into(),
                    version: Some(request.into()),
                    executable_name: "fixture".into(),
                },
                &source,
            )
            .unwrap();
        assert_eq!(receipt.version, expected);
        assert_eq!(
            store
                .installed_receipt("fixture", Some(expected))
                .unwrap()
                .unwrap(),
            receipt
        );
        #[cfg(unix)]
        {
            let output = std::process::Command::new(store.which("fixture").unwrap().unwrap())
                .output()
                .unwrap();
            assert!(output.status.success());
            assert_eq!(output.stdout, format!("{expected}\n").as_bytes());
        }
    }
}

#[test]
fn context_dependent_requests_refuse_before_asking_a_source_to_fetch() {
    let temporary = tempfile::tempdir().unwrap();
    let store = crate::ToolStore::open(temporary.path()).unwrap();
    let source = StaticSource::new();
    for request in ["system", "path:../tools", "branch:main"] {
        let result = store.acquire_requested_with(
            AcquireRequest {
                name: "fixture".into(),
                spec: "github:fixture/tool".into(),
                version: Some(request.into()),
                executable_name: "fixture".into(),
            },
            &source,
        );
        assert!(matches!(
            result,
            Err(ToolAcquireError::UnsupportedVersionRequest(_))
        ));
    }
    assert!(store.list_sync().unwrap().is_empty());
}

#[test]
fn the_existing_exact_pin_api_keeps_reserved_words_as_literal_versions() {
    let temporary = tempfile::tempdir().unwrap();
    let store = crate::ToolStore::open(temporary.path()).unwrap();
    let bytes = b"fixture executable".to_vec();
    let hash = store::hex_encode(&Sha256::digest(&bytes));
    let mut source = StaticSource::new();
    for version in ["system", "ref-main", "latest"] {
        source.insert(
            "github:fixture/tool",
            ToolPayload::new(version, &hash, bytes.clone()),
        );
        let receipt = store
            .acquire_with(
                AcquireRequest {
                    name: "fixture".into(),
                    spec: "github:fixture/tool".into(),
                    version: Some(version.into()),
                    executable_name: "fixture".into(),
                },
                &source,
            )
            .unwrap();
        assert_eq!(receipt.version, version);
    }
    assert!(!store.root_path().join("cache/versions").exists());
}

#[test]
fn immutable_node_latest_refreshes_metadata_without_downloading_installed_bytes_again() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct CountedSource {
        source: StaticSource,
        downloads: AtomicUsize,
        latest: AtomicUsize,
        metadata: AtomicUsize,
    }
    impl ToolSource for CountedSource {
        fn fetch(&self, query: &ToolQuery) -> Result<ToolPayload, ToolAcquireError> {
            self.downloads.fetch_add(1, Ordering::SeqCst);
            self.source.fetch(query)
        }
        fn latest(&self, spec: &crate::ToolSpec) -> Result<String, ToolAcquireError> {
            self.latest.fetch_add(1, Ordering::SeqCst);
            self.source.latest(spec)
        }
        fn versions(&self, _: &crate::ToolSpec) -> Result<crate::VersionListing, ToolAcquireError> {
            self.metadata.fetch_add(1, Ordering::SeqCst);
            Err(ToolAcquireError::VersionListingUnsupported)
        }
    }
    let temp = tempfile::tempdir().unwrap();
    let store = crate::ToolStore::open(temp.path()).unwrap();
    let bytes = b"fixture runtime bytes".to_vec();
    let mut source = StaticSource::new();
    for version in ["18.20.0", "20.11.0"] {
        source.insert(
            "core:node",
            ToolPayload::new(
                version,
                store::hex_encode(&Sha256::digest(&bytes)),
                bytes.clone(),
            ),
        );
    }
    source.insert_latest("core:node", "20.11.0");
    let source = CountedSource {
        source,
        downloads: AtomicUsize::new(0),
        latest: AtomicUsize::new(0),
        metadata: AtomicUsize::new(0),
    };
    for _ in 0..2 {
        assert_eq!(
            store
                .acquire_requested_with(
                    AcquireRequest {
                        name: "node".into(),
                        spec: "core:node".into(),
                        version: None,
                        executable_name: "node".into()
                    },
                    &source
                )
                .unwrap()
                .version,
            "20.11.0"
        );
    }
    assert_eq!(source.latest.load(Ordering::SeqCst), 2);
    assert_eq!(source.downloads.load(Ordering::SeqCst), 1);
    for version in ["18.20.0", "20.11.0"] {
        let receipt = store
            .acquire_requested_with(
                AcquireRequest {
                    name: "node".into(),
                    spec: "core:node".into(),
                    version: Some(version.into()),
                    executable_name: "node".into(),
                },
                &source,
            )
            .unwrap();
        assert_eq!(receipt.version, version);
        assert_eq!(
            store.installed_receipt("node", None).unwrap(),
            Some(receipt)
        );
    }
    assert_eq!(source.latest.load(Ordering::SeqCst), 2);
    assert_eq!(source.metadata.load(Ordering::SeqCst), 2);
    assert_eq!(source.downloads.load(Ordering::SeqCst), 2);
}
