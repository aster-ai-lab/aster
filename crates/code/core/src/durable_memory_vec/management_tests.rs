use super::*;

#[test]
fn management_search_reuses_ranked_recall_and_keeps_exact_read_only_revisions() {
    let root = tempfile::tempdir().unwrap();
    let store = VecMemoryStore::open(root.path()).unwrap();
    store
        .put(&record(
            "found",
            "inventorymarker verifies the inventory ledger",
            "semantic",
        ))
        .unwrap();
    store
        .put(&record("other", "an unrelated workflow", "procedural"))
        .unwrap();
    let before = store.inspect_id("found").unwrap();
    let MemoryManagementResponse::Search { items } = store
        .manage(MemoryManagementRequest::Search {
            query: "inventorymarker".into(),
            limit: 1,
        })
        .unwrap()
    else {
        panic!("search reply expected")
    };
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].id, "found");
    assert_eq!(items[0].content, before.content);
    assert_eq!(items[0].revision, before.revision);
    assert_eq!(store.inspect_id("found").unwrap().revision, before.revision);
    let MemoryManagementResponse::Search { items } = store
        .manage(MemoryManagementRequest::Search {
            query: "absentmarker".into(),
            limit: 12,
        })
        .unwrap()
    else {
        panic!("search reply expected")
    };
    assert!(items.is_empty());
    for (query, limit) in [("", 1), ("bad\0query", 1), ("valid", 0), ("valid", 13)] {
        assert!(MemoryManagementRequest::Search {
            query: query.into(),
            limit
        }
        .validate()
        .is_err());
    }
}

#[test]
fn management_search_matches_chinese_inside_saved_paragraphs_and_keeps_query_syntax_literal() {
    let root = tempfile::tempdir().unwrap();
    let store = VecMemoryStore::open(root.path()).unwrap();
    store
        .put(&record(
            "库存规则",
            "请先核对库存记录再计算净变化。",
            "semantic",
        ))
        .unwrap();
    let MemoryManagementResponse::Search { items } = store
        .manage(MemoryManagementRequest::Search {
            query: "库存".into(),
            limit: 12,
        })
        .unwrap()
    else {
        panic!("expected search")
    };
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].id, "库存规则");
    let MemoryManagementResponse::Search { items } = store
        .manage(MemoryManagementRequest::Search {
            query: "不存在 OR *".into(),
            limit: 12,
        })
        .unwrap()
    else {
        panic!("expected search")
    };
    assert!(items.is_empty());
}

#[test]
fn management_search_reads_a_large_cold_store_through_the_existing_index() {
    let root = tempfile::tempdir().unwrap();
    let store = VecMemoryStore::open(root.path()).unwrap();
    let padding = "saved evidence ".repeat(4000);
    for index in 0..300 {
        let content = if index == 123 {
            format!("largeindexmarker {index} {padding}")
        } else {
            format!("{index} {padding}")
        };
        store
            .put(&record(&format!("record-{index}"), &content, "semantic"))
            .unwrap();
    }
    assert!(store.collection.stats().unwrap().accounted_document_bytes > MAX_SCAN_BYTES as u64);
    drop(store);
    let started = std::time::Instant::now();
    let reopened = VecMemoryStore::open(root.path()).unwrap();
    let opened = started.elapsed();
    let query_started = std::time::Instant::now();
    let MemoryManagementResponse::Search { items } = reopened
        .manage(MemoryManagementRequest::Search {
            query: "largeindexmarker".into(),
            limit: 12,
        })
        .unwrap()
    else {
        panic!("search receipt expected")
    };
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].id, "record-123");
    eprintln!(
        "ORACLE memory-index records=300 cold-open-ms={} search-us={}",
        opened.as_millis(),
        query_started.elapsed().as_micros()
    );
}

#[test]
fn management_edits_content_conditionally_without_replacing_metadata_or_losing_recall() {
    let root = tempfile::tempdir().unwrap();
    let store = VecMemoryStore::open(root.path()).unwrap();
    store
        .put(&record("edited", "beforetoken", "semantic"))
        .unwrap();
    let before = store.inspect_id("edited").unwrap();
    let saved = match store
        .manage(MemoryManagementRequest::Update {
            id: before.id.clone(),
            revision: before.revision.clone(),
            content: "aftertoken".into(),
        })
        .unwrap()
    {
        MemoryManagementResponse::Update { item } => item,
        _ => panic!("expected update receipt"),
    };
    assert_eq!(saved.content, "aftertoken");
    assert_ne!(saved.revision, before.revision);
    assert_eq!(saved.metadata, before.metadata);
    assert_eq!(saved.tags, before.tags);
    assert_eq!(saved.timestamp, before.timestamp);
    assert_eq!(saved.access_count, before.access_count);
    assert!(store.recall("beforetoken", 5).unwrap().is_empty());
    assert_eq!(store.recall("aftertoken", 5).unwrap()[0].0, "edited");
    assert_eq!(
        store
            .manage(MemoryManagementRequest::Update {
                id: before.id,
                revision: before.revision,
                content: "stale".into()
            })
            .unwrap_err()
            .code,
        MemoryManagementCode::Conflict
    );
    drop(store);
    let reopened = VecMemoryStore::open(root.path()).unwrap();
    assert_eq!(reopened.inspect_id("edited").unwrap().content, "aftertoken");
}

#[test]
fn live_inspection_preserves_real_layers_owners_and_stable_revisions() {
    let item = MemoryItem::new("session source").with_type(MemoryType::Semantic);
    let root = Path::new("/fixture");
    let make = |session, layer| {
        ManagedMemory::from_session_item(root, session, layer, item.clone()).unwrap()
    };
    let working = make("first", MemoryInspectionLayer::Working);
    let session = make("first", MemoryInspectionLayer::Session);
    let other = make("second", MemoryInspectionLayer::Working);
    assert_ne!(working.id, session.id);
    assert_ne!(working.id, other.id);
    assert_eq!(
        working.revision,
        make("first", MemoryInspectionLayer::Working).revision
    );
    assert_eq!(working.memory_type, StoredMemoryType::Semantic);
    assert_eq!(working.metadata["a3s.session_id"], "first");
    assert_eq!(working.metadata["a3s.memory_id"], item.id);
    assert!(ManagedMemory::from_session_item(
        root,
        "first",
        MemoryInspectionLayer::Durable,
        item.clone()
    )
    .is_err());
    let mut changed = item;
    changed.content = "changed source".into();
    assert_ne!(
        working.revision,
        ManagedMemory::from_session_item(root, "first", MemoryInspectionLayer::Working, changed)
            .unwrap()
            .revision
    );
    assert!(ManagedMemory::from_session_item(
        root,
        "first",
        MemoryInspectionLayer::Working,
        MemoryItem::new("x".repeat(65 * 1024))
    )
    .is_err());
}

fn record(id: &str, content: &str, kind: &str) -> MemoryRecord {
    MemoryRecord {
        id: id.into(),
        content: content.into(),
        tags: vec!["test".into()],
        importance: 0.7,
        memory_type: format!("\"{kind}\""),
        timestamp: "2026-10-24T00:00:00Z".into(),
        meta: r#"{"metadata":{"source":"fixture"},"access_count":2,"last_accessed":null}"#.into(),
    }
}
fn list(
    store: &VecMemoryStore,
    query: &str,
    kind: Option<StoredMemoryType>,
    cursor: Option<String>,
) -> (Vec<ManagedMemory>, usize, Option<String>) {
    match store
        .manage(MemoryManagementRequest::List {
            query: query.into(),
            memory_type: kind,
            cursor,
            created_after: None,
            created_before: None,
        })
        .unwrap()
    {
        MemoryManagementResponse::List {
            items,
            total,
            next_cursor,
        } => (items, total, next_cursor),
        _ => panic!("expected catalog"),
    }
}

#[test]
fn management_pages_reach_every_match_without_recall_top_k_loss() {
    let root = tempfile::tempdir().unwrap();
    let store = VecMemoryStore::open(root.path()).unwrap();
    for i in 0..107 {
        store
            .put(&record(
                &format!("m{i:03}"),
                &format!("LOCAL memory {i}"),
                if i % 2 == 0 { "semantic" } else { "working" },
            ))
            .unwrap();
    }
    let (first, total, cursor) = list(&store, "local", None, None);
    assert_eq!((first.len(), total), (50, 107));
    let (second, _, cursor) = list(&store, "local", None, cursor);
    let (last, _, cursor) = list(&store, "local", None, cursor);
    let ids: std::collections::BTreeSet<_> = first
        .into_iter()
        .chain(second)
        .chain(last)
        .map(|item| item.id)
        .collect();
    assert_eq!(ids.len(), 107);
    assert!(ids.contains("m000") && ids.contains("m106") && cursor.is_none());
    let (_, total, _) = list(&store, "local", Some(StoredMemoryType::Semantic), None);
    assert_eq!(total, 54);
}

#[test]
fn management_refuses_stale_cursor_unknown_types_and_oversized_records() {
    let root = tempfile::tempdir().unwrap();
    let store = VecMemoryStore::open(root.path()).unwrap();
    for i in 0..51 {
        store
            .put(&record(
                &format!("{i:03}"),
                &format!("value {i}"),
                "episodic",
            ))
            .unwrap();
    }
    let (_, _, cursor) = list(&store, "", None, None);
    store.put(&record("new", "new value", "working")).unwrap();
    let error = store
        .manage(MemoryManagementRequest::List {
            query: "".into(),
            memory_type: None,
            cursor,
            created_after: None,
            created_before: None,
        })
        .unwrap_err();
    assert_eq!(error.code, MemoryManagementCode::Conflict);
    store
        .put(&record("unknown", "unknown type", "imagined"))
        .unwrap();
    assert!(store
        .manage(MemoryManagementRequest::Read {
            id: "unknown".into()
        })
        .is_err());
    store
        .put(&record(
            "large",
            &"x".repeat(MAX_RECORD_BYTES + 1),
            "semantic",
        ))
        .unwrap();
    assert_eq!(
        store
            .manage(MemoryManagementRequest::Read { id: "large".into() })
            .unwrap_err()
            .code,
        MemoryManagementCode::Limit
    );
    for raw in [
        r#"{"operation":"clear"}"#,
        r#"{"operation":"read","id":"a","path":"/other"}"#,
        r#"{"operation":"list","query":"","memory_type":"guess","cursor":null}"#,
    ] {
        assert!(MemoryManagementRequest::parse(raw).is_err());
    }
}

#[tokio::test]
async fn management_delete_is_conditional_durable_and_allows_remembering_again() {
    let root = tempfile::tempdir().unwrap();
    let store = VecMemoryStore::open(root.path()).unwrap();
    store
        .put(&record("a", "uniquefixture", "procedural"))
        .unwrap();
    let item = store.inspect_id("a").unwrap();
    assert_eq!(
        item.metadata.get("source").map(String::as_str),
        Some("fixture")
    );
    assert_eq!(item.access_count, 2);
    MemoryStore::delete(&store, "a").await.unwrap();
    store
        .put(&record("a", "replacement", "procedural"))
        .unwrap();
    assert_eq!(
        store
            .manage(MemoryManagementRequest::Delete {
                id: "a".into(),
                revision: item.revision
            })
            .unwrap_err()
            .code,
        MemoryManagementCode::Conflict
    );
    assert_eq!(store.inspect_id("a").unwrap().content, "replacement");
    let current = store.inspect_id("a").unwrap();
    store
        .manage(MemoryManagementRequest::Delete {
            id: "a".into(),
            revision: current.revision,
        })
        .unwrap();
    assert!(MemoryStore::search(&store, "replacement", 10)
        .await
        .unwrap()
        .is_empty());
    store
        .put(&record("again", "replacement", "semantic"))
        .unwrap();
    drop(store);
    let reopened = VecMemoryStore::open(root.path()).unwrap();
    assert_eq!(reopened.inspect_id("again").unwrap().content, "replacement");
    assert_eq!(
        reopened.inspect_id("a").unwrap_err().code,
        MemoryManagementCode::Missing
    );
}

#[test]
fn management_revision_binds_metadata_and_owner_scope() {
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let first = VecMemoryStore::open(a.path()).unwrap();
    let second = VecMemoryStore::open(b.path()).unwrap();
    first
        .put(&record("same", "same stored content", "semantic"))
        .unwrap();
    second
        .put(&record("same", "same stored content", "semantic"))
        .unwrap();
    let version = first.inspect_id("same").unwrap().revision;
    assert_eq!(
        second
            .manage(MemoryManagementRequest::Delete {
                id: "same".into(),
                revision: version
            })
            .unwrap_err()
            .code,
        MemoryManagementCode::Conflict
    );
    assert_eq!(
        second.inspect_id("same").unwrap().content,
        "same stored content"
    );
    for kind in [
        StoredMemoryType::Episodic,
        StoredMemoryType::Semantic,
        StoredMemoryType::Procedural,
        StoredMemoryType::Working,
    ] {
        assert_eq!(StoredMemoryType::parse(kind.as_str()), Some(kind));
    }
}

#[test]
fn management_bounds_page_bytes_and_rejects_unknown_auxiliary_state() {
    let root = tempfile::tempdir().unwrap();
    let store = VecMemoryStore::open(root.path()).unwrap();
    for i in 0..12 {
        store
            .put(&record(
                &format!("{i:02}"),
                &format!("{i} {}", "x".repeat(50 * 1024)),
                "semantic",
            ))
            .unwrap();
    }
    let (first, total, cursor) = list(&store, "", None, None);
    assert_eq!(total, 12);
    assert!(first.len() < 12 && cursor.is_some());
    assert!(serde_json::to_vec(&first).unwrap().len() < 256 * 1024);
    let mut broken = record("unknown-meta", "unknown meta", "semantic");
    broken.meta = r#"{"guessed_provenance":"unknown"}"#.into();
    store.put(&broken).unwrap();
    assert_eq!(
        store
            .manage(MemoryManagementRequest::Read { id: broken.id })
            .unwrap_err()
            .code,
        MemoryManagementCode::Failed
    );
}

#[test]
fn management_stale_deletion_never_removes_a_concurrent_sdk_replacement() {
    let root = tempfile::tempdir().unwrap();
    let store = Arc::new(VecMemoryStore::open(root.path()).unwrap());
    let runtime = Arc::new(
        tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap(),
    );
    for iteration in 0..32 {
        store
            .put(&record("target", &format!("old {iteration}"), "semantic"))
            .unwrap();
        let revision = store.inspect_id("target").unwrap().revision;
        let barrier = Arc::new(std::sync::Barrier::new(3));
        let delete = {
            let store = Arc::clone(&store);
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                store.manage(MemoryManagementRequest::Delete {
                    id: "target".into(),
                    revision,
                })
            })
        };
        let replace = {
            let store = Arc::clone(&store);
            let barrier = Arc::clone(&barrier);
            let runtime = Arc::clone(&runtime);
            std::thread::spawn(move || {
                barrier.wait();
                let _ = runtime.block_on(MemoryStore::delete(store.as_ref(), "target"));
                store
                    .put(&record("target", &format!("new {iteration}"), "semantic"))
                    .unwrap();
            })
        };
        barrier.wait();
        replace.join().unwrap();
        if let Err(error) = delete.join().unwrap() {
            assert!(matches!(
                error.code,
                MemoryManagementCode::Missing | MemoryManagementCode::Conflict
            ));
        }
        assert_eq!(
            store.inspect_id("target").unwrap().content,
            format!("new {iteration}")
        );
        runtime
            .block_on(MemoryStore::delete(store.as_ref(), "target"))
            .unwrap();
    }
}

#[test]
fn time_filters_are_applied_before_pagination_and_bound_to_the_cursor() {
    let dir = tempfile::tempdir().unwrap();
    let store = VecMemoryStore::open(dir.path()).unwrap();
    for i in 0..61 {
        let mut value = record(
            &format!("recent-{i:03}"),
            &format!("recent evidence {i}"),
            "semantic",
        );
        value.timestamp = if i == 60 {
            "2026-10-24T08:00:00+08:00"
        } else {
            "2026-10-24T00:00:00Z"
        }
        .into();
        store.put(&value).unwrap();
    }
    let mut old = record("old", "older evidence", "semantic");
    old.timestamp = "2026-09-01T00:00:00Z".into();
    store.put(&old).unwrap();
    let input = |cursor: Option<String>, after: &str| MemoryManagementRequest::List {
        query: "".into(),
        memory_type: None,
        cursor,
        created_after: Some(after.into()),
        created_before: Some("2026-10-24T00:00:00Z".into()),
    };
    let MemoryManagementResponse::List {
        items,
        total,
        next_cursor,
    } = store.manage(input(None, "2026-10-24T00:00:00Z")).unwrap()
    else {
        panic!("expected list")
    };
    assert_eq!(total, 61);
    assert_eq!(items.len(), 50);
    assert!(items.iter().all(|item| item.id != "old"));
    let MemoryManagementResponse::List {
        items,
        total,
        next_cursor: end,
    } = store
        .manage(input(next_cursor.clone(), "2026-10-24T00:00:00Z"))
        .unwrap()
    else {
        panic!("expected list")
    };
    assert_eq!(total, 61);
    assert_eq!(items.len(), 11);
    assert!(end.is_none());
    assert_eq!(
        store
            .manage(input(next_cursor, "2026-10-23T00:00:00Z"))
            .unwrap_err()
            .code,
        MemoryManagementCode::Conflict
    );
    assert!(MemoryManagementRequest::parse(r#"{"operation":"list","query":"","memory_type":null,"cursor":null,"created_after":"2026-10-25T00:00:00Z","created_before":"2026-10-24T00:00:00Z"}"#).is_err());
}
