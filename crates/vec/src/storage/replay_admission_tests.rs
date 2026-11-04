use super::test_support::{doc, schema};
use super::{wal, StorageHandle, WalOperation};
use crate::{ConfigBuilder, ErrorCode, StorageCeilings};

#[test]
fn wal_replay_ceiling_refuses_growth_before_changing_committed_bytes() {
    let root = tempfile::tempdir().unwrap();
    let limit = 1024;
    let ceilings = StorageCeilings::new()
        .try_with_max_wal_replay_bytes(limit)
        .unwrap();
    let mut storage = StorageHandle::create(root.path(), &schema(), false, ceilings).unwrap();
    let mut expected = Vec::new();
    let mut refused = false;
    for revision in 1..=128 {
        let item = doc(&format!("doc-{revision:03}"));
        let before = storage.manifest.clone();
        let path = wal::segment_path(root.path(), before.wal_active_seq);
        let bytes = if path.exists() {
            std::fs::read(&path).unwrap()
        } else {
            Vec::new()
        };
        match storage.append(
            revision,
            WalOperation::Insert {
                docs: vec![item.clone()],
            },
            &ConfigBuilder::default(),
        ) {
            Ok(_) => {
                expected.push(item);
                assert!(
                    storage.manifest.wal_bytes_since_checkpoint <= limit,
                    "acknowledged WAL crossed the admitted recovery ceiling: {} > {limit}",
                    storage.manifest.wal_bytes_since_checkpoint
                );
            }
            Err(error) => {
                assert_eq!(error.code, ErrorCode::ResourceExhausted);
                assert_eq!(storage.manifest, before);
                assert_eq!(std::fs::read(path).unwrap(), bytes);
                refused = true;
                break;
            }
        }
    }
    assert!(refused && !expected.is_empty());
    drop(storage);
    let (reopened, _, actual) = StorageHandle::open(root.path(), false, ceilings).unwrap();
    assert_eq!(actual, expected);
    assert_eq!(
        reopened.manifest.revision,
        u64::try_from(expected.len()).unwrap()
    );
}

#[test]
fn wal_replay_ceiling_refuses_one_oversized_frame_without_a_partial_append() {
    let root = tempfile::tempdir().unwrap();
    let ceilings = StorageCeilings::new()
        .try_with_max_wal_replay_bytes(32)
        .unwrap();
    let mut storage = StorageHandle::create(root.path(), &schema(), false, ceilings).unwrap();
    let before = storage.manifest.clone();
    let error = storage
        .append(
            1,
            WalOperation::Insert {
                docs: vec![doc("oversized")],
            },
            &ConfigBuilder::default(),
        )
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceExhausted);
    assert_eq!(storage.manifest, before);
    assert!(!wal::segment_path(root.path(), before.wal_active_seq).exists());
    drop(storage);
    let (reopened, _, actual) = StorageHandle::open(root.path(), false, ceilings).unwrap();
    assert!(actual.is_empty());
    assert_eq!(reopened.manifest.revision, 0);
}
