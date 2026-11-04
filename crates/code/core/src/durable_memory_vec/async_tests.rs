use super::*;
use management::{MemoryManagementCode, MemoryManagementRequest, MemoryManagementResponse};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::Duration;

#[tokio::test]
async fn lazy_open_leaves_the_executor_free_while_waiting_for_the_registry() {
    let root = tempfile::tempdir().unwrap();
    let store = LazyFileMemoryStore::new(root.path().join("记忆 with spaces"));
    assert_executor_progress(
        |ready, release| {
            let _guard = SHARED_STORES.lock().unwrap();
            ready.send(()).unwrap();
            release.recv_timeout(Duration::from_secs(2)).is_ok()
        },
        async move { MemoryStore::count(&store).await.map(|_| ()) },
    )
    .await;
}

#[tokio::test]
async fn lazy_write_leaves_the_executor_free_while_waiting_for_the_mutation_lock() {
    let root = tempfile::tempdir().unwrap();
    let store = LazyFileMemoryStore::new(root.path());
    MemoryStore::count(&store).await.unwrap();
    let owner = SHARED_STORES
        .lock()
        .unwrap()
        .get(&root.path().canonicalize().unwrap())
        .and_then(Weak::upgrade)
        .unwrap();
    assert_executor_progress(
        move |ready, release| {
            let _guard = owner.mutation.lock().unwrap();
            ready.send(()).unwrap();
            release.recv_timeout(Duration::from_secs(2)).is_ok()
        },
        async move { MemoryStore::store(&store, MemoryItem::new("executor boundary fact")).await },
    )
    .await;
}

#[tokio::test]
async fn lazy_management_leaves_the_executor_free_while_waiting_for_the_mutation_lock() {
    let root = tempfile::tempdir().unwrap();
    let store = LazyFileMemoryStore::new(root.path());
    let item = MemoryItem::new("management executor boundary fact");
    MemoryStore::store(&store, item.clone()).await.unwrap();
    let owner = SHARED_STORES
        .lock()
        .unwrap()
        .get(&root.path().canonicalize().unwrap())
        .and_then(Weak::upgrade)
        .unwrap();
    assert_executor_progress(
        move |ready, release| {
            let _guard = owner.mutation.lock().unwrap();
            ready.send(()).unwrap();
            release.recv_timeout(Duration::from_secs(2)).is_ok()
        },
        async move {
            store
                .manage(MemoryManagementRequest::Read { id: item.id })
                .await
                .map(|_| ())
                .map_err(Into::into)
        },
    )
    .await;
}

#[tokio::test]
async fn lazy_prune_leaves_the_executor_free_while_waiting_for_the_mutation_lock() {
    let root = tempfile::tempdir().unwrap();
    let store = LazyFileMemoryStore::new(root.path());
    let mut expired = MemoryItem::new("executor prune control").with_importance(0.1);
    expired.timestamp = Utc::now() - chrono::Duration::days(400);
    store.store(expired).await.unwrap();
    let owner = SHARED_STORES
        .lock()
        .unwrap()
        .get(&root.path().canonicalize().unwrap())
        .and_then(Weak::upgrade)
        .unwrap();
    assert_executor_progress(
        move |ready, release| {
            let _guard = owner.mutation.lock().unwrap();
            ready.send(()).unwrap();
            release.recv_timeout(Duration::from_secs(2)).is_ok()
        },
        async move {
            assert_eq!(store.prune(&Default::default()).await?, 1);
            Ok(())
        },
    )
    .await;
}

#[tokio::test]
async fn lazy_operations_and_management_share_the_original_collection() {
    let root = tempfile::tempdir().unwrap();
    let store = LazyFileMemoryStore::new(root.path().join("记忆 with spaces"));
    assert_eq!(store.prune(&Default::default()).await.unwrap(), 0);
    assert!(store.is_initialized());
    let mut first = MemoryItem::new("rankmarker first durable fact")
        .with_tag("rust")
        .with_importance(0.9)
        .with_type(MemoryType::Semantic);
    first.timestamp = DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let mut second = MemoryItem::new("second durable fact").with_importance(0.2);
    second.timestamp = first.timestamp + chrono::Duration::days(1);
    assert_eq!(
        store.store_and_return(first.clone()).await.unwrap().id,
        first.id
    );
    store.store(second.clone()).await.unwrap();
    assert_eq!(store.count().await.unwrap(), 2);
    assert_eq!(
        store.retrieve(&first.id).await.unwrap().unwrap().content,
        first.content
    );
    assert!(store.retrieve("absent").await.unwrap().is_none());
    assert_eq!(store.search("rankmarker", 1).await.unwrap()[0].id, first.id);
    assert_eq!(
        store.search_by_tags(&["rust".into()], 1).await.unwrap()[0].id,
        first.id
    );
    assert_eq!(store.get_recent(1).await.unwrap()[0].id, second.id);
    assert_eq!(store.get_important(0.8, 1).await.unwrap()[0].id, first.id);

    let MemoryManagementResponse::Read { item } = store
        .manage(MemoryManagementRequest::Read {
            id: first.id.clone(),
        })
        .await
        .unwrap()
    else {
        panic!("expected read")
    };
    store
        .manage(MemoryManagementRequest::Update {
            id: first.id.clone(),
            revision: item.revision.clone(),
            content: "updated durable fact".into(),
        })
        .await
        .unwrap();
    let conflict = store
        .manage(MemoryManagementRequest::Delete {
            id: first.id.clone(),
            revision: item.revision,
        })
        .await
        .unwrap_err();
    assert_eq!(conflict.code, MemoryManagementCode::Conflict);
    assert_eq!(
        store.retrieve(&first.id).await.unwrap().unwrap().content,
        "updated durable fact"
    );
    store.delete(&second.id).await.unwrap();
    assert_eq!(store.count().await.unwrap(), 1);
    store.clear().await.unwrap();
    assert_eq!(store.count().await.unwrap(), 0);
}

#[tokio::test]
async fn cancelled_await_keeps_the_running_workers_collection_until_write_finishes() {
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("记忆 with spaces");
    let store = Arc::new(LazyFileMemoryStore::new(&directory));
    store.count().await.unwrap();
    let (started, running) = tokio::sync::oneshot::channel();
    let (release, released) = mpsc::channel();
    let (done, settled) = tokio::sync::oneshot::channel();
    let worker_store = Arc::clone(&store);
    let task = tokio::spawn(async move {
        worker_store
            .with_store(move |store| {
                started.send(()).unwrap();
                released.recv_timeout(Duration::from_secs(2)).unwrap();
                let item = MemoryItem::new("cancelled await durable fact");
                store.put(&item_to_record(&item).unwrap()).unwrap();
                done.send(item.id).unwrap();
            })
            .await
    });
    running.await.unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    drop(store);
    assert!(VecMemoryStore::open(&directory).is_err());
    let peer = LazyFileMemoryStore::new(&directory);
    assert_eq!(peer.count().await.unwrap(), 0);
    drop(peer);
    release.send(()).unwrap();
    let id = settled.await.unwrap();
    let reopened = LazyFileMemoryStore::new(&directory);
    assert_eq!(reopened.count().await.unwrap(), 1);
    assert_eq!(
        reopened.retrieve(&id).await.unwrap().unwrap().content,
        "cancelled await durable fact"
    );
    drop(reopened);
    drop(VecMemoryStore::open(&directory).unwrap());
}

#[tokio::test]
async fn cancelled_initialization_can_retry_and_import_the_real_legacy_item() {
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("记忆 with spaces");
    let items = directory.join("items");
    std::fs::create_dir_all(&items).unwrap();
    let item = MemoryItem::new("cancelled initialization legacy fact");
    std::fs::write(
        items.join(format!("{}.json", item.id)),
        serde_json::to_vec(&item).unwrap(),
    )
    .unwrap();
    let store = Arc::new(LazyFileMemoryStore::new(&directory));
    let (ready, waiting) = tokio::sync::oneshot::channel();
    let (release, released) = mpsc::channel();
    let holder = std::thread::spawn(move || {
        let _guard = SHARED_STORES.lock().unwrap();
        ready.send(()).unwrap();
        released.recv_timeout(Duration::from_secs(2)).is_ok()
    });
    waiting.await.unwrap();
    let entered = Arc::new(AtomicBool::new(false));
    let started = Arc::clone(&entered);
    let first = Arc::clone(&store);
    let task = tokio::spawn(async move {
        started.store(true, Ordering::Release);
        first.count().await
    });
    while !entered.load(Ordering::Acquire) {
        tokio::task::yield_now().await;
    }
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert!(!store.is_initialized());
    release.send(()).unwrap();
    assert!(holder.join().unwrap());
    assert_eq!(store.count().await.unwrap(), 1);
    assert_eq!(
        store.retrieve(&item.id).await.unwrap().unwrap().content,
        item.content
    );
    assert!(directory.join("items.imported").is_dir());
    assert!(!items.exists());
}

#[tokio::test]
async fn management_admits_once_and_classifies_initialization_errors_by_type() {
    let root = tempfile::tempdir().unwrap();
    let collision = root.path().join("collection is locked");
    std::fs::write(&collision, b"a file is not a memory directory").unwrap();
    let failed = LazyFileMemoryStore::new(&collision);
    let invalid = failed
        .manage(MemoryManagementRequest::Read { id: String::new() })
        .await
        .unwrap_err();
    assert_eq!(invalid.code, MemoryManagementCode::Invalid);
    assert!(!failed.is_initialized());
    let error = failed
        .manage(MemoryManagementRequest::Read { id: "valid".into() })
        .await
        .unwrap_err();
    assert_eq!(error.code, MemoryManagementCode::Failed);
    let source = failed.count().await.unwrap_err();
    assert!(source.is::<std::io::Error>());

    let directory = root.path().join("busy");
    let collection = directory.join(COLLECTION_NAME);
    std::fs::create_dir_all(&collection).unwrap();
    let lock = std::fs::File::create(collection.join(".aster-vec.lock")).unwrap();
    fs2::FileExt::try_lock_exclusive(&lock).unwrap();
    let busy = LazyFileMemoryStore::new(&directory);
    let error = busy
        .manage(MemoryManagementRequest::Read { id: "valid".into() })
        .await
        .unwrap_err();
    assert_eq!(error.code, MemoryManagementCode::Busy);
    assert!(!busy.is_initialized());
}

#[test]
fn missing_runtime_is_a_recoverable_error_before_opening_the_store() {
    let root = tempfile::tempdir().unwrap();
    let store = LazyFileMemoryStore::new(root.path().join("memory"));
    let error = futures::executor::block_on(store.count()).unwrap_err();
    assert!(error.is::<tokio::runtime::TryCurrentError>());
    assert!(!store.is_initialized());
    assert!(!root.path().join("memory").exists());
}

#[tokio::test]
async fn worker_panic_is_a_recoverable_join_error_and_releases_the_handle() {
    let root = tempfile::tempdir().unwrap();
    let store = LazyFileMemoryStore::new(root.path());
    let error = store
        .with_store(|_| panic!("synthetic memory worker failure"))
        .await
        .unwrap_err();
    assert!(error
        .downcast_ref::<tokio::task::JoinError>()
        .unwrap()
        .is_panic());
    assert_eq!(store.count().await.unwrap(), 0);
    drop(store);
    drop(VecMemoryStore::open(root.path()).unwrap());
}

async fn assert_executor_progress(
    hold_lock: impl FnOnce(tokio::sync::oneshot::Sender<()>, mpsc::Receiver<()>) -> bool
        + Send
        + 'static,
    operation: impl std::future::Future<Output = Result<()>> + Send + 'static,
) {
    let (ready, waiting) = tokio::sync::oneshot::channel();
    let (release, released) = mpsc::channel();
    let holder = std::thread::spawn(move || hold_lock(ready, released));
    waiting.await.unwrap();
    let entered = Arc::new(AtomicBool::new(false));
    let started = Arc::clone(&entered);
    let task = tokio::spawn(async move {
        started.store(true, Ordering::Release);
        operation.await
    });
    while !entered.load(Ordering::Acquire) {
        tokio::task::yield_now().await;
    }
    let _ = release.send(());
    task.await.unwrap().unwrap();
    assert!(
        holder.join().unwrap(),
        "memory work blocked the executor until the lock watchdog expired"
    );
}
