#![cfg(test)]

use super::*;
use crate::verification::{VerificationCheck, VerificationReport, VerificationStatus};

struct FlipSource {
    calls: Arc<AtomicUsize>,
    advance: bool,
}

#[async_trait::async_trait]
impl Tool for FlipSource {
    fn name(&self) -> &str {
        "flip_source"
    }
    fn description(&self) -> &str {
        "Write one controlled source state."
    }
    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({"type":"object","properties":{"content":{"type":"string","enum":["A","B"]}},"required":["content"],"additionalProperties":false})
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        let content = if self.advance {
            call.to_string()
        } else {
            args["content"].as_str().unwrap().into()
        };
        std::fs::write(ctx.workspace.join("source.txt"), content)?;
        Ok(ToolOutput::success("Source updated."))
    }
}

fn flip(iteration: usize) -> LlmResponse {
    let mut response = repeat(iteration);
    response.message.content = vec![crate::llm::ContentBlock::ToolUse {
        id: format!("flip-{iteration}"),
        name: "flip_source".into(),
        input: serde_json::json!({"content":if iteration % 2 == 1 { "B" } else { "A" }}),
    }];
    response
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_single_run_cannot_keep_billing_by_alternating_already_observed_source_states() {
    repeated_state_run(None).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_single_external_change_cannot_disable_later_state_cycle_detection() {
    repeated_state_run(Some(2)).await;
}

async fn repeated_state_run(external_at: Option<usize>) {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("source.txt"), "A").unwrap();
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
                crate::permissions::PermissionPolicy::new().allow("flip_source(*)"),
            ),
        Some(policy(true, 8, 4, deadline)),
    )
    .await;
    let physical = Arc::new(AtomicUsize::new(0));
    session
        .register_dynamic_tool(Arc::new(FlipSource {
            calls: physical.clone(),
            advance: false,
        }))
        .unwrap();
    let weak = Arc::downgrade(&client);
    let source = root.path().join("source.txt");
    *client.before_response.lock().unwrap() = Some(Box::new(move |iteration| {
        if external_at == Some(iteration) {
            std::fs::write(&source, "external-input").unwrap();
        }
        weak.upgrade()
            .unwrap()
            .scripted
            .lock()
            .unwrap()
            .push_back(flip(iteration));
    }));
    let GoalTickAdmission::Admitted(AgentRunSpawn::Started { mut worker, .. }) = session
        .tick_goal(&request(&session, "wake-flip"))
        .await
        .unwrap()
    else {
        panic!("the original Run must start")
    };
    let stopped = tokio::time::timeout(Duration::from_secs(3), &mut worker).await;
    let calls = client.calls.load(Ordering::SeqCst);
    let executed = physical.load(Ordering::SeqCst);
    let before_deadline = now_ms() < deadline;
    let events = session.run_events("wake-flip").await;
    let facts = settlement::facts(root.path());
    let denials = facts
        .iter()
        .filter(|f| f.kind == "tool.result" && f.payload["receipt"]["outcome"] == "denied")
        .count();
    eprintln!(
        "NET_STATE_LOOP_OBSERVATION external_at={external_at:?} model_calls={calls} physical_calls={executed} stopped={} denials={denials} before_deadline={before_deadline}",
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
    assert!(before_deadline);
    assert!(
        stopped.as_ref().is_ok_and(|r| r.is_ok()),
        "A↔B writes repeatedly reset the duplicate guard without producing a new observed state"
    );
    assert_eq!(
        denials, 0,
        "this is a cycle of successful effects, not the earlier refused-call bug"
    );
    assert_eq!(calls, executed);
    assert_eq!(calls, if external_at.is_some() { 9 } else { 8 });
    assert!(
        calls <= 10,
        "use the existing duplicate allowance over original observed states: {calls}"
    );
    assert!(events.iter().any(|r| matches!(&r.event, crate::agent::AgentEvent::Error { message } if message.contains("run.tool_state_cycle:"))));
    assert!(!events
        .iter()
        .any(|r| matches!(r.event, crate::agent::AgentEvent::End { .. })));
    assert!(!facts.iter().any(|f| f.kind == "aster.goal.completed"));
}

struct ExpectedSource {
    root: std::path::PathBuf,
    expected: String,
}

impl crate::CompletionAttestor for ExpectedSource {
    fn attest(
        &self,
        digest: &str,
        paths: &[crate::MutatedPathRecord],
    ) -> Option<VerificationReport> {
        let bytes = std::fs::read(self.root.join("source.txt")).ok()?;
        if paths.len() != 1
            || paths[0].path != "source.txt"
            || paths[0].content_digest != sha256::digest(&bytes)
            || bytes != self.expected.as_bytes()
        {
            return None;
        }
        Some(
            VerificationReport::new(
                "host:bounded-updates",
                vec![VerificationCheck::required(
                    "inspect",
                    "source-bytes",
                    "Original source matches the bounded expected result",
                )
                .with_status(VerificationStatus::Passed)],
            )
            .with_effect_digest(digest),
        )
    }
}

async fn bounded_updates(advance: bool, threshold: u32, updates: usize, external: bool) {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("source.txt"), "A").unwrap();
    let (agent, session, client) = fixture(
        root.path(),
        true,
        options("goal-run")
            .with_max_tool_rounds(0)
            .with_auto_compact(false)
            .with_assistant_final_response(true)
            .with_duplicate_tool_call_threshold(threshold)
            .with_completion_attestor(Arc::new(ExpectedSource {
                root: root.path().to_path_buf(),
                expected: if advance {
                    updates.to_string()
                } else {
                    "A".into()
                },
            }))
            .with_permission_policy(
                crate::permissions::PermissionPolicy::new().allow("flip_source(*)"),
            ),
    )
    .await;
    let physical = Arc::new(AtomicUsize::new(0));
    session
        .register_dynamic_tool(Arc::new(FlipSource {
            calls: physical.clone(),
            advance,
        }))
        .unwrap();
    if external {
        let source = root.path().join("source.txt");
        *client.before_response.lock().unwrap() = Some(Box::new(move |iteration| {
            if iteration <= updates {
                std::fs::write(&source, format!("external-{iteration}")).unwrap();
            }
        }));
    }
    for iteration in 1..=updates {
        let mut response = flip(iteration);
        if advance {
            response.message.content = vec![crate::llm::ContentBlock::ToolUse {
                id: format!("advance-{iteration}"),
                name: "flip_source".into(),
                input: serde_json::json!({"content":"B"}),
            }];
        }
        client.scripted.lock().unwrap().push_back(response);
    }
    client.scripted.lock().unwrap().extend([
        productive::text("The required original source updates are recorded."),
        productive::text(r#"{"achieved":false,"progress":0.0,"remaining_criteria":["All original checks pass"]}"#),
    ]);
    let result = tokio::time::timeout(
        Duration::from_secs(5),
        session.send(
            "Perform the bounded source updates and report their outcome.",
            None,
        ),
    )
    .await;
    session.close().await;
    agent.close().await;
    let observation = result.as_ref().map(|r| {
        r.as_ref()
            .map(|r| r.tool_calls_count)
            .map_err(ToString::to_string)
    });
    eprintln!(
        "BOUNDED_UPDATE_OBSERVATION advance={advance} threshold={threshold} updates={updates} result={observation:?}"
    );
    assert!(
        result.unwrap().is_ok(),
        "bounded checks or fresh states must still finish"
    );
    assert_eq!(physical.load(Ordering::SeqCst), updates);
    assert_eq!(
        std::fs::read_to_string(root.path().join("source.txt")).unwrap(),
        if advance {
            updates.to_string()
        } else {
            "A".into()
        }
    );
}

#[tokio::test]
async fn three_required_round_trips_still_complete_with_the_existing_allowance() {
    bounded_updates(false, 3, 6, false).await;
}

#[tokio::test]
async fn genuinely_new_source_states_allow_many_identical_tool_arguments() {
    bounded_updates(true, 3, 12, false).await;
}

#[tokio::test]
async fn a_larger_existing_duplicate_allowance_permits_bounded_repeated_states() {
    bounded_updates(false, 8, 12, false).await;
}

#[tokio::test]
async fn intervening_external_source_states_are_not_a_closed_agent_cycle() {
    bounded_updates(false, 3, 12, true).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_owned_child_also_stops_a_successful_source_state_cycle() {
    owned_child_state_run(None).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_owned_child_detects_a_state_cycle_after_one_external_change() {
    owned_child_state_run(Some(2)).await;
}

async fn owned_child_state_run(external_at: Option<usize>) {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("source.txt"), "A").unwrap();
    let client = Arc::new(GoalClient::default());
    let weak = Arc::downgrade(&client);
    let source = root.path().join("source.txt");
    *client.before_response.lock().unwrap() = Some(Box::new(move |iteration| {
        if external_at == Some(iteration) {
            std::fs::write(&source, "external-input").unwrap();
        }
        weak.upgrade()
            .unwrap()
            .scripted
            .lock()
            .unwrap()
            .push_back(flip(iteration));
    }));
    let physical = Arc::new(AtomicUsize::new(0));
    let executor = Arc::new(crate::tools::ToolExecutor::new(
        root.path().display().to_string(),
    ));
    executor.register_dynamic_tool_if_absent(Arc::new(FlipSource {
        calls: physical.clone(),
        advance: false,
    }));
    let permission = crate::permissions::PermissionPolicy::new().allow("flip_source(*)");
    let cancel = tokio_util::sync::CancellationToken::new();
    let run = crate::agent_api::ChildRun {
        agent: crate::agent::AgentLoop::new(
            client.clone(),
            executor.clone(),
            ToolContext::new(root.path().to_path_buf()),
            crate::agent::AgentConfig {
                tools: executor.definitions(),
                max_tool_rounds: 0,
                duplicate_tool_call_threshold: 3,
                auto_compact: false,
                continuation_enabled: false,
                planning_mode: crate::PlanningMode::Disabled,
                permission_policy: Some(permission.clone()),
                permission_checker: Some(Arc::new(permission)),
                ..Default::default()
            },
        ),
        child_id: "state-cycle-child".into(),
        budget_session: None,
        events: None,
        cancel: cancel.clone(),
    };
    let mut worker =
        tokio::spawn(async move { run.run("Apply the controlled source transitions.").await });
    let stopped = tokio::time::timeout(Duration::from_secs(3), &mut worker).await;
    let calls = client.calls.load(Ordering::SeqCst);
    eprintln!(
        "CHILD_REANCHOR_OBSERVATION external_at={external_at:?} model_calls={calls} stopped={}",
        stopped.is_ok()
    );
    *client.before_response.lock().unwrap() = None;
    if stopped.is_err() {
        cancel.cancel();
        tokio::time::timeout(Duration::from_secs(5), worker)
            .await
            .unwrap()
            .unwrap()
            .ok();
    }
    assert!(
        stopped.is_ok(),
        "the owned child must use the same observed-state protection"
    );
    assert!(
        format!("{:#}", stopped.unwrap().unwrap().unwrap_err()).contains("run.tool_state_cycle:")
    );
    let expected = if external_at.is_some() { 9 } else { 8 };
    assert_eq!(calls, expected);
    assert_eq!(physical.load(Ordering::SeqCst), expected);
}
