use super::{
    WorkspaceChunk, WorkspaceHybridChannelRank, WorkspaceHybridSearchHit, WorkspaceRetrievalChannel,
};
use crate::rank_fusion::RankFusion;
use std::cmp::Ordering;
use std::sync::Arc;

pub(super) struct RankedCandidate {
    pub chunk: Arc<WorkspaceChunk>,
    pub channel: WorkspaceRetrievalChannel,
    pub rank: usize,
    pub exact_identifier: bool,
}

struct Accumulator {
    chunk: Arc<WorkspaceChunk>,
    exact_identifier: bool,
}

pub(super) fn fuse_candidates(candidates: Vec<RankedCandidate>) -> Vec<WorkspaceHybridSearchHit> {
    let mut fusion = RankFusion::<_, _, f64>::new();
    for candidate in candidates {
        fusion.add(
            candidate.chunk.id.as_str().to_owned(),
            candidate.channel,
            candidate.rank,
            Accumulator {
                chunk: candidate.chunk,
                exact_identifier: candidate.exact_identifier,
            },
            |current, incoming| current.exact_identifier |= incoming.exact_identifier,
        );
    }

    let mut fused = fusion
        .into_entries()
        .map(|(_, entry)| {
            let score = entry.score();
            let (candidate, ranks) = entry.into_parts();
            let mut channels = ranks
                .into_iter()
                .map(|(channel, rank)| WorkspaceHybridChannelRank {
                    channel,
                    rank: rank.get(),
                })
                .collect::<Vec<_>>();
            channels.sort_by_key(|rank| rank.channel);
            WorkspaceHybridSearchHit {
                chunk: candidate.chunk,
                fused_score: score,
                rerank_score: score,
                redundancy_score: 0.0,
                exact_identifier: candidate.exact_identifier,
                channels,
            }
        })
        .collect::<Vec<_>>();
    fused.sort_by(compare_fused);
    fused
}

pub(super) fn compare_fused(
    left: &WorkspaceHybridSearchHit,
    right: &WorkspaceHybridSearchHit,
) -> Ordering {
    right
        .exact_identifier
        .cmp(&left.exact_identifier)
        .then_with(|| right.fused_score.total_cmp(&left.fused_score))
        .then_with(|| left.chunk.path.cmp(&right.chunk.path))
        .then_with(|| left.chunk.start_byte.cmp(&right.chunk.start_byte))
        .then_with(|| left.chunk.id.cmp(&right.chunk.id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::retrieval::{ChunkCatalogLimits, ChunkingConfig, WorkspaceChunkCatalog};
    use crate::workspace::WorkspacePath;

    #[test]
    fn rrf_uses_channel_ranks_and_deduplicates_each_channel() {
        let chunks = chunks(&[("a.rs", "alpha\n"), ("b.rs", "beta\n")]);
        let hits = fuse_candidates(vec![
            candidate(&chunks[0], WorkspaceRetrievalChannel::Lexical, 1, false),
            candidate(&chunks[0], WorkspaceRetrievalChannel::Lexical, 2, false),
            candidate(&chunks[0], WorkspaceRetrievalChannel::Semantic, 2, false),
            candidate(&chunks[1], WorkspaceRetrievalChannel::Semantic, 1, false),
        ]);

        assert_eq!(hits[0].chunk.path.as_ref(), "a.rs");
        assert_eq!(hits[0].channels.len(), 2);
        assert_eq!(
            hits[0].channels[0].channel,
            WorkspaceRetrievalChannel::Lexical
        );
        assert_eq!(hits[0].channels[0].rank, 1);
    }

    #[test]
    fn exact_identifier_tier_cannot_be_displaced_by_semantic_only_hits() {
        let chunks = chunks(&[("exact.rs", "ExactType\n"), ("semantic.rs", "concept\n")]);
        let hits = fuse_candidates(vec![
            candidate(&chunks[0], WorkspaceRetrievalChannel::Exact, 25, true),
            candidate(&chunks[1], WorkspaceRetrievalChannel::Semantic, 1, false),
            candidate(&chunks[1], WorkspaceRetrievalChannel::Lexical, 1, false),
            candidate(&chunks[1], WorkspaceRetrievalChannel::Structural, 1, false),
        ]);

        assert_eq!(hits[0].chunk.path.as_ref(), "exact.rs");
        assert!(hits[0].exact_identifier);
    }

    #[test]
    fn fusion_is_deterministic_before_second_stage_diversity() {
        let catalog = WorkspaceChunkCatalog::new(
            ChunkingConfig {
                max_lines: 1,
                ..ChunkingConfig::default()
            },
            ChunkCatalogLimits::default(),
        )
        .unwrap();
        catalog
            .replace_file(
                &WorkspacePath::from_normalized("a.rs"),
                None,
                1,
                "one\ntwo\nthree\n",
            )
            .unwrap();
        catalog
            .replace_file(&WorkspacePath::from_normalized("b.rs"), None, 2, "other\n")
            .unwrap();
        let chunks = catalog.snapshot().unwrap().chunks().to_vec();
        let a_chunks = chunks
            .iter()
            .filter(|chunk| chunk.path.as_ref() == "a.rs")
            .collect::<Vec<_>>();
        let b_chunk = chunks
            .iter()
            .find(|chunk| chunk.path.as_ref() == "b.rs")
            .unwrap();
        let candidates = vec![
            candidate(a_chunks[0], WorkspaceRetrievalChannel::Semantic, 1, false),
            candidate(a_chunks[1], WorkspaceRetrievalChannel::Semantic, 2, false),
            candidate(a_chunks[2], WorkspaceRetrievalChannel::Semantic, 3, false),
            candidate(b_chunk, WorkspaceRetrievalChannel::Semantic, 4, false),
        ];
        let first = fuse_candidates(candidates);
        assert_eq!(
            first
                .iter()
                .filter(|hit| hit.chunk.path.as_ref() == "a.rs")
                .count(),
            3
        );
        assert_eq!(first[0].chunk.path.as_ref(), "a.rs");
        assert_eq!(first.last().unwrap().chunk.path.as_ref(), "b.rs");
    }

    fn candidate(
        chunk: &Arc<WorkspaceChunk>,
        channel: WorkspaceRetrievalChannel,
        rank: usize,
        exact_identifier: bool,
    ) -> RankedCandidate {
        RankedCandidate {
            chunk: Arc::clone(chunk),
            channel,
            rank,
            exact_identifier,
        }
    }

    fn chunks(files: &[(&str, &str)]) -> Vec<Arc<WorkspaceChunk>> {
        let catalog =
            WorkspaceChunkCatalog::new(ChunkingConfig::default(), ChunkCatalogLimits::default())
                .unwrap();
        for (revision, (path, content)) in files.iter().enumerate() {
            catalog
                .replace_file(
                    &WorkspacePath::from_normalized(*path),
                    None,
                    revision as u64 + 1,
                    content,
                )
                .unwrap();
        }
        catalog.snapshot().unwrap().chunks().to_vec()
    }
}
