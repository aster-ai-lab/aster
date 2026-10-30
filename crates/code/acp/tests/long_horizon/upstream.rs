#![cfg(test)]

use a3s_code_core::llm::{
    HttpClient, HttpResponse, LlmClient, LlmConfig, Message, OpenAiClient, StreamingHttpResponse,
};
use async_trait::async_trait;
use serde_json::Value;
use std::sync::{Arc, Mutex};
use tokio_util::sync::CancellationToken;

// Neither type implements Debug/Serialize: credentials stay in host memory.
pub struct Route {
    pub endpoint: String,
    pub headers: Vec<(String, String)>,
    pub model: String,
    pub protected_values: Vec<Vec<u8>>,
}

#[derive(Default)]
struct Capture {
    route: Mutex<Option<Route>>,
}

#[async_trait]
impl HttpClient for Capture {
    async fn post(
        &self,
        url: &str,
        headers: Vec<(&str, &str)>,
        _body: &Value,
        _cancel: CancellationToken,
    ) -> anyhow::Result<HttpResponse> {
        let mut captured = self.route.lock().unwrap();
        anyhow::ensure!(
            captured.is_none(),
            "endpoint discovery sent multiple requests"
        );
        *captured = Some(Route {
            endpoint: url.to_owned(),
            headers: headers
                .into_iter()
                .map(|(key, value)| (key.to_owned(), value.to_owned()))
                .collect(),
            model: String::new(),
            protected_values: Vec::new(),
        });
        Ok(HttpResponse {
            status: 200,
            body: serde_json::json!({
                "choices": [{"message":{"role":"assistant","content":"route discovery"},"finish_reason":"stop"}],
                "usage":{"prompt_tokens":0,"completion_tokens":0,"total_tokens":0}
            })
            .to_string(),
            retry_after: None,
        })
    }

    async fn post_streaming(
        &self,
        _url: &str,
        _headers: Vec<(&str, &str)>,
        _body: &Value,
        _cancel: CancellationToken,
    ) -> anyhow::Result<StreamingHttpResponse> {
        anyhow::bail!("endpoint discovery must not stream")
    }
}

/// Ask the public OpenAI-compatible adapter to compose its exact endpoint and
/// headers without network access. This is transport setup, not an LLM result.
pub async fn openai_route(config: LlmConfig) -> Result<Route, String> {
    if !matches!(config.provider.as_str(), "gateway" | "deepseek" | "openai") {
        return Err(
            "this relay profile requires an explicitly supported OpenAI-compatible provider".into(),
        );
    }
    if config.session_id_header.is_some() {
        return Err("runtime session-header forwarding needs its own relay profile".into());
    }
    let endpoint = config
        .base_url
        .ok_or("provider base URL must be explicit")?;
    let model = config.model.clone();
    let protected_values = std::iter::once(config.api_key.expose())
        .chain(config.headers.values().map(String::as_str))
        .filter(|value| !value.is_empty())
        .map(|value| value.as_bytes().to_vec())
        .collect::<Vec<_>>();
    if protected_values.iter().any(|value| value.len() > 4096) {
        return Err("upstream header exceeds this bounded relay profile".into());
    }
    let capture = Arc::new(Capture::default());
    let client = OpenAiClient::new(config.api_key.expose().to_owned(), config.model)
        .with_provider_name(config.provider)
        .with_base_url(endpoint)
        .with_headers(config.headers)
        .with_http_client(capture.clone());
    client
        .complete(&[Message::user("Discover the request route.")], None, &[])
        .await
        .map_err(|_| "could not discover the configured route without network access")?;
    let mut route = capture
        .route
        .lock()
        .unwrap()
        .take()
        .ok_or("adapter did not provide a route")?;
    route.model = model;
    route.protected_values = protected_values;
    Ok(route)
}
