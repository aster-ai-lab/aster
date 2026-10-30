#![cfg(test)]

use super::*;
use crate::hitl::{
    ConfirmationManager, ConfirmationPolicy, ConfirmationProvider, ConfirmationResponse,
};
use std::sync::atomic::AtomicBool;
use tokio::sync::{broadcast, oneshot};
use tokio_util::sync::CancellationToken;

mod direct;
mod identity;
mod policy_callbacks;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PolicyStage {
    Requires,
    Recheck,
    Available,
    Policy,
}

struct StalledConfirmation {
    inner: ConfirmationManager,
    request: bool,
    policy_stage: Option<PolicyStage>,
    requires_calls: AtomicUsize,
    entered: AtomicBool,
    dropped: Arc<AtomicBool>,
    release: CancellationToken,
}

struct CallbackDropped(Arc<AtomicBool>);

impl Drop for CallbackDropped {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

impl StalledConfirmation {
    async fn stall(&self) {
        let _dropped = CallbackDropped(self.dropped.clone());
        self.entered.store(true, Ordering::SeqCst);
        self.release.cancelled().await;
    }
}

#[async_trait::async_trait]
impl ConfirmationProvider for StalledConfirmation {
    async fn requires_confirmation(&self, _: &str) -> bool {
        let call = self.requires_calls.fetch_add(1, Ordering::SeqCst);
        if self.policy_stage == Some(PolicyStage::Requires)
            || (self.policy_stage == Some(PolicyStage::Recheck) && call == 1)
        {
            self.stall().await;
        }
        self.policy_stage.is_none()
    }
    async fn confirmation_available_for(&self, _: &str, _: &serde_json::Value) -> bool {
        if self.policy_stage == Some(PolicyStage::Available) {
            self.stall().await;
        }
        true
    }
    async fn request_confirmation(
        &self,
        id: &str,
        name: &str,
        args: &serde_json::Value,
    ) -> oneshot::Receiver<ConfirmationResponse> {
        let receiver = self.inner.request_confirmation(id, name, args).await;
        if self.request {
            self.stall().await;
        }
        receiver
    }
    async fn confirm(
        &self,
        id: &str,
        approved: bool,
        reason: Option<String>,
    ) -> std::result::Result<bool, String> {
        if !self.request && !approved {
            self.stall().await;
        }
        self.inner.confirm(id, approved, reason).await
    }
    async fn policy(&self) -> ConfirmationPolicy {
        if self.policy_stage == Some(PolicyStage::Policy) {
            self.stall().await;
        }
        self.inner.policy().await
    }
    async fn set_policy(&self, policy: ConfirmationPolicy) {
        self.inner.set_policy(policy).await;
    }
    async fn check_timeouts(&self) -> usize {
        self.inner.check_timeouts().await
    }
    async fn cancel_all(&self) -> usize {
        self.inner.cancel_all().await
    }
    async fn pending_confirmations(&self) -> Vec<crate::hitl::PendingConfirmationInfo> {
        self.inner.pending_confirmation_details().await
    }
}

async fn stalled_confirmation(request_stage: bool) {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("source.txt"), "current source").unwrap();
    let (events, _) = broadcast::channel(8);
    let provider = Arc::new(StalledConfirmation {
        inner: ConfirmationManager::new(ConfirmationPolicy::enabled(), events),
        request: request_stage,
        policy_stage: None,
        requires_calls: AtomicUsize::new(0),
        entered: AtomicBool::new(false),
        dropped: Arc::new(AtomicBool::new(false)),
        release: CancellationToken::new(),
    });
    let untouched = provider
        .inner
        .request_confirmation("untouched", "write", &serde_json::json!({}))
        .await;
    let deadline = now_ms() + 3_000;
    let (agent, session, client) = fixture_with(
        root.path(),
        true,
        options("goal-run")
            .with_permission_policy(
                crate::permissions::PermissionPolicy::new().ask("verify_source(*)"),
            )
            .with_confirmation_manager(provider.clone()),
        Some(policy(true, 8, 4, deadline)),
    )
    .await;
    session
        .register_dynamic_tool(Arc::new(acceptance::VerifySource("current source".into())))
        .unwrap();
    acceptance::script(&client, ACHIEVED);
    let GoalTickAdmission::Admitted(AgentRunSpawn::Started { mut worker, .. }) = session
        .tick_goal(&request(&session, "wake-confirmation"))
        .await
        .unwrap()
    else {
        panic!("the original automatic Run must start")
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
    let dropped_before_release = provider.dropped.load(Ordering::SeqCst);
    let pending_before_release = provider.inner.pending_count().await;
    let pending_id = provider
        .inner
        .pending_confirmation_details()
        .await
        .into_iter()
        .find(|pending| pending.tool_name == "verify_source")
        .map(|pending| pending.tool_id);
    let events = session.run_events("wake-confirmation").await;
    let calls = client.calls.load(Ordering::SeqCst);
    let status_before_release = session
        .run_snapshot("wake-confirmation")
        .await
        .unwrap()
        .status;
    // Release the exact test callback even on a red result, then reap its owner.
    provider.release.cancel();
    if stopped.is_err() {
        let cleanup = tokio::time::timeout(Duration::from_secs(5), worker).await;
        if cleanup.is_err() {
            eprintln!(
                "CONFIRMATION_CLEANUP_UNSETTLED status={:?} calls={} pending={} dropped={} fact_kinds={:?}",
                session
                    .run_snapshot("wake-confirmation")
                    .await
                    .unwrap()
                    .status,
                client.calls.load(Ordering::SeqCst),
                provider.inner.pending_count().await,
                provider.dropped.load(Ordering::SeqCst),
                settlement::facts(root.path())
                    .iter()
                    .map(|fact| &fact.kind)
                    .collect::<Vec<_>>()
            );
        }
        cleanup.unwrap().unwrap();
    }
    if let Some(id) = &pending_id {
        provider.inner.confirm(id, false, None).await.unwrap();
    }
    provider
        .inner
        .confirm("untouched", true, None)
        .await
        .unwrap();
    let untouched_approved = untouched.await.unwrap().approved;
    client.scripted.lock().unwrap().clear();
    let next = tokio::time::timeout(
        Duration::from_secs(5),
        session.send("Inspect the goal.", None),
    )
    .await;
    session.close().await;
    agent.close().await;
    assert!(
        reached,
        "the exact provider callback must actually be pending"
    );
    assert!(
        stopped.as_ref().is_ok_and(|joined| joined.is_ok()),
        "an automatic deadline must retire a Run waiting for host confirmation (request_stage={request_stage})"
    );
    assert!(
        dropped_before_release,
        "the callback future must be dropped before retirement"
    );
    assert_eq!(status_before_release, crate::run::RunStatus::Cancelled);
    assert_eq!(calls, 1, "the tool cannot run without approval");
    assert!(
        untouched_approved,
        "cancellation must preserve an unrelated pending request"
    );
    assert!(next.unwrap().is_ok(), "the original lease must be retired");
    assert!(!events.iter().any(|record| matches!(
        record.event,
        crate::agent::AgentEvent::ToolExecutionStart { .. }
    )));
    if request_stage {
        assert_eq!(
            pending_before_release, 1,
            "only the unrelated request remains"
        );
    } else {
        assert_eq!(
            pending_before_release, 2,
            "a timed-out provider cancellation is not proof of host cleanup"
        );
        let pending_id = pending_id.unwrap();
        assert!(events.iter().any(|record| matches!(&record.event, crate::agent::AgentEvent::ConfirmationTimeout { tool_id, action_taken } if tool_id == &pending_id && action_taken == "cancel_unresolved")));
        assert!(!events.iter().any(|record| matches!(&record.event, crate::agent::AgentEvent::ConfirmationReceived { tool_id, .. } if tool_id == &pending_id)));
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn automatic_deadline_drops_a_stalled_confirmation_registration() {
    stalled_confirmation(true).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn automatic_deadline_bounds_a_stalled_confirmation_cancellation() {
    stalled_confirmation(false).await;
}
