#![cfg(feature = "aster-vec-fts")]

use aster_core::durable_memory_vec::LazyFileMemoryStore;
use aster_durable_memory::{FileMemoryStore, InMemoryStore, MemoryItem, MemoryStore, PrunePolicy};
use chrono::Utc;
use std::sync::Arc;

#[tokio::test]
async fn native_prune_removes_only_expired_unprotected_records_and_persists() {
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("记忆 with spaces");
    let store = LazyFileMemoryStore::new(&directory);
    let mut expired = MemoryItem::new("expired durable retention fact").with_importance(0.1);
    expired.timestamp = Utc::now() - chrono::Duration::days(400);
    let mut protected = MemoryItem::new("protected durable retention fact")
        .with_importance(0.1)
        .with_tag("pinned");
    protected.timestamp = expired.timestamp;
    store.store(expired.clone()).await.unwrap();
    store.store(protected.clone()).await.unwrap();

    assert_eq!(store.prune(&PrunePolicy::default()).await.unwrap(), 1);
    assert!(store.retrieve(&expired.id).await.unwrap().is_none());
    assert!(store.retrieve(&protected.id).await.unwrap().is_some());
    drop(store);
    let reopened = LazyFileMemoryStore::new(&directory);
    assert_eq!(reopened.count().await.unwrap(), 1);
    assert!(reopened.retrieve(&protected.id).await.unwrap().is_some());
    let replacement = MemoryItem::new(expired.content);
    reopened.store(replacement.clone()).await.unwrap();
    assert!(reopened.retrieve(&replacement.id).await.unwrap().is_some());
}

#[tokio::test]
async fn every_adapter_refuses_nonfinite_policy_without_deleting_records() {
    let root = tempfile::tempdir().unwrap();
    for (name, store) in stores(root.path()).await {
        let mut item =
            MemoryItem::new(format!("{name} old low importance record")).with_importance(0.1);
        item.timestamp = Utc::now() - chrono::Duration::days(400);
        store.store(item).await.unwrap();
        let result = store
            .prune(&PrunePolicy {
                min_importance_to_keep: f32::NAN,
                ..Default::default()
            })
            .await;
        let count = store.count().await.unwrap();
        assert!(
            result.is_err() && count == 1,
            "{name}: invalid policy returned {result:?}; remaining records={count}"
        );
    }
}

#[tokio::test]
async fn every_adapter_refuses_cutoff_overflow_without_panicking_or_deleting_records() {
    let root = tempfile::tempdir().unwrap();
    for (name, store) in stores(root.path()).await {
        store
            .store(MemoryItem::new("overflow control record"))
            .await
            .unwrap();
        assert!(
            store
                .prune(&PrunePolicy {
                    max_age_days: u32::MAX,
                    ..Default::default()
                })
                .await
                .is_err(),
            "{name}: overflow must be a recoverable error"
        );
        assert_eq!(store.count().await.unwrap(), 1, "{name}");
    }
}

#[tokio::test]
async fn every_adapter_uses_the_same_protection_and_quota_policy() {
    let root = tempfile::tempdir().unwrap();
    let mut items = Vec::new();
    for (id, importance) in [
        ("expired", 0.1),
        ("important", 0.9),
        ("tagged", 0.1),
        ("related", 0.1),
        ("accessed", 0.1),
        ("recent", 0.1),
    ] {
        let mut item =
            MemoryItem::new(format!("{id} shared retention fixture")).with_importance(importance);
        item.id = id.into();
        if id != "recent" {
            item.timestamp = Utc::now() - chrono::Duration::days(400);
        }
        match id {
            "tagged" => item.tags.push("pinned".into()),
            "related" => {
                item.metadata
                    .insert("conflicts_with".into(), "source-memory".into());
            }
            "accessed" => item.access_count = 3,
            _ => {}
        }
        items.push(item);
    }
    for (name, store) in stores(root.path()).await {
        for item in &items {
            store.store(item.clone()).await.unwrap();
        }
        let deleted = store
            .prune(&PrunePolicy {
                max_items: 2,
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(deleted, 3, "{name}");
        let mut remaining: Vec<_> = store
            .get_recent(10)
            .await
            .unwrap()
            .into_iter()
            .map(|item| item.id)
            .collect();
        remaining.sort();
        assert_eq!(remaining, ["accessed", "related", "tagged"], "{name}");
    }
}

#[tokio::test(start_paused = true)]
async fn the_existing_maintenance_runtime_prunes_the_native_store_and_closes_cleanly() {
    use aster_core::memory::{
        AgentMemory, MemoryConfig, MemoryMaintenanceOptions, MemoryMaintenanceRuntime,
    };
    let root = tempfile::tempdir().unwrap();
    let store = Arc::new(LazyFileMemoryStore::new(root.path()));
    let mut item = MemoryItem::new("scheduled native retention record").with_importance(0.1);
    item.timestamp = Utc::now() - chrono::Duration::days(400);
    store.store(item).await.unwrap();
    let memory = Arc::new(AgentMemory::with_config(
        store.clone(),
        MemoryConfig {
            prune_policy: Some(PrunePolicy::default()),
            prune_interval_secs: 10,
            ..Default::default()
        },
    ));
    let runtime = MemoryMaintenanceRuntime::start(
        "native-prune-owner",
        memory,
        MemoryMaintenanceOptions::new(),
    )
    .unwrap();
    tokio::task::yield_now().await;
    tokio::time::sleep(std::time::Duration::from_secs(10)).await;
    tokio::task::yield_now().await;
    assert!(runtime.close().await.is_clean(), "{:?}", runtime.health());
    assert_eq!(store.count().await.unwrap(), 0, "{:?}", runtime.health());
    assert!(runtime
        .health()
        .jobs
        .iter()
        .any(|job| job.total_affected_items == 1));
}

async fn stores(root: &std::path::Path) -> Vec<(&'static str, Arc<dyn MemoryStore>)> {
    let stores: Vec<(&str, Arc<dyn MemoryStore>)> = vec![
        ("memory", Arc::new(InMemoryStore::new())),
        (
            "file",
            Arc::new(FileMemoryStore::new(root.join("file 空间")).await.unwrap()),
        ),
        (
            "native",
            Arc::new(LazyFileMemoryStore::new(root.join("native 空间"))),
        ),
    ];
    #[cfg(feature = "durable-memory-sqlite")]
    let stores = {
        let mut stores = stores;
        stores.push((
            "sqlite",
            Arc::new(
                aster_durable_memory::SqliteMemoryStore::new(root.join("sqlite 空间"))
                    .await
                    .unwrap(),
            ),
        ));
        stores
    };
    stores
}
