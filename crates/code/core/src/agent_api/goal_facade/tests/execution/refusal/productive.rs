#![cfg(test)]

use super::*;

pub(super) fn text(value: &str) -> LlmResponse {
    LlmResponse {
        message: Message::assistant(value),
        usage: TokenUsage {
            prompt_tokens: 1,
            completion_tokens: 1,
            total_tokens: 2,
            ..Default::default()
        },
        stop_reason: None,
        token_logprobs: vec![],
        meta: None,
    }
}

async fn completes(after_refusal: bool) {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("source.txt"), "unchanged source").unwrap();
    let (agent, session, client) = fixture(
        root.path(),
        true,
        options("goal-run")
            .with_max_tool_rounds(0)
            .with_auto_compact(false)
            .with_assistant_final_response(true)
            .with_duplicate_tool_call_threshold(3)
            .with_permission_policy(
                crate::permissions::PermissionPolicy::new()
                    .allow("verify_source(*)")
                    .allow("read(*)"),
            ),
    )
    .await;
    let physical = Arc::new(AtomicUsize::new(0));
    session
        .register_dynamic_tool(Arc::new(RepeatedSource(physical.clone())))
        .unwrap();
    let mut responses: Vec<_> = (1..=if after_refusal { 4 } else { 3 })
        .map(repeat)
        .collect();
    if after_refusal {
        let mut read = repeat(5);
        read.message.content = vec![crate::llm::ContentBlock::ToolUse {
            id: "new-read".into(),
            name: "read".into(),
            input: serde_json::json!({"file_path":"source.txt"}),
        }];
        responses.extend([read, repeat(6)]);
    }
    responses.push(text("The required source observations are recorded."));
    responses.push(text(r#"{"achieved":false,"progress":0.0,"reason":"More goal work remains","remaining_criteria":["All original checks pass"]}"#));
    client.scripted.lock().unwrap().extend(responses);
    let request = if after_refusal {
        "Inspect the source, obtain another original observation after a refusal, and report the outcome."
    } else {
        "Read the original source exactly three times, then report the result."
    };
    let result = tokio::time::timeout(Duration::from_secs(5), session.send(request, None)).await;
    let executions = physical.load(Ordering::SeqCst);
    let facts = settlement::facts(root.path());
    session.close().await;
    agent.close().await;
    assert!(
        result.unwrap().is_ok(),
        "a bounded required check or a changed approach must still finish"
    );
    assert_eq!(executions, 3);
    let denied = facts
        .iter()
        .filter(|fact| fact.kind == "tool.result" && fact.payload["receipt"]["outcome"] == "denied")
        .count();
    assert_eq!(denied, if after_refusal { 2 } else { 0 });
    assert!(!facts.iter().any(|fact| fact.kind == "a3s.goal.completed"));
}

#[tokio::test]
async fn three_identical_required_checks_still_finish_without_a_refusal_cycle() {
    completes(false).await;
}

#[tokio::test]
async fn new_original_information_between_refusals_keeps_the_run_eligible() {
    completes(true).await;
}
