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
        local_embedding_provider(cancelled.clone()).await,
        Err(LocalEmbeddingSetupError::Cancelled)
    ));
    let started = std::time::Instant::now();
    let local = local_embedding_provider(tokio_util::sync::CancellationToken::new())
        .await
        .expect("embedded assets load without registration");
    assert_eq!(local.descriptor().provider, "light");
    assert!(local.descriptor().dimension > 0);
    let cold = started.elapsed();
    let started = std::time::Instant::now();
    let reused = local_embedding_provider(tokio_util::sync::CancellationToken::new())
        .await
        .expect("later sessions reuse the embedded provider");
    let warm = started.elapsed();
    assert!(
        Arc::ptr_eq(&local, &reused),
        "sessions reloaded the same model"
    );
    assert!(matches!(
        local_embedding_provider(cancelled).await,
        Err(LocalEmbeddingSetupError::Cancelled)
    ));
    eprintln!(
        "ORACLE embedding-cold-ms={} embedding-reuse-us={}",
        cold.as_millis(),
        warm.as_micros()
    );
    register_embedding_provider(good).expect("first registration");
    let hosted = local_embedding_provider(tokio_util::sync::CancellationToken::new())
        .await
        .expect("registered host factory serves");
    assert_eq!(hosted.descriptor().provider, "fixture");
    assert_eq!(hosted.descriptor().dimension, 8);
    assert!(register_embedding_provider(good).is_err());
}
