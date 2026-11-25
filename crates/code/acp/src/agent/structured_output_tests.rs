use super::*;
use agent_client_protocol::{self as acp, Agent as _};
use aster_core::llm::{
    ContentBlock, LlmClient, LlmResponse, Message, StreamEvent, TokenUsage, ToolDefinition,
};
use aster_core::store::MemorySessionStore;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::mpsc;

struct StructuredModel {
    normal: &'static str,
    repaired: &'static str,
    denied_tool: bool,
    streaming_calls: AtomicUsize,
    repair_calls: AtomicUsize,
    schema_prompts: AtomicUsize,
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
        messages: &[Message],
        _system: Option<&str>,
        _tools: &[ToolDefinition],
        _cancel: tokio_util::sync::CancellationToken,
    ) -> anyhow::Result<mpsc::Receiver<StreamEvent>> {
        if messages
            .iter()
            .any(|message| message.text().contains("additionalProperties"))
        {
            assert!(messages
                .iter()
                .any(|message| message.text().contains("action source_lines")));
            self.schema_prompts.fetch_add(1, Ordering::SeqCst);
        }
        let call = self.streaming_calls.fetch_add(1, Ordering::SeqCst) + 1;
        let message = if self.denied_tool && call == 1 {
            let mut message = Message::assistant("");
            message.content = vec![ContentBlock::ToolUse {
                id: "denied-bash".into(),
                name: "bash".into(),
                input: json!({"command": "printf denied"}),
            }];
            message
        } else {
            Message::assistant(self.normal)
        };
        let (sender, receiver) = mpsc::channel(1);
        let response = LlmResponse {
            message,
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
    host: AsterCodeAgent,
    session_id: String,
    model: Arc<StructuredModel>,
}

async fn fixture(normal: &'static str, repaired: &'static str) -> Fixture {
    fixture_with_options(normal, repaired, false).await
}

async fn fixture_with_options(
    normal: &'static str,
    repaired: &'static str,
    denied_tool: bool,
) -> Fixture {
    let root = tempfile::tempdir().unwrap();
    let mut launch = test_launch(Some(root.path().join("sessions")));
    launch.workspace = root.path().to_path_buf();
    let host = AsterCodeAgent::new(launch.clone());
    let opened = host
        .new_session(
            acp::NewSessionRequest::new(root.path()).meta(
                json!({"asterEngineering":{"policy":"none"}})
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
        denied_tool,
        streaming_calls: AtomicUsize::new(0),
        repair_calls: AtomicUsize::new(0),
        schema_prompts: AtomicUsize::new(0),
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
                    .with_planning_mode(aster_core::PlanningMode::Disabled)
                    .with_continuation(false)
                    .with_permission_policy(
                        aster_core::permissions::PermissionPolicy::new().allow("read"),
                    ),
            ),
        )
        .await
        .unwrap();
    let original = {
        let mut sessions = host.sessions.lock().await;
        let live = sessions.get_mut(&session_id).unwrap();
        let original = std::mem::replace(&mut live.session, Arc::new(source));
        live.set_posture(PermissionPosture::Default, vec!["bash".into()])
            .unwrap();
        original
    };
    original.close().await;
    *host.core_agent.lock().await = Some(Arc::new(core));
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
    tokio::task::LocalSet::new()
        .run_until(
            fixture.host.prompt(
                acp::PromptRequest::new(
                    fixture.session_id.clone(),
                    vec![acp::ContentBlock::Text(acp::TextContent::new(
                        "Return the user's name",
                    ))],
                )
                .meta(json!({"outputSchema":schema}).as_object().unwrap().clone()),
            ),
        )
        .await
}

#[tokio::test(flavor = "current_thread")]
async fn valid_final_answer_returns_structured_output_without_another_model_call() {
    let fixture = fixture(r#"{"name":"Ada"}"#, r#"{"name":"repair"}"#).await;
    let response = prompt(&fixture, schema()).await.unwrap();
    assert_eq!(response.meta.unwrap()["structuredOutput"]["name"], "Ada");
    assert_eq!(fixture.model.streaming_calls.load(Ordering::SeqCst), 1);
    assert_eq!(fixture.model.repair_calls.load(Ordering::SeqCst), 0);
    assert_eq!(fixture.model.schema_prompts.load(Ordering::SeqCst), 1);
}

#[tokio::test(flavor = "current_thread")]
async fn valid_final_answer_preserves_nested_evidence_verbatim() {
    let answer =
        r#"{"finding":{"file":"pkg/setup.py","line":9,"code":"    subprocess.Popen(...)"}}"#;
    let fixture = fixture(
        answer,
        r#"{"finding":{"file":"wrong","line":1,"code":"wrong"}}"#,
    )
    .await;
    let evidence_schema = json!({
        "type":"object",
        "additionalProperties":false,
        "required":["finding"],
        "properties":{"finding":{
            "type":"object",
            "additionalProperties":false,
            "required":["file","line","code"],
            "properties":{
                "file":{"type":"string"},
                "line":{"type":"integer"},
                "code":{"type":"string"}
            }
        }}
    });
    let response = prompt(&fixture, evidence_schema).await.unwrap();
    assert_eq!(
        response.meta.unwrap()["structuredOutput"]["finding"],
        json!({"file":"pkg/setup.py","line":9,"code":"    subprocess.Popen(...)"})
    );
    assert_eq!(fixture.model.repair_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test(flavor = "current_thread")]
async fn invalid_final_answer_is_an_error_without_another_model_call() {
    let fixture = fixture("The user's name is Ada.", r#"{"name":"Ada"}"#).await;
    let response = prompt(&fixture, schema()).await.unwrap();
    let meta = response.meta.unwrap();
    assert!(meta.get("structuredOutput").is_none());
    assert!(meta.get("structuredOutputError").is_some());
    assert_eq!(fixture.model.streaming_calls.load(Ordering::SeqCst), 1);
    assert_eq!(fixture.model.repair_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test(flavor = "current_thread")]
async fn denied_tool_prevents_structured_output_even_when_final_text_is_valid() {
    let fixture = fixture_with_options(r#"{"name":"Ada"}"#, r#"{"name":"repair"}"#, true).await;
    let response = prompt(&fixture, schema()).await.unwrap();
    let meta = response.meta.unwrap();
    assert!(meta.get("structuredOutput").is_none());
    assert!(meta["structuredOutputError"]
        .as_str()
        .is_some_and(|error| error.contains("denied")));
    assert_eq!(fixture.model.repair_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test(flavor = "current_thread")]
async fn malformed_schema_or_metadata_is_refused_before_any_model_call() {
    for schema in [json!({"type":"not-a-real-json-schema-type"}), json!("text")] {
        let fixture = fixture("unused", "unused").await;
        let error = prompt(&fixture, schema).await.unwrap_err();
        assert!(error.to_string().contains("Invalid params"));
        assert_eq!(fixture.model.streaming_calls.load(Ordering::SeqCst), 0);
        assert_eq!(fixture.model.repair_calls.load(Ordering::SeqCst), 0);
    }
}
