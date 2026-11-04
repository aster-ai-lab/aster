#![cfg(test)]

use aster_core::config::ProviderProtocol;
use aster_core::llm::{
    create_client_with_config, HttpClient, HttpResponse, LlmConfig, Message, StreamingHttpResponse,
};
use async_trait::async_trait;
use serde_json::Value;
use std::sync::{Arc, Mutex};
use tokio_util::sync::CancellationToken;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Protocol {
    #[default]
    Openai,
    Anthropic,
}

impl Protocol {
    pub fn path(self) -> &'static str {
        match self {
            Self::Openai => "/v1/chat/completions",
            Self::Anthropic => "/v1/messages",
        }
    }

    pub fn credential_header(self) -> &'static str {
        match self {
            Self::Openai => "authorization",
            Self::Anthropic => "x-api-key",
        }
    }
}

// Route credentials stay in host memory; it has no Debug/Serialize implementation.
pub struct Route {
    pub protocol: Protocol,
    pub endpoint: String,
    pub headers: Vec<(String, String)>,
    pub model: String,
    pub protected_values: Vec<Vec<u8>>,
}

struct Capture {
    protocol: Protocol,
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
            protocol: self.protocol,
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
            body: match self.protocol {
                Protocol::Openai => serde_json::json!({
                "choices": [{"message":{"role":"assistant","content":"route discovery"},"finish_reason":"stop"}],
                "usage":{"prompt_tokens":0,"completion_tokens":0,"total_tokens":0}
            })
                .to_string(),
                Protocol::Anthropic => serde_json::json!({
                    "id":"route-discovery", "type":"message", "role":"assistant",
                    "content":[{"type":"text","text":"route discovery"}],
                    "stop_reason":"end_turn", "usage":{"input_tokens":0,"output_tokens":0}
                }).to_string(),
            },
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

/// Compose the configured native endpoint and headers without network access.
/// This is transport setup, not an LLM result or account activation.
pub async fn provider_route(config: LlmConfig) -> Result<Route, String> {
    let protocol = match config.protocol {
        ProviderProtocol::Openai => Protocol::Openai,
        ProviderProtocol::Anthropic => Protocol::Anthropic,
        _ => return Err("this relay profile supports OpenAI Chat and Anthropic Messages".into()),
    };
    if config.session_id_header.is_some() {
        return Err("runtime session-header forwarding needs its own relay profile".into());
    }
    if config.base_url.is_none() {
        return Err("provider base URL must be explicit".into());
    }
    let model = config.model.clone();
    let protected_values = std::iter::once(config.api_key.expose())
        .chain(config.headers.values().map(String::as_str))
        .filter(|value| !value.is_empty())
        .map(|value| value.as_bytes().to_vec())
        .collect::<Vec<_>>();
    if protected_values.iter().any(|value| value.len() > 4096) {
        return Err("upstream header exceeds this bounded relay profile".into());
    }
    let capture = Arc::new(Capture {
        protocol,
        route: Mutex::new(None),
    });
    let client = create_client_with_config(config.with_http_client(capture.clone()));
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
