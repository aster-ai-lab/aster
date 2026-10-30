use super::*;
use crate::agent::tests::test_launch;
use a3s_code_core::llm::ToolDefinition;
use a3s_code_core::llm::{LlmClient, LlmResponse, Message, StreamEvent, TokenUsage};
use a3s_code_core::store::MemorySessionStore;
use agent_client_protocol::Agent as _;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use tokio::sync::{broadcast, mpsc, Notify};

#[test]
fn every_host_method_parses_back_from_its_name() {
    for method in HostMethod::ALL {
        let wire = match method {
            HostMethod::GoalCommand => "a3s/goal/state",
            HostMethod::Review => "a3s/review/target",
            other => other.as_str(),
        };
        assert_eq!(HostMethod::parse(wire), Some(method), "{wire}");
    }
}

#[test]
fn protocol_and_unknown_methods_are_outside_the_host_set() {
    for wire in [
        "session/prompt",
        "a3s/goal",
        "a3s/goal/unknown",
        "a3s/review",
        "a3s/session",
        "a3s/session/infos",
        "a3s/permission_posture/set",
        "_meta/unknown",
        "",
    ] {
        assert_eq!(HostMethod::parse(wire), None, "{wire}");
    }
    assert_eq!(
        HostMethod::parse("a3s/compact_conversation/start"),
        Some(HostMethod::CompactConversation)
    );
    assert_eq!(
        HostMethod::parse("a3s/interject/cancel"),
        Some(HostMethod::Interject)
    );
}

struct WireModel {
    calls: AtomicUsize,
    block_third: AtomicBool,
    started: Notify,
    release: broadcast::Sender<()>,
}

impl Default for WireModel {
    fn default() -> Self {
        Self {
            calls: AtomicUsize::new(0),
            block_third: AtomicBool::new(false),
            started: Notify::new(),
            release: broadcast::channel(1).0,
        }
    }
}

#[async_trait::async_trait]
impl LlmClient for WireModel {
    async fn complete(
        &self,
        _messages: &[Message],
        _system: Option<&str>,
        _tools: &[ToolDefinition],
    ) -> anyhow::Result<LlmResponse> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        if call >= 2 && self.block_third.load(Ordering::SeqCst) {
            self.started.notify_one();
            let _ = self.release.subscribe().recv().await;
        }
        Ok(LlmResponse {
            message: Message::assistant(format!("answer-{}", call + 1).as_str()),
            usage: TokenUsage::default(),
            stop_reason: None,
            token_logprobs: Vec::new(),
            meta: None,
        })
    }

    async fn complete_streaming(
        &self,
        _messages: &[Message],
        _system: Option<&str>,
        _tools: &[ToolDefinition],
        _cancel: tokio_util::sync::CancellationToken,
    ) -> anyhow::Result<mpsc::Receiver<StreamEvent>> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        let (sender, receiver) = mpsc::channel(2);
        let blocking = call >= 2 && self.block_third.load(Ordering::SeqCst);
        let mut release = self.release.subscribe();
        tokio::spawn(async move {
            if blocking {
                let _ = release.recv().await;
            }
            let _ = sender
                .send(StreamEvent::Done(LlmResponse {
                    message: Message::assistant(format!("answer-{}", call + 1).as_str()),
                    usage: TokenUsage::default(),
                    stop_reason: None,
                    token_logprobs: Vec::new(),
                    meta: None,
                }))
                .await;
        });
        if blocking {
            self.started.notify_one();
        }
        Ok(receiver)
    }
}

struct WireFixture {
    root: tempfile::TempDir,
    host: A3sCodeAgent,
    session_id: String,
    source: Arc<AgentSession>,
    model: Arc<WireModel>,
}

async fn wire_fixture() -> WireFixture {
    wire_fixture_with_policy(None).await
}

async fn wire_fixture_with_policy(work_policy: Option<a3s_code_core::WorkPolicyV1>) -> WireFixture {
    let root = tempfile::tempdir().unwrap();
    let mut launch = test_launch(Some(root.path().join("sessions")));
    launch.workspace = root.path().to_path_buf();
    let host = A3sCodeAgent::new(launch.clone());
    let opened = host
        .new_session(
            acp::NewSessionRequest::new(root.path()).meta(
                json!({"a3sEngineering":{"policy":"none"}})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await
        .unwrap();
    let session_id = opened.session_id.0.to_string();
    let core = Agent::from_config(launch.config).await.unwrap();
    let model = Arc::new(WireModel::default());
    let mut options = SessionOptions::new()
        .with_session_id(session_id.clone())
        .with_session_store(Arc::new(MemorySessionStore::new()))
        .with_llm_client(model.clone())
        .with_auto_save(true)
        .with_planning_mode(a3s_code_core::PlanningMode::Disabled)
        .with_continuation(false);
    if let Some(policy) = work_policy {
        options = options.with_work_policy(policy);
    }
    let source = core
        .session_async(root.path().display().to_string(), Some(options))
        .await
        .unwrap();
    let source = Arc::new(source);
    let original = {
        let mut sessions = host.sessions.lock().await;
        std::mem::replace(
            &mut sessions.get_mut(&session_id).unwrap().session,
            Arc::clone(&source),
        )
    };
    original.close().await;
    *host.core_agent.lock().await = Some(core);
    source.send("question-1", None).await.unwrap();
    source.send("question-2", None).await.unwrap();
    source.save().await.unwrap();
    WireFixture {
        root,
        host,
        session_id,
        source,
        model,
    }
}

async fn ext(host: &A3sCodeAgent, method: &str, params: Value) -> acp::Result<Value> {
    let raw = serde_json::value::to_raw_value(&params).unwrap();
    let response = host
        .ext_method(acp::ExtRequest::new(method, raw.into()))
        .await?;
    Ok(serde_json::from_str(response.0.get()).unwrap())
}

#[tokio::test]
async fn pager_subagent_cancel_is_session_scoped_and_reports_observed_outcomes() {
    let fixture = wire_fixture().await;
    let calls_before = fixture.model.calls.load(Ordering::SeqCst);
    let tracker = fixture.source.subagent_tracker();
    let token = tokio_util::sync::CancellationToken::new();
    tracker
        .record_event(&AgentEvent::SubagentStart {
            task_id: "child-stop".into(),
            session_id: "child-stop".into(),
            parent_session_id: fixture.session_id.clone(),
            agent: "general".into(),
            description: "Stop child".into(),
            started_ms: 1,
        })
        .await;
    tracker
        .register_canceller("child-stop", token.clone())
        .await;
    let method = HostMethod::SubagentCancel.as_str();
    assert!(ext(
        &fixture.host,
        method,
        json!({"sessionId":"foreign","subagentId":"child-stop"})
    )
    .await
    .is_err());
    assert!(!token.is_cancelled());
    let result = ext(
        &fixture.host,
        method,
        json!({"sessionId":fixture.session_id,"subagentId":"child-stop"}),
    )
    .await
    .unwrap();
    assert_eq!(result["result"]["outcome"]["kind"], "cancelled");
    assert_eq!(result["result"]["cancelled"], true);
    assert!(token.is_cancelled());
    let again = ext(
        &fixture.host,
        method,
        json!({"sessionId":fixture.session_id,"subagentId":"child-stop"}),
    )
    .await
    .unwrap();
    assert_eq!(
        again["result"]["outcome"],
        json!({"kind":"already_finished","status":"cancelled"})
    );
    let missing = ext(
        &fixture.host,
        method,
        json!({"sessionId":fixture.session_id,"subagentId":"unknown"}),
    )
    .await
    .unwrap();
    assert_eq!(missing["result"]["outcome"]["kind"], "not_found");
    tracker
        .record_event(&AgentEvent::SubagentStart {
            task_id: "orphan".into(),
            session_id: "orphan".into(),
            parent_session_id: fixture.session_id.clone(),
            agent: "general".into(),
            description: "Orphan".into(),
            started_ms: 2,
        })
        .await;
    assert!(ext(
        &fixture.host,
        method,
        json!({"sessionId":fixture.session_id,"subagentId":"orphan"})
    )
    .await
    .is_err());
    assert_eq!(fixture.model.calls.load(Ordering::SeqCst), calls_before);
}

#[tokio::test]
async fn text_only_model_refuses_images_before_a_model_or_run_is_admitted() {
    let mut fixture = wire_fixture().await;
    let calls_before = fixture.model.calls.load(Ordering::SeqCst);
    let id = fixture.host.sessions.lock().await[&fixture.session_id]
        .model_id
        .clone();
    let mut configured = false;
    for provider in &mut fixture.host.launch.config.providers {
        for model in &mut provider.models {
            if format!("{}/{}", provider.name, model.id) == id {
                model.modalities.input = vec!["text".into()];
                configured = true;
            }
        }
    }
    assert!(configured);
    let error = fixture
        .host
        .prompt(acp::PromptRequest::new(
            acp::SessionId::new(fixture.session_id.clone()),
            vec![acp::ContentBlock::Image(acp::ImageContent::new(
                "AQID",
                "image/png",
            ))],
        ))
        .await
        .unwrap_err();
    assert!(error
        .data
        .unwrap()
        .to_string()
        .contains("does not support image input"));
    assert_eq!(fixture.model.calls.load(Ordering::SeqCst), calls_before);
}
