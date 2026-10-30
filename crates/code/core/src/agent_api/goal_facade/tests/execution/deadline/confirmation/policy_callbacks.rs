#![cfg(test)]

use super::*;
use crate::tools::{Tool, ToolCapabilities, ToolContext, ToolOutput};

struct EscalatedSource {
    escalation: bool,
    executions: Arc<AtomicUsize>,
}

#[async_trait::async_trait]
impl Tool for EscalatedSource {
    fn name(&self) -> &str {
        "verify_source"
    }
    fn description(&self) -> &str {
        "Read the controlled original source."
    }
    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({"type":"object","properties":{},"additionalProperties":false})
    }
    fn capabilities(&self, _: &serde_json::Value) -> ToolCapabilities {
        ToolCapabilities::read_only_paginated(1)
    }
    fn requires_confirmation(&self, _: &serde_json::Value) -> bool {
        self.escalation
    }
    async fn execute(
        &self,
        _: &serde_json::Value,
        ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        self.executions.fetch_add(1, Ordering::SeqCst);
        Ok(ToolOutput::success(std::fs::read_to_string(
            ctx.workspace.join("source.txt"),
        )?))
    }
}

async fn bounded_policy_callback(stage: PolicyStage) {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("source.txt"), "current source").unwrap();
    let (events, _) = broadcast::channel(8);
    let provider = Arc::new(StalledConfirmation {
        inner: ConfirmationManager::new(ConfirmationPolicy::enabled(), events),
        request: false,
        policy_stage: Some(stage),
        requires_calls: AtomicUsize::new(0),
        entered: AtomicBool::new(false),
        dropped: Arc::new(AtomicBool::new(false)),
        release: CancellationToken::new(),
    });
    let escalation = matches!(stage, PolicyStage::Available | PolicyStage::Policy);
    let permission = if escalation {
        crate::permissions::PermissionPolicy::new().allow("verify_source(*)")
    } else {
        crate::permissions::PermissionPolicy::new().ask("verify_source(*)")
    };
    let deadline = now_ms() + 3_000;
    let (agent, session, client) = fixture_with(
        root.path(),
        true,
        options("goal-run")
            .with_permission_policy(permission)
            .with_confirmation_manager(provider.clone()),
        Some(policy(true, 8, 4, deadline)),
    )
    .await;
    let executions = Arc::new(AtomicUsize::new(0));
    session
        .register_dynamic_tool(Arc::new(EscalatedSource {
            escalation,
            executions: executions.clone(),
        }))
        .unwrap();
    acceptance::script(&client, ACHIEVED);
    let GoalTickAdmission::Admitted(AgentRunSpawn::Started { mut worker, .. }) = session
        .tick_goal(&request(&session, "wake-policy"))
        .await
        .unwrap()
    else {
        panic!("the parent must start")
    };
    let reached = tokio::time::timeout(Duration::from_secs(4), async {
        while !provider.entered.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .is_ok();
    let stopped = tokio::time::timeout(
        Duration::from_millis(deadline.saturating_sub(now_ms()) + 1_500),
        &mut worker,
    )
    .await;
    let dropped = provider.dropped.load(Ordering::SeqCst);
    let status = session.run_snapshot("wake-policy").await.unwrap().status;
    let events = session.run_events("wake-policy").await;
    eprintln!(
        "POLICY_CALLBACK_OBSERVATION stage={stage:?} reached={reached} stopped={} dropped={dropped} status={status:?}",
        stopped.is_ok()
    );
    provider.release.cancel();
    if stopped.is_err() {
        tokio::time::timeout(Duration::from_secs(5), worker)
            .await
            .unwrap()
            .unwrap();
    }
    let facts = settlement::facts(root.path());
    session.close().await;
    agent.close().await;
    assert!(
        reached,
        "the selected original provider method must actually stall"
    );
    assert!(
        stopped.as_ref().is_ok_and(|joined| joined.is_ok()),
        "deadline could not settle {stage:?}"
    );
    assert!(
        dropped,
        "the exact callback future must be released before Run retirement"
    );
    assert_eq!(status, crate::run::RunStatus::Cancelled);
    assert_eq!(executions.load(Ordering::SeqCst), 0);
    assert_eq!(client.calls.load(Ordering::SeqCst), 1);
    assert!(!events.iter().any(|v| matches!(
        v.event,
        crate::agent::AgentEvent::ToolExecutionStart { .. }
            | crate::agent::AgentEvent::ConfirmationReceived { .. }
    )));
    assert_eq!(
        facts
            .iter()
            .filter(|v| v.kind == "a3s.goal.slice.settled")
            .count(),
        1
    );
    assert!(!facts.iter().any(|v| v.kind == "a3s.goal.completed"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn automatic_deadline_cancels_a_stalled_confirmation_requirement() {
    bounded_policy_callback(PolicyStage::Requires).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn automatic_deadline_cancels_a_stalled_confirmation_recheck() {
    bounded_policy_callback(PolicyStage::Recheck).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn automatic_deadline_cancels_a_stalled_confirmation_availability() {
    bounded_policy_callback(PolicyStage::Available).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn automatic_deadline_cancels_a_stalled_confirmation_policy() {
    bounded_policy_callback(PolicyStage::Policy).await;
}
