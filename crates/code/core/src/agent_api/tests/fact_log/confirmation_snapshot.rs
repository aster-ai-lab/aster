#![cfg(test)]

use super::*;
use crate::hitl::{ConfirmationPolicy, ConfirmationProvider, ConfirmationResponse};
use serde_json::{json, Value};
use std::sync::atomic::AtomicBool;
use tokio::sync::oneshot;

#[tokio::test]
async fn confirmed_usage_is_saved_before_waiting_for_host_input() {
    use crate::store::{FileSessionStore, SessionStore};

    for question in [false, true] {
        let workspace = tempfile::tempdir().unwrap();
        let storage = tempfile::tempdir().unwrap();
        let store = Arc::new(FileSessionStore::new(storage.path()).await.unwrap());
        let mut config = test_config();
        config.memory = Some(crate::memory::MemoryConfig {
            llm_extraction: false,
            ..Default::default()
        });
        let agent = crate::Agent::from_config(config).await.unwrap();
        let mut response = if question {
            tool_response(
                "pending-question",
                "ask_user",
                json!({
                    "question":"Continue?", "allow_free_text":true,
                }),
            )
        } else {
            tool_response(
                "pending-write",
                "write",
                json!({
                    "file_path":"not-approved.txt", "content":"must not be written",
                }),
            )
        };
        response.usage = TokenUsage {
            prompt_tokens: 13,
            completion_tokens: 6,
            total_tokens: 19,
            cache_read_tokens: Some(8),
            ..Default::default()
        };
        let client = Arc::new(ScriptedClient {
            seen: Default::default(),
            responses: std::sync::Mutex::new(vec![response, text_response("unused")]),
        });
        let (broadcast, _) = tokio::sync::broadcast::channel(8);
        let session = agent
            .build_session(
                workspace.path().to_string_lossy().into_owned(),
                client.clone(),
                &crate::SessionOptions::new()
                    .with_session_id("pending-input-usage")
                    .with_session_store(store.clone())
                    .with_auto_save(true)
                    .with_confirmation_manager(Arc::new(crate::hitl::ConfirmationManager::new(
                        ConfirmationPolicy::enabled(),
                        broadcast,
                    )))
                    .with_permission_policy(crate::permissions::PermissionPolicy::new())
                    .with_planning_mode(crate::PlanningMode::Disabled)
                    .with_auto_delegation_enabled(false)
                    .with_manual_delegation_enabled(false)
                    .with_continuation(false),
            )
            .unwrap();
        let (mut events, worker) = session
            .stream("Run the controlled host input probe", None)
            .await
            .unwrap();
        let presented = tokio::time::timeout(std::time::Duration::from_secs(10), async {
            while let Some(event) = events.recv().await {
                if matches!(
                    (question, event),
                    (true, crate::agent::AgentEvent::UserQuestion { .. })
                        | (false, crate::agent::AgentEvent::ConfirmationRequired { .. })
                ) {
                    return true;
                }
            }
            false
        })
        .await
        .unwrap_or(false);
        let live = session.recorded_usage_tokens().2;
        let reopened = FileSessionStore::new(storage.path()).await.unwrap();
        let saved_before_answer = reopened.load_snapshot("pending-input-usage").await.unwrap();
        let drain = tokio::spawn(async move { while events.recv().await.is_some() {} });
        let cancelled = session.cancel().await;
        tokio::time::timeout(std::time::Duration::from_secs(10), worker)
            .await
            .unwrap()
            .unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(10), drain)
            .await
            .unwrap()
            .unwrap();
        session.close().await;
        agent.close().await;

        assert!(presented && cancelled, "question={question}");
        assert_eq!(client.seen.lock().unwrap().len(), 1);
        assert_eq!(live, 19);
        assert!(!workspace.path().join("not-approved.txt").exists());
        let saved = saved_before_answer
            .expect("host input must not appear before its confirmed usage is saved");
        assert_eq!(
            saved.session.total_usage.total_tokens, 19,
            "question={question}"
        );
        assert_eq!(saved.session.total_usage.cache_read_tokens, Some(8));
    }
}

#[derive(Clone)]
struct SnapshotConfirmation {
    enabled: Arc<AtomicBool>,
    frozen: Option<bool>,
    approved: bool,
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
                approved: self.approved,
                reason: Some("controlled host decision".into()),
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
                    approved: true,
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
        .find(|fact| {
            fact.kind == "model.turn" && fact.payload["call"]["name"] == "confirmation_probe"
        })
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

#[tokio::test]
async fn a_rejected_fact_confirmation_publishes_a_terminal_without_executing() {
    let workspace = tempfile::tempdir().unwrap();
    let agent = crate::Agent::from_config(test_config()).await.unwrap();
    let executions = Arc::new(AtomicUsize::new(0));
    let requests = Arc::new(AtomicUsize::new(0));
    let session = agent
        .build_session(
            workspace.path().to_str().unwrap().to_owned(),
            Arc::new(ScriptedClient {
                seen: Default::default(),
                responses: std::sync::Mutex::new(vec![tool_response(
                    "rejected-probe",
                    "confirmation_probe",
                    json!({}),
                )]),
            }),
            &crate::SessionOptions::new()
                .with_session_id("rejected-confirmation-terminal")
                .with_confirmation_manager(Arc::new(SnapshotConfirmation {
                    enabled: Arc::new(AtomicBool::new(true)),
                    frozen: None,
                    approved: false,
                    requests: requests.clone(),
                    snapshots: Default::default(),
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
    let (mut receiver, worker) = session
        .stream("Request the controlled probe.", None)
        .await
        .unwrap();
    let events = tokio::time::timeout(std::time::Duration::from_secs(15), async {
        let mut events = Vec::new();
        while let Some(event) = receiver.recv().await {
            events.push(event);
        }
        worker.await.unwrap();
        events
    })
    .await
    .unwrap();
    let facts = read_workspace_facts(workspace.path(), session.session_id()).unwrap();
    session.close().await;
    agent.close().await;
    assert_eq!(requests.load(Ordering::SeqCst), 1);
    assert_eq!(executions.load(Ordering::SeqCst), 0);
    assert!(!facts
        .iter()
        .any(|fact| fact.kind == "tool.prepared" || fact.kind == "tool.result"));
    let (required, rejected_id) = events
        .iter()
        .enumerate()
        .find_map(|(index, event)| match event {
            crate::AgentEvent::ConfirmationRequired {
                tool_id, tool_name, ..
            } if tool_name == "confirmation_probe" => Some((index, tool_id)),
            _ => None,
        })
        .expect("the real confirmation must be announced");
    let terminal: Vec<_> = events
        .iter()
        .enumerate()
        .filter(|(_, event)| {
            matches!(event,
                crate::AgentEvent::PermissionDenied { tool_id, .. } if tool_id == rejected_id
            )
        })
        .collect();
    assert_eq!(
        terminal.len(),
        1,
        "every announced rejected call needs exactly one terminal permission event"
    );
    assert!(required < terminal[0].0);
    assert!(events
        .iter()
        .skip(terminal[0].0 + 1)
        .any(|event| matches!(event, crate::AgentEvent::End { .. })));
    assert!(!events.iter().any(|event| matches!(event,
        crate::AgentEvent::ToolExecutionStart { id, .. } if id == rejected_id
    )));
}
