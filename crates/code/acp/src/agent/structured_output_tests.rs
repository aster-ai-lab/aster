use super::*;
use a3s_code_core::llm::{
    LlmClient, LlmResponse, Message, StreamEvent, TokenUsage, ToolDefinition,
};
use a3s_code_core::store::MemorySessionStore;
use agent_client_protocol::{self as acp, Agent as _};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::mpsc;

struct StructuredModel {
    normal: &'static str,
    repaired: &'static str,
    streaming_calls: AtomicUsize,
    repair_calls: AtomicUsize,
}

#[async_trait::async_trait]
impl LlmClient for StructuredModel {
    async fn complete(
        &self,
        _messages: &[Message],
        system: Option<&str>,
        _tools: &[ToolDefinition],
    ) -> anyhow::Result<LlmResponse> {
        if system.is_some_and(|system| system.contains("Convert the completed agent answer")) {
            self.repair_calls.fetch_add(1, Ordering::SeqCst);
        }
        Ok(LlmResponse {
            message: Message::assistant(self.repaired),
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
        self.streaming_calls.fetch_add(1, Ordering::SeqCst);
        let (sender, receiver) = mpsc::channel(1);
        let response = LlmResponse {
            message: Message::assistant(self.normal),
            usage: TokenUsage::default(),
            stop_reason: None,
            token_logprobs: Vec::new(),
            meta: None,
        };
        tokio::spawn(async move {
            let _ = sender.send(StreamEvent::Done(response)).await;
        });
        Ok(receiver)
    }
}

struct Fixture {
    _root: tempfile::TempDir,
    host: A3sCodeAgent,
    session_id: String,
    model: Arc<StructuredModel>,
}

async fn fixture(normal: &'static str, repaired: &'static str) -> Fixture {
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
    let model = Arc::new(StructuredModel {
        normal,
        repaired,
        streaming_calls: AtomicUsize::new(0),
        repair_calls: AtomicUsize::new(0),
    });
    let source = core
        .session_async(
            root.path().display().to_string(),
            Some(
                SessionOptions::new()
                    .with_session_id(session_id.clone())
                    .with_session_store(Arc::new(MemorySessionStore::new()))
                    .with_llm_client(model.clone())
                    .with_auto_save(true)
                    .with_planning_mode(a3s_code_core::PlanningMode::Disabled)
                    .with_continuation(false),
            ),
        )
        .await
        .unwrap();
    let original = {
        let mut sessions = host.sessions.lock().await;
        std::mem::replace(
            &mut sessions.get_mut(&session_id).unwrap().session,
            Arc::new(source),
        )
    };
    original.close().await;
    *host.core_agent.lock().await = Some(core);
    Fixture {
        _root: root,
        host,
        session_id,
        model,
    }
}

fn schema() -> Value {
    json!({
        "type":"object",
        "additionalProperties":false,
        "required":["name"],
        "properties":{"name":{"type":"string","minLength":1}}
    })
}

async fn prompt(fixture: &Fixture, schema: Value) -> acp::Result<acp::PromptResponse> {
    fixture
        .host
        .prompt(
            acp::PromptRequest::new(
                fixture.session_id.clone(),
                vec![acp::ContentBlock::Text(acp::TextContent::new(
                    "Return the user's name",
                ))],
            )
            .meta(json!({"outputSchema":schema}).as_object().unwrap().clone()),
        )
        .await
}

#[tokio::test]
async fn valid_final_answer_returns_structured_output_without_another_model_call() {
    let fixture = fixture(r#"{"name":"Ada"}"#, r#"{"name":"repair"}"#).await;
    let response = prompt(&fixture, schema()).await.unwrap();
    assert_eq!(response.meta.unwrap()["structuredOutput"]["name"], "Ada");
    assert_eq!(fixture.model.streaming_calls.load(Ordering::SeqCst), 1);
    assert_eq!(fixture.model.repair_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn invalid_final_answer_uses_core_structured_repair_and_returns_the_object() {
    let fixture = fixture("The user's name is Ada.", r#"{"name":"Ada"}"#).await;
    let response = prompt(&fixture, schema()).await.unwrap();
    assert_eq!(response.meta.unwrap()["structuredOutput"]["name"], "Ada");
    assert_eq!(fixture.model.streaming_calls.load(Ordering::SeqCst), 1);
    assert_eq!(fixture.model.repair_calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn malformed_schema_or_metadata_is_refused_before_any_model_call() {
    for schema in [json!({"type":"not-a-real-json-schema-type"}), json!("text")] {
        let fixture = fixture("unused", "unused").await;
        let error = prompt(&fixture, schema).await.unwrap_err();
        assert!(error.to_string().contains("Invalid params"));
        assert_eq!(fixture.model.streaming_calls.load(Ordering::SeqCst), 0);
        assert_eq!(fixture.model.repair_calls.load(Ordering::SeqCst), 0);
    }
}
