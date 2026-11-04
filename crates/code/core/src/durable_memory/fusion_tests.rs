use super::*;
use aster_durable_memory::repository::{
    DurableMemoryKind, MemoryNamespace, MemoryNode, MemoryRevisionKind, MemoryStatus,
};
use std::collections::BTreeMap;

fn node(id: &str, revision: u64) -> MemoryNode {
    let time = chrono::DateTime::from_timestamp(1_777_000_000, 0).unwrap();
    MemoryNode {
        id: id.into(),
        namespace: MemoryNamespace::try_new("tenant", "principal", "scope").unwrap(),
        revision,
        kind: DurableMemoryKind::Procedural,
        status: MemoryStatus::Active,
        content: format!("content-{id}"),
        confidence: 0.5,
        importance: 0.5,
        evidence: Vec::new(),
        relations: Vec::new(),
        labels: BTreeMap::new(),
        created_at: time,
        updated_at: time,
        revision_kind: MemoryRevisionKind::Created,
        history: Vec::new(),
    }
}

fn lexical(id: &str, score: f32) -> RecallCandidate {
    RecallCandidate {
        node: node(id, 1),
        score,
        channel: DurableMemoryRecallChannel::Lexical,
        related_from: None,
    }
}

#[test]
fn linear_preserves_similarity_margin_and_later_verified_revision() {
    let lexical = vec![lexical("weather", 0.25)];
    let semantic = || {
        vec![
            SemanticRecallCandidate {
                node: node("settings", 1),
                score: 0.5,
            },
            SemanticRecallCandidate {
                node: node("weather", 2),
                score: 0.0,
            },
        ]
    };
    let old = fuse_lexical_semantic(lexical.clone(), semantic(), false);
    assert_eq!(old[0].node.id, "weather");
    let linear = fuse_lexical_semantic(lexical, semantic(), true);
    assert_eq!(linear[0].node.id, "settings");
    assert_eq!(linear[0].score, 1.0);
    assert_eq!(linear[1].score, 0.5);
    assert_eq!(linear[1].node.revision, 2);
    assert_eq!(linear[1].channel, DurableMemoryRecallChannel::Hybrid);
}

#[test]
fn zero_scores_and_single_channel_keep_finite_native_values() {
    let fused = fuse_lexical_semantic(
        vec![lexical("b", 0.0), lexical("b", 0.0)],
        vec![SemanticRecallCandidate {
            node: node("a", 1),
            score: 0.0,
        }],
        true,
    );
    assert_eq!(fused.len(), 2);
    assert!(fused.iter().all(|candidate| candidate.score == 0.0));
    assert_eq!(fused[0].node.id, "b");
    let lexical_only = fuse_lexical_semantic(vec![lexical("b", 0.25)], Vec::new(), true);
    assert_eq!(lexical_only[0].score, 0.25);
    let semantic_only = fuse_lexical_semantic(
        Vec::new(),
        vec![SemanticRecallCandidate {
            node: node("a", 1),
            score: 0.5,
        }],
        true,
    );
    assert_eq!(semantic_only[0].score, 0.5);
    assert!(fuse_lexical_semantic(Vec::new(), Vec::new(), true).is_empty());
}

#[test]
fn linear_repository_score_boundary_rejects_non_unit_totals() {
    use crate::durable_memory::context::validate_linear_lexical_scores;
    use aster_durable_memory::repository::MemoryRepositoryError;

    validate_linear_lexical_scores([0.0, 0.5, 1.0]).unwrap();
    validate_linear_lexical_scores([]).unwrap();
    for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -0.1, 1.1] {
        assert!(matches!(validate_linear_lexical_scores([0.5, invalid]),
            Err(MemoryRepositoryError::InvalidInput { field, .. }) if field == "recall.lexicalScore"));
    }
}
