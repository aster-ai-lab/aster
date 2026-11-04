use super::*;

#[tokio::test]
async fn pruning_refuses_malformed_fields_before_deleting_any_record() {
    let root = tempfile::tempdir().unwrap();
    let mut failures = Vec::new();
    for (name, sql) in [
        (
            "tags",
            "UPDATE memories SET tags = '{' WHERE id = 'protected'",
        ),
        (
            "metadata",
            "UPDATE memories SET metadata = '{' WHERE id = 'protected'",
        ),
        (
            "kind",
            "UPDATE memories SET memory_type = 'unknown-kind' WHERE id = 'protected'",
        ),
        (
            "timestamp",
            "UPDATE memories SET timestamp_ms = 9223372036854775807 WHERE id = 'protected'",
        ),
        (
            "last_accessed",
            "UPDATE memories SET last_accessed_ms = 9223372036854775807 WHERE id = 'protected'",
        ),
        (
            "importance",
            "UPDATE memories SET importance = 1e999 WHERE id = 'protected'",
        ),
    ] {
        let store = seeded_store(&root.path().join(name)).await;
        store
            .with_conn(move |connection| {
                connection.execute(sql, [])?;
                Ok(())
            })
            .await
            .unwrap();
        let result = store.prune(&PrunePolicy::default()).await;
        let count = store.count().await.unwrap();
        if result.is_ok() || count != 2 {
            failures.push(format!("{name}: {result:?}; remaining={count}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[tokio::test]
async fn pruning_propagates_row_type_errors_before_deleting_other_records() {
    let root = tempfile::tempdir().unwrap();
    let store = seeded_store(root.path()).await;
    store
        .with_conn(|connection| {
            connection.execute(
                "UPDATE memories SET tags = ?1 WHERE id = 'protected'",
                params![vec![0_u8, 1]],
            )?;
            Ok(())
        })
        .await
        .unwrap();
    let result = store.prune(&PrunePolicy::default()).await;
    let count = store.count().await.unwrap();
    assert!(
        result.is_err() && count == 2,
        "row decode must abort destructive selection: {result:?}; remaining={count}"
    );
}

async fn seeded_store(root: &Path) -> SqliteMemoryStore {
    let store = SqliteMemoryStore::new(root).await.unwrap();
    for id in ["protected", "expired"] {
        let mut item = MemoryItem::new(format!("{id} real retention source")).with_importance(0.1);
        item.id = id.into();
        item.timestamp = Utc::now() - chrono::Duration::days(400);
        if id == "protected" {
            item.tags.push("pinned".into());
        }
        store.store(item).await.unwrap();
    }
    store
}
