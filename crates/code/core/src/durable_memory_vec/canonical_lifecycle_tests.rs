use super::*;
use management::{MemoryManagementRequest, MemoryManagementResponse};

fn item(id: &str, content: &str) -> MemoryItem {
    let mut item = MemoryItem::new(content).with_importance(0.2);
    item.id = id.into();
    item
}

#[tokio::test]
async fn management_delete_prune_and_clear_keep_one_duplicate_lookup() {
    let root = tempfile::tempdir().unwrap();
    let store = VecMemoryStore::open(root.path()).unwrap();
    let old = "original durable lifecycle configuration preference";
    let changed = "changed durable lifecycle configuration preference";
    store.store(item("original", old)).await.unwrap();
    let MemoryManagementResponse::Read { item: inspected } = store
        .manage(MemoryManagementRequest::Read {
            id: "original".into(),
        })
        .unwrap()
    else {
        panic!("read response")
    };
    store
        .manage(MemoryManagementRequest::Update {
            id: "original".into(),
            revision: inspected.revision,
            content: changed.into(),
        })
        .unwrap();
    assert_eq!(
        store
            .store_and_return(item("old-again", old))
            .await
            .unwrap()
            .id,
        "old-again"
    );
    assert_eq!(
        store
            .store_and_return(item("merged", &changed.to_uppercase()))
            .await
            .unwrap()
            .id,
        "original"
    );
    store.delete("original").await.unwrap();
    let mut expired = item("expired", changed);
    expired.timestamp = Utc::now() - chrono::Duration::days(400);
    assert_eq!(store.store_and_return(expired).await.unwrap().id, "expired");
    assert_eq!(store.prune(&Default::default()).await.unwrap(), 1);
    assert_eq!(
        store
            .store_and_return(item("after-prune", changed))
            .await
            .unwrap()
            .id,
        "after-prune"
    );
    assert_eq!(store.count().await.unwrap(), 2);
    store.clear().await.unwrap();
    assert_eq!(
        store
            .store_and_return(item("after-clear", changed))
            .await
            .unwrap()
            .id,
        "after-clear"
    );
    assert_eq!(store.count().await.unwrap(), 1);
}

#[tokio::test]
async fn historical_duplicate_ids_survive_removal_of_one_canonical_id() {
    let root = tempfile::tempdir().unwrap();
    let content = "historical durable configuration preference";
    {
        let store = VecMemoryStore::open(root.path()).unwrap();
        store
            .put(&item_to_record(&item("a", content)).unwrap())
            .unwrap();
        store
            .put(&item_to_record(&item("b", &content.to_uppercase())).unwrap())
            .unwrap();
    }
    let store = VecMemoryStore::open(root.path()).unwrap();
    assert_eq!(store.record_count().unwrap(), 2);
    assert_eq!(
        store.store_and_return(item("c", content)).await.unwrap().id,
        "a"
    );
    store.delete("a").await.unwrap();
    assert_eq!(
        store.store_and_return(item("d", content)).await.unwrap().id,
        "b"
    );
    store
        .put(&item_to_record(&item("raw", &content.to_uppercase())).unwrap())
        .unwrap();
    assert_eq!(store.record_count().unwrap(), 1);
}

#[tokio::test]
async fn concurrent_lazy_observations_return_one_persisted_id() {
    let root = tempfile::tempdir().unwrap();
    let store = Arc::new(LazyFileMemoryStore::new(root.path()));
    let mut tasks = Vec::new();
    for index in 0..12 {
        let store = store.clone();
        tasks.push(tokio::spawn(async move {
            let content = if index % 2 == 0 {
                "one durable preference for explicit configuration"
            } else {
                " ONE   DURABLE preference for explicit\nconfiguration "
            };
            store
                .store_and_return(
                    item(&format!("observation-{index}"), content)
                        .with_tag(format!("source-{index}")),
                )
                .await
                .unwrap()
        }));
    }
    let mut ids = std::collections::HashSet::new();
    for task in tasks {
        ids.insert(task.await.unwrap().id);
    }
    assert_eq!(ids.len(), 1);
    assert_eq!(store.count().await.unwrap(), 1);
    let canonical = store
        .retrieve(ids.iter().next().unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(canonical.tags.len(), 12);
    assert_eq!(canonical.metadata["duplicate_count"], "11");
}

#[tokio::test]
async fn rejected_primary_write_does_not_create_a_duplicate_target() {
    let root = tempfile::tempdir().unwrap();
    let store = VecMemoryStore::open(root.path()).unwrap();
    let content = "retry durable content after rejected primary write";
    let error = store.store_and_return(item("", content)).await.unwrap_err();
    assert_eq!(
        error.downcast_ref::<aster_vec::Error>().unwrap().code,
        aster_vec::ErrorCode::InvalidArgument
    );
    assert_eq!(store.count().await.unwrap(), 0);
    let stored = store
        .store_and_return(item("valid", content))
        .await
        .unwrap();
    assert_eq!(stored.id, "valid");
    assert_eq!(store.count().await.unwrap(), 1);
}

#[tokio::test]
async fn legacy_import_merges_observations_through_the_same_canonical_owner() {
    let root = tempfile::tempdir().unwrap();
    let legacy = root.path().join("items");
    std::fs::create_dir_all(&legacy).unwrap();
    for memory in [
        item("first", "durable import configuration preference").with_tag("first"),
        item("second", " DURABLE  IMPORT configuration\npreference ").with_tag("second"),
    ] {
        std::fs::write(
            legacy.join(format!("{}.json", memory.id)),
            serde_json::to_vec(&memory).unwrap(),
        )
        .unwrap();
    }
    let store = LazyFileMemoryStore::new(root.path());
    assert_eq!(store.count().await.unwrap(), 1);
    let items = store.get_recent(10).await.unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].tags.len(), 2);
    assert_eq!(items[0].metadata["duplicate_count"], "1");
    assert!(root.path().join("items.imported").is_dir());
}

#[tokio::test]
async fn canonical_tags_keep_spaces_and_unicode_through_reopen_and_management() {
    let root = tempfile::tempdir().unwrap();
    let first = item(
        "canonical",
        "durable tags preserve their exact source labels",
    )
    .with_tags(vec!["cli settings".into(), "配置 笔记".into()]);
    {
        let store = VecMemoryStore::open(root.path()).unwrap();
        let stored = store.store_and_return(first.clone()).await.unwrap();
        assert_eq!(
            store.retrieve(&stored.id).await.unwrap().unwrap().tags,
            first.tags
        );
    }
    let store = VecMemoryStore::open(root.path()).unwrap();
    let incoming = item("incoming", &first.content).with_tag("follow up");
    let stored = store.store_and_return(incoming).await.unwrap();
    assert_eq!(stored.id, first.id);
    assert_eq!(stored.tags, vec!["cli settings", "配置 笔记", "follow up"]);
    assert_eq!(
        store.retrieve(&stored.id).await.unwrap().unwrap().tags,
        stored.tags
    );
    let MemoryManagementResponse::Read { item: managed } = store
        .manage(MemoryManagementRequest::Read { id: stored.id })
        .unwrap()
    else {
        panic!("read response")
    };
    assert_eq!(managed.tags, stored.tags);
}

#[tokio::test]
async fn canonical_empty_tag_roundtrips_without_changing_plain_tag_encoding() {
    let root = tempfile::tempdir().unwrap();
    let store = VecMemoryStore::open(root.path()).unwrap();
    let empty = store
        .store_and_return(item("empty", "empty label durable memory").with_tag(""))
        .await
        .unwrap();
    assert_eq!(
        store.retrieve(&empty.id).await.unwrap().unwrap().tags,
        vec![""]
    );
    let plain = item("plain", "plain label durable memory").with_tag("rust");
    let encoded: serde_json::Value =
        serde_json::from_str(&item_to_record(&plain).unwrap().meta).unwrap();
    assert!(encoded.get("tags").is_none());
    assert_eq!(
        store.store_and_return(plain).await.unwrap().tags,
        vec!["rust"]
    );
}
