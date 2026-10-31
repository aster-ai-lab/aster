//! Hermetic round trip for the Codex account path: the factory arm, the
//! exact request the fixed Responses endpoint would receive, and the SSE
//! frames it parses. The transport is injected (`http_override`), so
//! nothing leaves the process — the endpoint is asserted, never dialed.

use std::pin::Pin;
use std::sync::{Arc, Mutex};

use aster_core::llm::factory::create_client_with_config;
use aster_core::llm::http::{HttpClient, HttpResponse, StreamingHttpResponse};
use aster_core::llm::{
    ContentBlock, LlmConfig, LlmResponse, Message, SecretString, ToolDefinition,
};
use tokio_util::sync::CancellationToken;

type Hit = (String, Vec<(String, String)>, serde_json::Value);

struct RecordingTransport {
    hits: Mutex<Vec<Hit>>,
    /// Raw SSE bytes the endpoint answers with.
    reply: String,
    status: u16,
}

impl RecordingTransport {
    fn ok(reply: &str) -> Self {
        Self {
            hits: Mutex::new(Vec::new()),
            reply: reply.to_string(),
            status: 200,
        }
    }
}

#[async_trait::async_trait]
impl HttpClient for RecordingTransport {
    async fn post(
        &self,
        _url: &str,
        _headers: Vec<(&str, &str)>,
        _body: &serde_json::Value,
        _cancel_token: CancellationToken,
    ) -> anyhow::Result<HttpResponse> {
        unreachable!("the Codex account client only streams")
    }

    async fn post_streaming(
        &self,
        url: &str,
        headers: Vec<(&str, &str)>,
        body: &serde_json::Value,
        _cancel_token: CancellationToken,
    ) -> anyhow::Result<StreamingHttpResponse> {
        let owned: Vec<(String, String)> = headers
            .into_iter()
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect();
        self.hits
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .push((url.to_string(), owned, body.clone()));
        Ok(StreamingHttpResponse {
            status: self.status,
            retry_after: None,
            byte_stream: Pin::from(Box::pin(futures::stream::iter(vec![Ok(
                bytes::Bytes::from(self.reply.clone()),
            )]))),
            error_body: String::new(),
        })
    }
}

fn sse_reply() -> String {
    [
        r#"data: {"type":"response.created","response":{"id":"resp_1"}}"#,
        "",
        r#"data: {"type":"response.reasoning_text.delta","delta":"thinking"}"#,
        "",
        r#"data: {"type":"response.output_text.delta","delta":"fixture "}"#,
        "",
        r#"data: {"type":"response.output_text.delta","delta":"reply"}"#,
        "",
        r#"data: {"type":"response.output_item.added","item":{"type":"function_call","id":"fc_1","call_id":"call_1","name":"lookup"}}"#,
        "",
        r#"data: {"type":"response.function_call_arguments.delta","item_id":"fc_1","delta":"{\"q\":\"x\"}"}"#,
        "",
        r#"data: {"type":"response.output_item.done","item":{"type":"reasoning","encrypted_content":"enc-1"}}"#,
        "",
        r#"data: {"type":"response.output_item.done","item":{"type":"function_call","id":"fc_1","call_id":"call_1","name":"lookup","arguments":"{\"q\":\"x\"}"}}"#,
        "",
        r#"data: {"type":"response.completed","response":{"id":"resp_1","usage":{"input_tokens":11,"output_tokens":7,"total_tokens":18,"input_tokens_details":{"cached_tokens":3}}}}"#,
        "",
        "",
    ]
    .join("\n")
}

fn codex_config(transport: Arc<RecordingTransport>) -> LlmConfig {
    let mut headers = std::collections::HashMap::new();
    headers.insert("chatgpt-account-id".to_string(), "acc-7".to_string());
    LlmConfig {
        provider: "codex".to_string(),
        model: "gpt-6.1-sol".to_string(),
        api_key: SecretString::new("fixture-codex-bearer"),
        headers,
        session_id: Some("sess-42".to_string()),
        reasoning_effort: Some("medium".to_string()),
        http_override: Some(transport as Arc<dyn HttpClient>),
        ..LlmConfig::default()
    }
}

fn user_turn() -> Vec<Message> {
    vec![Message {
        role: "user".to_string(),
        content: vec![ContentBlock::Text {
            text: "look it up".to_string(),
        }],
        reasoning_content: None,
        transcript_text: None,
        transcript_visibility: Default::default(),
    }]
}

fn lookup_tool() -> Vec<ToolDefinition> {
    vec![ToolDefinition {
        name: "lookup".to_string(),
        description: "look something up".to_string(),
        parameters: serde_json::json!({"type":"object","properties":{"q":{"type":"string"}}}),
    }]
}

fn header<'a>(hit: &'a Hit, name: &str) -> Option<&'a str> {
    hit.1
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.as_str())
}

#[tokio::test]
async fn the_factory_sends_the_account_request_and_parses_the_stream() {
    let transport = Arc::new(RecordingTransport::ok(&sse_reply()));
    let client = create_client_with_config(codex_config(transport.clone()));

    let response: LlmResponse = client
        .complete(&user_turn(), Some("be brief"), &lookup_tool())
        .await
        .expect("fixture turn");

    let hits = transport
        .hits
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    assert_eq!(hits.len(), 1, "one request per turn");
    let hit = &hits[0];

    // The endpoint is the account contract, never redirected.
    assert_eq!(hit.0, "https://chatgpt.com/backend-api/codex/responses");
    assert_eq!(
        header(hit, "Authorization"),
        Some("Bearer fixture-codex-bearer")
    );
    assert_eq!(header(hit, "chatgpt-account-id"), Some("acc-7"));
    assert_eq!(header(hit, "originator"), Some("codex_cli_rs"));
    assert_eq!(header(hit, "session_id"), Some("sess-42"));
    assert_eq!(header(hit, "OpenAI-Beta"), Some("responses=experimental"));
    assert_eq!(header(hit, "Accept"), Some("text/event-stream"));
    assert!(header(hit, "User-Agent").is_some_and(|agent| agent.starts_with("codex_cli_rs")));

    let body = &hit.2;
    assert_eq!(body["model"], "gpt-6.1-sol");
    assert_eq!(body["instructions"], "be brief");
    assert_eq!(body["store"], false);
    assert_eq!(body["prompt_cache_key"], "sess-42");
    assert_eq!(body["reasoning"]["effort"], "medium");
    assert_eq!(body["include"][0], "reasoning.encrypted_content");
    assert_eq!(body["tools"][0]["type"], "function");
    assert_eq!(body["tools"][0]["name"], "lookup");
    assert_eq!(
        body["tools"][0]["parameters"]["properties"]["q"]["type"],
        "string"
    );

    // The parsed terminal response carries text, the tool call, reasoning,
    // and usage — the shapes the TUI turn loop consumes.
    let blocks = &response.message.content;
    match &blocks[0] {
        ContentBlock::Text { text } => assert_eq!(text, "fixture reply"),
        other => panic!("expected text, got {other:?}"),
    }
    match &blocks[1] {
        ContentBlock::ToolUse { id, name, input } => {
            assert_eq!(id, "call_1");
            assert_eq!(name, "lookup");
            assert_eq!(input["q"], "x");
        }
        other => panic!("expected a tool call block, got {other:?}"),
    }
    assert_eq!(
        response.message.reasoning_content.as_deref(),
        Some("thinking")
    );
    assert_eq!(response.usage.prompt_tokens, 11);
    assert_eq!(response.usage.completion_tokens, 7);
    assert_eq!(response.usage.total_tokens, 18);
    assert_eq!(response.usage.cache_read_tokens, Some(3));
    assert_eq!(response.stop_reason.as_deref(), Some("tool_calls"));
}

#[tokio::test]
async fn an_empty_account_id_omits_the_account_header() {
    let transport = Arc::new(RecordingTransport::ok(&sse_reply()));
    let mut config = codex_config(transport.clone());
    config.headers.clear();
    config.reasoning_effort = None;
    let client = create_client_with_config(config);

    let response = client
        .complete(&user_turn(), None, &[])
        .await
        .expect("fixture turn");
    match &response.message.content[0] {
        ContentBlock::Text { text } => assert_eq!(text, "fixture reply"),
        other => panic!("expected text, got {other:?}"),
    }

    let hits = transport
        .hits
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    assert_eq!(header(&hits[0], "chatgpt-account-id"), None);
    assert_eq!(
        header(&hits[0], "Authorization"),
        Some("Bearer fixture-codex-bearer")
    );
}

#[tokio::test]
async fn an_auth_failure_is_a_terminal_refusal_not_a_retry_loop() {
    let transport = Arc::new(RecordingTransport {
        hits: Mutex::new(Vec::new()),
        reply: String::new(),
        status: 401,
    });
    let client = create_client_with_config(codex_config(transport.clone()));

    let error = client
        .complete(&user_turn(), None, &[])
        .await
        .expect_err("401 must refuse");
    assert!(error.to_string().contains("codex"), "{error:#}");
    let hits = transport
        .hits
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    assert_eq!(hits.len(), 1, "a terminal status is not retried");
}
