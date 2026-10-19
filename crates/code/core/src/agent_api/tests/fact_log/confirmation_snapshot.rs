#![cfg(test)]

use super::*;
use crate::hitl::{ConfirmationPolicy, ConfirmationProvider, ConfirmationResponse};
use serde_json::{json, Value};
use std::sync::atomic::AtomicBool;
use tokio::sync::oneshot;

#[derive(Clone)]
struct SnapshotConfirmation {
    enabled: Arc<AtomicBool>,
    frozen: Option<bool>,
    requests: Arc<AtomicUsize>,
    snapshots: Arc<AtomicUsize>,
}

#[async_trait]
impl ConfirmationProvider for SnapshotConfirmation {
    fn snapshot_for_run(&self) -> Option<Arc<dyn ConfirmationProvider>> {
        self.snapshots.fetch_add(1, Ordering::SeqCst);
        Some(Arc::new(Self {
            frozen: Some(
                self.frozen
                    .unwrap_or_else(|| self.enabled.load(Ordering::SeqCst)),
            ),
            ..self.clone()
        }))
    }

    async fn requires_confirmation(&self, _: &str) -> bool {
        self.frozen
            .unwrap_or_else(|| self.enabled.load(Ordering::SeqCst))
    }

    async fn request_confirmation(
        &self,
        _: &str,
        _: &str,
        _: &Value,
    ) -> oneshot::Receiver<ConfirmationResponse> {
        self.requests.fetch_add(1, Ordering::SeqCst);
        let (sender, receiver) = oneshot::channel();
        sender
            .send(ConfirmationResponse {
                approved: true,
                reason: Some("controlled approval".into()),
            })
            .unwrap();
        receiver
    }

    async fn confirm(&self, _: &str, _: bool, _: Option<String>) -> Result<bool, String> {
        Ok(false)
    }
    async fn policy(&self) -> ConfirmationPolicy {
        ConfirmationPolicy {
            enabled: self.requires_confirmation("").await,
            ..Default::default()
        }
    }
    async fn set_policy(&self, policy: ConfirmationPolicy) {
        self.enabled.store(policy.enabled, Ordering::SeqCst);
    }
    async fn check_timeouts(&self) -> usize {
        0
    }
    async fn cancel_all(&self) -> usize {
        0
    }
}

struct SwitchAfterAdmission {
    live_enabled: Arc<AtomicBool>,
    inner: ScriptedClient,
}

#[async_trait]
impl LlmClient for SwitchAfterAdmission {
    async fn complete(
        &self,
        messages: &[Message],
        system: Option<&str>,
        tools: &[ToolDefinition],
    ) -> anyhow::Result<LlmResponse> {
        let response = self.inner.complete(messages, system, tools).await?;
        if !response.tool_calls().is_empty() {
            // The model is called only after the original Run froze its provider.
            self.live_enabled.store(false, Ordering::SeqCst);
        }
        Ok(response)
    }
    async fn complete_streaming(
        &self,
        _: &[Message],
        _: Option<&str>,
        _: &[ToolDefinition],
        _: CancellationToken,
    ) -> anyhow::Result<mpsc::Receiver<StreamEvent>> {
        anyhow::bail!("unused")
    }
}

struct ConfirmationProbe(Arc<AtomicUsize>);
#[async_trait]
impl crate::tools::Tool for ConfirmationProbe {
    fn name(&self) -> &str {
        "confirmation_probe"
    }
    fn description(&self) -> &str {
        "Count a controlled confirmed execution"
    }
    fn parameters(&self) -> Value {
        json!({"type":"object","properties":{},"additionalProperties":false})
    }
    async fn execute(
        &self,
        _: &Value,
        _: &crate::tools::ToolContext,
    ) -> anyhow::Result<crate::tools::ToolOutput> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(crate::tools::ToolOutput::success("confirmed execution"))
    }
}

#[tokio::test]
async fn owned_fact_confirmation_uses_the_original_run_snapshot_after_session_mode_changes() {
    let workspace = tempfile::tempdir().unwrap();
    let agent = crate::Agent::from_config(test_config()).await.unwrap();
    let enabled = Arc::new(AtomicBool::new(true));
    let requests = Arc::new(AtomicUsize::new(0));
    let snapshots = Arc::new(AtomicUsize::new(0));
    let executions = Arc::new(AtomicUsize::new(0));
    let session = agent
        .build_session(
            workspace.path().to_string_lossy().into_owned(),
            Arc::new(SwitchAfterAdmission {
                live_enabled: enabled.clone(),
                inner: ScriptedClient {
                    seen: Default::default(),
                    responses: std::sync::Mutex::new(vec![
                        tool_response("snapshot-probe", "confirmation_probe", json!({})),
                        text_response("done"),
                    ]),
                },
            }),
            &crate::SessionOptions::new()
                .with_session_id("fact-confirmation-snapshot")
                .with_confirmation_manager(Arc::new(SnapshotConfirmation {
                    enabled: enabled.clone(),
                    frozen: None,
                    requests: requests.clone(),
                    snapshots: snapshots.clone(),
                }))
                .with_permission_policy(crate::permissions::PermissionPolicy::new())
                .with_planning_mode(crate::PlanningMode::Disabled)
                .with_auto_delegation_enabled(false)
                .with_continuation(false),
        )
        .unwrap();
    session
        .register_dynamic_tool(Arc::new(ConfirmationProbe(executions.clone())))
        .unwrap();
    let sent = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        session.send("Run the controlled confirmation probe", None),
    )
    .await;
    let facts = read_workspace_facts(workspace.path(), "fact-confirmation-snapshot");
    session.close().await;
    agent.close().await;
    let facts = facts.unwrap();
    assert!(!enabled.load(Ordering::SeqCst));
    assert!(
        snapshots.load(Ordering::SeqCst) > 0,
        "the public Run admission captured a provider"
    );
    let decision = facts
        .iter()
        .find(|fact| fact.kind == "model.turn" && fact.payload["call"]["id"] == "snapshot-probe")
        .unwrap();
    assert_eq!(
        decision.payload["call"]["needs_confirmation"], true,
        "decision and preparation must use the same frozen Run authority"
    );
    assert_eq!(requests.load(Ordering::SeqCst), 1);
    assert_eq!(executions.load(Ordering::SeqCst), 1);
    assert!(facts
        .iter()
        .any(|fact| fact.kind == "confirmation.answered" && fact.payload["approved"] == true));
    let prepared = facts
        .iter()
        .find(|fact| fact.kind == "tool.prepared")
        .unwrap();
    assert_eq!(
        prepared.payload["prepared"]["original_args_digest"],
        prepared.payload["prepared"]["effective_args_digest"]
    );
    let result = facts
        .iter()
        .find(|fact| fact.kind == "tool.result")
        .unwrap();
    assert_eq!(result.payload["receipt"]["outcome"], "succeeded");
    assert!(sent.unwrap().is_ok());
}
