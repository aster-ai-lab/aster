use super::*;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use a3s_code_core::EmbeddingInput;
use a3s_code_core::{
    EmbeddingError, EmbeddingExecutor, EmbeddingExecutorConfig, EmbeddingFailureKind,
};
use a3s_use_bge::{EmbeddingItem, EmbeddingResponse, Usage};
use tokio::sync::{Notify, Semaphore};

fn revision() -> String {
    "a".repeat(64)
}

fn output(vectors: Vec<Vec<f32>>) -> EmbeddingOutput {
    EmbeddingOutput {
        response: EmbeddingResponse {
            object: "list",
            data: vectors
                .into_iter()
                .enumerate()
                .map(|(index, embedding)| EmbeddingItem {
                    object: "embedding",
                    index,
                    embedding,
                })
                .collect(),
            model: MODEL.to_string(),
            usage: Usage {
                prompt_tokens: 1,
                total_tokens: 1,
            },
        },
        revision: revision(),
    }
}

struct FakeBackend {
    revision_error: Option<&'static str>,
    calls: AtomicUsize,
    outputs: Mutex<VecDeque<UseResult<EmbeddingOutput>>>,
}

impl FakeBackend {
    fn with_outputs(outputs: Vec<EmbeddingOutput>) -> Arc<Self> {
        Arc::new(Self {
            revision_error: None,
            calls: AtomicUsize::new(0),
            outputs: Mutex::new(outputs.into_iter().map(Ok).collect()),
        })
    }
}

#[async_trait]
impl LocalBackend for FakeBackend {
    fn installed_revision(&self) -> UseResult<String> {
        match self.revision_error {
            Some(code) => Err(UseError::new(code, "fixture asset refusal")),
            None => Ok(revision()),
        }
    }

    async fn embed(&self, _: EmbeddingRequest, _: CancellationToken) -> UseResult<EmbeddingOutput> {
        self.calls.fetch_add(1, Ordering::AcqRel);
        self.outputs
            .lock()
            .unwrap()
            .pop_front()
            .expect("fixture output")
    }
}

fn request(texts: &[&str]) -> EmbeddingBatchRequest {
    EmbeddingBatchRequest::new(
        texts
            .iter()
            .enumerate()
            .map(|(index, text)| EmbeddingInput::new(format!("input-{index}"), *text))
            .collect(),
    )
}

async fn prepare(backend: Arc<dyn LocalBackend>) -> LocalEmbeddingProvider {
    LocalEmbeddingProvider::prepare(backend, CancellationToken::new(), Duration::from_secs(1))
        .await
        .expect("fixture provider")
}

#[tokio::test]
async fn missing_and_corrupt_assets_refuse_before_any_probe() {
    for (code, expected) in [
        (
            "use.bge.model_missing",
            LocalEmbeddingSetupError::MissingAssets,
        ),
        (
            "use.bge.model_invalid",
            LocalEmbeddingSetupError::InvalidAssets,
        ),
    ] {
        let backend = Arc::new(FakeBackend {
            revision_error: Some(code),
            calls: AtomicUsize::new(0),
            outputs: Mutex::new(VecDeque::new()),
        });
        let result = LocalEmbeddingProvider::prepare(
            backend.clone(),
            CancellationToken::new(),
            Duration::from_secs(1),
        )
        .await;
        assert!(matches!(result, Err(error) if error == expected));
        assert_eq!(backend.calls.load(Ordering::Acquire), 0);
    }
}

#[tokio::test]
async fn actual_probe_shape_and_receipt_bind_the_generation_and_input_ids() {
    let mut response = output(vec![vec![0.0, 1.0], vec![1.0, 0.0]]);
    response.response.data.reverse();
    let backend = FakeBackend::with_outputs(vec![output(vec![vec![1.0, 0.0]]), response]);
    let provider = prepare(backend.clone()).await;
    let descriptor = provider.descriptor();
    assert_eq!(descriptor.dimension, 2);
    assert_eq!(descriptor.revision, Some(format!("sha256:{}", revision())));
    assert_eq!(descriptor.normalization, EmbeddingNormalization::None);
    let result = provider
        .embed(request(&["one", "two"]), CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(result.vectors[0].id.as_ref(), "input-0");
    assert_eq!(result.vectors[0].values, vec![0.0, 1.0]);
    assert_eq!(result.vectors[1].id.as_ref(), "input-1");
    assert_eq!(backend.calls.load(Ordering::Acquire), 2);
}

#[tokio::test]
async fn invalid_probe_output_never_creates_a_provider() {
    for vectors in [
        vec![],
        vec![vec![]],
        vec![vec![f32::NAN]],
        vec![vec![0.0; MAX_DIMENSION + 1]],
    ] {
        let backend = FakeBackend::with_outputs(vec![output(vectors)]);
        let result = LocalEmbeddingProvider::prepare(
            backend,
            CancellationToken::new(),
            Duration::from_secs(1),
        )
        .await;
        assert!(matches!(
            result,
            Err(LocalEmbeddingSetupError::InvalidOutput)
        ));
    }
}

#[tokio::test]
async fn batch_bounds_refuse_without_entering_the_backend() {
    let backend = FakeBackend::with_outputs(vec![output(vec![vec![1.0, 0.0]])]);
    let provider = prepare(backend.clone()).await;
    let long = "x".repeat(MAX_INPUT_BYTES + 1);
    let aggregate = "x".repeat(MAX_INPUT_BYTES);
    let cases = [
        request(&[]),
        request(&vec!["x"; MAX_BATCH_INPUTS + 1]),
        request(&[long.as_str()]),
        request(&[aggregate.as_str(); 5]),
    ];
    for request in cases {
        let result = provider.embed(request, CancellationToken::new()).await;
        assert!(matches!(
            result,
            Err(EmbeddingProviderError::InvalidRequest)
        ));
    }
    assert_eq!(backend.calls.load(Ordering::Acquire), 1);
}

#[tokio::test]
async fn runtime_shape_and_generation_changes_are_permanent_failures() {
    let mut changed = output(vec![vec![1.0, 0.0]]);
    changed.revision = "b".repeat(64);
    for response in [
        changed,
        output(vec![vec![1.0]]),
        output(vec![vec![f32::INFINITY, 0.0]]),
    ] {
        let backend = FakeBackend::with_outputs(vec![output(vec![vec![1.0, 0.0]]), response]);
        let provider = prepare(backend).await;
        let result = provider
            .embed(request(&["one"]), CancellationToken::new())
            .await;
        assert!(result.is_err());
        if let Err(error) = result {
            assert!(!error.is_retryable());
        }
    }
}

#[tokio::test]
async fn duplicate_or_unknown_output_indices_do_not_cross_input_identities() {
    let mut duplicate = output(vec![vec![1.0, 0.0], vec![0.0, 1.0]]);
    duplicate.response.data[1].index = 0;
    let mut unknown = output(vec![vec![1.0, 0.0], vec![0.0, 1.0]]);
    unknown.response.data[1].index = 2;
    for response in [duplicate, unknown] {
        let backend = FakeBackend::with_outputs(vec![output(vec![vec![1.0, 0.0]]), response]);
        let provider = prepare(backend).await;
        assert!(matches!(
            provider
                .embed(request(&["one", "two"]), CancellationToken::new())
                .await,
            Err(EmbeddingProviderError::Other)
        ));
    }
}

struct BlockingBackend {
    probe_ready: bool,
    calls: AtomicUsize,
    started: Notify,
    cancelled: Arc<Notify>,
    release: Arc<Semaphore>,
    admission: Arc<Semaphore>,
}

impl BlockingBackend {
    fn new(probe_ready: bool) -> Arc<Self> {
        Arc::new(Self {
            probe_ready,
            calls: AtomicUsize::new(0),
            started: Notify::new(),
            cancelled: Arc::new(Notify::new()),
            release: Arc::new(Semaphore::new(0)),
            admission: Arc::new(Semaphore::new(1)),
        })
    }
}

#[async_trait]
impl LocalBackend for BlockingBackend {
    fn installed_revision(&self) -> UseResult<String> {
        Ok(revision())
    }

    async fn embed(
        &self,
        _: EmbeddingRequest,
        cancellation: CancellationToken,
    ) -> UseResult<EmbeddingOutput> {
        let call = self.calls.fetch_add(1, Ordering::AcqRel);
        if call == 0 && self.probe_ready {
            return Ok(output(vec![vec![1.0, 0.0]]));
        }
        let permit = Arc::clone(&self.admission).acquire_owned().await.unwrap();
        let release = Arc::clone(&self.release);
        let (sender, receiver) = tokio::sync::oneshot::channel();
        self.started.notify_one();
        // The task owns admission even when the adapter future is dropped.
        // This fixture proves adapter cancellation, not ONNX termination.
        let cancelled = Arc::clone(&self.cancelled);
        tokio::spawn(async move {
            cancellation.cancelled().await;
            cancelled.notify_one();
            let release = release.acquire().await.unwrap();
            release.forget();
            drop(permit);
            let _ = sender.send(Err(UseError::new("use.bge.cancelled", "fixture cancelled")));
        });
        receiver.await.unwrap()
    }
}

#[tokio::test]
async fn pre_cancelled_preparation_does_not_enter_the_backend() {
    let backend = FakeBackend::with_outputs(vec![]);
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    let result =
        LocalEmbeddingProvider::prepare(backend.clone(), cancellation, Duration::from_secs(1))
            .await;
    assert!(matches!(result, Err(LocalEmbeddingSetupError::Cancelled)));
    assert_eq!(backend.calls.load(Ordering::Acquire), 0);
}

#[tokio::test]
async fn dropped_provider_future_cancels_backend_without_releasing_owned_worker() {
    let backend = BlockingBackend::new(true);
    let provider = Arc::new(prepare(backend.clone()).await);
    let task = tokio::spawn(async move {
        provider
            .embed(request(&["one"]), CancellationToken::new())
            .await
    });
    tokio::time::timeout(Duration::from_secs(1), backend.started.notified())
        .await
        .unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    tokio::time::timeout(Duration::from_secs(1), backend.cancelled.notified())
        .await
        .unwrap();
    assert_eq!(backend.admission.available_permits(), 0);
    backend.release.add_permits(1);
    let permit = tokio::time::timeout(Duration::from_secs(1), backend.admission.acquire())
        .await
        .unwrap()
        .unwrap();
    drop(permit);
}

#[tokio::test]
async fn probe_timeout_cancels_work_and_does_not_claim_native_settlement() {
    let backend = BlockingBackend::new(false);
    let result = LocalEmbeddingProvider::prepare(
        backend.clone(),
        CancellationToken::new(),
        Duration::from_millis(20),
    )
    .await;
    assert!(matches!(result, Err(LocalEmbeddingSetupError::TimedOut)));
    tokio::time::timeout(Duration::from_secs(1), backend.cancelled.notified())
        .await
        .unwrap();
    assert_eq!(backend.admission.available_permits(), 0);
    backend.release.add_permits(1);
    let permit = tokio::time::timeout(Duration::from_secs(1), backend.admission.acquire())
        .await
        .unwrap()
        .unwrap();
    drop(permit);
}

#[tokio::test]
async fn core_executor_timeout_drops_provider_future_and_keeps_worker_admission() {
    let backend = BlockingBackend::new(true);
    let provider = Arc::new(prepare(backend.clone()).await);
    let executor = EmbeddingExecutor::new(
        provider,
        EmbeddingExecutorConfig {
            max_retries: 0,
            request_timeout: Duration::from_millis(20),
            ..EmbeddingExecutorConfig::default()
        },
    )
    .unwrap();
    let result = executor
        .embed(
            vec![EmbeddingInput::new("one", "one")],
            CancellationToken::new(),
        )
        .await;
    assert!(matches!(
        result,
        Err(EmbeddingError::RetriesExhausted {
            kind: EmbeddingFailureKind::Timeout,
            attempts: 1
        })
    ));
    tokio::time::timeout(Duration::from_secs(1), backend.cancelled.notified())
        .await
        .unwrap();
    assert_eq!(backend.admission.available_permits(), 0);
    backend.release.add_permits(1);
    let permit = tokio::time::timeout(Duration::from_secs(1), backend.admission.acquire())
        .await
        .unwrap()
        .unwrap();
    drop(permit);
}
