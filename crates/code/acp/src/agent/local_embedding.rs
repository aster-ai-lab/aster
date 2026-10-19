//! Host-owned local BGE adapter. No installer, remote fallback, or source text
//! is exposed through provider errors or descriptors.

use std::fmt;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use a3s_code_core::{
    EmbeddingBatchRequest, EmbeddingBatchResponse, EmbeddingNormalization, EmbeddingProvider,
    EmbeddingProviderDescriptor, EmbeddingProviderError, EmbeddingVector,
};
use a3s_use_bge::{EmbeddingClient, EmbeddingOutput, EmbeddingRequest, UseError, UseResult};
use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

const PROVIDER: &str = "local.bge";
const MODEL: &str = "bge-m3";
const MAX_BATCH_INPUTS: usize = 64;
const MAX_INPUT_BYTES: usize = 64 * 1024;
const MAX_BATCH_BYTES: usize = 256 * 1024;
const MAX_DIMENSION: usize = 65_536;
const PROBE_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum LocalEmbeddingSetupError {
    MissingAssets,
    InvalidAssets,
    ChangedAssets,
    InvalidOutput,
    RuntimeUnavailable,
    Cancelled,
    TimedOut,
}

impl fmt::Display for LocalEmbeddingSetupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::MissingAssets => "local BGE model assets are not installed",
            Self::InvalidAssets => "local BGE model assets are invalid",
            Self::ChangedAssets => "local BGE model assets changed during preparation",
            Self::InvalidOutput => "local BGE returned an invalid embedding shape",
            Self::RuntimeUnavailable => "local BGE runtime is unavailable",
            Self::Cancelled => "local BGE preparation was cancelled",
            Self::TimedOut => "local BGE preparation exceeded its 30-second deadline",
        })
    }
}

impl std::error::Error for LocalEmbeddingSetupError {}

/// Called only after a host-trusted local retrieval opt-in. Missing assets
/// refuse before the dimension probe, which is local and explicitly bounded.
pub(super) async fn local_embedding_provider(
    cancellation: CancellationToken,
) -> Result<Arc<dyn EmbeddingProvider>, LocalEmbeddingSetupError> {
    static CLIENT: OnceLock<EmbeddingClient> = OnceLock::new();
    let client = match CLIENT.get() {
        Some(client) => client.clone(),
        None => {
            let client = EmbeddingClient::from_env().map_err(|error| setup_error(&error))?;
            let _ = CLIENT.set(client.clone());
            CLIENT.get().cloned().unwrap_or(client)
        }
    };
    let backend = Arc::new(BgeBackend(client));
    let provider = LocalEmbeddingProvider::prepare(backend, cancellation, PROBE_TIMEOUT).await?;
    Ok(Arc::new(provider))
}

#[async_trait]
trait LocalBackend: Send + Sync {
    fn installed_revision(&self) -> UseResult<String>;
    async fn embed(
        &self,
        request: EmbeddingRequest,
        cancellation: CancellationToken,
    ) -> UseResult<EmbeddingOutput>;
}

struct BgeBackend(EmbeddingClient);

#[async_trait]
impl LocalBackend for BgeBackend {
    fn installed_revision(&self) -> UseResult<String> {
        self.0.installed_revision()
    }

    async fn embed(
        &self,
        request: EmbeddingRequest,
        cancellation: CancellationToken,
    ) -> UseResult<EmbeddingOutput> {
        if cancellation.is_cancelled() {
            return Err(UseError::new(
                "use.bge.cancelled",
                "The local embedding request was cancelled.",
            ));
        }
        self.0
            .start_embedding(request)?
            .settle_until_cancelled(cancellation.cancelled())
            .await
    }
}

struct LocalEmbeddingProvider {
    backend: Arc<dyn LocalBackend>,
    descriptor: EmbeddingProviderDescriptor,
}

impl LocalEmbeddingProvider {
    async fn prepare(
        backend: Arc<dyn LocalBackend>,
        cancellation: CancellationToken,
        timeout: Duration,
    ) -> Result<Self, LocalEmbeddingSetupError> {
        if cancellation.is_cancelled() {
            return Err(LocalEmbeddingSetupError::Cancelled);
        }
        let revision = backend
            .installed_revision()
            .map_err(|error| setup_error(&error))?;
        if !valid_revision(&revision) {
            return Err(LocalEmbeddingSetupError::InvalidAssets);
        }
        let child = cancellation.child_token();
        let _cancel_on_drop = child.clone().drop_guard();
        let probe = backend.embed(
            EmbeddingRequest {
                model: None,
                input: vec!["Aster local embedding dimension probe".to_string()],
            },
            child,
        );
        let output = tokio::time::timeout(timeout, probe)
            .await
            .map_err(|_| LocalEmbeddingSetupError::TimedOut)?
            .map_err(|error| setup_error(&error))?;
        if cancellation.is_cancelled() {
            return Err(LocalEmbeddingSetupError::Cancelled);
        }
        if output.revision != revision {
            return Err(LocalEmbeddingSetupError::ChangedAssets);
        }
        let vectors = ordered_vectors(output, 1, None)
            .map_err(|_| LocalEmbeddingSetupError::InvalidOutput)?;
        let dimension = vectors[0].len();
        let descriptor = EmbeddingProviderDescriptor::new(PROVIDER, MODEL, dimension)
            .with_revision(format!("sha256:{revision}"))
            // BGE preserves a degenerate zero vector; it cannot promise every
            // vector is unit normalized.
            .with_normalization(EmbeddingNormalization::None);
        Ok(Self {
            backend,
            descriptor,
        })
    }
}

#[async_trait]
impl EmbeddingProvider for LocalEmbeddingProvider {
    fn descriptor(&self) -> EmbeddingProviderDescriptor {
        self.descriptor.clone()
    }

    async fn embed(
        &self,
        request: EmbeddingBatchRequest,
        cancellation: CancellationToken,
    ) -> Result<EmbeddingBatchResponse, EmbeddingProviderError> {
        if cancellation.is_cancelled() {
            return Err(EmbeddingProviderError::Cancelled);
        }
        if request.inputs().is_empty()
            || request.inputs().len() > MAX_BATCH_INPUTS
            || request.text_bytes() > MAX_BATCH_BYTES
            || request
                .inputs()
                .iter()
                .any(|input| input.text_bytes() > MAX_INPUT_BYTES)
        {
            return Err(EmbeddingProviderError::InvalidRequest);
        }
        let child = cancellation.child_token();
        let _cancel_on_drop = child.clone().drop_guard();
        let output = self
            .backend
            .embed(
                EmbeddingRequest {
                    model: None,
                    input: request
                        .inputs()
                        .iter()
                        .map(|input| input.text().to_string())
                        .collect(),
                },
                child,
            )
            .await
            .map_err(|error| provider_error(&error))?;
        if cancellation.is_cancelled() {
            return Err(EmbeddingProviderError::Cancelled);
        }
        if self.descriptor.revision.as_deref()
            != Some(format!("sha256:{}", output.revision).as_str())
        {
            // A fixed provider generation cannot publish vectors from newly
            // installed weights. This is a permanent provider failure, not a
            // retryable outage or a malformed caller request.
            return Err(EmbeddingProviderError::Other);
        }
        let vectors = ordered_vectors(
            output,
            request.inputs().len(),
            Some(self.descriptor.dimension),
        )?;
        Ok(EmbeddingBatchResponse::new(
            self.descriptor.clone(),
            request
                .inputs()
                .iter()
                .zip(vectors)
                .map(|(input, vector)| EmbeddingVector::new(input.id(), vector))
                .collect(),
        ))
    }
}

fn valid_revision(revision: &str) -> bool {
    revision.len() == 64
        && revision
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn ordered_vectors(
    output: EmbeddingOutput,
    expected: usize,
    dimension: Option<usize>,
) -> Result<Vec<Vec<f32>>, EmbeddingProviderError> {
    if output.response.model != MODEL || output.response.data.len() != expected {
        return Err(EmbeddingProviderError::Other);
    }
    let mut ordered = vec![None; expected];
    for item in output.response.data {
        if item.index >= expected
            || ordered[item.index].is_some()
            || item.embedding.is_empty()
            || item.embedding.len() > MAX_DIMENSION
            || dimension.is_some_and(|dimension| item.embedding.len() != dimension)
            || item.embedding.iter().any(|value| !value.is_finite())
        {
            return Err(EmbeddingProviderError::Other);
        }
        ordered[item.index] = Some(item.embedding);
    }
    ordered
        .into_iter()
        .map(|vector| vector.ok_or(EmbeddingProviderError::Other))
        .collect()
}

fn setup_error(error: &UseError) -> LocalEmbeddingSetupError {
    match error.code.as_str() {
        "use.bge.model_missing" => LocalEmbeddingSetupError::MissingAssets,
        "use.bge.model_invalid" => LocalEmbeddingSetupError::InvalidAssets,
        "use.bge.cancelled" => LocalEmbeddingSetupError::Cancelled,
        "use.bge.provider_output_invalid" => LocalEmbeddingSetupError::InvalidOutput,
        _ => LocalEmbeddingSetupError::RuntimeUnavailable,
    }
}

fn provider_error(error: &UseError) -> EmbeddingProviderError {
    match error.code.as_str() {
        "use.bge.cancelled" => EmbeddingProviderError::Cancelled,
        "use.bge.provider_input_invalid" | "use.bge.request_invalid" => {
            EmbeddingProviderError::InvalidRequest
        }
        _ => EmbeddingProviderError::Other,
    }
}

#[cfg(test)]
#[path = "local_embedding_tests.rs"]
mod tests;
