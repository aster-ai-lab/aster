use super::context::RecallCandidate;
use super::semantic::SemanticRecallCandidate;
use super::DurableMemoryRecallChannel;
use crate::rank_fusion::RankFusion;

pub(super) fn fuse_lexical_semantic(
    lexical: Vec<RecallCandidate>,
    semantic: Vec<SemanticRecallCandidate>,
    linear: bool,
) -> Vec<RecallCandidate> {
    if semantic.is_empty() {
        return lexical;
    }
    if lexical.is_empty() {
        return semantic
            .into_iter()
            .map(|candidate| RecallCandidate {
                node: candidate.node,
                score: candidate.score,
                channel: DurableMemoryRecallChannel::Semantic,
                related_from: None,
            })
            .collect();
    }

    let mut fusion = RankFusion::<_, _, f32>::new();
    for (index, candidate) in lexical.into_iter().enumerate() {
        fusion.add_with_score(
            candidate.node.id.clone(),
            DurableMemoryRecallChannel::Lexical,
            index + 1,
            linear.then_some(candidate.score),
            candidate,
            |_, _| {},
        );
    }
    for (index, semantic) in semantic.into_iter().enumerate() {
        fusion.add_with_score(
            semantic.node.id.clone(),
            DurableMemoryRecallChannel::Semantic,
            index + 1,
            linear.then_some(semantic.score),
            RecallCandidate {
                node: semantic.node,
                score: semantic.score,
                channel: DurableMemoryRecallChannel::Semantic,
                related_from: None,
            },
            |current, later| {
                // Semantic candidates were re-read from the repository after
                // lexical search, so prefer that later verified revision.
                current.node = later.node;
            },
        );
    }
    let maximum = fusion
        .values()
        .map(|entry| entry.score())
        .max_by(f32::total_cmp)
        .unwrap_or(1.0);
    let mut fused = fusion
        .into_entries()
        .map(|(_, entry)| {
            let score = entry.score();
            let lexical = entry.rank(DurableMemoryRecallChannel::Lexical).is_some();
            let semantic = entry.rank(DurableMemoryRecallChannel::Semantic).is_some();
            let (mut candidate, _) = entry.into_parts();
            candidate.score = if maximum > 0.0 {
                (score / maximum).clamp(0.0, 1.0)
            } else {
                0.0
            };
            candidate.channel = if lexical && semantic {
                DurableMemoryRecallChannel::Hybrid
            } else if lexical {
                DurableMemoryRecallChannel::Lexical
            } else {
                DurableMemoryRecallChannel::Semantic
            };
            candidate
        })
        .collect::<Vec<_>>();
    fused.sort_by(|left, right| {
        right
            .score
            .total_cmp(&left.score)
            .then_with(|| {
                super::context::channel_rank(left.channel)
                    .cmp(&super::context::channel_rank(right.channel))
            })
            .then_with(|| right.node.updated_at.cmp(&left.node.updated_at))
            .then_with(|| left.node.id.cmp(&right.node.id))
    });
    fused
}

#[cfg(test)]
#[path = "fusion_tests.rs"]
mod tests;
