#![cfg(test)]

use super::*;

#[tokio::test]
async fn owned_consent_ids_are_fresh_without_hiding_same_response_collisions_or_changing_legacy_ids(
) {
    let root = tempfile::tempdir().unwrap();
    let mut surface = tool_history_tests::surface(
        root.path(),
        Arc::new(tool_history_tests::ObservedClient::default()),
    );
    let (events, _) = tokio::sync::broadcast::channel(8);
    surface.confirmation = Some(Arc::new(crate::hitl::ConfirmationManager::new(
        crate::hitl::ConfirmationPolicy::enabled(),
        events,
    )));
    surface.run_id = Some("original-run".into());
    let calls = [
        ("provider-reused", "a.txt"),
        ("provider-reused", "b.txt"),
        ("provider-other", "c.txt"),
    ];
    let response = LlmResponse {
        message: Message {
            role: "assistant".into(),
            content: calls
                .iter()
                .map(|(id, path)| crate::llm::ContentBlock::ToolUse {
                    id: (*id).into(),
                    name: "read".into(),
                    input: serde_json::json!({"file_path":path}),
                })
                .collect(),
            reasoning_content: None,
            transcript_text: None,
            transcript_visibility: crate::llm::TranscriptVisibility::Wire,
        },
        usage: Default::default(),
        stop_reason: None,
        token_logprobs: vec![],
        meta: None,
    };
    let mut previous = std::collections::BTreeSet::new();
    for owned in [true, true, false] {
        surface.run_id = owned.then(|| "original-run".into());
        let ModelDecision::Tool { call, also } =
            decision_from_response(&response, &PermissionPolicy::new(), Some(&surface)).await
        else {
            panic!("all original structured calls must be retained")
        };
        let mapped: Vec<_> = std::iter::once(call).chain(also).collect();
        assert_eq!(mapped.len(), 3);
        assert_eq!(
            mapped[0].id, mapped[1].id,
            "a conflicting provider ID remains detectable in its original batch"
        );
        assert_ne!(mapped[0].id, mapped[2].id);
        for (mapped, (raw, path)) in mapped.iter().zip(calls) {
            assert_eq!(mapped.name, "read");
            assert_eq!(mapped.args, serde_json::json!({"file_path":path}));
            assert!(mapped.needs_confirmation);
            if owned {
                assert_ne!(mapped.id, raw);
                assert!(!previous.contains(&mapped.id));
            } else {
                assert_eq!(
                    mapped.id, raw,
                    "unbound legacy confirmations keep their existing API identity"
                );
            }
        }
        previous.extend(mapped.into_iter().map(|call| call.id));
    }
}
