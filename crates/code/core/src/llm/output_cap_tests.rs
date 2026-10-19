#![cfg(test)]

use super::*;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct CapturedHttp(Mutex<Vec<serde_json::Value>>);

#[async_trait]
impl HttpClient for CapturedHttp {
    async fn post(
        &self,
        _url: &str,
        _headers: Vec<(&str, &str)>,
        body: &serde_json::Value,
        _cancel: CancellationToken,
    ) -> Result<HttpResponse> {
        self.0.lock().unwrap().push(body.clone());
        Ok(HttpResponse {
            status: 200,
            body: serde_json::json!({
                "content": [{"type":"text","text":"ok"}],
                "stop_reason": "end_turn",
                "choices": [{"message":{"role":"assistant","content":"ok"},"finish_reason":"stop"}],
                "usage": {"input_tokens":1,"output_tokens":1,"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}
            }).to_string(),
            retry_after: None,
        })
    }

    async fn post_streaming(
        &self,
        _url: &str,
        _headers: Vec<(&str, &str)>,
        _body: &serde_json::Value,
        _cancel: CancellationToken,
    ) -> Result<StreamingHttpResponse> {
        anyhow::bail!("this wire test uses non-streaming requests")
    }
}

fn clients(
    http: Arc<CapturedHttp>,
    effort: Option<&str>,
    max_tokens: Option<usize>,
) -> [Arc<dyn LlmClient>; 5] {
    let mut anthropic = AnthropicClient::new("test-key".into(), "claude-opus-4-6".into())
        .with_http_client(http.clone());
    let mut anthropic_glm =
        AnthropicClient::new("test-key".into(), "glm-5.3".into()).with_http_client(http.clone());
    let mut openai =
        OpenAiClient::new("test-key".into(), "gpt-5.4".into()).with_http_client(http.clone());
    let mut zhipu =
        ZhipuClient::new("test-key".into(), "glm-5.3".into()).with_http_client(http.clone());
    let mut deepseek =
        OpenAiClient::new("test-key".into(), "deepseek-v4-flash".into()).with_http_client(http);
    if let Some(effort) = effort {
        anthropic = anthropic.with_reasoning_effort(effort);
        anthropic_glm = anthropic_glm.with_reasoning_effort(effort);
        openai = openai.with_reasoning_effort(effort);
        zhipu = zhipu.with_reasoning_effort(effort);
        deepseek = deepseek.with_reasoning_effort(effort);
    }
    if let Some(max_tokens) = max_tokens {
        anthropic = anthropic.with_max_tokens(max_tokens);
        anthropic_glm = anthropic_glm.with_max_tokens(max_tokens);
        openai = openai.with_max_tokens(max_tokens);
        zhipu = zhipu.with_max_tokens(max_tokens);
        deepseek = deepseek.with_max_tokens(max_tokens);
    }
    [
        Arc::new(anthropic),
        Arc::new(anthropic_glm),
        Arc::new(openai),
        Arc::new(zhipu),
        Arc::new(deepseek),
    ]
}

#[tokio::test]
async fn sampling_output_caps_cannot_expand_default_wire_windows() {
    for effort in [None, Some("high"), Some("none")] {
        for cap in [16_384, 65_537, 131_072] {
            let http = Arc::new(CapturedHttp::default());
            for client in clients(http.clone(), effort, None) {
                let original = request(client.as_ref(), &http).await;
                let expected = original["max_tokens"]
                    .as_u64()
                    .map_or(u64::from(cap), |limit| limit.min(u64::from(cap)));
                let capped = client.with_output_cap(cap).unwrap();
                let body = request(capped.as_ref(), &http).await;
                assert_eq!(body["max_tokens"], expected, "{original} became {body}");
            }
        }
    }
    let http = Arc::new(CapturedHttp::default());
    let legacy = AnthropicClient::new("test-key".into(), "claude-opus-4-6".into())
        .with_http_client(http.clone())
        .with_thinking_budget(2048);
    let original = request(&legacy, &http).await;
    let capped = legacy.with_output_cap(16_384).unwrap();
    let body = request(capped.as_ref(), &http).await;
    assert_eq!(body["max_tokens"], original["max_tokens"]);
    assert_eq!(body["thinking"], original["thinking"]);
}

async fn request(client: &dyn LlmClient, http: &CapturedHttp) -> serde_json::Value {
    client
        .complete(&[Message::user("test")], None, &[])
        .await
        .unwrap();
    http.0.lock().unwrap().pop().expect("captured wire request")
}

#[tokio::test]
async fn explicit_output_caps_survive_effort_wire_requests() {
    for effort in [None, Some("high"), Some("none")] {
        for cap in [1, 2048, 8192, 65_535, 65_536, 65_537] {
            let http = Arc::new(CapturedHttp::default());
            for client in clients(http.clone(), effort, Some(cap)) {
                let body = request(client.as_ref(), &http).await;
                assert_eq!(body["max_tokens"], cap, "effort={effort:?}; {body}");
            }
        }
    }
}

#[test]
fn zero_sampling_output_cap_refuses_without_a_request() {
    for effort in [None, Some("high"), Some("none")] {
        for configured in [None, Some(0), Some(64)] {
            let http = Arc::new(CapturedHttp::default());
            for client in clients(http.clone(), effort, configured) {
                assert!(client.with_output_cap(0).is_none());
                if configured == Some(0) {
                    assert!(client.with_output_cap(1024).is_none());
                }
            }
            assert!(http.0.lock().unwrap().is_empty());
        }
    }
}

#[tokio::test]
async fn sampling_output_caps_only_tighten_existing_limits() {
    for effort in [None, Some("high"), Some("none")] {
        for configured in [None, Some(64), Some(16_384)] {
            for cap in [1024, 8192] {
                let http = Arc::new(CapturedHttp::default());
                for client in clients(http.clone(), effort, configured) {
                    let capped = client.with_output_cap(cap).expect("supported output cap");
                    let body = request(capped.as_ref(), &http).await;
                    assert_eq!(
                        body["max_tokens"],
                        configured.map_or(cap as usize, |limit| limit.min(cap as usize)),
                        "effort={effort:?}; configured={configured:?}; {body}"
                    );
                    let recapped = capped.with_output_cap(cap * 2).unwrap();
                    assert_eq!(
                        request(recapped.as_ref(), &http).await["max_tokens"],
                        body["max_tokens"]
                    );
                }
            }
        }
    }
}

#[tokio::test]
async fn explicit_output_cap_preserves_legacy_thinking_answer_reserve() {
    for cap in [1024, 2048, 2049, 4096] {
        for budget in [1023, 1024, 2048, 3071, 3072, 4096] {
            let http = Arc::new(CapturedHttp::default());
            let client = AnthropicClient::new("test-key".into(), "claude-opus-4-6".into())
                .with_http_client(http.clone())
                .with_max_tokens(cap)
                .with_thinking_budget(budget);
            let body = request(&client, &http).await;
            assert_eq!(body["max_tokens"], cap);
            if budget >= 1024 && budget < cap.saturating_sub(1024) {
                assert_eq!(body["thinking"]["budget_tokens"], budget);
            } else {
                assert!(body.get("thinking").is_none(), "{body}");
            }
        }
    }
}

#[tokio::test]
async fn uncapped_effort_requests_keep_existing_output_windows() {
    for effort in [None, Some("high"), Some("none")] {
        let http = Arc::new(CapturedHttp::default());
        for (index, client) in clients(http.clone(), effort, None).into_iter().enumerate() {
            let body = request(client.as_ref(), &http).await;
            if index == 4 {
                assert!(body.get("max_tokens").is_none(), "{body}");
            } else if effort.is_some() || index == 1 {
                assert_eq!(body["max_tokens"], 65_536);
            } else if index == 0 {
                assert_eq!(body["max_tokens"], 8192);
            } else {
                assert!(body.get("max_tokens").is_none(), "{body}");
            }
        }
    }
}
