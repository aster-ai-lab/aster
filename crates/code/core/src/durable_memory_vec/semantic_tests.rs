use super::*;

struct Provider(fn(&str) -> Option<Vec<f32>>);

impl SemanticEmbed for Provider {
    fn embed(&self, text: &str) -> Option<Vec<f32>> {
        (self.0)(text)
    }
}

#[tokio::test]
async fn pruning_removes_expired_records_from_the_existing_semantic_owner() {
    let root = tempfile::tempdir().unwrap();
    let mut store = VecMemoryStore::open(root.path()).unwrap();
    store.set_semantic_provider(Arc::new(Provider(|_| Some(vec![1.0, 0.0]))));
    let mut expired = MemoryItem::new("expired semantic retention fact").with_importance(0.1);
    expired.timestamp = Utc::now() - chrono::Duration::days(400);
    MemoryStore::store(&store, expired.clone()).await.unwrap();
    assert_eq!(store.semantic_recall("probe", 1)[0].0, expired.id);
    MemoryStore::prune(&store, &Default::default())
        .await
        .unwrap();
    assert!(store.semantic_recall("probe", 1).is_empty());
}

#[tokio::test]
async fn failed_prune_keeps_the_persisted_record_and_original_semantic_revision() {
    let root = tempfile::tempdir().unwrap();
    let mut store = VecMemoryStore::open(root.path()).unwrap();
    store.set_semantic_provider(Arc::new(Provider(|_| Some(vec![1.0, 0.0]))));
    let mut expired = MemoryItem::new("failed prune semantic control").with_importance(0.1);
    expired.timestamp = Utc::now() - chrono::Duration::days(400);
    MemoryStore::store(&store, expired).await.unwrap();
    let before = store
        .semantic_index
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .status();
    store.collection.clone().close().unwrap();
    assert!(MemoryStore::prune(&store, &Default::default())
        .await
        .is_err());
    assert_eq!(
        store
            .semantic_index
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .status(),
        before
    );
    drop(store);
    assert_eq!(
        VecMemoryStore::open(root.path())
            .unwrap()
            .record_count()
            .unwrap(),
        1
    );
}

#[tokio::test]
async fn partial_prune_keeps_protected_vectors_and_releases_the_deleted_content_digest() {
    let root = tempfile::tempdir().unwrap();
    let mut store = VecMemoryStore::open(root.path()).unwrap();
    store.set_semantic_provider(Arc::new(Provider(|_| Some(vec![1.0, 0.0]))));
    let mut expired = MemoryItem::new("expired selective semantic record").with_importance(0.1);
    expired.timestamp = Utc::now() - chrono::Duration::days(400);
    let mut protected = MemoryItem::new("protected selective semantic record")
        .with_importance(0.1)
        .with_tag("pinned");
    protected.timestamp = expired.timestamp;
    MemoryStore::store(&store, expired.clone()).await.unwrap();
    MemoryStore::store(&store, protected.clone()).await.unwrap();
    assert_eq!(
        MemoryStore::prune(&store, &Default::default())
            .await
            .unwrap(),
        1
    );
    let hits = store.semantic_recall("probe", 10);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].0, protected.id);
    let replacement = MemoryItem::new(expired.content);
    MemoryStore::store(&store, replacement.clone())
        .await
        .unwrap();
    assert_eq!(store.record_count().unwrap(), 2);
    assert!(store
        .semantic_recall("probe", 10)
        .iter()
        .any(|(id, _)| id == &replacement.id));
}

#[tokio::test]
async fn rejected_deletion_retains_typed_status_and_both_live_owners() {
    let root = tempfile::tempdir().unwrap();
    let mut store = VecMemoryStore::open(root.path()).unwrap();
    store.set_semantic_provider(Arc::new(Provider(|_| Some(vec![1.0, 0.0]))));
    MemoryStore::store(&store, MemoryItem::new("retained deletion error control"))
        .await
        .unwrap();
    let before = store
        .semantic_index
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .status();
    for (id, expected) in [
        ("missing", aster_vec::ErrorCode::NotFound),
        ("", aster_vec::ErrorCode::InvalidArgument),
        ("bad\0id", aster_vec::ErrorCode::InvalidArgument),
    ] {
        let error = MemoryStore::delete(&store, id).await.unwrap_err();
        assert_eq!(
            error.downcast_ref::<aster_vec::Error>().unwrap().code,
            expected
        );
        assert_eq!(store.record_count().unwrap(), 1);
        assert_eq!(
            store
                .semantic_index
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .status(),
            before
        );
    }
}

fn record(id: &str, content: &str) -> MemoryRecord {
    MemoryRecord {
        id: id.into(),
        content: content.into(),
        tags: Vec::new(),
        importance: 0.5,
        memory_type: "semantic".into(),
        timestamp: "2026-11-01T00:00:00Z".into(),
        meta: "{}".into(),
    }
}

#[test]
fn cosine_recall_normalizes_provider_vectors() {
    let root = tempfile::Builder::new()
        .prefix("memory vector 空间 ")
        .tempdir()
        .unwrap();
    let mut store = VecMemoryStore::open(root.path()).unwrap();
    store.set_semantic_provider(Arc::new(Provider(|text| {
        Some(match text {
            "scaled" => vec![2.0, 100.0],
            _ => vec![1.0, 0.0],
        })
    })));
    store.put(&record("axis", "axis")).unwrap();
    store.put(&record("scaled", "scaled")).unwrap();
    let hits = store.semantic_recall("probe", 2);
    assert_eq!(
        hits[0].0, "axis",
        "the documented cosine contract must ignore vector magnitude"
    );
}

#[test]
fn failed_primary_write_never_publishes_a_phantom_vector() {
    let root = tempfile::tempdir().unwrap();
    let mut store = VecMemoryStore::open(root.path()).unwrap();
    store.set_semantic_provider(Arc::new(Provider(|_| Some(vec![1.0, 0.0]))));
    assert!(store.put(&record("", "retryable content")).is_err());
    assert_eq!(store.record_count().unwrap(), 0);
    assert!(store.semantic_recall("probe", 2).is_empty());
}

#[test]
fn failed_primary_write_does_not_poison_same_content_retry() {
    let root = tempfile::tempdir().unwrap();
    let store = VecMemoryStore::open(root.path()).unwrap();
    assert!(store.put(&record("", "retryable content")).is_err());
    store.put(&record("valid", "retryable content")).unwrap();
    assert_eq!(store.record_count().unwrap(), 1);
    assert_eq!(store.recall("retryable content", 2).unwrap()[0].0, "valid");
}

#[test]
fn invalid_embedding_does_not_enter_recall_or_destroy_primary_data() {
    let root = tempfile::tempdir().unwrap();
    let mut store = VecMemoryStore::open(root.path()).unwrap();
    store.set_semantic_provider(Arc::new(Provider(|text| {
        Some(if text == "invalid" {
            vec![f32::NAN, 0.0]
        } else {
            vec![1.0, 0.0]
        })
    })));
    store.put(&record("invalid", "invalid")).unwrap();
    assert_eq!(store.record_count().unwrap(), 1);
    assert!(store.semantic_recall("probe", 2).is_empty());
}

#[test]
fn put_replaces_the_documented_id_and_releases_the_old_digest() {
    let root = tempfile::tempdir().unwrap();
    let mut store = VecMemoryStore::open(root.path()).unwrap();
    store.set_semantic_provider(Arc::new(Provider(|text| {
        Some(if text == "orthogonal" {
            vec![0.0, 1.0]
        } else {
            vec![1.0, 0.0]
        })
    })));
    store.put(&record("same", "axis")).unwrap();
    store.put(&record("same", "orthogonal")).unwrap();
    assert_eq!(store.record_count().unwrap(), 1);
    assert_eq!(store.semantic_recall("probe", 2), [("same".into(), 0.0)]);
    assert_eq!(store.recall("orthogonal", 2).unwrap()[0].0, "same");
    store.put(&record("other", "axis")).unwrap();
    assert_eq!(store.record_count().unwrap(), 2);
    assert_eq!(store.semantic_recall("probe", 2)[0].0, "other");
}

#[test]
fn provider_replacement_discards_old_vectors_and_content_noop_reprojects_only_stored_ids() {
    let root = tempfile::tempdir().unwrap();
    let mut store = VecMemoryStore::open(root.path()).unwrap();
    store.set_semantic_provider(Arc::new(Provider(|_| Some(vec![1.0, 0.0]))));
    store.put(&record("stored", "axis")).unwrap();
    store.set_semantic_provider(Arc::new(Provider(|text| {
        Some(if text == "probe" {
            vec![1.0, 0.0]
        } else {
            vec![0.0, 1.0]
        })
    })));
    assert!(store.semantic_recall("probe", 2).is_empty());
    store.put(&record("stored", "axis")).unwrap();
    store.put(&record("skipped", "axis")).unwrap();
    assert_eq!(store.record_count().unwrap(), 1);
    assert_eq!(store.semantic_recall("probe", 2), [("stored".into(), 0.0)]);
}

#[test]
fn query_dimension_and_nonfinite_errors_degrade_without_mutating_records() {
    let root = tempfile::tempdir().unwrap();
    let mut store = VecMemoryStore::open(root.path()).unwrap();
    store.set_semantic_provider(Arc::new(Provider(|text| {
        Some(match text {
            "bad-dimension" => vec![1.0],
            "bad-query" => vec![f32::NAN, 0.0],
            _ => vec![1.0, 0.0],
        })
    })));
    store.put(&record("stored", "axis")).unwrap();
    assert!(store.semantic_recall("bad-dimension", 2).is_empty());
    assert!(store.semantic_recall("bad-query", 2).is_empty());
    assert_eq!(store.semantic_recall("probe", 2)[0].0, "stored");
    assert_eq!(store.record_count().unwrap(), 1);
}

#[test]
fn shared_partition_ids_preserve_whitespace_and_unicode_source_ids() {
    let root = tempfile::tempdir().unwrap();
    let mut store = VecMemoryStore::open(root.path()).unwrap();
    store.set_semantic_provider(Arc::new(Provider(|_| Some(vec![1.0, 0.0]))));
    for (id, text) in [
        ("item", "first"),
        (" item ", "second"),
        (" ", "third"),
        ("记忆", "fourth"),
    ] {
        store.put(&record(id, text)).unwrap();
    }
    let ids = store
        .semantic_recall("probe", 10)
        .into_iter()
        .map(|(id, _)| id)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        ids,
        ["item", " item ", " ", "记忆"]
            .map(String::from)
            .into_iter()
            .collect()
    );
}

fn revision(store: &VecMemoryStore, id: &str) -> String {
    use super::management::{MemoryManagementRequest, MemoryManagementResponse};
    match store
        .manage(MemoryManagementRequest::Read { id: id.into() })
        .unwrap()
    {
        MemoryManagementResponse::Read { item } => item.revision,
        other => panic!("expected read response, got {other:?}"),
    }
}

#[tokio::test]
async fn management_edit_delete_and_clear_use_the_shared_derived_owner() {
    use super::management::MemoryManagementRequest;
    let root = tempfile::tempdir().unwrap();
    let mut store = VecMemoryStore::open(root.path()).unwrap();
    store.set_semantic_provider(Arc::new(Provider(|text| {
        Some(match text {
            "orthogonal" => vec![0.0, 1.0],
            "bad-shape" => vec![1.0],
            _ => vec![1.0, 0.0],
        })
    })));
    let mut item = MemoryItem::new("axis").with_type(MemoryType::Semantic);
    item.id = "stored".into();
    MemoryStore::store(&store, item).await.unwrap();
    for content in ["orthogonal", "bad-shape", "axis"] {
        store
            .manage(MemoryManagementRequest::Update {
                id: "stored".into(),
                revision: revision(&store, "stored"),
                content: content.into(),
            })
            .unwrap();
        let hits = store.semantic_recall("probe", 2);
        if content == "bad-shape" {
            assert!(hits.is_empty());
        } else {
            assert_eq!(hits[0].1, if content == "axis" { 1.0 } else { 0.0 });
        }
    }
    store
        .manage(MemoryManagementRequest::Delete {
            id: "stored".into(),
            revision: revision(&store, "stored"),
        })
        .unwrap();
    assert!(store.semantic_recall("probe", 2).is_empty());
    store.put(&record("fresh", "axis")).unwrap();
    MemoryStore::clear(&store).await.unwrap();
    assert_eq!(store.record_count().unwrap(), 0);
    assert!(store.semantic_recall("probe", 2).is_empty());
}

#[test]
fn reopened_records_can_be_reprojected_by_the_existing_put_contract() {
    let root = tempfile::tempdir().unwrap();
    {
        let store = VecMemoryStore::open(root.path()).unwrap();
        store.put(&record("stored", "axis")).unwrap();
    }
    let mut store = VecMemoryStore::open(root.path()).unwrap();
    store.set_semantic_provider(Arc::new(Provider(|_| Some(vec![1.0, 0.0]))));
    assert!(store.semantic_recall("probe", 2).is_empty());
    store.put(&record("stored", "axis")).unwrap();
    assert_eq!(store.record_count().unwrap(), 1);
    assert_eq!(store.semantic_recall("probe", 2), [("stored".into(), 1.0)]);
}

#[test]
fn primary_rejection_preserves_the_engine_error_code() {
    let root = tempfile::tempdir().unwrap();
    let store = VecMemoryStore::open(root.path()).unwrap();
    let error = store.put(&record("", "retryable content")).unwrap_err();
    assert_eq!(
        error.downcast_ref::<aster_vec::Error>().unwrap().code,
        aster_vec::ErrorCode::InvalidArgument
    );
    assert_eq!(store.record_count().unwrap(), 0);
}
