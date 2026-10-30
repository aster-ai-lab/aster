#![cfg(test)]

use super::*;
use crate::tools::{Tool, ToolCapabilities, ToolContext, ToolOutput};
use std::sync::atomic::AtomicBool;

struct StalledSource {
    started: Arc<AtomicBool>,
    dropped: Arc<AtomicBool>,
}

struct ToolDropped(Arc<AtomicBool>);

impl Drop for ToolDropped {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

#[async_trait::async_trait]
impl Tool for StalledSource {
    fn name(&self) -> &str {
        "verify_source"
    }
    fn description(&self) -> &str {
        "Wait indefinitely while reading the source."
    }
    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({"type":"object","properties":{},"additionalProperties":false})
    }
    fn capabilities(&self, _: &serde_json::Value) -> ToolCapabilities {
        ToolCapabilities::read_only_paginated(1)
    }
    async fn execute(&self, _: &serde_json::Value, _: &ToolContext) -> anyhow::Result<ToolOutput> {
        let _dropped = ToolDropped(self.dropped.clone());
        self.started.store(true, Ordering::SeqCst);
        std::future::pending().await
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn automatic_deadline_retires_a_stalled_tool_and_keeps_the_session_available() {
    let root = tempfile::tempdir().unwrap();
    let deadline = now_ms() + 3_000;
    let (agent, session, client) = fixture_with(
        root.path(),
        true,
        options("goal-run")
            .with_llm_api_timeout(60_000)
            .with_permission_policy(
                crate::permissions::PermissionPolicy::new().allow("verify_source(*)"),
            ),
        Some(policy(true, 8, 4, deadline)),
    )
    .await;
    let started = Arc::new(AtomicBool::new(false));
    let dropped = Arc::new(AtomicBool::new(false));
    session
        .register_dynamic_tool(Arc::new(StalledSource {
            started: started.clone(),
            dropped: dropped.clone(),
        }))
        .unwrap();
    acceptance::script(&client, ACHIEVED);
    let GoalTickAdmission::Admitted(AgentRunSpawn::Started { mut worker, .. }) = session
        .tick_goal(&request(&session, "wake-tool-deadline"))
        .await
        .unwrap()
    else {
        panic!("the original automatic Run must start")
    };
    let reached = tokio::time::timeout(Duration::from_secs(2), async {
        while !started.load(Ordering::SeqCst) {
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
    let calls = client.calls.load(Ordering::SeqCst);
    let dropped_before_close = dropped.load(Ordering::SeqCst);
    let session_cancelled = session.session_cancel.is_cancelled();
    let next = if stopped.is_ok() {
        client.scripted.lock().unwrap().clear();
        Some(
            tokio::time::timeout(
                Duration::from_secs(5),
                session.send("Inspect the current goal.", None),
            )
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
    assert!(
        reached,
        "the original tool must actually enter its pending execution"
    );
    assert!(
        stopped.as_ref().is_ok_and(|joined| joined.is_ok()),
        "the automatic deadline must retire the stalled tool Run"
    );
    assert!(
        dropped_before_close,
        "the original pending tool must be dropped before retirement"
    );
    assert_eq!(calls, 1, "expiry must not make a new model attempt");
    assert!(
        !session_cancelled,
        "the Goal deadline only cancels its original Run"
    );
    assert!(
        next.unwrap().unwrap().is_ok(),
        "a manual Run must still acquire the released session"
    );
}
