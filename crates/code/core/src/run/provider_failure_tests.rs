use super::*;

#[tokio::test]
async fn provider_failure_follows_run_retention_without_creating_unknown_runs() {
    let store = InMemoryRunStore::with_retention(Some(1), Some(1));
    let error: anyhow::Error =
        crate::llm::RetryExhaustedError::from_status(2, 503, "fixture").into();
    let failure = crate::llm::RetryableProviderFailure::from_error(&error).unwrap();
    store.record_provider_failure("unknown", failure).await;
    assert!(store.events.read().await.is_empty());
    let first = store.create_run("session", "first").await;
    store.record_provider_failure(&first.id, failure).await;
    assert_eq!(store.provider_failure(&first.id).await, Some(failure));
    let next = store.create_run("session", "next").await;
    assert!(store.provider_failure(&first.id).await.is_none());
    assert!(store.provider_failure(&next.id).await.is_none());
}
