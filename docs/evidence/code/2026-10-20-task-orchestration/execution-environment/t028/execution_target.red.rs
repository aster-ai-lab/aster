#![cfg(test)]

use super::*;
use crate::sandbox::{BashSandbox, SandboxOutput};
use std::sync::atomic::AtomicBool;

struct UnreportedSandbox;

#[async_trait]
impl BashSandbox for UnreportedSandbox {
    async fn exec_command(&self, _: &str, _: &str) -> anyhow::Result<SandboxOutput> {
        anyhow::bail!("this context fixture must never execute a command")
    }

    async fn shutdown(&self) {}
}

struct PendingContext {
    entered: Arc<AtomicBool>,
    dropped: Arc<AtomicBool>,
}

struct MarkDropped(Arc<AtomicBool>);

impl Drop for MarkDropped {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

#[async_trait]
impl crate::context::ContextProvider for PendingContext {
    fn name(&self) -> &str {
        "pending-execution-context"
    }

    async fn query(
        &self,
        _: &crate::context::ContextQuery,
    ) -> anyhow::Result<crate::context::ContextResult> {
        let _drop = MarkDropped(Arc::clone(&self.dropped));
        self.entered.store(true, Ordering::SeqCst);
        std::future::pending().await
    }
}

async fn capture_unknown_target(pending_context: bool) {
    let workspace = tempfile::tempdir().unwrap();
    let agent = durable_source::source_agent().await;
    let systems = Arc::new(std::sync::Mutex::new(Vec::new()));
    let entered = Arc::new(AtomicBool::new(false));
    let dropped = Arc::new(AtomicBool::new(false));
    let mut options = crate::SessionOptions::new()
        .with_session_id("execution-target-unknown")
        .with_planning_mode(crate::PlanningMode::Disabled)
        .with_auto_delegation_enabled(false)
        .with_continuation(false)
        .with_sandbox_handle(Arc::new(UnreportedSandbox));
    if pending_context {
        options = options
            .with_llm_api_timeout(100)
            .with_context_provider(Arc::new(PendingContext {
                entered: Arc::clone(&entered),
                dropped: Arc::clone(&dropped),
            }));
    }
    let session = agent
        .build_session(
            workspace.path().display().to_string(),
            Arc::new(SkillCapturingClient {
                systems: Arc::clone(&systems),
            }),
            &options,
        )
        .unwrap();
    session
        .send("Describe the current execution target without tools.", None)
        .await
        .unwrap();
    let requests = systems.lock().unwrap().clone();
    assert_eq!(requests.len(), 1, "one actual fact main-model request");
    let system = &requests[0];
    assert!(
        system.contains(&format!(
            "Host platform: {} ({})",
            std::env::consts::OS,
            std::env::consts::ARCH
        )),
        "the host must remain explicit in the actual model input: {system}"
    );
    assert!(
        system.contains("Default Bash execution target: unknown (backend did not report OS/shell)"),
        "an unreported bound backend must not inherit host shell claims: {system}"
    );
    assert!(!system.lines().any(|line| line.starts_with("Platform:")));
    if pending_context {
        assert!(entered.load(Ordering::SeqCst));
        assert!(dropped.load(Ordering::SeqCst), "timed-out query is dropped");
    }
    session.close().await;
    agent.close().await;
}

#[tokio::test]
async fn fact_model_input_preserves_unknown_bound_execution_target() {
    capture_unknown_target(false).await;
}

#[tokio::test]
async fn fact_model_input_keeps_required_target_after_optional_context_timeout() {
    capture_unknown_target(true).await;
}
