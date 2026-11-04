#![cfg(feature = "aster-vec-fts")]

use aster_core::durable_memory_vec::{MemoryRecord, VecMemoryStore};
use aster_vec::{
    Collection, CollectionSchema, ConfigBuilder, DataType, Doc, Durability, FieldSchema, IoBackend,
    StorageCeilings,
};

#[test]
fn native_memory_preserves_embedder_process_defaults() {
    let root = tempfile::tempdir().unwrap();
    let snapshot_limit = 2 * 1024 * 1024;
    let ceilings = StorageCeilings::new()
        .try_with_max_snapshot_bytes(snapshot_limit)
        .unwrap();
    let host = ConfigBuilder::default()
        .durability(Durability::Manual)
        .io_backend(IoBackend::Mmap)
        .storage_ceilings(ceilings)
        .wal_max_ops(1)
        .wal_max_bytes(1);
    aster_vec::initialize(Some(&host)).unwrap();
    let memory = VecMemoryStore::open(&root.path().join("memory 空间")).unwrap();
    memory
        .put(&MemoryRecord {
            id: "original".into(),
            content: "native memory keeps the host defaults intact".into(),
            tags: Vec::new(),
            importance: 0.5,
            memory_type: "semantic".into(),
            timestamp: "2026-11-01T00:00:00Z".into(),
            meta: "{}".into(),
        })
        .unwrap();

    let schema = CollectionSchema::builder("host-probe")
        .add_field(FieldSchema::new("title", DataType::String, false, 0).unwrap())
        .build()
        .unwrap();
    let directory = root.path().join("host probe");
    let probe = Collection::create(directory.to_str().unwrap(), &schema, None).unwrap();
    assert_eq!(probe.stats().unwrap().io_backend, IoBackend::Mmap);
    assert_eq!(
        probe.stats().unwrap().storage_ceilings.max_snapshot_bytes(),
        snapshot_limit
    );
    for index in 0..3 {
        let mut item = Doc::with_pk(format!("probe-{index}")).unwrap();
        item.add_string("title", "host remains manual").unwrap();
        probe.insert(&[&item]).unwrap();
    }
    assert_eq!(probe.stats().unwrap().wal_ops_since_checkpoint, 3);
    assert!(probe.health().unwrap().is_healthy());
    probe.close().unwrap();
}
