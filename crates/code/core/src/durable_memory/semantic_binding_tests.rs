use super::*;
use crate::embedding::EmbeddingNormalization;

fn binding(policy: DurableMemorySemanticRecallPolicy) -> DurableMemorySemanticBindingV1 {
    DurableMemorySemanticBindingV1::new(
        format!("sha256:{}", "a".repeat(64)),
        EmbeddingProviderDescriptor::new("fixture", "semantic-binding", 2)
            .with_revision("fixture-r1")
            .with_normalization(EmbeddingNormalization::Unit),
        EmbeddingExecutorConfig::default(),
        VectorIndexDescriptor::new(2),
        policy,
    )
    .unwrap()
}

#[test]
fn legacy_policy_bytes_stay_unchanged_and_linear_round_trips() {
    let original = r#"{"candidateLimit":10,"minScore":0.0}"#;
    let policy: DurableMemorySemanticRecallPolicy = serde_json::from_str(original).unwrap();
    assert_eq!(
        policy,
        DurableMemorySemanticRecallPolicy::try_new(10, 0.0).unwrap()
    );
    assert_eq!(serde_json::to_string(&policy).unwrap(), original);
    let linear = policy.with_linear_fusion();
    assert_eq!(
        serde_json::to_string(&linear).unwrap(),
        r#"{"candidateLimit":10,"minScore":0.0,"linearFusion":true}"#
    );
    assert_eq!(
        serde_json::from_str::<DurableMemorySemanticRecallPolicy>(
            &serde_json::to_string(&linear).unwrap()
        )
        .unwrap(),
        linear
    );
    for invalid in [
        r#"{"candidateLimit":10,"minScore":0.0,"linearFusion":"true"}"#,
        r#"{"candidateLimit":10,"minScore":0.0,"linearFusion":null}"#,
        r#"{"candidateLimit":10,"minScore":0.0,"fusion":"linear"}"#,
    ] {
        assert!(serde_json::from_str::<DurableMemorySemanticRecallPolicy>(invalid).is_err());
    }
}

#[test]
fn profile_must_match_policy_and_changes_serving_generation() {
    let policy = DurableMemorySemanticRecallPolicy::try_new(10, 0.0).unwrap();
    let old = binding(policy);
    let linear = binding(policy.with_linear_fusion());
    assert_eq!(
        old.fusion_profile(),
        DURABLE_MEMORY_SEMANTIC_FUSION_PROFILE_V1
    );
    assert_eq!(
        linear.fusion_profile(),
        DURABLE_MEMORY_SEMANTIC_LINEAR_FUSION_PROFILE_V1
    );
    assert_ne!(
        old.serving_generation_digest().unwrap(),
        linear.serving_generation_digest().unwrap()
    );
    let encoded = serde_json::to_string(&old).unwrap();
    assert!(!encoded.contains("linearFusion"));
    let restored: DurableMemorySemanticBindingV1 = serde_json::from_str(&encoded).unwrap();
    restored.validate().unwrap();
    assert_eq!(
        restored.serving_generation_digest().unwrap(),
        old.serving_generation_digest().unwrap()
    );
    for (mut mismatched, profile) in [
        (
            old.clone(),
            DURABLE_MEMORY_SEMANTIC_LINEAR_FUSION_PROFILE_V1,
        ),
        (linear, DURABLE_MEMORY_SEMANTIC_FUSION_PROFILE_V1),
        (old, "unknown"),
    ] {
        mismatched.fusion_profile = profile.into();
        assert!(matches!(
            mismatched.validate(),
            Err(DurableMemorySemanticError::InvalidConfiguration {
                field: "binding.fusionProfile",
                ..
            })
        ));
    }
}
