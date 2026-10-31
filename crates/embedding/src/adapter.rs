//! The EmbeddingProvider adapter: the light tier behind the core trait.
//!
//! This is the wiring point between crates/embedding's StaticEmbeddingProvider
//! and core's EmbeddingProvider contract. A host (or the acp factory) wraps
//! the light tier in this adapter and the retrieval stack sees a uniform
//! provider regardless of tier.

use std::sync::Arc;

use aster_core::{
    EmbeddingBatchRequest, EmbeddingBatchResponse, EmbeddingNormalization, EmbeddingProvider,
    EmbeddingProviderDescriptor, EmbeddingProviderError, EmbeddingVector,
};
use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use crate::provider::StaticEmbeddingProvider;

pub struct LightTierProvider {
    inner: StaticEmbeddingProvider,
}

impl LightTierProvider {
    /// Load the light tier from embedded assets. A digest drift or a decode
    /// failure refuses with the named reason.
    pub fn load() -> Result<Self, String> {
        Ok(Self {
            inner: StaticEmbeddingProvider::from_embedded()?,
        })
    }
}

#[async_trait]
impl EmbeddingProvider for LightTierProvider {
    fn descriptor(&self) -> EmbeddingProviderDescriptor {
        EmbeddingProviderDescriptor {
            provider: "light".into(),
            model: self.inner.id().into(),
            revision: None,
            dimension: self.inner.dimension(),
            normalization: EmbeddingNormalization::Unit,
        }
    }

    async fn embed(
        &self,
        request: EmbeddingBatchRequest,
        cancellation: CancellationToken,
    ) -> Result<EmbeddingBatchResponse, EmbeddingProviderError> {
        if cancellation.is_cancelled() {
            return Err(EmbeddingProviderError::Cancelled);
        }
        let inputs: Vec<(Arc<str>, &str)> = request
            .inputs()
            .iter()
            .map(|input| (input.id().into(), input.text()))
            .collect();
        let sentences: Vec<&str> = inputs.iter().map(|(_, text)| *text).collect();
        let vectors = self.inner.encode_batch(&sentences);
        let mapped: Vec<EmbeddingVector> = inputs
            .iter()
            .zip(vectors)
            .map(|((id, _), values)| EmbeddingVector {
                id: Arc::clone(id),
                values,
            })
            .collect();
        let descriptor = self.descriptor();
        Ok(EmbeddingBatchResponse::new(descriptor, mapped))
    }
}

#[cfg(test)]
mod adapter_tests {
    use super::*;

    #[tokio::test]
    async fn the_adapter_serves_a_batch_through_the_core_trait() {
        let provider = LightTierProvider::load().expect("embedded assets load");
        let inputs = vec![
            aster_core::EmbeddingInput::new("a", "parse the config"),
            aster_core::EmbeddingInput::new("b", "hello"),
        ];
        let request = EmbeddingBatchRequest::new(inputs);
        let response = provider
            .embed(request, CancellationToken::new())
            .await
            .expect("light tier serves the batch");
        assert_eq!(response.vectors.len(), 2);
        assert_eq!(response.vectors[0].id.as_ref(), "a");
        assert_eq!(
            response.vectors[0].values.len(),
            provider.descriptor().dimension
        );
        assert_eq!(
            response.descriptor.normalization,
            EmbeddingNormalization::Unit
        );
    }
}
