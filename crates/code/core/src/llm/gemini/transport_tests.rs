use super::*;
use crate::config::ProviderProtocol;
use crate::llm::{
    create_client_with_config, structured, HttpClient, HttpResponse, LlmClient, LlmConfig,
    NonRetryableLlmError, StreamEvent, StreamingHttpResponse, ToolDefinition,
};
use anyhow::Result;
use async_trait::async_trait;
use bytes::Bytes;
use futures::stream;
use serde_json::Value;
use std::collections::VecDeque;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use tokio_util::sync::CancellationToken;

type Call = (String, Vec<(String, String)>, Value);

#[derive(Default)]
struct Fixture {
    calls: Mutex<Vec<Call>>,
    replies: Mutex<VecDeque<StreamingHttpResponse>>,
    header_tokens: Mutex<Vec<CancellationToken>>,
}

#[async_trait]
impl HttpClient for Fixture {
    async fn post(
        &self,
        url: &str,
        headers: Vec<(&str, &str)>,
        body: &Value,
        _cancel: CancellationToken,
    ) -> Result<HttpResponse> {
        self.record(url, headers, body);
        anyhow::bail!("Gemini uses bounded response streaming for both HTTP methods")
    }
    async fn post_streaming(
        &self,
        url: &str,
        headers: Vec<(&str, &str)>,
        body: &Value,
        cancel: CancellationToken,
    ) -> Result<StreamingHttpResponse> {
        self.record(url, headers, body);
        if let Some(reply) = self.replies.lock().unwrap().pop_front() {
            return Ok(reply);
        }
        self.header_tokens.lock().unwrap().push(cancel.clone());
        cancel.cancelled().await;
        Err(crate::llm::HttpClientError::cancelled("fixture headers").into())
    }
}

impl Fixture {
    fn record(&self, url: &str, headers: Vec<(&str, &str)>, body: &Value) {
        self.calls.lock().unwrap().push((
            url.into(),
            headers
                .into_iter()
                .map(|(key, value)| (key.into(), value.into()))
                .collect(),
            body.clone(),
        ));
    }
    fn client(self: &Arc<Self>) -> Arc<dyn LlmClient> {
        create_client_with_config(self.config())
    }
    fn config(self: &Arc<Self>) -> LlmConfig {
        let mut config = LlmConfig::new("provider-a", "stored/model", "synthetic-provider-key")
            .with_protocol(ProviderProtocol::Gemini)
            .with_base_url("https://fixture.invalid/proxy/v1beta?tenant=a%20b&tenant=c&alt=json");
        config.http_override = Some(self.clone());
        config.retry_config = Some(crate::retry::RetryConfig {
            max_retries: 0,
            base_delay_ms: 0,
            max_delay_ms: 0,
            ..Default::default()
        });
        config
    }
    fn json(&self, status: u16, value: Value) {
        self.replies
            .lock()
            .unwrap()
            .push_back(reply(status, vec![Ok(Bytes::from(value.to_string()))]));
    }
}

fn reply(status: u16, chunks: Vec<Result<Bytes>>) -> StreamingHttpResponse {
    StreamingHttpResponse {
        status,
        retry_after: None,
        byte_stream: Box::pin(stream::iter(chunks)),
        error_body: "synthetic-private-provider-error".into(),
    }
}

fn response(parts: Vec<Value>) -> Value {
    json!({"responseId":"response-1","modelVersion":"actual-api-model","candidates":[{"index":0,"content":{"role":"model","parts":parts},"finishReason":"STOP"}],"usageMetadata":{"promptTokenCount":10,"toolUsePromptTokenCount":3,"candidatesTokenCount":6,"thoughtsTokenCount":4,"cachedContentTokenCount":4,"totalTokenCount":23}})
}

fn tool() -> ToolDefinition {
    ToolDefinition {
        name: "lookup".into(),
        description: "Find a record".into(),
        parameters: json!({"type":"object","properties":{"q":{"type":"string"}},"additionalProperties":false}),
    }
}

#[tokio::test]
async fn factory_native_json_replays_persisted_parts_and_links_parallel_tool_results() {
    let fixture = Arc::new(Fixture::default());
    fixture.json(200, response(parts()));
    fixture.json(200, response(vec![json!({"text":"done"})]));
    let client = fixture.client();
    let first = client
        .complete(&[Message::user("hello")], Some("system"), &[tool()])
        .await
        .unwrap();
    assert_eq!(
        (
            first.usage.prompt_tokens,
            first.usage.completion_tokens,
            first.usage.total_tokens
        ),
        (13, 10, 23)
    );
    assert_eq!(first.usage.cache_read_tokens, Some(4));
    assert_eq!(first.stop_reason.as_deref(), Some("tool_use"));
    let calls = first.tool_calls();
    let restored: Message =
        serde_json::from_slice(&serde_json::to_vec(&first.message).unwrap()).unwrap();
    let history = vec![
        Message::user("hello"),
        restored,
        Message::tool_result(&calls[0].id, "result", false),
        Message::tool_result(&calls[1].id, "failure", true),
    ];
    let last = client
        .complete(&history, Some("system"), &[tool()])
        .await
        .unwrap();
    assert_eq!(last.text(), "done");
    assert_eq!(
        last.meta.as_ref().unwrap().response_model.as_deref(),
        Some("actual-api-model")
    );
    assert!(!last
        .meta
        .as_ref()
        .unwrap()
        .request_url
        .as_ref()
        .unwrap()
        .contains('?'));
    let sent = fixture.calls.lock().unwrap();
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[0].0,"https://fixture.invalid/proxy/v1beta/models/stored%2Fmodel:generateContent?tenant=a+b&tenant=c");
    assert_eq!(
        sent[0]
            .1
            .iter()
            .find(|(key, _)| key == "x-goog-api-key")
            .unwrap()
            .1,
        "synthetic-provider-key"
    );
    assert!(sent[0].2.get("model").is_none() && sent[0].2.get("stream").is_none());
    assert_eq!(sent[1].2["contents"][1]["parts"], json!(parts()));
    let results = &sent[1].2["contents"][2]["parts"];
    assert_eq!(
        results[0]["functionResponse"],
        json!({"name":"lookup","response":{"output":"result"}})
    );
    assert_eq!(
        results[1]["functionResponse"],
        json!({"id":"vendor-call","name":"other","response":{"error":"failure"}})
    );
    assert_eq!(
        sent[0].2["tools"][0]["functionDeclarations"][0]["parametersJsonSchema"],
        tool().parameters
    );
    assert!(!sent[1].2.to_string().contains("gemini_state"));
}

#[tokio::test]
async fn native_stream_preserves_fragmented_signatures_call_ids_and_trailing_usage() {
    for size in [1, 7, usize::MAX] {
        let fixture = Arc::new(Fixture::default());
        let full = response(parts());
        let wire = format!(
            "data: {}\r\n\r\ndata: {}\r\n\r\ndata: {}\r\n\r\n",
            json!({"responseId":"response-1","candidates":[{"index":0,"content":full["candidates"][0]["content"]}]}),
            json!({"candidates":[{"index":0,"finishReason":"STOP"}]}),
            json!({"usageMetadata":full["usageMetadata"]})
        );
        fixture.replies.lock().unwrap().push_back(reply(
            200,
            wire.as_bytes()
                .chunks(size)
                .map(|bytes| Ok(Bytes::copy_from_slice(bytes)))
                .collect(),
        ));
        let cancel = CancellationToken::new();
        let mut events = fixture
            .client()
            .complete_streaming(&[Message::user("hello")], None, &[tool()], cancel.clone())
            .await
            .unwrap();
        let mut ids = Vec::new();
        let mut text = String::new();
        let mut reasoning = String::new();
        let mut final_response = None;
        while let Some(event) = events.recv().await {
            match event {
                StreamEvent::TextDelta(delta) => text.push_str(&delta),
                StreamEvent::ReasoningDelta(delta) => reasoning.push_str(&delta),
                StreamEvent::ToolUseStart { id, .. } => ids.push(id),
                StreamEvent::Done(response) => {
                    assert!(final_response.is_none());
                    final_response = Some(response);
                }
                _ => {}
            }
        }
        assert_eq!((text.as_str(), reasoning.as_str()), ("answer", "why"));
        let final_response = final_response.unwrap();
        assert_eq!(
            final_response
                .tool_calls()
                .iter()
                .map(|call| call.id.clone())
                .collect::<Vec<_>>(),
            ids
        );
        assert_eq!(final_response.usage.total_tokens, 23);
        let ContentBlock::GeminiState { state } = final_response.message.content.last().unwrap()
        else {
            panic!("state missing")
        };
        assert_eq!(
            state
                .replay(&final_response.message, "provider-a", "stored/model")
                .unwrap(),
            parts()
        );
        assert!(fixture.calls.lock().unwrap()[0]
            .0
            .ends_with("streamGenerateContent?tenant=a+b&tenant=c&alt=sse"));
        assert!(!cancel.is_cancelled());
    }
}

#[tokio::test]
async fn native_stream_failure_never_replays_after_visible_tool_output() {
    let fixture = Arc::new(Fixture::default());
    let prefix = format!(
        "data: {}\n\n",
        json!({"candidates":[{"index":0,"content":{"parts":[{"functionCall":{"name":"lookup","args":{}},"thoughtSignature":"opaque"}]}}]})
    );
    fixture.replies.lock().unwrap().push_back(reply(
        200,
        vec![
            Ok(Bytes::from(prefix)),
            Err(anyhow::anyhow!("synthetic-private-error")),
        ],
    ));
    fixture.json(200, response(vec![json!({"text":"must not run"})]));
    let cancel = CancellationToken::new();
    let mut events = fixture
        .client()
        .complete_streaming(&[Message::user("hello")], None, &[tool()], cancel.clone())
        .await
        .unwrap();
    let mut starts = 0;
    while let Some(event) = events.recv().await {
        match event {
            StreamEvent::ToolUseStart { .. } => starts += 1,
            StreamEvent::Done(_) => panic!("failed stream returned Done"),
            _ => {}
        }
    }
    assert_eq!(starts, 1);
    assert_eq!(fixture.calls.lock().unwrap().len(), 1);
    assert!(!cancel.is_cancelled());
}

#[tokio::test]
async fn native_requests_honor_structured_modes_thinking_and_hard_output_caps() {
    let fixture = Arc::new(Fixture::default());
    let mut config = fixture.config();
    config.reasoning_capabilities = Some(
        crate::config::ReasoningCapabilities::new(crate::config::ReasoningMode::Budget, Vec::new())
            .unwrap(),
    );
    config.thinking_budget = Some(0);
    config.max_tokens = Some(8);
    let client = create_client_with_config(config)
        .with_output_cap(16)
        .unwrap();
    for directive in [
        structured::StructuredDirective {
            force_tool: Some("lookup".into()),
            ..Default::default()
        },
        structured::StructuredDirective {
            response_format: Some(structured::ResponseFormat::JsonSchema {
                name: "test".into(),
                schema: json!({"type":"object","properties":{"ok":{"type":"boolean"}}}),
            }),
            ..Default::default()
        },
    ] {
        fixture.json(200, response(vec![json!({"text":"{}"})]));
        client
            .complete_structured(&[Message::user("hello")], None, &[tool()], &directive)
            .await
            .unwrap();
    }
    let calls = fixture.calls.lock().unwrap();
    for (_, _, request) in calls.iter() {
        assert_eq!(request["generationConfig"]["maxOutputTokens"], 8);
        assert_eq!(request["generationConfig"]["candidateCount"], 1);
        assert_eq!(
            request["generationConfig"]["thinkingConfig"]["thinkingBudget"],
            0
        );
    }
    assert_eq!(
        calls[0].2["toolConfig"]["functionCallingConfig"],
        json!({"mode":"ANY","allowedFunctionNames":["lookup"]})
    );
    assert_eq!(
        calls[1].2["generationConfig"]["responseMimeType"],
        "application/json"
    );
    assert_eq!(
        calls[1].2["generationConfig"]["responseJsonSchema"]["type"],
        "object"
    );
    assert!(client.with_output_cap(0).is_none());
}

#[tokio::test]
async fn every_foreign_client_refuses_native_state_before_transport() {
    for protocol in [
        ProviderProtocol::Openai,
        ProviderProtocol::Anthropic,
        ProviderProtocol::OpenaiResponses,
        ProviderProtocol::Codex,
        ProviderProtocol::Cursor,
        ProviderProtocol::Glm,
    ] {
        let fixture = Arc::new(Fixture::default());
        let mut config = fixture.config();
        config.protocol = protocol;
        let client = create_client_with_config(config);
        let history = [GeminiState::message(
            "provider-a",
            "stored/model",
            vec![json!({"text":"old","thoughtSignature":"synthetic-secret"})],
        )
        .unwrap()];
        let error = client.complete(&history, None, &[]).await.unwrap_err();
        assert!(
            error.downcast_ref::<NonRetryableLlmError>().is_some(),
            "{protocol:?}: {error:#}"
        );
        assert!(client
            .complete_streaming(&history, None, &[], CancellationToken::new())
            .await
            .is_err());
        assert!(client
            .complete_structured(&history, None, &[], &Default::default())
            .await
            .is_err());
        assert!(client
            .complete_streaming_structured(
                &history,
                None,
                &[],
                &Default::default(),
                CancellationToken::new()
            )
            .await
            .is_err());
        assert!(fixture.calls.lock().unwrap().is_empty(), "{protocol:?}");
    }
}

struct Closed(Arc<AtomicBool>);
impl Drop for Closed {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

#[tokio::test]
async fn cancelling_headers_or_dropping_the_receiver_releases_native_io() {
    let fixture = Arc::new(Fixture::default());
    let client = fixture.client();
    let cancel = CancellationToken::new();
    let task = {
        let client = client.clone();
        let cancel = cancel.clone();
        tokio::spawn(async move {
            client
                .complete_streaming(&[Message::user("hello")], None, &[], cancel)
                .await
        })
    };
    tokio::task::yield_now().await;
    cancel.cancel();
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(1), task)
            .await
            .unwrap()
            .unwrap()
            .is_err()
    );
    let caller = CancellationToken::new();
    let waiting = {
        let client = client.clone();
        let caller = caller.clone();
        tokio::spawn(async move {
            client
                .complete_streaming(&[Message::user("hello")], None, &[], caller)
                .await
        })
    };
    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        loop {
            let active = {
                fixture
                    .header_tokens
                    .lock()
                    .unwrap()
                    .last()
                    .is_some_and(|token| !token.is_cancelled())
            };
            if active {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    waiting.abort();
    let _ = waiting.await;
    assert!(fixture
        .header_tokens
        .lock()
        .unwrap()
        .last()
        .unwrap()
        .is_cancelled());
    assert!(!caller.is_cancelled());
    for cancelled in [false, true] {
        let closed = Arc::new(AtomicBool::new(false));
        let guard = Closed(closed.clone());
        fixture
            .replies
            .lock()
            .unwrap()
            .push_back(StreamingHttpResponse {
                status: 200,
                retry_after: None,
                error_body: String::new(),
                byte_stream: Box::pin(stream::unfold(guard, |guard| async move {
                    std::future::pending::<()>().await;
                    Some((Ok(Bytes::new()), guard))
                })),
            });
        let cancel = CancellationToken::new();
        let events = client
            .complete_streaming(&[Message::user("hello")], None, &[], cancel.clone())
            .await
            .unwrap();
        if cancelled {
            cancel.cancel();
        } else {
            drop(events);
        }
        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            while !closed.load(Ordering::SeqCst) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert_eq!(cancel.is_cancelled(), cancelled);
    }
}

#[test]
fn gemini_protocol_is_explicit_and_round_trips_without_model_name_inference() {
    assert_eq!(
        ProviderProtocol::parse("gemini").unwrap().as_str(),
        "gemini"
    );
    assert_eq!(
        serde_json::from_str::<ProviderProtocol>("\"gemini\"").unwrap(),
        ProviderProtocol::Gemini
    );
    assert_eq!(
        ProviderProtocol::resolve(None, "gemini", None),
        ProviderProtocol::Openai
    );
    assert_eq!(
        ProviderProtocol::resolve(Some(ProviderProtocol::Gemini), "custom", None),
        ProviderProtocol::Gemini
    );
    assert_eq!(
        ProviderProtocol::resolve(
            None,
            "other",
            Some("https://generativelanguage.googleapis.com")
        ),
        ProviderProtocol::Openai
    );
    assert!(ProviderProtocol::parse("google-guess").is_err());
}

#[tokio::test]
async fn native_refuses_incomplete_or_contradictory_json_without_inventing_usage() {
    for value in [
        json!({"candidates":[{"content":{"role":"user","parts":[{"text":"invalid role"}]},"finishReason":"STOP"}],"usageMetadata":{"promptTokenCount":1,"candidatesTokenCount":1,"totalTokenCount":2}}),
        json!({"candidates":[{"content":{"parts":[{"text":"partial"}]} }],"usageMetadata":{"promptTokenCount":1,"candidatesTokenCount":1,"totalTokenCount":2}}),
        json!({"candidates":[{"finishReason":"STOP","content":{"parts":[{"text":"missing usage"}]}}]}),
        json!({"candidates":[{"index":0,"finishReason":"STOP"},{"index":1,"finishReason":"STOP"}],"usageMetadata":{"promptTokenCount":1,"candidatesTokenCount":1,"totalTokenCount":2}}),
        json!({"candidates":[{"index":0,"finishReason":"STOP"}],"usageMetadata":{"promptTokenCount":10,"toolUsePromptTokenCount":3,"cachedContentTokenCount":11,"candidatesTokenCount":10,"totalTokenCount":23}}),
        json!({"promptFeedback":{"blockReason":"SAFETY"},"usageMetadata":{"promptTokenCount":10,"totalTokenCount":10}}),
    ] {
        let fixture = Arc::new(Fixture::default());
        fixture.json(200, value);
        assert!(fixture
            .client()
            .complete(&[Message::user("hello")], None, &[])
            .await
            .is_err());
        assert_eq!(fixture.calls.lock().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn native_stream_does_not_complete_on_bad_usage_truncation_or_extra_candidates() {
    let valid = response(vec![json!({"text":"visible"})]);
    let complete = format!("data: {}\n\n", valid);
    for wire in [
        complete.trim_end().as_bytes().to_vec(),
        b"data: {\"candidates\":[{\"index\":0,\"content\":{\"parts\":[{\"text\":\"partial\"}]}}]}\n\n".to_vec(),
        b"data: {\"candidates\":[{\"index\":1,\"finishReason\":\"STOP\"}]}\n\n".to_vec(),
        complete.replace("\"totalTokenCount\":23","\"totalTokenCount\":24").into_bytes(),
        [complete.as_bytes(),b"data: \xff\n\n"].concat(),
    ] {
        let fixture=Arc::new(Fixture::default());
        fixture.replies.lock().unwrap().push_back(reply(200,vec![Ok(Bytes::from(wire))]));
        let mut events=fixture.client().complete_streaming(&[Message::user("hello")],None,&[],CancellationToken::new()).await.unwrap();
        while let Some(event)=events.recv().await { assert!(!matches!(event,StreamEvent::Done(_))); }
        assert_eq!(fixture.calls.lock().unwrap().len(),1);
    }
}

#[tokio::test]
async fn native_retries_only_before_headers_and_preserves_one_logical_request_identity() {
    let fixture = Arc::new(Fixture::default());
    fixture.json(
        503,
        json!({"error":{"message":"synthetic-private-provider-error"}}),
    );
    fixture.json(200, response(vec![json!({"text":"done"})]));
    fixture.json(200, response(vec![json!({"text":"next"})]));
    let mut config = fixture.config();
    config.retry_config.as_mut().unwrap().max_retries = 1;
    config
        .headers
        .insert("x-aster-source".into(), "aster-chat".into());
    let client = create_client_with_config(config);
    for expected in ["done", "next"] {
        assert_eq!(
            client
                .complete(&[Message::user("hello")], None, &[])
                .await
                .unwrap()
                .text(),
            expected
        );
    }
    {
        let calls = fixture.calls.lock().unwrap();
        let id = |call: &Call| {
            call.1
                .iter()
                .find(|(key, _)| key == "x-aster-request-id")
                .unwrap()
                .1
                .clone()
        };
        assert_eq!(calls.len(), 3);
        assert_eq!(id(&calls[0]), id(&calls[1]));
        assert_ne!(id(&calls[0]), id(&calls[2]));
    }
    fixture.json(
        401,
        json!({"error":{"message":"synthetic-private-provider-error"}}),
    );
    let error = client
        .complete(&[Message::user("hello")], None, &[])
        .await
        .unwrap_err();
    assert!(!format!("{error:#}").contains("synthetic-private-provider-error"));
    assert!(!format!("{error:?}").contains("synthetic-private-provider-error"));
}

#[tokio::test]
async fn native_provider_identity_remains_distinct_when_runtime_families_and_model_names_match() {
    let fixture = Arc::new(Fixture::default());
    fixture.json(
        200,
        response(vec![json!({"text":"prior","thoughtSignature":"opaque"})]),
    );
    let mut a = fixture.config();
    a.provider = "gemini".into();
    a.headers
        .insert("x-aster-provider-id".into(), "provider-a".into());
    let first = create_client_with_config(a)
        .complete(&[Message::user("hello")], None, &[])
        .await
        .unwrap();
    let mut b = fixture.config();
    b.provider = "gemini".into();
    b.headers
        .insert("x-aster-provider-id".into(), "provider-b".into());
    let error = create_client_with_config(b)
        .complete(&[first.message, Message::user("next")], None, &[])
        .await
        .unwrap_err();
    assert!(error.downcast_ref::<GeminiError>().is_some());
    assert!(error.downcast_ref::<NonRetryableLlmError>().is_some());
    assert_eq!(fixture.calls.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn native_tool_images_preserve_result_context_without_transporting_trust_metadata() {
    let fixture = Arc::new(Fixture::default());
    fixture.json(200, response(vec![json!({"text":"done"})]));
    let call=GeminiState::message("provider-a","stored/model",vec![json!({"functionCall":{"id":"call-a","name":"lookup","args":{}},"thoughtSignature":"opaque"})]).unwrap();
    let result = Message::tool_result_with_images(
        "call-a",
        "result",
        &[crate::llm::Attachment::png(b"image".to_vec())],
        false,
    );
    fixture
        .client()
        .complete(&[Message::user("hello"), call, result], None, &[tool()])
        .await
        .unwrap();
    let calls = fixture.calls.lock().unwrap();
    let result = &calls[0].2["contents"][2]["parts"][0]["functionResponse"];
    assert_eq!(result["response"], json!({"output":"result"}));
    assert_eq!(
        result["parts"],
        json!([{"inlineData":{"mimeType":"image/png","data":"aW1hZ2U="}}])
    );
    assert!(!calls[0].2.to_string().contains("redaction_reviewed"));
    assert!(!calls[0].2.to_string().contains("trust"));
}

#[tokio::test]
async fn invalid_native_configuration_and_orphan_results_refuse_before_transport() {
    let fixture = Arc::new(Fixture::default());
    for case in 0..5 {
        let mut config = fixture.config();
        match case {
            0 => config.reasoning_effort = Some("xhigh".into()),
            1 => config.max_tokens = Some(0),
            2 => config.base_url = Some("https://user:secret@fixture.invalid?key=secret".into()),
            3 => config.model = " model ".into(),
            _ => {
                config
                    .headers
                    .insert("x-aster-provider-id".into(), String::new());
            }
        }
        assert!(create_client_with_config(config)
            .complete(&[Message::user("hello")], None, &[])
            .await
            .is_err());
    }
    assert!(fixture
        .client()
        .complete(
            &[Message::tool_result("missing", "result", false)],
            None,
            &[]
        )
        .await
        .is_err());
    assert!(fixture.calls.lock().unwrap().is_empty());
}
