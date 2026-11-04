//! Embedding backend for trusted `workspace_retrieval.local_cpu` opt-in.
//!
//! A registered Rust host factory takes precedence. Without one, ACP loads
//! the in-tree light tier once per process from embedded assets; it does not
//! download a model. Session indexes and their source text remain separate.
//! A broken registered factory refuses instead of falling back silently.

use std::fmt;
use std::sync::{Arc, OnceLock};

use aster_core::EmbeddingProvider;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum LocalEmbeddingSetupError {
    InvalidFactory,
    LightTierLoad,
    Cancelled,
    WorkerFailed,
}

impl fmt::Display for LocalEmbeddingSetupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidFactory => {
                "the registered embedding provider factory returned no provider"
            }
            Self::LightTierLoad => "the in-tree light tier failed to load from its embedded assets",
            Self::Cancelled => "workspace embedding setup was cancelled",
            Self::WorkerFailed => "workspace embedding setup worker failed",
        })
    }
}

type Factory = fn() -> Option<Arc<dyn EmbeddingProvider>>;

static FACTORY: OnceLock<Factory> = OnceLock::new();
static LIGHT_TIER: OnceLock<Result<Arc<dyn EmbeddingProvider>, LocalEmbeddingSetupError>> =
    OnceLock::new();

/// A host registers its provider factory exactly once, before the first
/// session. A second registration refuses with its reason.
pub fn register_embedding_provider(factory: Factory) -> Result<(), String> {
    FACTORY
        .set(factory)
        .map_err(|_| "an embedding provider factory is already registered".to_owned())
}

pub(super) async fn local_embedding_provider(
    cancellation: tokio_util::sync::CancellationToken,
) -> Result<Arc<dyn EmbeddingProvider>, LocalEmbeddingSetupError> {
    if cancellation.is_cancelled() {
        return Err(LocalEmbeddingSetupError::Cancelled);
    }
    if let (None, Some(provider)) = (FACTORY.get(), LIGHT_TIER.get()) {
        return provider.clone();
    }
    // A registered host factory wins; otherwise the in-tree light tier
    // serves the trusted local_cpu opt-in (specs/013 P2 wiring).
    let load = tokio::task::spawn_blocking(|| match FACTORY.get() {
        Some(factory) => factory().ok_or(LocalEmbeddingSetupError::InvalidFactory),
        None => LIGHT_TIER
            .get_or_init(|| {
                let started = std::time::Instant::now();
                let loaded = aster_embedding::LightTierProvider::load()
                    .map(|provider| Arc::new(provider) as Arc<dyn EmbeddingProvider>)
                    .map_err(|_| LocalEmbeddingSetupError::LightTierLoad);
                tracing::info!(
                    target: "aster_acp::retrieval_phase",
                    phase = "bundled_model_load",
                    success = loaded.is_ok(),
                    elapsed_ms = started.elapsed().as_secs_f64() * 1_000.0,
                    "ACP phase completed"
                );
                loaded
            })
            .clone(),
    });
    tokio::select! {
        biased;
        _ = cancellation.cancelled() => Err(LocalEmbeddingSetupError::Cancelled),
        loaded = load => loaded.map_err(|_| LocalEmbeddingSetupError::WorkerFailed)?,
    }
}

#[cfg(test)]
#[path = "local_embedding_factory_tests.rs"]
mod byo_factory_tests;
