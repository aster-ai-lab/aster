use super::Collection;
use crate::{CollectionSchema, ConfigBuilder, DataType, Doc, Durability, ErrorCode, FieldSchema};

#[test]
fn always_sync_uses_the_existing_operation_checkpoint_limit() {
    let root = tempfile::tempdir().unwrap();
    let collection = configured_collection(
        root.path(),
        ConfigBuilder::default()
            .durability(Durability::Always)
            .wal_max_ops(3)
            .wal_max_bytes(1024 * 1024),
    );
    for revision in 1..=3 {
        let item = doc(&format!("doc-{revision}"), "original");
        assert_eq!(collection.insert(&[&item]).unwrap().success_count, 1);
        assert_eq!(
            collection.stats().unwrap().wal_ops_since_checkpoint,
            revision % 3
        );
        assert!(collection.health().unwrap().is_healthy());
    }
    let edited = doc("doc-1", "edited");
    collection.update(&[&edited]).unwrap();
    assert_eq!(collection.stats().unwrap().wal_ops_since_checkpoint, 1);
    collection.delete(&["doc-2"]).unwrap();
    assert_eq!(collection.stats().unwrap().wal_ops_since_checkpoint, 2);
    collection.upsert(&[&doc("doc-4", "upserted")]).unwrap();
    assert_eq!(collection.stats().unwrap().wal_ops_since_checkpoint, 0);
    drop(collection);
    let reopened = Collection::open(root.path().to_str().unwrap(), None).unwrap();
    assert_eq!(reopened.count().unwrap(), 3);
    assert_eq!(
        reopened.fetch(&["doc-1"]).unwrap()[0]
            .get_string("title")
            .unwrap()
            .as_deref(),
        Some("edited")
    );
}

#[test]
fn always_sync_uses_the_existing_byte_checkpoint_limit() {
    let root = tempfile::tempdir().unwrap();
    let collection = configured_collection(
        root.path(),
        ConfigBuilder::default()
            .durability(Durability::Always)
            .wal_max_ops(1000)
            .wal_max_bytes(1),
    );
    collection
        .insert(&[&doc("large", &"x".repeat(2048))])
        .unwrap();
    assert_eq!(collection.stats().unwrap().wal_ops_since_checkpoint, 0);
    assert_eq!(collection.stats().unwrap().wal_bytes_since_checkpoint, 0);
    assert!(collection.health().unwrap().is_healthy());
}

#[test]
fn manual_sync_preserves_explicit_checkpoint_semantics() {
    let root = tempfile::tempdir().unwrap();
    let collection = configured_collection(
        root.path(),
        ConfigBuilder::default()
            .durability(Durability::Manual)
            .wal_max_ops(1)
            .wal_max_bytes(1),
    );
    for index in 0..3 {
        collection
            .insert(&[&doc(&format!("doc-{index}"), "manual")])
            .unwrap();
    }
    assert_eq!(collection.stats().unwrap().wal_ops_since_checkpoint, 3);
    collection.flush().unwrap();
    assert_eq!(collection.stats().unwrap().wal_ops_since_checkpoint, 0);
}

#[test]
fn checkpoint_failure_preserves_the_committed_revision_for_retry_and_reopen() {
    let root = tempfile::tempdir().unwrap();
    let collection = configured_collection(
        root.path(),
        ConfigBuilder::default()
            .durability(Durability::Always)
            .wal_max_ops(3),
    );
    for index in 1..=2 {
        collection
            .insert(&[&doc(&format!("doc-{index}"), "before checkpoint")])
            .unwrap();
    }
    let collision = root
        .path()
        .join("segments")
        .join("snapshot-00000000000000000002.bin");
    std::fs::create_dir(&collision).unwrap();
    let error = collection
        .insert(&[&doc("doc-3", "committed before failed checkpoint")])
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::InternalError);
    assert_eq!(collection.count().unwrap(), 3);
    assert_eq!(collection.stats().unwrap().revision, 3);
    assert_eq!(collection.stats().unwrap().wal_ops_since_checkpoint, 3);
    assert!(collection.health().unwrap().is_healthy());
    std::fs::remove_dir(collision).unwrap();
    collection.flush().unwrap();
    assert_eq!(collection.stats().unwrap().wal_ops_since_checkpoint, 0);
    drop(collection);
    let reopened = Collection::open(root.path().to_str().unwrap(), None).unwrap();
    assert_eq!(reopened.count().unwrap(), 3);
    assert_eq!(reopened.stats().unwrap().revision, 3);
    assert_eq!(
        reopened.fetch(&["doc-3"]).unwrap()[0]
            .get_string("title")
            .unwrap()
            .as_deref(),
        Some("committed before failed checkpoint")
    );
}

fn configured_collection(root: &std::path::Path, config: ConfigBuilder) -> Collection {
    let schema = CollectionSchema::builder("wal-checkpoint")
        .add_field(FieldSchema::new("title", DataType::String, false, 0).unwrap())
        .build()
        .unwrap();
    let collection = Collection::create(root.to_str().unwrap(), &schema, None).unwrap();
    // Configure only this real handle, leaving unrelated process defaults alone.
    collection.inner.state.write().unwrap().config = config;
    collection
}

fn doc(id: &str, title: &str) -> Doc {
    let mut item = Doc::with_pk(id).unwrap();
    item.add_string("title", title).unwrap();
    item
}
