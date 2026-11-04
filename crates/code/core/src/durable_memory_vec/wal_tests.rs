use super::*;

#[test]
fn native_put_acknowledges_wal_and_reopens_every_pending_record() {
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("wal 记忆");
    let store = VecMemoryStore::open(&directory).unwrap();
    let mut expected = Vec::new();
    for index in 0..129 {
        let mut item = MemoryItem::new(format!("native WAL original source {index:03}"));
        item.id = format!("wal-{index:03}");
        store.put(&item_to_record(&item).unwrap()).unwrap();
        expected.push(item);
        let stats = store.collection.stats().unwrap();
        assert_eq!(stats.revision, index + 1);
        assert_eq!(stats.wal_ops_since_checkpoint, (index + 1) % 128);
        assert!(store.collection.health().unwrap().is_healthy());
    }
    drop(store);
    let manifest: serde_json::Value = serde_json::from_slice(
        &std::fs::read(directory.join(COLLECTION_NAME).join("manifest.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(manifest["wal_ops_since_checkpoint"], 1);
    let reopened = VecMemoryStore::open(&directory).unwrap();
    assert_eq!(reopened.collection.count().unwrap(), 129);
    for item in expected {
        let actual = reopened.retrieve_blocking(&item.id).unwrap().unwrap();
        assert_eq!(actual.id, item.id);
        assert_eq!(actual.content, item.content);
    }
}

#[tokio::test]
async fn lazy_put_uses_the_same_pending_wal_owner() {
    let root = tempfile::tempdir().unwrap();
    let store = LazyFileMemoryStore::new(root.path());
    let item = MemoryItem::new("lazy WAL committed original source");
    store.store(item.clone()).await.unwrap();
    let stats = store
        .with_store(|owner| owner.collection.stats())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stats.revision, 1);
    assert_eq!(stats.wal_ops_since_checkpoint, 1);
    assert_eq!(
        store.retrieve(&item.id).await.unwrap().unwrap().content,
        item.content
    );
    drop(store);
    let reopened = LazyFileMemoryStore::new(root.path());
    assert_eq!(reopened.count().await.unwrap(), 1);
}
