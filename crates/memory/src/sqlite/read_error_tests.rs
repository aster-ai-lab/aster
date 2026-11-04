use super::*;

#[tokio::test]
async fn list_reads_propagate_malformed_json() {
    let root = tempfile::tempdir().unwrap();
    let store = seeded_store(root.path()).await;
    store
        .with_conn(|connection| {
            connection.execute("UPDATE memories SET metadata = '{' WHERE id = 'newer'", [])?;
            Ok(())
        })
        .await
        .unwrap();

    for (name, result) in list_reads(&store).await {
        let error = result.unwrap_err();
        assert!(
            matches!(
                error.downcast_ref::<rusqlite::Error>(),
                Some(rusqlite::Error::FromSqlConversionFailure(6, _, source))
                    if source.is::<serde_json::Error>()
            ),
            "{name}: {error:?}"
        );
    }
}

#[tokio::test]
async fn list_reads_propagate_row_type_errors() {
    let root = tempfile::tempdir().unwrap();
    let store = seeded_store(root.path()).await;
    store
        .with_conn(|connection| {
            connection.execute(
                "UPDATE memories SET tags = ?1 WHERE id = 'newer'",
                params![vec![0_u8, 1]],
            )?;
            Ok(())
        })
        .await
        .unwrap();

    for (name, result) in list_reads(&store).await {
        let error = result.unwrap_err();
        assert!(
            matches!(
                error.downcast_ref::<rusqlite::Error>(),
                Some(rusqlite::Error::InvalidColumnType(
                    4,
                    _,
                    rusqlite::types::Type::Blob
                ))
            ),
            "{name}: {error:?}"
        );
    }
}

#[tokio::test]
async fn failed_duplicate_scan_preserves_database_and_markdown() {
    let root = tempfile::tempdir().unwrap();
    let store = seeded_store(root.path()).await;
    let markdown_path = root.path().join("MEMORY.md");
    let markdown = tokio::fs::read(&markdown_path).await.unwrap();
    store
        .with_conn(|connection| {
            connection.execute("UPDATE memories SET metadata = '{' WHERE id = 'newer'", [])?;
            Ok(())
        })
        .await
        .unwrap();
    let before = database_snapshot(&store).await;
    let mut duplicate = MemoryItem::new("newer sqlite source for read boundary checks")
        .with_importance(0.99)
        .with_tag("incoming-only");
    duplicate.id = "incoming".into();

    let result = store.store_and_return(duplicate).await;
    let after = database_snapshot(&store).await;
    assert_eq!(
        before, after,
        "a failed scan must not persist a partial decision"
    );
    assert_eq!(markdown, tokio::fs::read(&markdown_path).await.unwrap());
    assert!(matches!(
        result.unwrap_err().downcast_ref::<rusqlite::Error>(),
        Some(rusqlite::Error::FromSqlConversionFailure(6, _, _))
    ));
}

#[tokio::test]
async fn bounded_reads_keep_order_and_any_tag_semantics() {
    let root = tempfile::tempdir().unwrap();
    let store = seeded_store(root.path()).await;
    assert_eq!(store.get_recent(1).await.unwrap()[0].id, "newer");
    assert_eq!(store.get_important(0.7, 1).await.unwrap()[0].id, "newer");
    assert_eq!(store.search("sqlite", 1).await.unwrap().len(), 1);
    let tags = vec!["newer-only".into(), "older-only".into()];
    let items = store.search_by_tags(&tags, 2).await.unwrap();
    assert_eq!(
        items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        ["newer", "older"]
    );

    store
        .with_conn(|connection| {
            connection.execute("UPDATE memories SET metadata = '{' WHERE id = 'older'", [])?;
            Ok(())
        })
        .await
        .unwrap();
    assert_eq!(store.search_by_tags(&tags, 1).await.unwrap()[0].id, "newer");
    assert!(store.search_by_tags(&tags, 0).await.unwrap().is_empty());
    assert!(store.search_by_tags(&[], 2).await.unwrap().is_empty());
    assert!(store.search("sqlite", 0).await.unwrap().is_empty());
    assert!(store.get_recent(0).await.unwrap().is_empty());
    assert!(store.get_important(0.7, 0).await.unwrap().is_empty());
    assert!(store.search_by_tags(&["absent".into()], 1).await.is_err());
}

async fn list_reads(store: &SqliteMemoryStore) -> [(&'static str, Result<Vec<MemoryItem>>); 4] {
    [
        ("fts", store.search("sqlite", 2).await),
        ("tags", store.search_by_tags(&["shared".into()], 2).await),
        ("recent", store.get_recent(2).await),
        ("important", store.get_important(0.7, 2).await),
    ]
}

async fn seeded_store(root: &Path) -> SqliteMemoryStore {
    let store = SqliteMemoryStore::new(root).await.unwrap();
    for (id, days, importance) in [("older", 2, 0.75), ("newer", 1, 0.9)] {
        let mut item = MemoryItem::new(format!("{id} sqlite source for read boundary checks"))
            .with_importance(importance)
            .with_tags(vec!["shared".into(), format!("{id}-only")]);
        item.id = id.into();
        item.timestamp = Utc::now() - chrono::Duration::days(days);
        store.store(item).await.unwrap();
    }
    store
}

async fn database_snapshot(store: &SqliteMemoryStore) -> Vec<Vec<String>> {
    store
        .with_conn(|connection| {
            let mut stmt = connection.prepare(
                "SELECT quote(id), quote(content), quote(timestamp_ms), quote(importance),
                        quote(tags), quote(memory_type), quote(metadata), quote(access_count),
                        quote(last_accessed_ms) FROM memories ORDER BY id",
            )?;
            let rows = stmt.query_map([], |row| (0..9).map(|column| row.get(column)).collect())?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })
        .await
        .unwrap()
}
