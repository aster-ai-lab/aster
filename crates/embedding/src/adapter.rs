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
use sha2::{Digest, Sha256};
use tokio_util::sync::CancellationToken;

use crate::assets;
use crate::provider::StaticEmbeddingProvider;

pub struct LightTierProvider {
    inner: StaticEmbeddingProvider,
    revision: String,
}

impl std::fmt::Debug for LightTierProvider {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LightTierProvider")
            .field("descriptor", &self.descriptor())
            .finish_non_exhaustive()
    }
}

impl LightTierProvider {
    /// Load the light tier from embedded assets. A digest drift or a decode
    /// failure refuses with the named reason.
    pub fn load() -> Result<Self, String> {
        let inner = StaticEmbeddingProvider::from_embedded()?;
        let mut revision = Sha256::new();
        revision.update(b"aster.embedding.light-provider.v1\0");
        for digest in [
            assets::MODEL_SHA256,
            assets::TOKENIZER_SHA256,
            assets::CONFIG_SHA256,
        ] {
            revision.update(digest.as_bytes());
            revision.update(b"\0");
        }
        Ok(Self {
            inner,
            revision: format!("sha256:{:x}", revision.finalize()),
        })
    }
}

#[async_trait]
impl EmbeddingProvider for LightTierProvider {
    fn descriptor(&self) -> EmbeddingProviderDescriptor {
        EmbeddingProviderDescriptor {
            provider: "light".into(),
            model: self.inner.id().into(),
            revision: Some(self.revision.clone()),
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
#[path = "adapter_tests.rs"]
mod adapter_tests;
