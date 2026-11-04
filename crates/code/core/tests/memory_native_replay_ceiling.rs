#![cfg(feature = "aster-vec-fts")]

use aster_core::durable_memory_vec::{MemoryRecord, VecMemoryStore};
use aster_durable_memory::MemoryStore;
use aster_vec::{ConfigBuilder, StorageCeilings};
use std::collections::BTreeMap;

#[tokio::test]
async fn acknowledged_native_records_reopen_with_the_same_replay_ceiling() {
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("replay 记忆");
    let ceilings = StorageCeilings::new()
        .try_with_max_wal_replay_bytes(1024)
        .unwrap();
    aster_vec::initialize(Some(&ConfigBuilder::default().storage_ceilings(ceilings))).unwrap();
    let memory = VecMemoryStore::open(&directory).unwrap();
    let mut expected = BTreeMap::new();
    for index in 0..10 {
        let id = format!("source-{index}");
        let content = format!("acknowledged original replay source {index}");
        memory
            .put(&MemoryRecord {
                id: id.clone(),
                content: content.clone(),
                tags: Vec::new(),
                importance: 0.5,
                memory_type: "semantic".into(),
                timestamp: "2026-11-01T00:00:00Z".into(),
                meta: "{}".into(),
            })
            .unwrap();
        expected.insert(id, content);
    }
    drop(memory);
    let reopened = VecMemoryStore::open(&directory).unwrap();
    let actual = reopened
        .get_recent(10)
        .await
        .unwrap()
        .into_iter()
        .map(|item| (item.id, item.content))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(actual, expected);
}
