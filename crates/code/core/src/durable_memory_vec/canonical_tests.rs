use super::*;
use management::{MemoryManagementCode, MemoryManagementRequest, MemoryManagementResponse};

fn item(id: &str, content: &str, importance: f32) -> MemoryItem {
    let mut item = MemoryItem::new(content).with_importance(importance);
    item.id = id.into();
    item
}

fn assert_merged(
    actual: &MemoryItem,
    expected: &MemoryItem,
    started: DateTime<Utc>,
    finished: DateTime<Utc>,
) {
    let observed = &actual.metadata["last_duplicate_at"];
    let timestamp = DateTime::parse_from_rfc3339(observed).unwrap();
    assert!(timestamp >= started && timestamp <= finished);
    let mut expected = expected.clone();
    expected
        .metadata
        .insert("last_duplicate_at".into(), observed.clone());
    assert_eq!(
        serde_json::to_value(actual).unwrap(),
        serde_json::to_value(expected).unwrap()
    );
}

#[tokio::test]
async fn direct_store_returns_and_persists_the_merged_canonical_item() {
    let root = tempfile::tempdir().unwrap();
    let store = VecMemoryStore::open(root.path()).unwrap();
    let first = item(
        "canonical",
        "durable preference for explicit configuration",
        0.2,
    )
    .with_tag("original");
    let incoming = item("incoming", &first.content, 0.9).with_tag("later");
    let expected = first.clone().merge_duplicate(incoming.clone());
    store.store(first).await.unwrap();
    let started = Utc::now();
    let stored = store.store_and_return(incoming).await.unwrap();
    assert_eq!(stored.id, expected.id);
    assert_merged(&stored, &expected, started, Utc::now());
    assert_eq!(
        serde_json::to_value(store.retrieve(&stored.id).await.unwrap().unwrap()).unwrap(),
        serde_json::to_value(stored).unwrap(),
    );
    assert_eq!(store.count().await.unwrap(), 1);
}

#[tokio::test]
async fn lazy_store_reuses_normalized_fingerprints_after_reopen() {
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("记忆 with spaces");
    let first = item(
        "canonical",
        "Durable preference for explicit configuration",
        0.2,
    );
    {
        let store = LazyFileMemoryStore::new(&directory);
        store.store(first.clone()).await.unwrap();
    }
    let store = LazyFileMemoryStore::new(&directory);
    let incoming = item(
        "incoming",
        "  DURABLE   preference for explicit\nconfiguration  ",
        0.9,
    );
    let expected = first.merge_duplicate(incoming.clone());
    let started = Utc::now();
    let stored = store.store_and_return(incoming).await.unwrap();
    assert_eq!(stored.id, expected.id);
    assert_eq!(store.count().await.unwrap(), 1);
    assert_eq!(stored.content_lower, stored.content.to_lowercase());
    assert_merged(&stored, &expected, started, Utc::now());
}

#[test]
fn raw_put_keeps_exact_duplicate_noop_after_reopen() {
    let root = tempfile::tempdir().unwrap();
    let first = item("canonical", "raw short", 0.2);
    {
        let store = VecMemoryStore::open(root.path()).unwrap();
        store.put(&item_to_record(&first).unwrap()).unwrap();
    }
    let store = VecMemoryStore::open(root.path()).unwrap();
    let incoming = item("incoming", "  raw short  ", 0.9);
    store.put(&item_to_record(&incoming).unwrap()).unwrap();
    assert_eq!(store.record_count().unwrap(), 1);
    assert!(store
        .collection
        .fetch_with_options(&["incoming"], None, false)
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn same_id_same_content_replaces_metadata_through_memory_store() {
    let root = tempfile::tempdir().unwrap();
    let store = VecMemoryStore::open(root.path()).unwrap();
    let first = item("same", "a durable same id metadata replacement", 0.2).with_tag("old");
    store.store(first.clone()).await.unwrap();
    let replacement = item("same", &first.content, 0.9).with_tag("replacement");
    store.store(replacement.clone()).await.unwrap();
    let actual = store.retrieve("same").await.unwrap().unwrap();
    assert_eq!(
        serde_json::to_value(actual).unwrap(),
        serde_json::to_value(replacement).unwrap()
    );
}

#[tokio::test]
async fn memory_store_keeps_short_generic_fragments_distinct() {
    let root = tempfile::tempdir().unwrap();
    let store = VecMemoryStore::open(root.path()).unwrap();
    store.store(item("one", "ok", 0.2)).await.unwrap();
    store.store(item("two", "ok", 0.9)).await.unwrap();
    assert_eq!(store.count().await.unwrap(), 2);
}

#[test]
fn management_rejects_exact_content_conflicts_after_reopen() {
    let root = tempfile::tempdir().unwrap();
    let first = item("first", "first durable content", 0.2);
    let second = item("second", "second durable content", 0.2);
    {
        let store = VecMemoryStore::open(root.path()).unwrap();
        store.put(&item_to_record(&first).unwrap()).unwrap();
        store.put(&item_to_record(&second).unwrap()).unwrap();
    }
    let store = VecMemoryStore::open(root.path()).unwrap();
    let MemoryManagementResponse::Read { item } = store
        .manage(MemoryManagementRequest::Read {
            id: second.id.clone(),
        })
        .unwrap()
    else {
        panic!("read response")
    };
    let error = store
        .manage(MemoryManagementRequest::Update {
            id: second.id,
            revision: item.revision,
            content: first.content,
        })
        .unwrap_err();
    assert_eq!(error.code, MemoryManagementCode::Conflict);
}

#[test]
fn raw_same_content_retry_keeps_its_original_noop_policy() {
    let root = tempfile::tempdir().unwrap();
    let store = VecMemoryStore::open(root.path()).unwrap();
    let first = item("raw", "ok", 0.2);
    store.put(&item_to_record(&first).unwrap()).unwrap();
    let changed = item("raw", "ok", 0.9);
    store.put(&item_to_record(&changed).unwrap()).unwrap();
    let docs = store
        .collection
        .fetch_with_options(&["raw"], None, false)
        .unwrap();
    assert_eq!(doc_to_item(&docs[0]).unwrap().importance, first.importance);
}
