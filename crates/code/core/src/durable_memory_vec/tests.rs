#![cfg(test)]
use super::*;

#[tokio::test]
async fn lazy_store_opens_on_first_search_and_imports_legacy_json_once() {
    let root = tempfile::tempdir().unwrap();
    let memory_dir = root.path().join("memory");
    std::fs::create_dir_all(memory_dir.join("items")).unwrap();
    let item = MemoryItem::new("legacy memory about e2e fixtures");
    std::fs::write(
        memory_dir.join(format!("items/{}.json", item.id)),
        serde_json::to_vec(&item).unwrap(),
    )
    .unwrap();

    let store = LazyFileMemoryStore::new(&memory_dir);
    assert!(!store.is_initialized());
    assert!(!memory_dir.join("durable_memory").exists());

    let matches = MemoryStore::search(&store, "legacy e2e fixtures", 5)
        .await
        .unwrap();
    assert_eq!(matches.len(), 1);
    assert!(matches[0].content.contains("legacy memory"));
    assert!(store.is_initialized());
    assert!(memory_dir.join("durable_memory").exists());
    assert!(memory_dir.join("items.imported").is_dir());
    assert!(!memory_dir.join("items").exists());

    drop(store);
    let store = LazyFileMemoryStore::new(&memory_dir);
    assert!(!store.is_initialized());
    assert_eq!(MemoryStore::count(&store).await.unwrap(), 1);
    assert!(!memory_dir.join("items").exists());
}

#[tokio::test]
async fn a_skipped_legacy_file_keeps_the_items_directory() {
    let root = tempfile::tempdir().unwrap();
    let memory_dir = root.path().join("memory");
    let items = memory_dir.join("items");
    std::fs::create_dir_all(&items).unwrap();
    let item = MemoryItem::new("legacy memory about e2e fixtures");
    std::fs::write(
        items.join(format!("{}.json", item.id)),
        serde_json::to_vec(&item).unwrap(),
    )
    .unwrap();
    std::fs::write(items.join("broken.json"), b"not-json").unwrap();

    let store = LazyFileMemoryStore::new(&memory_dir);
    let matches = MemoryStore::search(&store, "legacy e2e fixtures", 5)
        .await
        .unwrap();
    assert_eq!(matches.len(), 1);
    assert!(items.join("broken.json").is_file());
    assert!(!memory_dir.join("items.imported").exists());

    drop(store);
    let store = LazyFileMemoryStore::new(&memory_dir);
    assert_eq!(MemoryStore::count(&store).await.unwrap(), 1);
    assert!(items.join("broken.json").is_file());
    assert!(!memory_dir.join("items.imported").exists());
}

#[test]
fn an_existing_embedding_column_keeps_its_dimension() {
    let root = tempfile::tempdir().unwrap();
    let memory_dir = root.path().join("memory");
    let collection_path = create_embedding_collection(&memory_dir, 4);
    let store = VecMemoryStore::open(&memory_dir).unwrap();
    store
        .put(&record("m1", "legacy memory about e2e fixtures", 0.5))
        .unwrap();
    let hits = store.recall("legacy e2e fixtures", 5).unwrap();
    assert_eq!(hits.len(), 1);
    drop(store);

    let collection = a3s_vec::Collection::open(&collection_path, None).unwrap();
    let schema = collection.schema().unwrap();
    assert_eq!(schema.vectors().len(), 1);
    assert_eq!(schema.vectors()[0].name(), "embedding");
    assert_eq!(schema.vectors()[0].dimension(), 4);
}

#[test]
fn a_new_collection_has_no_embedding_column() {
    let root = tempfile::tempdir().unwrap();
    let memory_dir = root.path().join("memory");
    drop(VecMemoryStore::open(&memory_dir).unwrap());
    let collection_path = memory_dir.join(COLLECTION_NAME);
    let collection = a3s_vec::Collection::open(collection_path.to_str().unwrap(), None).unwrap();
    assert!(collection.schema().unwrap().vectors().is_empty());
}

#[test]
fn a_foreign_schema_refuses_the_embedding_column() {
    let root = tempfile::tempdir().unwrap();
    let memory_dir = root.path().join("memory");
    let _path = create_named_vector_collection(&memory_dir, "other");
    let error = match VecMemoryStore::open(&memory_dir) {
        Ok(_) => panic!("foreign schema opened"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("embedding column"), "{error}");
    assert!(memory_dir.join(COLLECTION_NAME).exists());
}

#[test]
fn a_second_open_is_refused_while_the_collection_is_already_open() {
    let root = tempfile::tempdir().unwrap();
    let memory_dir = root.path().join("memory");
    let first = VecMemoryStore::open(&memory_dir).unwrap();
    let error = match VecMemoryStore::open(&memory_dir) {
        Ok(_) => panic!("second open succeeded"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("already open"), "{error}");
    drop(first);
    drop(VecMemoryStore::open(&memory_dir).unwrap());
}

#[tokio::test]
async fn lazy_store_writes_and_recalls_an_existing_embedding_column() {
    let root = tempfile::tempdir().unwrap();
    let memory_dir = root.path().join("memory");
    let collection_path = create_embedding_collection(&memory_dir, 4);
    let store = LazyFileMemoryStore::new(&memory_dir);
    MemoryStore::store(&store, MemoryItem::new("legacy memory about e2e fixtures"))
        .await
        .unwrap();
    let matches = MemoryStore::search(&store, "legacy e2e fixtures", 5)
        .await
        .unwrap();
    assert!(
        matches
            .iter()
            .any(|item| item.content.contains("legacy memory")),
        "{matches:?}"
    );
    drop(store);

    let collection = a3s_vec::Collection::open(&collection_path, None).unwrap();
    let schema = collection.schema().unwrap();
    assert_eq!(schema.vectors().len(), 1);
    assert_eq!(schema.vectors()[0].name(), "embedding");
    assert_eq!(schema.vectors()[0].dimension(), 4);
}

#[tokio::test]
async fn lazy_store_opens_without_an_embedding_column() {
    let root = tempfile::tempdir().unwrap();
    let memory_dir = root.path().join("memory");
    let store = LazyFileMemoryStore::new(&memory_dir);
    assert_eq!(MemoryStore::count(&store).await.unwrap(), 0);
    drop(store);
    let collection_path = memory_dir.join(COLLECTION_NAME);
    let collection = a3s_vec::Collection::open(collection_path.to_str().unwrap(), None).unwrap();
    assert!(collection.schema().unwrap().vectors().is_empty());
}

#[tokio::test]
async fn a_foreign_schema_refuses_the_embedding_column_on_the_lazy_store() {
    let root = tempfile::tempdir().unwrap();
    let memory_dir = root.path().join("memory");
    create_named_vector_collection(&memory_dir, "other");
    let store = LazyFileMemoryStore::new(&memory_dir);
    let error = match MemoryStore::count(&store).await {
        Ok(count) => panic!("foreign schema opened and counted {count}"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("embedding column"), "{error}");
    assert!(memory_dir.join(COLLECTION_NAME).exists());
}

#[tokio::test]
async fn lazy_stores_share_the_canonical_collection_until_the_last_handle_drops() {
    let root = tempfile::tempdir().unwrap();
    let memory_dir = root.path().join("memory");
    let first = LazyFileMemoryStore::new(&memory_dir);
    let item = MemoryItem::new("shared collection handoff fact");
    MemoryStore::store(&first, item.clone()).await.unwrap();
    let second = LazyFileMemoryStore::new(memory_dir.join("."));
    assert_eq!(
        MemoryStore::retrieve(&second, &item.id)
            .await
            .unwrap()
            .unwrap()
            .content,
        item.content
    );
    drop(first);
    assert_eq!(MemoryStore::count(&second).await.unwrap(), 1);
    assert!(VecMemoryStore::open(&memory_dir).is_err());
    drop(second);
    let third = LazyFileMemoryStore::new(&memory_dir);
    assert_eq!(MemoryStore::count(&third).await.unwrap(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrent_lazy_first_use_shares_one_open_and_retries_a_failed_open() {
    let root = tempfile::tempdir().unwrap();
    let memory_dir = root.path().join("memory");
    let owner = VecMemoryStore::open(&memory_dir).unwrap();
    let first = std::sync::Arc::new(LazyFileMemoryStore::new(&memory_dir));
    let legacy_items = memory_dir.join("items");
    std::fs::create_dir_all(&legacy_items).unwrap();
    let legacy = MemoryItem::new("legacy atomic import fact");
    std::fs::write(
        legacy_items.join(format!("{}.json", legacy.id)),
        serde_json::to_vec(&legacy).unwrap(),
    )
    .unwrap();
    assert!(MemoryStore::count(first.as_ref()).await.is_err());
    assert!(!first.is_initialized());
    assert!(legacy_items.is_dir());
    drop(owner);
    let second = std::sync::Arc::new(LazyFileMemoryStore::new(&memory_dir));
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(2));
    let run = |store: std::sync::Arc<LazyFileMemoryStore>| {
        let barrier = barrier.clone();
        tokio::spawn(async move {
            barrier.wait().await;
            MemoryStore::count(store.as_ref()).await
        })
    };
    let (left, right) = tokio::join!(run(first.clone()), run(second.clone()));
    assert_eq!(left.unwrap().unwrap(), 1);
    assert_eq!(right.unwrap().unwrap(), 1);
    assert!(Arc::ptr_eq(
        first.store.get().unwrap(),
        second.store.get().unwrap()
    ));
    assert!(memory_dir.join("items.imported").is_dir());
    assert!(!legacy_items.exists());
    MemoryStore::store(first.as_ref(), MemoryItem::new("concurrent shared fact"))
        .await
        .unwrap();
    assert_eq!(
        MemoryStore::search(second.as_ref(), "concurrent", 1)
            .await
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn last_lazy_release_and_new_first_use_share_or_reopen_without_a_lock_collision() {
    let root = tempfile::tempdir().unwrap();
    let memory_dir = root.path().join("memory");
    for _ in 0..16 {
        let current = LazyFileMemoryStore::new(&memory_dir);
        assert_eq!(MemoryStore::count(&current).await.unwrap(), 0);
        let barrier = Arc::new(std::sync::Barrier::new(2));
        let releasing = barrier.clone();
        let (sender, receiver) = std::sync::mpsc::channel();
        let dropper = std::thread::spawn(move || {
            releasing.wait();
            drop(current);
            sender.send(()).unwrap();
        });
        barrier.wait();
        let replacement = LazyFileMemoryStore::new(&memory_dir);
        assert_eq!(MemoryStore::count(&replacement).await.unwrap(), 0);
        receiver
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        dropper.join().unwrap();
        drop(replacement);
    }
    drop(VecMemoryStore::open(&memory_dir).unwrap());
}

fn create_embedding_collection(memory_dir: &std::path::Path, dimension: u32) -> String {
    create_named_vector_collection_with(memory_dir, "embedding", dimension)
}

fn create_named_vector_collection(memory_dir: &std::path::Path, name: &str) -> String {
    create_named_vector_collection_with(memory_dir, name, 4)
}

fn create_named_vector_collection_with(
    memory_dir: &std::path::Path,
    vector_name: &str,
    dimension: u32,
) -> String {
    std::fs::create_dir_all(memory_dir).unwrap();
    a3s_vec::initialize(None).unwrap();
    let collection_path = memory_dir.join(COLLECTION_NAME);
    let path = collection_path.to_str().unwrap().to_owned();
    let mut content = FieldSchema::new("content", DataType::String, false, 0).unwrap();
    content
        .set_index_params(&IndexParams::fts(Some("whitespace"), None, None).unwrap())
        .unwrap();
    let schema = CollectionSchema::builder(COLLECTION_NAME)
        .add_field(content)
        .add_field(FieldSchema::new("tags", DataType::String, true, 0).unwrap())
        .add_field(FieldSchema::new("importance", DataType::Float, true, 0).unwrap())
        .add_field(FieldSchema::new("memory_type", DataType::String, true, 0).unwrap())
        .add_field(FieldSchema::new("timestamp", DataType::String, true, 0).unwrap())
        .add_field(FieldSchema::new("meta", DataType::String, true, 0).unwrap())
        .add_vector_field(
            vector_name,
            DataType::VectorFp32,
            dimension,
            IndexParams::flat(a3s_vec::MetricType::Cosine).unwrap(),
        )
        .build()
        .unwrap();
    drop(a3s_vec::Collection::create_and_open(&path, &schema, None).unwrap());
    path
}

fn record(id: &str, content: &str, importance: f32) -> MemoryRecord {
    MemoryRecord {
        id: id.to_owned(),
        content: content.to_owned(),
        tags: vec!["testing".to_owned()],
        importance,
        memory_type: "semantic".to_owned(),
        timestamp: "2026-01-01T00:00:00+00:00".to_owned(),
        meta: "{}".to_owned(),
    }
}

#[test]
fn insert_and_fts_recall_roundtrip() {
    let root = tempfile::tempdir().unwrap();
    let store = VecMemoryStore::open(root.path()).unwrap();
    store
        .put(&record(
            "m1",
            "the workspace uses rust-analyzer for navigation",
            0.8,
        ))
        .unwrap();
    store
        .put(&record(
            "m2",
            "e2e fixtures live under tests/e2e/fixtures",
            0.6,
        ))
        .unwrap();

    let hits = store.recall("rust-analyzer navigation", 5).unwrap();
    assert!(!hits.is_empty(), "expected at least one hit");
    assert_eq!(hits[0].0, "m1");
}

#[test]
fn reopen_sees_persisted_records() {
    let root = tempfile::tempdir().unwrap();
    {
        let store = VecMemoryStore::open(root.path()).unwrap();
        store
            .put(&record("m3", "memory survives process restart", 0.5))
            .unwrap();
    }
    let reopened = VecMemoryStore::open(root.path()).unwrap();
    let hits = reopened.recall("process restart", 5).unwrap();
    assert!(hits.iter().any(|(id, _)| id == "m3"));
}

#[tokio::test]
async fn memory_store_trait_roundtrip() {
    use a3s_memory::MemoryStore as _;
    let root = tempfile::tempdir().unwrap();
    let store = VecMemoryStore::open(root.path()).unwrap();
    let mut item = MemoryItem::new("the workspace prefers rust-analyzer navigation");
    item.importance = 0.9;
    item.tags = vec!["testing".to_owned()];
    item.memory_type = MemoryType::Procedural;
    store.store(item.clone()).await.unwrap();

    assert_eq!(store.count().await.unwrap(), 1);
    let fetched = store
        .retrieve(&item.id)
        .await
        .unwrap()
        .expect("stored item");
    assert_eq!(fetched.content, item.content);
    assert_eq!(fetched.memory_type, item.memory_type);

    let searched = store.search("rust-analyzer navigation", 5).await.unwrap();
    assert_eq!(searched.len(), 1);
    let by_tag = store
        .search_by_tags(&["testing".to_owned()], 5)
        .await
        .unwrap();
    assert_eq!(by_tag.len(), 1);
    let recent = store.get_recent(5).await.unwrap();
    assert_eq!(recent.len(), 1);
    let important = store.get_important(0.8, 5).await.unwrap();
    assert_eq!(important.len(), 1);
    let not_important = store.get_important(0.95, 5).await.unwrap();
    assert!(not_important.is_empty());

    store.delete(&item.id).await.unwrap();
    assert_eq!(store.count().await.unwrap(), 0);
    assert!(store.retrieve(&item.id).await.unwrap().is_none());
}

#[tokio::test]
async fn search_over_many_records_returns_ranked_hits_without_scan_order() {
    // The id-mapping behind `search` runs on EVERY model prompt; it
    // once walked the whole collection. This pin holds the behavioral
    // contract of the primary-key fetch: the RIGHT hits, in FTS
    // ranking order, with unrelated records absent — across a store
    // large enough that a scan-order bug would surface as ordering
    // drift.
    use a3s_memory::MemoryStore as _;
    let root = tempfile::tempdir().unwrap();
    let store = VecMemoryStore::open(root.path()).unwrap();
    for index in 0..200 {
        let mut filler = MemoryItem::new(format!(
            "filler record {index} about harbor cranes and tide tables"
        ));
        filler.importance = 0.5;
        store.store(filler).await.unwrap();
    }
    for (id, content) in [
        ("m1", "the workspace prefers rust-analyzer navigation"),
        ("m2", "navigation preference: rust-analyzer over rls"),
        ("m3", "unrelated note about compiler flags"),
    ] {
        let mut item = MemoryItem::new(content);
        item.importance = 0.9;
        store.store(item).await.unwrap();
        let _ = id;
    }
    let hits = store
        .search("rust-analyzer navigation preference", 5)
        .await
        .unwrap();
    assert!(
        hits.len() >= 2,
        "both navigation memories must surface: {:?}",
        hits.iter()
            .map(|hit| hit.content.as_str())
            .collect::<Vec<_>>()
    );
    assert!(
        hits[0].content.contains("preference") || hits[0].content.contains("prefers"),
        "FTS ranking order preserved (the preference memories outrank filler): {:?}",
        hits.iter()
            .map(|hit| hit.content.as_str())
            .collect::<Vec<_>>()
    );
    assert!(
        hits.iter()
            .all(|hit| !hit.content.starts_with("filler record")),
        "filler must not crowd out the matches"
    );
}

#[test]
fn empty_query_returns_nothing() {
    let root = tempfile::tempdir().unwrap();
    let store = VecMemoryStore::open(root.path()).unwrap();
    store.put(&record("m4", "something", 0.5)).unwrap();
    assert!(store.recall("   ", 5).unwrap().is_empty());
}
