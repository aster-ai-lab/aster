#![cfg(test)]

use super::*;
use crate::tools::{Tool, ToolCapabilities, ToolContext, ToolOutput};

struct ActiveSource(Arc<AtomicUsize>);

#[async_trait::async_trait]
impl Tool for ActiveSource {
    fn name(&self) -> &str {
        "verify_source"
    }
    fn description(&self) -> &str {
        "Return another bounded source observation."
    }
    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({"type":"object","properties":{"iteration":{"type":"integer","minimum":0}},"required":["iteration"],"additionalProperties":false})
    }
    fn capabilities(&self, _: &serde_json::Value) -> ToolCapabilities {
        ToolCapabilities::read_only_paginated(64)
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        _: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        tokio::time::sleep(Duration::from_millis(100)).await;
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(ToolOutput::success(format!(
            "Source observation {}",
            args["iteration"]
        )))
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_absolute_deadline_stops_continuously_active_model_and_tool_rounds() {
    let root = tempfile::tempdir().unwrap();
    let deadline = now_ms() + 3_000;
    let (agent, session, client) = fixture_with(
        root.path(),
        true,
        options("goal-run")
            .with_max_tool_rounds(0)
            .with_auto_compact(false)
            .with_permission_policy(
                crate::permissions::PermissionPolicy::new().allow("verify_source(*)"),
            ),
        Some(policy(true, 8, 4, deadline)),
    )
    .await;
    let completed = Arc::new(AtomicUsize::new(0));
    session
        .register_dynamic_tool(Arc::new(ActiveSource(completed.clone())))
        .unwrap();
    let weak = Arc::downgrade(&client);
    *client.before_response.lock().unwrap() = Some(Box::new(move |iteration| {
        weak.upgrade()
            .unwrap()
            .scripted
            .lock()
            .unwrap()
            .push_back(LlmResponse {
                message: Message {
                    role: "assistant".into(),
                    content: vec![crate::llm::ContentBlock::ToolUse {
                        id: format!("active-{iteration}"),
                        name: "verify_source".into(),
                        input: serde_json::json!({"iteration":iteration}),
                    }],
                    reasoning_content: None,
                    transcript_text: None,
                    transcript_visibility: crate::llm::TranscriptVisibility::Wire,
                },
                usage: TokenUsage {
                    prompt_tokens: 1,
                    completion_tokens: 1,
                    total_tokens: 2,
                    ..Default::default()
                },
                stop_reason: None,
                token_logprobs: vec![],
                meta: None,
            });
    }));
    let GoalTickAdmission::Admitted(AgentRunSpawn::Started { mut worker, .. }) = session
        .tick_goal(&request(&session, "wake-active"))
        .await
        .unwrap()
    else {
        panic!("the original automatic Run must start")
    };
    let stopped = tokio::time::timeout(
        Duration::from_millis(deadline.saturating_sub(now_ms()) + 1_500),
        &mut worker,
    )
    .await;
    let events = session.run_events("wake-active").await;
    let count = completed.load(Ordering::SeqCst);
    let calls = client.calls.load(Ordering::SeqCst);
    tokio::time::sleep(Duration::from_millis(200)).await;
    let calls_after_join = client.calls.load(Ordering::SeqCst);
    session.close().await;
    agent.close().await;
    if stopped.is_err() {
        tokio::time::timeout(Duration::from_secs(5), worker)
            .await
            .unwrap()
            .unwrap();
    }
    assert!(stopped.as_ref().is_ok_and(|joined| joined.is_ok()));
    assert!(
        count >= 5,
        "multiple bounded tool bodies must finish before expiry; observed {count}"
    );
    assert!(calls >= count);
    assert_eq!(
        calls, calls_after_join,
        "no original generation may survive retirement"
    );
    assert!(events.iter().any(|record| matches!(&record.event, crate::agent::AgentEvent::Error { message } if message.starts_with("goal.deadline_expired:"))));
    assert!(!events
        .iter()
        .any(|record| matches!(record.event, crate::agent::AgentEvent::End { .. })));
    let facts = settlement::facts(root.path());
    assert_eq!(
        facts
            .iter()
            .filter(|fact| fact.kind == "a3s.goal.slice.settled")
            .count(),
        1
    );
    assert!(!facts.iter().any(|fact| matches!(
        fact.kind.as_str(),
        "a3s.goal.step.accepted" | "a3s.goal.completed"
    )));
}
