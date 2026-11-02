use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::{Notify, Semaphore};

struct DelayedFirstSave {
    inner: crate::store::MemorySessionStore,
    calls: AtomicUsize,
    first_entered: Notify,
    second_entered: Notify,
    release_first: Semaphore,
}

#[async_trait::async_trait]
impl SessionStore for DelayedFirstSave {
    async fn save(&self, session: &SessionData) -> anyhow::Result<()> {
        self.inner.save(session).await
    }
    async fn load(&self, id: &str) -> anyhow::Result<Option<SessionData>> {
        self.inner.load(id).await
    }
    async fn delete(&self, id: &str) -> anyhow::Result<()> {
        self.inner.delete(id).await
    }
    async fn list(&self) -> anyhow::Result<Vec<String>> {
        self.inner.list().await
    }
    async fn exists(&self, id: &str) -> anyhow::Result<bool> {
        self.inner.exists(id).await
    }
    async fn save_snapshot(&self, snapshot: &SessionSnapshotV1) -> anyhow::Result<()> {
        if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
            self.first_entered.notify_one();
            self.release_first.acquire().await.unwrap().forget();
        } else {
            self.second_entered.notify_one();
        }
        self.inner.save_snapshot(snapshot).await
    }
}

#[tokio::test]
async fn overlapping_snapshot_saves_cannot_publish_an_older_usage_snapshot_last() {
    let root = tempfile::tempdir().unwrap();
    let store = Arc::new(DelayedFirstSave {
        inner: crate::store::MemorySessionStore::new(),
        calls: AtomicUsize::new(0),
        first_entered: Notify::new(),
        second_entered: Notify::new(),
        release_first: Semaphore::new(0),
    });
    let agent = crate::Agent::from_config(crate::agent_api::tests::test_config())
        .await
        .unwrap();
    let session = Arc::new(
        agent
            .build_session(
                root.path().display().to_string(),
                Arc::new(crate::agent_api::tests::StaticStreamingClient {
                    text: "unused".into(),
                }),
                &SessionOptions::new()
                    .with_session_id("overlapping-usage")
                    .with_session_store(store.clone()),
            )
            .unwrap(),
    );
    let add = |tokens| {
        write_or_recover(&session.persistence_state).record_usage(&crate::llm::TokenUsage {
            prompt_tokens: tokens,
            total_tokens: tokens,
            ..Default::default()
        })
    };
    let context = SessionPersistenceContext::from_session(&session);
    add(30);
    let first = tokio::spawn({
        let context = context.clone();
        async move { context.save().await }
    });
    store.first_entered.notified().await;
    add(18);
    let second = tokio::spawn(async move { context.save().await });
    // Let an uncoordinated second writer commit first; a serialized writer waits.
    let _ = tokio::time::timeout(
        std::time::Duration::from_millis(50),
        store.second_entered.notified(),
    )
    .await;
    store.release_first.add_permits(1);
    first.await.unwrap().unwrap();
    second.await.unwrap().unwrap();
    let saved = store
        .inner
        .load_snapshot("overlapping-usage")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(saved.session.total_usage.total_tokens, 48);
    assert_eq!(store.calls.load(Ordering::SeqCst), 2);
    session.close().await;
    agent.close().await;
}

#[test]
fn usage_subscription_seeds_restored_totals_and_coalesces_updates() {
    let mut state = SessionPersistenceState::default();
    let mut baseline = snapshot_for("usage-watch").session;
    baseline.total_usage = crate::llm::TokenUsage {
        prompt_tokens: 30,
        total_tokens: 30,
        ..Default::default()
    };
    state.restore(baseline);
    let mut updates = state.subscribe_usage();
    assert_eq!(updates.borrow_and_update().total_tokens, 30);
    state.record_usage(&crate::llm::TokenUsage {
        completion_tokens: 7,
        total_tokens: 7,
        ..Default::default()
    });
    state.record_usage(&crate::llm::TokenUsage {
        completion_tokens: 11,
        total_tokens: 11,
        ..Default::default()
    });
    assert_eq!(updates.borrow_and_update().total_tokens, 48);
    drop(state);
    assert!(updates.has_changed().is_err());
}
