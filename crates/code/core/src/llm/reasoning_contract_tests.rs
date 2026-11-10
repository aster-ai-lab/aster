use super::*;
use crate::config::{documented_reasoning, ProviderProtocol};
use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use std::sync::{Arc, Mutex};
use tokio_util::sync::CancellationToken;

#[derive(Default)]
struct Capture(Mutex<Vec<Value>>);

#[async_trait]
impl HttpClient for Capture {
    async fn post(
        &self,
        _: &str,
        _: Vec<(&str, &str)>,
        body: &Value,
        _: CancellationToken,
    ) -> Result<HttpResponse> {
        self.0.lock().unwrap().push(body.clone());
        Ok(HttpResponse {
            status: 400,
            body: "wire fixture stops after request capture".into(),
            retry_after: None,
        })
    }

    async fn post_streaming(
        &self,
        _: &str,
        _: Vec<(&str, &str)>,
        body: &Value,
        _: CancellationToken,
    ) -> Result<StreamingHttpResponse> {
        self.0.lock().unwrap().push(body.clone());
        Ok(StreamingHttpResponse {
            status: 400,
            error_body: "wire fixture stops after request capture".into(),
            byte_stream: Box::pin(futures::stream::empty()),
            retry_after: None,
        })
    }
}

const CASES: &[(ProviderProtocol, &str, &str)] = &[
    (ProviderProtocol::Openai, "gpt-5.4", "/reasoning_effort"),
    (
        ProviderProtocol::Openai,
        "deepseek-flash",
        "/reasoning_effort",
    ),
    (ProviderProtocol::Openai, "k3.1-pro", "/reasoning_effort"),
    (
        ProviderProtocol::OpenaiResponses,
        "gpt-5.4",
        "/reasoning/effort",
    ),
    (ProviderProtocol::Codex, "gpt-6.1-sol", "/reasoning/effort"),
    (
        ProviderProtocol::Anthropic,
        "claude-opus-4-6",
        "/output_config/effort",
    ),
    (ProviderProtocol::Anthropic, "glm-5.3", "/reasoning_effort"),
    (ProviderProtocol::Anthropic, "glm-5.2", "/reasoning_effort"),
    (ProviderProtocol::Glm, "glm-5.3", "/reasoning_effort"),
    (
        ProviderProtocol::Gemini,
        "gemini-3-pro-preview",
        "/generationConfig/thinkingConfig/thinkingLevel",
    ),
];

fn config(protocol: ProviderProtocol, model: &str, http: Arc<Capture>) -> LlmConfig {
    LlmConfig::new("account-alias", model, "synthetic-secret")
        .with_protocol(protocol)
        .with_http_client(http)
}

#[tokio::test]
async fn every_advertised_level_reaches_its_native_field() {
    for &(protocol, model, field) in CASES {
        let capabilities = documented_reasoning(protocol, model).unwrap();
        for level in capabilities.levels() {
            let http = Arc::new(Capture::default());
            let client = create_client_with_config(
                config(protocol, model, http.clone()).with_reasoning_effort(level.as_str()),
            );
            assert!(client
                .complete(&[Message::user("wire probe")], None, &[])
                .await
                .is_err());
            let calls = http.0.lock().unwrap();
            assert_eq!(calls.len(), 1, "{protocol:?}/{model}/{}", level.as_str());
            let expected = if protocol == ProviderProtocol::Gemini {
                level.as_str().to_uppercase()
            } else {
                level.as_str().to_string()
            };
            assert_eq!(
                calls[0].pointer(field).and_then(Value::as_str),
                Some(expected.as_str()),
                "{protocol:?}/{model}: {}",
                calls[0]
            );
        }
    }
}

#[tokio::test]
async fn reset_omits_all_native_overrides() {
    for &(protocol, model, _) in CASES {
        for selection in [None, Some("default")] {
            let http = Arc::new(Capture::default());
            let mut settings = config(protocol, model, http.clone());
            if let Some(token) = selection {
                settings = settings.with_reasoning_effort(token);
            }
            let client = create_client_with_config(settings);
            assert!(client
                .complete(&[Message::user("wire probe")], None, &[])
                .await
                .is_err());
            let calls = http.0.lock().unwrap();
            assert_eq!(calls.len(), 1, "{protocol:?}/{model}");
            for field in [
                "/reasoning_effort",
                "/reasoning/effort",
                "/output_config/effort",
                "/thinking",
                "/generationConfig/thinkingConfig",
            ] {
                assert!(
                    calls[0].pointer(field).is_none(),
                    "reset {protocol:?}/{model}: {}",
                    calls[0]
                );
            }
        }
    }
}

#[tokio::test]
async fn reported_output_effort_does_not_enable_adaptive_thinking() {
    let http = Arc::new(Capture::default());
    let mut settings = config(ProviderProtocol::Anthropic, "reported-model", http.clone())
        .with_reasoning_effort("high");
    settings.reasoning_capabilities = Some(
        crate::config::ReasoningCapabilities::new(
            crate::config::ReasoningMode::OutputConfig,
            vec![crate::config::ReasoningEffort::High],
        )
        .unwrap(),
    );
    assert!(create_client_with_config(settings)
        .complete(&[Message::user("wire probe")], None, &[])
        .await
        .is_err());
    let calls = http.0.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(
        calls[0]
            .pointer("/output_config/effort")
            .and_then(Value::as_str),
        Some("high")
    );
    assert!(calls[0].get("thinking").is_none());
}

#[tokio::test]
async fn invalid_and_unknown_selections_never_reach_http() {
    for &(protocol, model, _) in CASES {
        for token in ["turbo", "ultracode"] {
            let http = Arc::new(Capture::default());
            let client = create_client_with_config(
                config(protocol, model, http.clone()).with_reasoning_effort(token),
            );
            assert!(client
                .complete(&[Message::user("wire probe")], None, &[])
                .await
                .is_err());
            assert!(
                http.0.lock().unwrap().is_empty(),
                "{protocol:?}/{model}/{token}"
            );
        }
    }
    for (protocol, model) in [
        (ProviderProtocol::Cursor, "auto"),
        (ProviderProtocol::Openai, "unknown-model"),
        (ProviderProtocol::Openai, "gpt-4o"),
    ] {
        let http = Arc::new(Capture::default());
        let client = create_client_with_config(
            config(protocol, model, http.clone()).with_reasoning_effort("high"),
        );
        assert!(client
            .complete(&[Message::user("wire probe")], None, &[])
            .await
            .is_err());
        assert!(http.0.lock().unwrap().is_empty());
    }
}

#[tokio::test]
async fn token_budgets_remain_separate_from_effort() {
    for (protocol, model, field) in [
        (
            ProviderProtocol::Anthropic,
            "claude-sonnet-4-20250514",
            "/thinking/budget_tokens",
        ),
        (
            ProviderProtocol::Gemini,
            "gemini-2.5-flash",
            "/generationConfig/thinkingConfig/thinkingBudget",
        ),
    ] {
        assert!(documented_reasoning(protocol, model)
            .unwrap()
            .levels()
            .is_empty());
        let http = Arc::new(Capture::default());
        let client = create_client_with_config(
            config(protocol, model, http.clone()).with_thinking_budget(2048),
        );
        assert!(client
            .complete(&[Message::user("wire probe")], None, &[])
            .await
            .is_err());
        assert_eq!(
            http.0.lock().unwrap()[0]
                .pointer(field)
                .and_then(Value::as_u64),
            Some(2048)
        );
    }
}
