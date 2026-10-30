#![cfg(test)]

use super::automatic::{now_ms, policy, request, ACHIEVED};
use super::*;
use crate::agent_api::{AgentRunSpawn, GoalTickAdmission};
use std::time::Duration;

mod active;
mod confirmation;
mod persistence;
mod tool;

#[tokio::test]
async fn an_expired_preview_preserves_the_typed_host_refusal_without_model_work() {
    let root = tempfile::tempdir().unwrap();
    let (agent, session, client) = fixture_with(
        root.path(),
        true,
        options("goal-run"),
        Some(policy(true, 8, 4, 1)),
    )
    .await;
    let result = session
        .preview_goal_tick(&request(&session, "expired-preview"))
        .await;
    session.close().await;
    agent.close().await;
    let error = anyhow::Error::new(result.unwrap_err()).context("next goal slice");
    assert!(error.chain().any(|source| matches!(
        source.downcast_ref::<crate::goal_control::GoalControlError>(),
        Some(crate::goal_control::GoalControlError::Refused {
            refusal: crate::goal_control::GoalRefusal::DeadlineExpired,
            ..
        })
    )));
    assert_eq!(client.calls.load(Ordering::SeqCst), 0);
}

async fn stalled_automatic_call(stall_at: usize, streaming: bool) {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("source.txt"), "current source").unwrap();
    let mut opts = options("goal-run")
        .with_llm_api_timeout(60_000)
        .with_permission_policy(
            crate::permissions::PermissionPolicy::new().allow("verify_source(*)"),
        );
    opts.max_execution_time_ms = None;
    let deadline = now_ms() + 3_000;
    let (agent, session, client) =
        fixture_with(root.path(), true, opts, Some(policy(true, 8, 4, deadline))).await;
    assert!(session.config.max_execution_time_ms.is_none());
    session
        .register_dynamic_tool(Arc::new(acceptance::VerifySource("current source".into())))
        .unwrap();
    acceptance::script(&client, ACHIEVED);
    client.stall_at.store(stall_at, Ordering::SeqCst);
    assert!(
        now_ms() < deadline,
        "the automatic source must still be valid before admission"
    );
    let tick = request(&session, "wake-deadline");
    let (mut worker, stream_events) = if streaming {
        let crate::agent_api::GoalTickStream::Started(mut stream) =
            session.stream_goal_tick(&tick).await.unwrap()
        else {
            panic!("the live automatic source must admit its original stream")
        };
        let drain = tokio::spawn(async move {
            let mut events = Vec::new();
            while let Some(event) = stream.events.recv().await {
                events.push(event);
            }
            events
        });
        (stream.worker, Some(drain))
    } else {
        let GoalTickAdmission::Admitted(AgentRunSpawn::Started { worker, .. }) =
            session.tick_goal(&tick).await.unwrap()
        else {
            panic!("the live automatic source must admit its original Run")
        };
        (worker, None)
    };
    let reached = tokio::time::timeout(Duration::from_secs(2), async {
        while client.calls.load(Ordering::SeqCst) < stall_at {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .is_ok();
    let remaining = deadline.saturating_sub(now_ms());
    let stopped = tokio::time::timeout(Duration::from_millis(remaining + 1_000), &mut worker).await;
    let status = session.run_snapshot("wake-deadline").await.unwrap().status;
    let events = session.run_events("wake-deadline").await;
    let facts = settlement::facts(root.path());
    let calls = client.calls.load(Ordering::SeqCst);
    let preview = if stopped.is_ok() {
        Some(
            session
                .preview_goal_tick(&request(&session, "wake-later"))
                .await,
        )
    } else {
        None
    };
    session.close().await;
    agent.close().await;
    if stopped.is_err() {
        tokio::time::timeout(Duration::from_secs(5), worker)
            .await
            .unwrap()
            .unwrap();
    }
    if let Some(drain) = stream_events {
        let seen = tokio::time::timeout(Duration::from_secs(5), drain)
            .await
            .unwrap()
            .unwrap();
        assert!(seen.iter().any(|event| matches!(event,
            crate::agent::AgentEvent::Error { message } if message.contains("goal.deadline_expired")
        )));
        assert!(!seen
            .iter()
            .any(|event| matches!(event, crate::agent::AgentEvent::End { .. })));
    }
    assert!(
        reached,
        "the test must reach the selected actual model call"
    );
    assert!(
        stopped.as_ref().is_ok_and(|joined| joined.is_ok()),
        "automatic deadline did not settle model call {stall_at}; before cleanup status was {status:?}"
    );
    assert!(matches!(
        status,
        crate::run::RunStatus::Cancelled | crate::run::RunStatus::Failed
    ));
    assert_eq!(events.iter().filter(|record| matches!(
        &record.event,
        crate::agent::AgentEvent::Error { message } if message.contains("goal.deadline_expired")
    )).count(), 1, "the exact Run must retain one named deadline diagnostic");
    let closeout = facts
        .iter()
        .find(|fact| fact.kind == "a3s.goal.slice.settled")
        .unwrap();
    assert_eq!(closeout.payload["body"]["outcome"], "interrupted");
    assert!(
        closeout.payload["body"]["usage"].is_null(),
        "an in-flight provider call has unknown usage"
    );
    assert!(!facts.iter().any(|fact| matches!(
        fact.kind.as_str(),
        "a3s.goal.step.accepted" | "a3s.goal.completed"
    )));
    assert!(!crate::goal_control::slice_unsettled(&facts));
    if stall_at == 3 {
        let inputs = client.inputs.lock().unwrap();
        assert!(inputs[2].contains("host:source-check"));
        assert!(inputs[2].contains("Original source is checked"));
    }
    assert!(preview
        .unwrap()
        .unwrap_err()
        .to_string()
        .contains("goal.deadline_expired"));
    assert_eq!(
        calls, stall_at,
        "expiry must not launch another model attempt"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn automatic_deadline_stops_a_stalled_main_model_without_a_session_timeout() {
    stalled_automatic_call(1, false).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn automatic_deadline_stops_a_stalled_goal_assessment_without_a_session_timeout() {
    stalled_automatic_call(3, false).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn automatic_deadline_stream_retains_the_reason_without_a_success_end() {
    stalled_automatic_call(3, true).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_expired_automatic_policy_does_not_disable_an_explicit_manual_run() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("source.txt"), "current source").unwrap();
    let (agent, session, client) = fixture_with(
        root.path(),
        true,
        options("goal-run").with_permission_policy(
            crate::permissions::PermissionPolicy::new().allow("verify_source(*)"),
        ),
        Some(policy(true, 8, 4, 1)),
    )
    .await;
    session
        .register_dynamic_tool(Arc::new(acceptance::VerifySource("current source".into())))
        .unwrap();
    acceptance::script(&client, ACHIEVED);
    let result = tokio::time::timeout(
        Duration::from_secs(5),
        session.send("Check the original source.", None),
    )
    .await;
    let cancelled = session.session_cancel.is_cancelled();
    session.close().await;
    agent.close().await;
    assert!(result.unwrap().is_ok());
    assert!(!cancelled);
    assert_eq!(client.calls.load(Ordering::SeqCst), 3);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_source_that_expires_before_its_worker_starts_cannot_reach_the_model() {
    let root = tempfile::tempdir().unwrap();
    let limits = policy(true, 8, 4, now_ms() - 1);
    let (agent, session, client) =
        fixture_with(root.path(), true, options("goal-run"), Some(limits.clone())).await;
    // Simulate a valid preview followed by a delayed handoff to its worker.
    let source = crate::goal_control::ContinuationSource::turn_progress(
        limits.clone(),
        limits.deadline_unix_ms.get() - 1,
    );
    let started = crate::agent_api::conversation_runtime::spawn_goal_tick(
        &session,
        request(&session, "wake-expired"),
        source,
        "inspect",
    )
    .await;
    let stopped = match started {
        Ok(AgentRunSpawn::Started { mut worker, .. }) => {
            let stopped = tokio::time::timeout(Duration::from_secs(2), &mut worker).await;
            session.close().await;
            if stopped.is_err() {
                tokio::time::timeout(Duration::from_secs(5), worker)
                    .await
                    .unwrap()
                    .unwrap();
            }
            stopped.as_ref().is_ok_and(|joined| joined.is_ok())
        }
        Err(_) => {
            session.close().await;
            true
        }
        Ok(AgentRunSpawn::Replayed { .. }) => false,
    };
    agent.close().await;
    assert!(stopped);
    assert_eq!(client.calls.load(Ordering::SeqCst), 0);
    assert!(!settlement::facts(root.path()).iter().any(|fact| matches!(
        fact.kind.as_str(),
        "a3s.goal.step.accepted" | "a3s.goal.completed"
    )));
}
