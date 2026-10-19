//! Local BGE engines for A3S Use: embeddings and reranking.
//!
//! [`EmbeddingClient`] and [`RerankClient`] own bounded request validation
//! while inference runs on ONNX Runtime against locally installed weights.
//! The installer is manifest-driven: every file arrives with an explicit
//! URL, byte length, and sha256 provided by the platform model supply,
//! so swapping weights is an install action, never a code change.

pub use a3s_use_core::{Artifact, Readiness, UseError, UseResult};

mod assets;
mod install;
mod math;
mod wire;

pub use assets::{bge_status, BgeInstallSource, BgeRuntimeStatus, Family};
pub use install::{install_family, uninstall_family, InstallManifest, ManifestFile};
pub use wire::{
    EmbeddingItem, EmbeddingRequest, EmbeddingResponse, RerankRequest, RerankResponse,
    RerankResult, Usage,
};

#[cfg(any(feature = "bge-m3", feature = "bge-reranker-v2-m3"))]
mod tokenizer;

#[cfg(feature = "bge-m3")]
mod embedding;

#[cfg(feature = "bge-reranker-v2-m3")]
mod rerank;

#[cfg(feature = "bge-m3")]
pub use embedding::{EmbeddingClient, EmbeddingOperation, EmbeddingOutput};

#[cfg(feature = "bge-reranker-v2-m3")]
pub use rerank::RerankClient;

#[cfg(feature = "cli")]
pub mod cli;

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

pub(crate) fn bge_error(code: &str, message: impl Into<String>) -> UseError {
    UseError::new(code, message)
}
