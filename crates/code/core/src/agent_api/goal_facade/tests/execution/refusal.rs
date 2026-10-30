#![cfg(test)]

use super::automatic::{now_ms, policy, request};
use super::*;
use crate::agent_api::{AgentRunSpawn, GoalTickAdmission};
use crate::tools::{Tool, ToolCapabilities, ToolContext, ToolOutput};
use std::time::Duration;

mod child;
mod mutation;
mod productive;

struct RepeatedSource(Arc<AtomicUsize>);

#[async_trait::async_trait]
impl Tool for RepeatedSource {
    fn name(&self) -> &str {
        "verify_source"
    }
    fn description(&self) -> &str {
        "Read the unchanged original source."
    }
    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({"type":"object","properties":{},"additionalProperties":false})
    }
    fn capabilities(&self, _: &serde_json::Value) -> ToolCapabilities {
        ToolCapabilities::read_only_paginated(1)
    }
    async fn execute(
        &self,
        _: &serde_json::Value,
        ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(ToolOutput::success(std::fs::read_to_string(
            ctx.workspace.join("source.txt"),
        )?))
    }
}

fn repeat(iteration: usize) -> LlmResponse {
    LlmResponse {
        message: Message {
            role: "assistant".into(),
            content: vec![crate::llm::ContentBlock::ToolUse {
                id: format!("repeat-{iteration}"),
                name: "verify_source".into(),
                input: serde_json::json!({}),
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
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_single_run_cannot_keep_billing_after_its_identical_tool_calls_are_refused() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("source.txt"), "unchanged source").unwrap();
    let deadline = now_ms() + 60_000;
    let (agent, session, client) = fixture_with(
        root.path(),
        true,
        options("goal-run")
            .with_max_tool_rounds(0)
            .with_auto_compact(false)
            .with_assistant_final_response(true)
            .with_duplicate_tool_call_threshold(3)
            .with_permission_policy(
                crate::permissions::PermissionPolicy::new().allow("verify_source(*)"),
            ),
        Some(policy(true, 8, 4, deadline)),
    )
    .await;
    let physical = Arc::new(AtomicUsize::new(0));
    session
        .register_dynamic_tool(Arc::new(RepeatedSource(physical.clone())))
        .unwrap();
    let weak = Arc::downgrade(&client);
    *client.before_response.lock().unwrap() = Some(Box::new(move |iteration| {
        weak.upgrade()
            .unwrap()
            .scripted
            .lock()
            .unwrap()
            .push_back(repeat(iteration));
    }));
    let GoalTickAdmission::Admitted(AgentRunSpawn::Started { mut worker, .. }) = session
        .tick_goal(&request(&session, "wake-repeated"))
        .await
        .unwrap()
    else {
        panic!("the original automatic Run must start")
    };
    let stopped = tokio::time::timeout(Duration::from_secs(1), &mut worker).await;
    let calls = client.calls.load(Ordering::SeqCst);
    let executions = physical.load(Ordering::SeqCst);
    let before_deadline = now_ms() < deadline;
    let events = session.run_events("wake-repeated").await;
    eprintln!(
        "REPEATED_REFUSAL_OBSERVATION model_calls={calls} physical_calls={executions} stopped={} before_deadline={before_deadline}",
        stopped.is_ok()
    );
    *client.before_response.lock().unwrap() = None;
    if stopped.is_err() {
        session.cancel().await;
        tokio::time::timeout(Duration::from_secs(5), worker)
            .await
            .unwrap()
            .unwrap();
    }
    session.close().await;
    agent.close().await;
    assert!(
        before_deadline,
        "the one-minute source deadline must not be the stopping mechanism"
    );
    assert!(
        stopped.as_ref().is_ok_and(|joined| joined.is_ok()),
        "the same original Run kept generating after repeated typed denials"
    );
    assert_eq!(
        executions, 3,
        "the existing duplicate guard already refuses physical execution"
    );
    assert!(
        calls <= 6,
        "duplicate refusals must also stop further billable generation: {calls}"
    );
    assert!(events.iter().any(|event| matches!(&event.event, crate::agent::AgentEvent::Error { message } if message.contains("run.tool_refusal_cycle:"))));
    assert!(!events
        .iter()
        .any(|event| matches!(event.event, crate::agent::AgentEvent::End { .. })));
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
