#![cfg(test)]

use super::automatic::{now_ms, policy, request, slices};
use super::*;
use crate::agent_api::{AgentRunSpawn, GoalTickAdmission};
use crate::goal_control::{GoalControlError, GoalRefusal, GoalTickDecisionV1};
use crate::store::{FileSessionStore, MemorySessionStore, SessionStore};
use std::time::Duration;

const UNFINISHED: &str = r#"{"achieved":false,"progress":0.0,"remaining_criteria":["more work"]}"#;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Limit {
    Slices,
    NoProgress,
    Deadline,
    Disabled,
}

async fn recover_limits(file: bool, constraint: Limit) {
    let deadline_case = constraint == Limit::Deadline;
    let root = tempfile::tempdir().unwrap();
    let data = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("source.txt"), "current source").unwrap();
    let mut store: Arc<dyn SessionStore> = if file {
        Arc::new(FileSessionStore::new(data.path()).await.unwrap())
    } else {
        Arc::new(MemorySessionStore::new())
    };
    let opts = |store: Arc<dyn SessionStore>| {
        options("goal-run")
            .with_session_store(store)
            .with_auto_save(true)
            .with_planning_mode(crate::PlanningMode::Enabled)
            .with_goal_tracking(true)
            .with_assistant_final_response(true)
            .with_permission_policy(
                crate::permissions::PermissionPolicy::new().allow("verify_source(*)"),
            )
    };
    let initial_deadline = now_ms() + if deadline_case { 3_000 } else { 60_000 };
    let initial_policy = policy(
        true,
        if constraint == Limit::Slices { 2 } else { 8 },
        if constraint == Limit::NoProgress {
            2
        } else {
            8
        },
        initial_deadline,
    );
    let script = |client: &GoalClient| {
        if constraint == Limit::NoProgress {
            client.scripted.lock().unwrap().push_back(LlmResponse {
                message: Message::assistant("No original evidence."),
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
        } else {
            acceptance::script(client, UNFINISHED);
        }
    };
    let (per_calls, per_usage) = if constraint == Limit::NoProgress {
        (1, (1, 1, 2))
    } else {
        (3, (15, 6, 21))
    };
    let (agent, session, client) = fixture_with(
        root.path(),
        true,
        opts(store.clone()),
        Some(initial_policy.clone()),
    )
    .await;
    session
        .register_dynamic_tool(Arc::new(acceptance::VerifySource("current source".into())))
        .unwrap();
    script(&client);
    let GoalTickAdmission::Admitted(AgentRunSpawn::Started { worker, .. }) = session
        .tick_goal(&request(&session, "initial"))
        .await
        .unwrap()
    else {
        panic!("initial original Run must start")
    };
    tokio::time::timeout(Duration::from_secs(5), worker)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(client.calls.load(Ordering::SeqCst), per_calls);
    assert_eq!(session.recorded_usage_tokens(), per_usage);
    assert_eq!(slices(root.path()).len(), 1);
    session.save().await.unwrap();
    session.close().await;
    agent.close().await;
    if deadline_case {
        tokio::time::sleep(Duration::from_millis(
            initial_deadline.saturating_sub(now_ms()) + 50,
        ))
        .await;
    }
    let extended_deadline = now_ms() + 60_000;
    let mut expected_slices = 1;
    let mut expected_total = per_usage.2;
    for phase in 0..3 {
        let expiry = deadline_case && phase == 0;
        let allow = phase == 1;
        let deadline = if deadline_case && phase > 0 {
            extended_deadline
        } else {
            initial_deadline
        };
        let (enabled, max, stalled) = match (constraint, phase) {
            (Limit::Slices, 0) => (true, 1, 8),
            (Limit::Slices, 1) => (true, 3, 8),
            (Limit::NoProgress, 0) => (true, 8, 1),
            (Limit::NoProgress, 1) => (true, 8, 3),
            (Limit::NoProgress, _) => (true, 8, 2),
            (Limit::Disabled, 0) => (false, 8, 8),
            (_, 2) => (true, 2, 8),
            _ => (true, 8, 8),
        };
        let limits = policy(enabled, max, stalled, deadline);
        if file {
            store = Arc::new(FileSessionStore::new(data.path()).await.unwrap());
        }
        let agent = Agent::from_config(fixture_config(Some(limits.clone())))
            .await
            .unwrap();
        let client = Arc::new(GoalClient::default());
        let restored = agent
            .resume_session_async(
                "goal-run",
                opts(store.clone()).with_llm_client(client.clone()),
            )
            .await
            .unwrap();
        restored
            .register_dynamic_tool(Arc::new(acceptance::VerifySource("current source".into())))
            .unwrap();
        assert_eq!(
            restored.recorded_usage_tokens().2,
            expected_total,
            "reopen must retain original synthetic usage"
        );
        let command = request(&restored, &format!("reopen-{phase}"));
        let before = crate::fact_control::read_workspace_facts(root.path(), "goal-run").unwrap();
        if expiry {
            for error in [
                restored.preview_goal_tick(&command).await.err().unwrap(),
                restored.tick_goal(&command).await.err().unwrap(),
            ] {
                assert!(anyhow::Error::new(error).chain().any(|cause| matches!(
                    cause.downcast_ref::<GoalControlError>(),
                    Some(GoalControlError::Refused {
                        refusal: GoalRefusal::DeadlineExpired,
                        ..
                    })
                )));
            }
        } else if allow {
            assert_eq!(
                restored.preview_goal_tick(&command).await.unwrap(),
                GoalTickDecisionV1::Advance
            );
            script(&client);
            let GoalTickAdmission::Admitted(AgentRunSpawn::Started { worker, .. }) =
                restored.tick_goal(&command).await.unwrap()
            else {
                panic!("explicit trusted extension must admit one more Run")
            };
            tokio::time::timeout(Duration::from_secs(5), worker)
                .await
                .unwrap()
                .unwrap();
            expected_slices += 1;
            expected_total += per_usage.2;
            assert_eq!(client.calls.load(Ordering::SeqCst), per_calls);
            let admitted = slices(root.path());
            assert_eq!(
                admitted[0].payload["body"]["continuation"]["policy"],
                serde_json::to_value(&initial_policy).unwrap(),
                "new host policy must not rewrite historical authority"
            );
            assert_eq!(
                admitted.last().unwrap().payload["body"]["continuation"]["policy"],
                serde_json::to_value(&limits).unwrap()
            );
        } else {
            let expected = if constraint == Limit::NoProgress {
                GoalTickDecisionV1::NeedsInput
            } else {
                GoalTickDecisionV1::BudgetExhausted
            };
            assert_eq!(
                restored.preview_goal_tick(&command).await.unwrap(),
                expected
            );
            let GoalTickAdmission::Decided(actual) = restored.tick_goal(&command).await.unwrap()
            else {
                panic!("a refused continuation must not create an owner")
            };
            assert_eq!(actual, expected);
        }
        if !allow {
            assert_eq!(client.calls.load(Ordering::SeqCst), 0);
            assert_eq!(
                crate::fact_control::read_workspace_facts(root.path(), "goal-run").unwrap(),
                before
            );
        }
        assert_eq!(slices(root.path()).len(), expected_slices);
        assert_eq!(restored.recorded_usage_tokens().2, expected_total);
        let settled: Vec<_> = settlement::facts(root.path())
            .into_iter()
            .filter(|f| f.kind == "a3s.goal.slice.settled")
            .collect();
        assert_eq!(settled.len(), expected_slices);
        assert_eq!(
            settled
                .iter()
                .map(|f| f.payload["body"]["usage"]["total_tokens"].as_u64().unwrap())
                .sum::<u64>(),
            expected_total
        );
        eprintln!(
            "RECOVERED_LIMITS_OBSERVATION file={file} constraint={constraint:?} phase={phase} admitted={expected_slices} synthetic_total={expected_total} model_calls={}",
            client.calls.load(Ordering::SeqCst)
        );
        restored.save().await.unwrap();
        restored.close().await;
        agent.close().await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reopened_slice_policy_preserves_cumulative_opportunities_and_recorded_usage() {
    for file in [false, true] {
        recover_limits(file, Limit::Slices).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_expired_reopened_deadline_requires_explicit_host_extension_and_keeps_usage() {
    for file in [false, true] {
        recover_limits(file, Limit::Deadline).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reopened_stall_policy_keeps_the_original_no_progress_count() {
    for file in [false, true] {
        recover_limits(file, Limit::NoProgress).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_disabled_reopened_policy_requires_explicit_host_reenablement() {
    for file in [false, true] {
        recover_limits(file, Limit::Disabled).await;
    }
}
