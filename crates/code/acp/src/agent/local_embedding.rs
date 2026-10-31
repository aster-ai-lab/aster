//! Embedding backend for trusted `workspace_retrieval.local_cpu` opt-in.
//!
//! A registered Rust host factory takes precedence. Without one, ACP loads
//! the in-tree light tier from embedded assets; it does not download a model.
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
    // A registered host factory wins; otherwise the in-tree light tier
    // serves the trusted local_cpu opt-in (specs/013 P2 wiring).
    let load = tokio::task::spawn_blocking(|| match FACTORY.get() {
        Some(factory) => factory().ok_or(LocalEmbeddingSetupError::InvalidFactory),
        None => aster_embedding::LightTierProvider::load()
            .map(|provider| Arc::new(provider) as Arc<dyn EmbeddingProvider>)
            .map_err(|_| LocalEmbeddingSetupError::LightTierLoad),
    });
    tokio::select! {
        biased;
        _ = cancellation.cancelled() => Err(LocalEmbeddingSetupError::Cancelled),
        loaded = load => loaded.map_err(|_| LocalEmbeddingSetupError::WorkerFailed)?,
    }
}

#[cfg(test)]
mod byo_factory_tests {
    use super::*;
    use aster_core::{
        EmbeddingBatchRequest, EmbeddingBatchResponse, EmbeddingProviderDescriptor,
        EmbeddingProviderError,
    };
    use async_trait::async_trait;

    struct FixtureProvider;

    #[async_trait]
    impl EmbeddingProvider for FixtureProvider {
        fn descriptor(&self) -> EmbeddingProviderDescriptor {
            EmbeddingProviderDescriptor {
                provider: "fixture".into(),
                model: "fixture-model".into(),
                revision: None,
                dimension: 8,
                normalization: aster_core::EmbeddingNormalization::Unit,
            }
        }
        async fn embed(
            &self,
            _request: EmbeddingBatchRequest,
            _cancellation: tokio_util::sync::CancellationToken,
        ) -> Result<EmbeddingBatchResponse, EmbeddingProviderError> {
            let descriptor = self.descriptor();
            Ok(EmbeddingBatchResponse::new(
                descriptor,
                vec![aster_core::EmbeddingVector {
                    id: "fixture-0".into(),
                    values: vec![0.0; 8],
                }],
            ))
        }
    }

    fn good() -> Option<Arc<dyn EmbeddingProvider>> {
        Some(Arc::new(FixtureProvider))
    }

    // One test owns the process-global registration transition.
    #[tokio::test]
    async fn embedded_light_then_registered_factory_serves_the_opt_in() {
        let cancelled = tokio_util::sync::CancellationToken::new();
        cancelled.cancel();
        assert!(matches!(
            local_embedding_provider(cancelled).await,
            Err(LocalEmbeddingSetupError::Cancelled)
        ));
        let local = local_embedding_provider(tokio_util::sync::CancellationToken::new())
            .await
            .expect("embedded assets load without registration");
        assert_eq!(local.descriptor().provider, "light");
        assert!(local.descriptor().dimension > 0);
        register_embedding_provider(good).expect("first registration");
        let hosted = local_embedding_provider(tokio_util::sync::CancellationToken::new())
            .await
            .expect("registered host factory serves");
        assert_eq!(hosted.descriptor().provider, "fixture");
        assert_eq!(hosted.descriptor().dimension, 8);
        assert!(register_embedding_provider(good).is_err());
    }
}
