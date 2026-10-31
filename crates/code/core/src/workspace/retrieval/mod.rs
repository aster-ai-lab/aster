//! Session-local workspace text catalog and incremental lexical index.
//!
//! The catalog is intentionally independent from semantic embedding and model
//! providers. It is the single source of chunk boundaries for lexical and
//! future semantic retrieval.

#[cfg(feature = "aster-vec-fts")]
mod aster_vec;
mod catalog;
mod chunk;
mod chunking_strategy;
mod collection;
mod eligibility;
mod hybrid_candidates;
mod hybrid_query;
mod hybrid_rank;
mod hybrid_types;
pub(crate) mod lexical;
mod memory_vector_adapter;
mod persistent;
mod reconcile;
mod rerank;
mod runtime;
mod semantic_batch;
mod semantic_projection;
mod semantic_query;
mod semantic_runtime;
mod semantic_status;
mod semantic_types;
mod types;
mod vector_contract;
mod verification;
#[cfg(feature = "aster-vec-fts")]
mod wiki;
mod wiki_sources;

pub use catalog::{ChunkCatalogSnapshot, WorkspaceCatalogDocument, WorkspaceChunkCatalog};
pub(crate) use chunk::digest_content;
pub use chunking_strategy::{
    CustomWorkspaceChunkingStrategy, FixedWindowChunkingOptions, RecursiveChunkingOptions,
    WorkspaceChunkRange, WorkspaceChunkingError, WorkspaceChunkingInput, WorkspaceChunkingStrategy,
};
pub use collection::{
    WorkspaceCollectionError, WorkspaceCollectionHit, WorkspaceCollectionMatch,
    WorkspaceCollectionOrder, WorkspaceCollectionPage, WorkspaceCollectionQuery,
    WorkspaceCollectionRequest, WorkspaceCollectionSort, WorkspaceCollectionTextRange,
};
pub use eligibility::WorkspaceEligibilityPolicy;
pub(crate) use hybrid_candidates::ValidatedHybridRequest;
pub(crate) use hybrid_query::hybrid_search;
pub use hybrid_types::{
    WorkspaceHybridChannelRank, WorkspaceHybridChannelStatus, WorkspaceHybridFallbackReason,
    WorkspaceHybridSearchHit, WorkspaceHybridSearchRequest, WorkspaceHybridSearchResult,
    WorkspaceRerankAlgorithm, WorkspaceRerankFallbackReason, WorkspaceRerankMode,
    WorkspaceRerankOptions, WorkspaceRerankStatus, WorkspaceRetrievalChannel,
};
pub use lexical::{LexicalSearchHit, LexicalSearchRequest, LexicalSearchResult};
pub use persistent::{
    WorkspacePersistentIndex, WorkspacePersistentIndexPhase, WorkspacePersistentIndexStatus,
};
pub(crate) use runtime::LocalWorkspaceCatalogRuntime;
pub use semantic_runtime::WorkspaceRetrievalRuntime;
pub use semantic_types::{
    WorkspaceEmbeddingBatchMetrics, WorkspaceRetrievalError, WorkspaceRetrievalOptions,
    WorkspaceRetrievalPhase, WorkspaceRetrievalResult, WorkspaceRetrievalStatus,
    WorkspaceSemanticFallbackReason, WorkspaceSemanticIndexLimits, WorkspaceSemanticSearchHit,
    WorkspaceSemanticSearchRequest, WorkspaceSemanticSearchResult,
};
pub use types::{
    ChunkCatalogLimits, ChunkingConfig, WorkspaceChunk, WorkspaceChunkId, WorkspaceIndexError,
    WorkspaceLexicalEngine,
};
pub(crate) use verification::retain_verified;
#[cfg(feature = "aster-vec-fts")]
pub use wiki::{
    WikiIndex, WikiReconcileReport, WikiSearchHit, WikiSearchPhase, WikiSearchResult,
    WIKI_PASSAGE_CHARS,
};
pub use wiki_sources::{
    capture_workspace_wiki_sources, WorkspaceWikiSourceCoverage, WorkspaceWikiSourceFile,
    WorkspaceWikiSourceFormat, WorkspaceWikiSourceLimits, WorkspaceWikiSourceSkip,
    WorkspaceWikiSourceSkipReason, WorkspaceWikiSources,
};

#[cfg(test)]
mod tests;
