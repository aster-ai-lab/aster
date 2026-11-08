use super::test_support::{doc, schema};
use super::{snapshot, StorageHandle, WalOperation};
use crate::config::{ConfigBuilder, Durability};
use crate::storage_ceilings::StorageCeilings;
use tempfile::tempdir;

#[test]
fn repeated_checkpoints_bound_retained_history_and_preserve_reopen() {
    let temporary = tempdir().unwrap();
    let root = temporary.path().join("checkpoint 空间");
    let schema = schema();
    let mut storage =
        StorageHandle::create(&root, &schema, false, StorageCeilings::default()).unwrap();
    let config = ConfigBuilder::default().durability(Durability::Always);
    let mut expected = Vec::new();
    for revision in 1..=70 {
        let item = doc(&format!("doc-{revision:03}"));
        storage
            .append(
                revision,
                WalOperation::Insert {
                    docs: vec![item.clone()],
                },
                &config,
            )
            .unwrap();
        expected.push(item);
        storage
            .checkpoint(&schema, &expected, revision, true)
            .unwrap();
        let snapshots = std::fs::read_dir(root.join("segments")).unwrap().count();
        assert!(
            snapshots <= 32,
            "unbounded snapshot history at revision {revision}: {snapshots}"
        );
        if storage.manifest.generation % 32 == 0 {
            assert_eq!(
                snapshots, 1,
                "the committed full snapshot releases its ancestors"
            );
            let bytes = std::fs::read(
                root.join(snapshot::binary_relative_path(storage.manifest.generation)),
            )
            .unwrap();
            assert_eq!(&bytes[..2], &[0x95, 0x04], "reuse the original full format");
        }
        if [31, 63].contains(&storage.manifest.generation) {
            let generation = storage.manifest.generation;
            storage
                .checkpoint(&schema, &expected, revision, true)
                .unwrap();
            assert_eq!(
                storage.manifest.generation, generation,
                "an unchanged boundary flush must not compact"
            );
        }
        if revision % 17 == 0 {
            drop(storage);
            let (reopened, loaded_schema, loaded) =
                StorageHandle::open(&root, false, StorageCeilings::default()).unwrap();
            assert_eq!(loaded_schema, schema);
            assert_eq!(loaded, expected);
            storage = reopened;
        }
    }
    let before = storage.manifest.generation;
    storage.checkpoint(&schema, &expected, 70, true).unwrap();
    assert_eq!(
        storage.manifest.generation, before,
        "an unchanged flush preserves its generation"
    );
    drop(storage);
    let (_, loaded_schema, loaded) =
        StorageHandle::open(&root, false, StorageCeilings::default()).unwrap();
    assert_eq!(loaded_schema, schema);
    assert_eq!(loaded, expected);
}
