use super::super::{HttpClient, HttpResponse, StreamingHttpResponse};
use super::*;
use async_trait::async_trait;
use serde_json::json;
use std::sync::Mutex;

struct Fixture {
    value: Value,
    calls: Mutex<Vec<(String, Vec<(String, String)>)>>,
}

#[async_trait]
impl HttpClient for Fixture {
    async fn get(
        &self,
        url: &str,
        headers: Vec<(&str, &str)>,
        _: CancellationToken,
    ) -> anyhow::Result<HttpResponse> {
        self.calls.lock().unwrap().push((
            url.into(),
            headers
                .iter()
                .map(|(key, value)| (key.to_string(), value.to_string()))
                .collect(),
        ));
        Ok(HttpResponse {
            status: 200,
            retry_after: None,
            body: self.value.to_string(),
        })
    }
    async fn post(
        &self,
        url: &str,
        headers: Vec<(&str, &str)>,
        body: &Value,
        cancel: CancellationToken,
    ) -> anyhow::Result<HttpResponse> {
        assert_eq!(body, &json!({"customModelIds":[]}));
        self.get(url, headers, cancel).await
    }
    async fn post_streaming(
        &self,
        _: &str,
        _: Vec<(&str, &str)>,
        _: &Value,
        _: CancellationToken,
    ) -> anyhow::Result<StreamingHttpResponse> {
        anyhow::bail!("discovery must not generate a completion")
    }
}

#[tokio::test]
async fn selected_metadata_uses_the_provider_account() {
    let fixture = Arc::new(Fixture {
        value: json!({"data":[{"id":"other","context_window":1},{"id":"selected","context_window":131072,"effort":{"supported_levels":["low","high","max"]}}]}),
        calls: Mutex::new(Vec::new()),
    });
    let config = LlmConfig::new("renamed-account", "selected", "synthetic-key")
        .with_base_url("https://fixture.invalid/api/v4")
        .with_http_client(fixture.clone());
    let found = discover(&config).await.unwrap();
    assert_eq!(found.context_tokens, Some(131072));
    let controls = found.reasoning.unwrap();
    assert_eq!(controls.source(), "provider");
    assert_eq!(
        controls.levels(),
        &[
            ReasoningEffort::Low,
            ReasoningEffort::High,
            ReasoningEffort::Max
        ]
    );
    let calls = fixture.calls.lock().unwrap();
    assert_eq!(calls[0].0, "https://fixture.invalid/api/v4/models");
    assert!(calls[0]
        .1
        .iter()
        .any(|(key, value)| key.eq_ignore_ascii_case("Authorization")
            && value == "Bearer synthetic-key"));
}

#[test]
fn missing_fields_remain_unknown() {
    let found = parse(
        ProviderProtocol::Openai,
        "unknown",
        &json!({"data":[{"id":"unknown"}]}),
    )
    .unwrap();
    assert!(found.reasoning.is_none());
    assert!(found.context_tokens.is_none());
    assert!(parse(
        ProviderProtocol::Openai,
        "unknown",
        &json!({"id":"unknown","context_window":0})
    )
    .unwrap()
    .context_tokens
    .is_none());
    for value in [
        json!({"data":[{"id":"different"}]}),
        json!({"id":"different","context_window":100000}),
        json!({"id":"unknown","context_window":-1}),
    ] {
        assert!(parse(ProviderProtocol::Openai, "unknown", &value).is_err());
    }
}

#[tokio::test]
async fn chat_catalog_keeps_one_version_prefix() {
    for base in [
        "https://fixture.invalid/v1",
        "https://fixture.invalid/v1/",
        "https://fixture.invalid/coding/v1",
    ] {
        let fixture = Arc::new(Fixture {
            value: json!({"data":[{"id":"selected"}]}),
            calls: Mutex::new(Vec::new()),
        });
        let config = LlmConfig::new("account", "selected", "key")
            .with_protocol(ProviderProtocol::Openai)
            .with_base_url(base)
            .with_http_client(fixture.clone());
        discover(&config).await.unwrap();
        assert_eq!(
            fixture.calls.lock().unwrap()[0].0,
            format!("{}/models", base.trim_end_matches('/'))
        );
    }
}

#[tokio::test]
async fn responses_uses_the_catalog_beside_its_endpoint() {
    let fixture = Arc::new(Fixture {
        value: json!({"data":[{"id":"selected"}]}),
        calls: Mutex::new(Vec::new()),
    });
    let config = LlmConfig::new("account", "selected", "key")
        .with_protocol(ProviderProtocol::OpenaiResponses)
        .with_base_url("https://fixture.invalid/custom/v1/responses")
        .with_http_client(fixture.clone());
    discover(&config).await.unwrap();
    assert_eq!(
        fixture.calls.lock().unwrap()[0].0,
        "https://fixture.invalid/custom/v1/models"
    );
}

#[tokio::test]
async fn deepseek_keeps_the_documented_thinking_switch() {
    let fixture = Arc::new(Fixture {
        value: json!({"data":[{"id":"deepseek-flash","effort":{"supported_levels":["low","high","max"],"default_level":"high"}}]}),
        calls: Mutex::new(Vec::new()),
    });
    let config = LlmConfig::new("account", "deepseek-flash", "key")
        .with_base_url("https://api.deepseek.com/v1")
        .with_http_client(fixture);
    let found = discover(&config).await.unwrap().reasoning.unwrap();
    assert!(found.supports("none"));
    assert_eq!(found.default_level(), Some(ReasoningEffort::High));
    assert_eq!(found.source(), "provider+documented");
}

#[test]
fn anthropic_reports_each_native_level_and_window() {
    let found = parse(ProviderProtocol::Anthropic, "future-model", &json!({"id":"future-model","max_input_tokens":1000000,"max_tokens":128000,"capabilities":{"effort":{"supported":true,"low":{"supported":true},"medium":{"supported":true},"high":{"supported":true},"max":{"supported":false}},"thinking":{"types":{"adaptive":{"supported":true}}}}})).unwrap();
    assert_eq!(found.context_tokens, Some(1000000));
    assert_eq!(found.output_tokens, Some(128000));
    let caps = found.reasoning.unwrap();
    assert_eq!(caps.mode(), ReasoningMode::Adaptive);
    assert_eq!(
        caps.levels(),
        &[
            ReasoningEffort::Low,
            ReasoningEffort::Medium,
            ReasoningEffort::High
        ]
    );
}

#[test]
fn cursor_presets_keep_alias_identity_without_an_effort_menu() {
    let found = parse(
        ProviderProtocol::Cursor,
        "auto",
        &json!({"models":[{"modelId":"default","displayModelId":"auto","contextWindow":200000}]}),
    )
    .unwrap();
    assert_eq!(found.context_tokens, Some(200000));
    assert_eq!(found.reasoning.unwrap().mode(), ReasoningMode::Preset);
}

#[test]
fn gemini_uses_its_reported_input_limit() {
    let found = parse(
        ProviderProtocol::Gemini,
        "model",
        &json!({"name":"models/model","inputTokenLimit":1048576,"outputTokenLimit":65536}),
    )
    .unwrap();
    assert_eq!(found.context_tokens, Some(1048576));
    assert_eq!(found.output_tokens, Some(65536));
    assert!(found.reasoning.is_none());
    let disabled = parse(
        ProviderProtocol::Gemini,
        "model",
        &json!({"name":"models/model","thinking":false}),
    )
    .unwrap();
    assert_eq!(
        disabled.reasoning.unwrap().mode(),
        ReasoningMode::Unsupported
    );
}
