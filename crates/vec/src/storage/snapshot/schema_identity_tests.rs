use super::*;
use crate::schema::digest_tests::indexed_schema;

#[test]
fn binary_snapshot_schema_is_canonical_for_existing_readers() {
    let schema = indexed_schema();
    let mut sorted = schema.clone();
    sorted.fields[0]
        .index_params
        .as_mut()
        .unwrap()
        .params
        .sort_keys();
    let expected = rmp_serde::to_vec(&codec::BinarySnapshot::new(4, 1, 0, &sorted, &[])).unwrap();
    let actual = codec::encode_binary_snapshot(4, 1, 0, &schema, &[]).unwrap();
    assert_eq!(actual, expected);
    let actual_delta = codec::DeltaSnapshot::new(5, 2, 1, &schema, 1, 0, Vec::new(), &[]);
    let expected_delta = codec::DeltaSnapshot::new(5, 2, 1, &sorted, 1, 0, Vec::new(), &[]);
    assert_eq!(
        rmp_serde::to_vec(&actual_delta).unwrap(),
        rmp_serde::to_vec(&expected_delta).unwrap()
    );
}

#[test]
fn legacy_preserved_order_snapshot_keeps_exact_digest_validation() {
    let bytes = include_bytes!("fixtures/schema-preserved-order-v4.bin");
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/schema-preserved-order-v4.json")).unwrap();
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("segments")).unwrap();
    fs::write(root.path().join(binary_relative_path(1)), bytes).unwrap();
    let mut manifest = Manifest::new("identity", fixture["schema_digest"].as_str().unwrap());
    manifest.generation = 1;
    manifest.docs_checksum = checksum(bytes);
    let (schema, docs) = read(root.path(), &manifest, MAX_SNAPSHOT_BYTES).unwrap();
    assert!(docs.is_empty());
    assert_eq!(schema.digest(), "f0535b24");
    assert_ne!(schema.digest(), manifest.schema_digest);
    manifest.schema_digest = "00000000".into();
    assert_eq!(
        read(root.path(), &manifest, MAX_SNAPSHOT_BYTES)
            .unwrap_err()
            .code,
        crate::ErrorCode::InternalError
    );
    manifest.schema_digest = fixture["schema_digest"].as_str().unwrap().into();
    manifest.wal_active_seq = 1;
    crate::storage::manifest::write_with_faults(
        root.path(),
        &manifest,
        true,
        &FaultInjector::default(),
    )
    .unwrap();
    let (mut storage, schema, docs) =
        crate::storage::StorageHandle::open(root.path(), false, crate::StorageCeilings::default())
            .unwrap();
    storage.checkpoint(&schema, &docs, 0, true).unwrap();
    assert_eq!(storage.manifest.schema_digest, "f0535b24");
    assert_eq!(storage.manifest.generation, 2);
    drop(storage);
    let (_, reopened, records) =
        crate::storage::StorageHandle::open(root.path(), false, crate::StorageCeilings::default())
            .unwrap();
    assert_eq!(reopened.digest(), "f0535b24");
    assert!(records.is_empty());
}
