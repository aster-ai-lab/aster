use super::*;
use std::sync::mpsc;
use std::time::Duration;

fn output() -> EmbeddingOutput {
    EmbeddingOutput {
        response: EmbeddingResponse {
            object: "list",
            data: vec![],
            model: "fixture".to_string(),
            usage: Usage {
                prompt_tokens: 0,
                total_tokens: 0,
            },
        },
        revision: "fixture-revision".to_string(),
    }
}

#[tokio::test]
async fn cancellation_waits_for_worker_and_keeps_admission_until_settlement() {
    let admission = Arc::new(Semaphore::new(1));
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let first = EmbeddingOperation::start(Arc::clone(&admission), move |_| {
        started_tx.send(()).unwrap();
        release_rx.recv().unwrap();
        Ok(output())
    })
    .unwrap();
    started_rx.await.unwrap();
    first.cancel();
    let mut first = Box::pin(first.settle());
    assert!(tokio::time::timeout(Duration::from_millis(20), &mut first)
        .await
        .is_err());
    assert_eq!(admission.available_permits(), 0);

    let second_started = Arc::new(AtomicBool::new(false));
    let observed = Arc::clone(&second_started);
    let second = EmbeddingOperation::start(Arc::clone(&admission), move |_| {
        observed.store(true, Ordering::Release);
        Ok(output())
    })
    .unwrap();
    let mut second = Box::pin(second.settle());
    assert!(tokio::time::timeout(Duration::from_millis(20), &mut second)
        .await
        .is_err());
    assert!(!second_started.load(Ordering::Acquire));

    release_tx.send(()).unwrap();
    assert_eq!(first.await.unwrap_err().code, "use.bge.cancelled");
    second.await.unwrap();
    assert_eq!(admission.available_permits(), 1);
}

#[tokio::test]
async fn dropped_operation_requests_cancel_without_releasing_worker_admission() {
    let admission = Arc::new(Semaphore::new(1));
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let operation = EmbeddingOperation::start(Arc::clone(&admission), move |cancellation| {
        assert!(started_tx.send(cancellation).is_ok());
        release_rx.recv().unwrap();
        Ok(output())
    })
    .unwrap();
    let cancellation = started_rx.await.unwrap();
    drop(operation);
    assert!(cancellation.check().is_err());
    assert_eq!(admission.available_permits(), 0);
    release_tx.send(()).unwrap();
    let permit = tokio::time::timeout(Duration::from_secs(1), admission.acquire())
        .await
        .unwrap()
        .unwrap();
    drop(permit);
    assert_eq!(admission.available_permits(), 1);
}

#[tokio::test]
async fn queued_cancelled_operation_never_enters_blocking_work() {
    let admission = Arc::new(Semaphore::new(1));
    let permit = Arc::clone(&admission).acquire_owned().await.unwrap();
    let ran = Arc::new(AtomicBool::new(false));
    let observed = Arc::clone(&ran);
    let operation = EmbeddingOperation::start(Arc::clone(&admission), move |_| {
        observed.store(true, Ordering::Release);
        Ok(output())
    })
    .unwrap();
    tokio::task::yield_now().await;
    operation.cancel();
    let result = tokio::time::timeout(Duration::from_secs(1), operation.settle())
        .await
        .unwrap();
    assert_eq!(result.unwrap_err().code, "use.bge.cancelled");
    assert!(!ran.load(Ordering::Acquire));
    assert_eq!(admission.available_permits(), 0);
    drop(permit);
}

#[tokio::test]
async fn dropped_settlement_future_requests_cancel_and_keeps_worker_owned() {
    let admission = Arc::new(Semaphore::new(1));
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let operation = EmbeddingOperation::start(Arc::clone(&admission), move |cancellation| {
        assert!(started_tx.send(cancellation).is_ok());
        release_rx.recv().unwrap();
        Ok(output())
    })
    .unwrap();
    let settlement = tokio::spawn(operation.settle());
    let cancellation = started_rx.await.unwrap();
    settlement.abort();
    assert!(settlement.await.unwrap_err().is_cancelled());
    assert!(cancellation.check().is_err());
    assert_eq!(admission.available_permits(), 0);
    release_tx.send(()).unwrap();
    let permit = tokio::time::timeout(Duration::from_secs(1), admission.acquire())
        .await
        .unwrap()
        .unwrap();
    drop(permit);
}

#[test]
fn starting_outside_a_runtime_refuses_without_spawning() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<EmbeddingClient>();
    assert_send_sync::<EmbeddingOperation>();
    assert_send_sync::<EmbeddingOutput>();
    let result = EmbeddingOperation::start(Arc::new(Semaphore::new(1)), |_| Ok(output()));
    assert!(matches!(result, Err(error) if error.code == "use.bge.runtime_failed"));
}

#[test]
fn typed_request_bounds_refuse_before_asset_or_runtime_access() {
    let client = EmbeddingClient::from_env().unwrap();
    let requests = [
        EmbeddingRequest {
            model: None,
            input: vec![],
        },
        EmbeddingRequest {
            model: None,
            input: vec![String::new(); MAX_ITEMS + 1],
        },
        EmbeddingRequest {
            model: None,
            input: vec!["x".repeat(MAX_ITEM_BYTES + 1)],
        },
        EmbeddingRequest {
            model: Some("x".repeat(MAX_MODEL_BYTES + 1)),
            input: vec!["one".to_string()],
        },
    ];
    for request in requests {
        let result = client.start_embedding(request);
        assert!(matches!(result, Err(error) if error.code == "use.bge.request_invalid"));
    }
    // Match the existing wire contract: an empty string is legal, while an
    // empty input list is not a request and must not load native assets.
    assert!(validate_request(&EmbeddingRequest {
        model: None,
        input: vec![String::new()]
    })
    .is_ok());
}
