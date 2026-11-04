use super::*;
use crate::{Collection, CollectionSchema, DataType, Doc, ErrorCode, FieldSchema};

#[test]
fn scoped_thresholds_override_defaults_without_changing_other_handles() {
    let process = ConfigBuilder::default()
        .durability(Durability::Manual)
        .wal_max_ops(3)
        .wal_max_bytes(4096);
    let mut options = CollectionOptions::default();
    options.set_durability(Durability::Always).unwrap();
    options
        .set_wal_checkpoint_thresholds(128, 8 * 1024 * 1024)
        .unwrap();
    let resolved = resolve_options_config(&options, process.clone());
    assert_eq!(resolved.durability, Durability::Always);
    assert_eq!(resolved.wal_max_ops, Some(128));
    assert_eq!(resolved.wal_max_bytes, Some(8 * 1024 * 1024));
    let inherited = resolve_options_config(&CollectionOptions::default(), process);
    assert_eq!(inherited.durability, Durability::Manual);
    assert_eq!(inherited.wal_max_ops, Some(3));
    assert_eq!(inherited.wal_max_bytes, Some(4096));
}

#[test]
fn zero_thresholds_refuse_admission_without_changing_the_override() {
    let mut options = CollectionOptions::default();
    for (operations, bytes) in [(0, 1), (1, 0), (0, 0)] {
        let error = options
            .set_wal_checkpoint_thresholds(operations, bytes)
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidArgument);
        assert!(options.wal_checkpoint_thresholds.is_none());
    }
    options
        .set_wal_checkpoint_thresholds(128, 8 * 1024 * 1024)
        .unwrap();
    let before = options.wal_checkpoint_thresholds;
    for (operations, bytes) in [(0, 1), (1, 0), (0, 0)] {
        assert_eq!(
            options
                .set_wal_checkpoint_thresholds(operations, bytes)
                .unwrap_err()
                .code,
            ErrorCode::InvalidArgument
        );
        assert_eq!(options.wal_checkpoint_thresholds, before);
    }
}

#[test]
fn scoped_thresholds_reach_the_real_collection_constructor() {
    let root = tempfile::tempdir().unwrap();
    let schema = CollectionSchema::builder("scoped-wal")
        .add_field(FieldSchema::new("title", DataType::String, false, 0).unwrap())
        .build()
        .unwrap();
    let mut options = CollectionOptions::default();
    options.set_durability(Durability::Always).unwrap();
    options
        .set_wal_checkpoint_thresholds(3, 1024 * 1024)
        .unwrap();
    let collection =
        Collection::create(root.path().to_str().unwrap(), &schema, Some(&options)).unwrap();
    for index in 1..=3 {
        let mut item = Doc::with_pk(format!("doc-{index}")).unwrap();
        item.add_string("title", "admitted source").unwrap();
        collection.insert(&[&item]).unwrap();
        assert_eq!(
            collection.stats().unwrap().wal_ops_since_checkpoint,
            index % 3
        );
    }
}
