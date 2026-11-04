#![cfg(feature = "aster-vec-fts")]

use aster_core::durable_memory_vec::VecMemoryStore;
use aster_durable_memory::{MemoryItem, MemoryStore};
use aster_vec::{ConfigBuilder, ErrorCode, StorageCeilings};

#[tokio::test]
async fn failed_checkpoint_cannot_hide_a_committed_canonical_memory() {
    let root = tempfile::tempdir().unwrap();
    let ceilings = StorageCeilings::new()
        .try_with_max_snapshot_bytes(2048)
        .unwrap()
        .try_with_max_wal_replay_bytes(32 * 1024)
        .unwrap();
    aster_vec::initialize(Some(&ConfigBuilder::default().storage_ceilings(ceilings))).unwrap();
    let store = VecMemoryStore::open(&root.path().join("failed checkpoint 记忆")).unwrap();
    let mut first =
        MemoryItem::new("Durable configuration preference ".repeat(180)).with_importance(0.2);
    first.id = "canonical".into();
    let error = store.store(first.clone()).await.unwrap_err();
    assert_eq!(
        error.downcast_ref::<aster_vec::Error>().unwrap().code,
        ErrorCode::ResourceExhausted
    );
    assert_eq!(store.count().await.unwrap(), 1);
    let mut duplicate = MemoryItem::new(first.content.to_uppercase()).with_importance(0.9);
    duplicate.id = "incoming".into();
    let error = store.store_and_return(duplicate).await.unwrap_err();
    assert_eq!(
        error.downcast_ref::<aster_vec::Error>().unwrap().code,
        ErrorCode::ResourceExhausted
    );
    assert_eq!(store.count().await.unwrap(), 1);
    assert!(store.retrieve("incoming").await.unwrap().is_none());
    assert_eq!(
        store
            .retrieve("canonical")
            .await
            .unwrap()
            .unwrap()
            .importance,
        0.9
    );
    store.delete("canonical").await.unwrap();
    assert_eq!(store.count().await.unwrap(), 0);
    let stored = store
        .store_and_return(MemoryItem::new("small replacement after recovery"))
        .await
        .unwrap();
    assert_eq!(store.count().await.unwrap(), 1);
    assert_eq!(
        store.retrieve(&stored.id).await.unwrap().unwrap().content,
        stored.content
    );
}
