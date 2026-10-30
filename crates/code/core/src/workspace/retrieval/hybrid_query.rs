//! Deterministic orchestration for exact, lexical, structural, and semantic retrieval.

use super::hybrid_candidates::{
    exact_candidates, structural_candidates, StructuralCandidates, ValidatedHybridRequest,
    CHANNEL_CANDIDATE_LIMIT,
};
use super::hybrid_rank::{fuse_candidates, RankedCandidate};
use super::rerank::{rerank_fused_candidates, rerank_status_not_run};
use super::{
    retain_verified, ChunkCatalogSnapshot, WorkspaceChunkCatalog, WorkspaceHybridChannelStatus,
    WorkspaceHybridFallbackReason, WorkspaceHybridSearchResult, WorkspaceRerankStatus,
    WorkspaceRetrievalChannel, WorkspaceRetrievalError, WorkspaceRetrievalRuntime,
    WorkspaceRetrievalStatus, WorkspaceSemanticFallbackReason, WorkspaceSemanticSearchResult,
};
use crate::code_intelligence::WorkspaceCodeIntelligence;
use crate::workspace::WorkspaceFileSystem;
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

const MAX_RESULTS: usize = 25;
const VERIFICATION_OVERFETCH: usize = 4;

#[cfg(test)]
impl WorkspaceRetrievalRuntime {
    /// Fuse bounded retrieval channels, then reread every selected source file
    /// once before returning any chunk text.
    pub(crate) async fn hybrid_search(
        &self,
        request: super::WorkspaceHybridSearchRequest,
        file_system: Arc<dyn WorkspaceFileSystem>,
        code_intelligence: Option<Arc<dyn WorkspaceCodeIntelligence>>,
        operation_timeout: Option<Duration>,
        cancellation: CancellationToken,
    ) -> Result<WorkspaceHybridSearchResult, WorkspaceRetrievalError> {
        hybrid_search(
            Arc::clone(&self.catalog),
            Some(self),
            None,
            ValidatedHybridRequest::new(request)?,
            file_system,
            code_intelligence,
            operation_timeout,
            cancellation,
        )
        .await
        .map(|(result, _)| result)
    }
}

pub(crate) async fn hybrid_search(
    catalog: Arc<WorkspaceChunkCatalog>,
    runtime: Option<&WorkspaceRetrievalRuntime>,
    persistent: Option<Arc<super::WorkspacePersistentIndex>>,
    request: ValidatedHybridRequest,
    file_system: Arc<dyn WorkspaceFileSystem>,
    code_intelligence: Option<Arc<dyn WorkspaceCodeIntelligence>>,
    operation_timeout: Option<Duration>,
    cancellation: CancellationToken,
) -> Result<
    (
        WorkspaceHybridSearchResult,
        Option<super::WorkspaceLexicalEngine>,
    ),
    WorkspaceRetrievalError,
> {
    let lifetime = runtime
        .map(WorkspaceRetrievalRuntime::child_lifetime)
        .unwrap_or_else(|| cancellation.child_token());
    ensure_active(&lifetime, &cancellation)?;
    let mut rerank_options = runtime
        .map(|runtime| runtime.rerank_options.clone())
        .unwrap_or_default();
    rerank_options.query = Some(request.query_text().to_owned());
    let mut updates = catalog.subscribe();
    if updates.borrow().source_revision() == 0 {
        // Lazy catalog admission must get a bounded first-query opportunity.
        let admitted = async {
            while updates.borrow().source_revision() == 0 {
                if updates.changed().await.is_err() {
                    break;
                }
            }
        };
        tokio::select! {
            _ = cancellation.cancelled() => return Err(WorkspaceRetrievalError::Cancelled),
            _ = lifetime.cancelled() => return Err(WorkspaceRetrievalError::Cancelled),
            _ = tokio::time::timeout(
                operation_timeout.unwrap_or(Duration::from_secs(1)).min(Duration::from_secs(1)),
                admitted,
            ) => {}
        }
    }
    let snapshot = catalog.snapshot()?;

    let lexical_request = request.lexical_request();
    let native = if let Some(index) = persistent {
        let request = lexical_request.clone();
        let snapshot = snapshot.clone();
        let queried =
            tokio::task::spawn_blocking(move || index.search_current_snapshot(&request, &snapshot));
        tokio::select! {
            _ = cancellation.cancelled() => return Err(WorkspaceRetrievalError::Cancelled),
            _ = lifetime.cancelled() => return Err(WorkspaceRetrievalError::Cancelled),
            result = tokio::time::timeout(operation_timeout.unwrap_or(Duration::from_secs(1)).min(Duration::from_secs(1)), queried) =>
                match result {
                    Ok(Ok(Ok(result))) => result,
                    _ => None,
                },
        }
    } else {
        None
    };
    let lexical_engine = native.as_ref().map(|result| result.lexical_engine);
    let lexical = match native.map_or_else(|| snapshot.lexical_search(&lexical_request), Ok) {
        Ok(result) => Some(result),
        Err(super::WorkspaceIndexError::InvalidQuery(_)) => None,
        Err(error) => return Err(error.into()),
    };

    let semantic_request = request.semantic_request();
    let query_lifetime = lifetime.child_token();
    let exact_call = exact_candidates(&snapshot, &request, &cancellation);
    let semantic_call = async {
        match runtime {
            Some(runtime) => {
                runtime
                    .search_candidates(semantic_request, query_lifetime.child_token())
                    .await
            }
            None => Ok(WorkspaceSemanticSearchResult {
                hits: Vec::new(),
                status: WorkspaceRetrievalStatus::disabled(),
                searched_records: 0,
                truncated: false,
                fallback: None,
            }),
        }
    };
    let structural_call = structural_candidates(
        code_intelligence,
        &snapshot,
        &request,
        query_lifetime.child_token(),
    );
    let operations = async { tokio::join!(exact_call, semantic_call, structural_call) };
    tokio::pin!(operations);
    let (exact, semantic, structural) = tokio::select! {
        biased;
        _ = cancellation.cancelled() => {
            query_lifetime.cancel();
            return Err(WorkspaceRetrievalError::Cancelled);
        }
        _ = query_lifetime.cancelled() => {
            return Err(WorkspaceRetrievalError::Cancelled);
        }
        result = &mut operations => result,
    };
    let (exact, exact_truncated) = exact?;
    let exact_count = exact.len();
    let semantic = semantic?;
    let structural = structural?;

    if !catalog_revision_matches(&catalog.snapshot()?, &snapshot) {
        return Ok((
            revision_changed_result(
                semantic.status,
                &snapshot,
                channel_statuses(
                    exact_count,
                    exact_truncated,
                    lexical.as_ref(),
                    &structural,
                    0,
                    false,
                    Some(WorkspaceHybridFallbackReason::RevisionChanged),
                ),
                rerank_status_not_run(rerank_options.mode),
            ),
            lexical_engine,
        ));
    }

    let mut semantic_fallback = semantic.fallback.map(map_semantic_fallback).or_else(|| {
        runtime
            .is_none()
            .then_some(WorkspaceHybridFallbackReason::Unavailable)
    });
    let semantic_compatible = semantic.hits.is_empty()
        || (semantic.status.catalog_revision == snapshot.revision()
            && semantic.status.source_revision == snapshot.source_revision());
    let semantic_hits = if semantic_compatible {
        semantic
            .hits
            .into_iter()
            .filter(|hit| hit.score.is_finite() && hit.score > 0.0)
            .collect()
    } else {
        semantic_fallback = Some(WorkspaceHybridFallbackReason::RevisionChanged);
        Vec::new()
    };
    let semantic_count = semantic_hits.len();
    let semantic_truncated = semantic.truncated;

    let mut candidates = Vec::with_capacity(
        exact.len()
            + lexical.as_ref().map_or(0, |result| result.hits.len())
            + structural.candidates.len()
            + semantic_count,
    );
    candidates.extend(exact);
    if let Some(lexical) = lexical.as_ref() {
        candidates.extend(
            lexical
                .hits
                .iter()
                .enumerate()
                .map(|(rank, hit)| RankedCandidate {
                    chunk: Arc::clone(&hit.chunk),
                    channel: WorkspaceRetrievalChannel::Lexical,
                    rank: rank + 1,
                    exact_identifier: false,
                }),
        );
    }
    candidates.extend(
        structural
            .candidates
            .iter()
            .map(|candidate| RankedCandidate {
                chunk: Arc::clone(&candidate.chunk),
                channel: WorkspaceRetrievalChannel::Structural,
                rank: candidate.rank,
                exact_identifier: candidate.exact_identifier,
            }),
    );
    candidates.extend(
        semantic_hits
            .into_iter()
            .enumerate()
            .map(|(rank, hit)| RankedCandidate {
                chunk: hit.chunk,
                channel: WorkspaceRetrievalChannel::Semantic,
                rank: rank + 1,
                exact_identifier: false,
            }),
    );

    let verification_limit = request
        .limit
        .saturating_mul(VERIFICATION_OVERFETCH)
        .min(MAX_RESULTS.saturating_mul(VERIFICATION_OVERFETCH));
    let fused = fuse_candidates(candidates);
    let reranked = rerank_fused_candidates(fused, verification_limit, rerank_options);
    let rerank_status = reranked.status;
    let (hits, verification_filtered, verification_truncated) = retain_verified(
        reranked.hits,
        request.limit,
        |hit| hit.chunk.as_ref(),
        file_system.as_ref(),
        operation_timeout,
        &lifetime,
        &cancellation,
    )
    .await?;

    if !catalog_revision_matches(&catalog.snapshot()?, &snapshot) {
        return Ok((
            revision_changed_result(
                runtime
                    .map(WorkspaceRetrievalRuntime::status)
                    .unwrap_or_else(WorkspaceRetrievalStatus::disabled),
                &snapshot,
                channel_statuses(
                    exact_count,
                    exact_truncated,
                    lexical.as_ref(),
                    &structural,
                    semantic_count,
                    semantic_truncated,
                    semantic_fallback,
                ),
                rerank_status,
            ),
            lexical_engine,
        ));
    }

    let mut channels = channel_statuses(
        exact_count,
        exact_truncated,
        lexical.as_ref(),
        &structural,
        semantic_count,
        semantic_truncated,
        semantic_fallback,
    );
    let channel_truncated = channels.iter().any(|channel| channel.truncated);
    let semantic_fallback = semantic_fallback
        .filter(|fallback| !matches!(fallback, WorkspaceHybridFallbackReason::FilteredStaleHits));
    let fallback = if snapshot.source_revision() == 0 {
        for channel in &mut channels {
            if matches!(
                channel.channel,
                WorkspaceRetrievalChannel::Exact | WorkspaceRetrievalChannel::Lexical
            ) {
                channel.fallback = Some(WorkspaceHybridFallbackReason::Building);
            }
        }
        Some(WorkspaceHybridFallbackReason::Building)
    } else if verification_filtered {
        Some(WorkspaceHybridFallbackReason::FilteredStaleHits)
    } else {
        semantic_fallback.or_else(|| structural.global_fallback())
    };
    Ok((
        WorkspaceHybridSearchResult {
            hits,
            semantic_status: semantic.status,
            catalog_revision: snapshot.revision(),
            source_revision: snapshot.source_revision(),
            channels,
            truncated: channel_truncated
                || verification_truncated
                || rerank_status.candidate_truncated,
            rerank: rerank_status,
            fallback,
        },
        lexical_engine,
    ))
}

fn channel_statuses(
    exact_count: usize,
    exact_truncated: bool,
    lexical: Option<&super::LexicalSearchResult>,
    structural: &StructuralCandidates,
    semantic_count: usize,
    semantic_truncated: bool,
    semantic_fallback: Option<WorkspaceHybridFallbackReason>,
) -> Vec<WorkspaceHybridChannelStatus> {
    vec![
        WorkspaceHybridChannelStatus {
            channel: WorkspaceRetrievalChannel::Exact,
            candidate_count: exact_count,
            truncated: exact_truncated,
            fallback: None,
        },
        WorkspaceHybridChannelStatus {
            channel: WorkspaceRetrievalChannel::Lexical,
            candidate_count: lexical.map_or(0, |result| result.hits.len()),
            truncated: lexical.is_some_and(|result| {
                result.candidate_truncated || result.hits.len() == CHANNEL_CANDIDATE_LIMIT
            }),
            fallback: None,
        },
        WorkspaceHybridChannelStatus {
            channel: WorkspaceRetrievalChannel::Structural,
            candidate_count: structural.candidates.len(),
            truncated: structural.truncated,
            fallback: structural.fallback,
        },
        WorkspaceHybridChannelStatus {
            channel: WorkspaceRetrievalChannel::Semantic,
            candidate_count: semantic_count,
            truncated: semantic_truncated,
            fallback: semantic_fallback,
        },
    ]
}

fn map_semantic_fallback(
    fallback: WorkspaceSemanticFallbackReason,
) -> WorkspaceHybridFallbackReason {
    match fallback {
        WorkspaceSemanticFallbackReason::Building => WorkspaceHybridFallbackReason::Building,
        WorkspaceSemanticFallbackReason::Degraded => WorkspaceHybridFallbackReason::Degraded,
        WorkspaceSemanticFallbackReason::Closed => WorkspaceHybridFallbackReason::Unavailable,
        WorkspaceSemanticFallbackReason::QueryEmbeddingFailed => {
            WorkspaceHybridFallbackReason::QueryEmbeddingFailed
        }
        WorkspaceSemanticFallbackReason::VectorSearchFailed => {
            WorkspaceHybridFallbackReason::VectorSearchFailed
        }
        WorkspaceSemanticFallbackReason::RevisionChanged => {
            WorkspaceHybridFallbackReason::RevisionChanged
        }
        WorkspaceSemanticFallbackReason::FilteredStaleHits => {
            WorkspaceHybridFallbackReason::FilteredStaleHits
        }
    }
}

fn revision_changed_result(
    semantic_status: WorkspaceRetrievalStatus,
    snapshot: &ChunkCatalogSnapshot,
    channels: Vec<WorkspaceHybridChannelStatus>,
    rerank: WorkspaceRerankStatus,
) -> WorkspaceHybridSearchResult {
    WorkspaceHybridSearchResult {
        hits: Vec::new(),
        semantic_status,
        catalog_revision: snapshot.revision(),
        source_revision: snapshot.source_revision(),
        channels,
        rerank,
        truncated: false,
        fallback: Some(WorkspaceHybridFallbackReason::RevisionChanged),
    }
}

fn catalog_revision_matches(
    current: &ChunkCatalogSnapshot,
    expected: &ChunkCatalogSnapshot,
) -> bool {
    current.revision() == expected.revision()
        && current.source_revision() == expected.source_revision()
}

fn ensure_active(
    runtime: &CancellationToken,
    caller: &CancellationToken,
) -> Result<(), WorkspaceRetrievalError> {
    if runtime.is_cancelled() || caller.is_cancelled() {
        return Err(WorkspaceRetrievalError::Cancelled);
    }
    Ok(())
}
