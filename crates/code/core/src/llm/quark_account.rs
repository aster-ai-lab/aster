//! Import the logged-in Quark browser as an account-backed model provider.
//!
//! The host opts in with `import_accounts = ["quark"]`. The provider uses the
//! existing account/model catalog, not a new provider protocol. Browser cookies
//! and credentials stay inside Quark; Aster receives only the model answer.

use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use aster_use_browser::{LocalCdpEndpoint, QuarkModelPage};
use async_trait::async_trait;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::cursor_account::{account_text_prompt, account_text_response};
use super::{
    LlmClient, LlmResponse, LlmResponseMeta, Message, StreamEvent, TokenUsage, ToolDefinition,
};
use crate::{CodeConfig, ModelConfig, ModelModalities, ProviderConfig};

pub const PROVIDER_NAME: &str = "quark";
pub const DEFAULT_MODEL_ID: &str = "kbot";
pub const DEFAULT_CDP_ENDPOINT: &str = "http://127.0.0.1:9222";
const DEFAULT_GENERATION_TIMEOUT: Duration = Duration::from_secs(120);
const KBOT_URL: &str = "https://kbot.quark.cn/quarkchat";

/// Merge the explicitly selected Quark account binding into the model catalog.
/// No browser or credential is read until this model actually starts a request.
pub fn import_local_account(config: &mut CodeConfig) -> Result<bool> {
    if env_flag("ASTER_DISABLE_QUARK_ACCOUNT") {
        return Ok(false);
    }
    let endpoint = std::env::var("ASTER_QUARK_CDP_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_CDP_ENDPOINT.to_string());
    merge_provider(config, &endpoint)
}

fn merge_provider(config: &mut CodeConfig, endpoint: &str) -> Result<bool> {
    let endpoint = LocalCdpEndpoint::parse(endpoint)
        .map_err(|error| anyhow!("invalid Quark CDP endpoint: {}", error.message))?;
    let provider = ProviderConfig {
        name: PROVIDER_NAME.to_string(),
        protocol: None,
        api_key: None,
        base_url: Some(endpoint.as_str().to_string()),
        headers: Default::default(),
        session_id_header: None,
        models: vec![ModelConfig {
            id: DEFAULT_MODEL_ID.to_string(),
            name: "夸克 Kbot".to_string(),
            family: "quark-kbot".to_string(),
            api_key: None,
            base_url: None,
            headers: Default::default(),
            session_id_header: None,
            attachment: false,
            reasoning: false,
            reasoning_control: None,
            tool_call: true,
            temperature: false,
            release_date: None,
            modalities: ModelModalities {
                input: vec!["text".to_string()],
                output: vec!["text".to_string()],
            },
            cost: Default::default(),
            limit: Default::default(),
        }],
    };
    config
        .providers
        .retain(|current| current.name != PROVIDER_NAME);
    config.providers.push(provider);
    let qualified = format!("{PROVIDER_NAME}/{DEFAULT_MODEL_ID}");
    let needs_default = config
        .default_model
        .as_deref()
        .is_none_or(|value| value.trim().is_empty());
    if needs_default || env_flag("ASTER_PREFER_QUARK") {
        config.default_model = Some(qualified);
    }
    Ok(true)
}

fn env_flag(name: &str) -> bool {
    std::env::var(name).ok().is_some_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
}

#[derive(Clone)]
pub struct QuarkAccountClient {
    model: String,
    endpoint: String,
    generation_timeout: Duration,
}

impl std::fmt::Debug for QuarkAccountClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QuarkAccountClient")
            .field("model", &self.model)
            .field(
                "endpoint",
                &crate::config::redact_url_for_debug(&self.endpoint),
            )
            .field("generation_timeout", &self.generation_timeout)
            .finish()
    }
}

impl QuarkAccountClient {
    pub(crate) fn from_config(config: super::LlmConfig) -> Self {
        Self {
            model: config.model,
            endpoint: config
                .base_url
                .unwrap_or_else(|| DEFAULT_CDP_ENDPOINT.to_string()),
            generation_timeout: config
                .api_timeout_ms
                .map(|timeout| Duration::from_millis(timeout.max(1)))
                .unwrap_or(DEFAULT_GENERATION_TIMEOUT),
        }
    }

    async fn perform(
        &self,
        messages: &[Message],
        system: Option<&str>,
        tools: &[ToolDefinition],
        cancel_token: CancellationToken,
    ) -> Result<LlmResponse> {
        let endpoint = LocalCdpEndpoint::parse(&self.endpoint)
            .map_err(|error| anyhow!("invalid Quark CDP endpoint: {}", error.message))?;
        let mut prompt = account_text_prompt(messages, system, tools)
            .context("could not encode the Quark account conversation")?;
        if !tools.is_empty() {
            prompt.push_str(
                "\n\nReturn exactly one JSON object and no Markdown fence:\n\
                 {\"text\":\"assistant text or empty\",\"tool_calls\":[{\"name\":\"presented tool name\",\"arguments\":{}}]}\n\
                 To use a tool, put it in tool_calls now and wait for its tool_result in the next conversation. \
                 Never claim a tool is unavailable or already ran. Arguments must match that tool's parameters schema. \
                 If no tool is needed, return tool_calls as an empty array.",
            );
        }
        let started = Instant::now();
        let connection = QuarkModelPage::connect(endpoint, self.generation_timeout);
        tokio::pin!(connection);
        let page = tokio::select! {
            biased;
            _ = cancel_token.cancelled() => {
                return Err(anyhow!("Quark account request was cancelled"));
            }
            result = &mut connection => result
                .map_err(|error| anyhow!("Quark account is unavailable: {}", error.message))?,
        };
        let remaining = self.generation_timeout.saturating_sub(started.elapsed());
        if remaining.is_zero() {
            return Err(anyhow!("Quark account generation timed out"));
        }
        let generation = page.complete(&prompt, remaining, |_delta| Ok(()));
        tokio::pin!(generation);
        let reply = tokio::select! {
            biased;
            _ = cancel_token.cancelled() => {
                return Err(anyhow!("Quark account request was cancelled"));
            }
            result = &mut generation => result
                .map_err(|error| anyhow!("Quark account generation failed: {}", error.message))?,
        };
        let message = account_text_response(&reply.text, tools)
            .context("Quark returned an invalid account model response")?;
        let stop_reason = if message.tool_calls().is_empty() {
            "end_turn"
        } else {
            "tool_use"
        };
        Ok(LlmResponse {
            message,
            // The web account does not expose trustworthy token accounting.
            usage: TokenUsage::default(),
            stop_reason: Some(stop_reason.to_string()),
            token_logprobs: Vec::new(),
            meta: Some(LlmResponseMeta {
                provider: Some(PROVIDER_NAME.to_string()),
                request_model: Some(self.model.clone()),
                request_url: Some(KBOT_URL.to_string()),
                duration_ms: Some(started.elapsed().as_millis().min(u64::MAX as u128) as u64),
                ..LlmResponseMeta::default()
            }),
        })
    }
}

#[async_trait]
impl LlmClient for QuarkAccountClient {
    fn with_active_generation_timeout(&self, timeout: Duration) -> Option<Arc<dyn LlmClient>> {
        let mut client = self.clone();
        client.generation_timeout = timeout;
        Some(Arc::new(client))
    }

    async fn complete(
        &self,
        messages: &[Message],
        system: Option<&str>,
        tools: &[ToolDefinition],
    ) -> Result<LlmResponse> {
        self.perform(messages, system, tools, CancellationToken::new())
            .await
    }

    async fn complete_streaming(
        &self,
        messages: &[Message],
        system: Option<&str>,
        tools: &[ToolDefinition],
        cancel_token: CancellationToken,
    ) -> Result<mpsc::Receiver<StreamEvent>> {
        let response = self.perform(messages, system, tools, cancel_token).await?;
        let (sender, receiver) = mpsc::channel(2);
        sender
            .send(StreamEvent::TextDelta(response.text()))
            .await
            .map_err(|_| anyhow!("Quark response receiver closed"))?;
        sender
            .send(StreamEvent::Done(response))
            .await
            .map_err(|_| anyhow!("Quark response receiver closed"))?;
        Ok(receiver)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_import_registers_a_keyless_model_without_a_new_protocol() {
        let mut config = CodeConfig::default();
        merge_provider(&mut config, DEFAULT_CDP_ENDPOINT).unwrap();
        let provider = config.find_provider(PROVIDER_NAME).unwrap();
        assert_eq!(provider.name, PROVIDER_NAME);
        assert_eq!(provider.protocol, None);
        assert_eq!(provider.api_key, None);
        assert_eq!(provider.models.len(), 1);
        assert_eq!(provider.models[0].id, DEFAULT_MODEL_ID);
        assert!(!provider.models[0].temperature);
        assert_eq!(config.default_model.as_deref(), Some("quark/kbot"));
        let llm = config.default_llm_config().unwrap();
        assert_eq!(llm.provider, PROVIDER_NAME);
        assert_eq!(llm.api_key.expose(), "");
        assert_eq!(llm.base_url.as_deref(), Some("http://127.0.0.1:9222/"));
    }

    #[test]
    fn account_import_does_not_replace_an_existing_default() {
        let mut config = CodeConfig::default();
        config.default_model = Some("other/model".to_string());
        merge_provider(&mut config, DEFAULT_CDP_ENDPOINT).unwrap();
        assert_eq!(config.default_model.as_deref(), Some("other/model"));
    }

    #[test]
    fn account_import_rejects_a_remote_cdp_endpoint() {
        let mut config = CodeConfig::default();
        let error = merge_provider(&mut config, "http://192.168.1.2:9222").unwrap_err();
        assert!(error.to_string().contains("invalid Quark CDP endpoint"));
        assert!(config.providers.is_empty());
    }

    #[tokio::test]
    async fn cancelled_account_request_stops_before_opening_a_browser() {
        let client = QuarkAccountClient {
            model: DEFAULT_MODEL_ID.to_string(),
            endpoint: "http://127.0.0.1:9".to_string(),
            generation_timeout: Duration::from_secs(60),
        };
        let cancel = CancellationToken::new();
        cancel.cancel();
        let error = client
            .complete_streaming(&[Message::user("test")], None, &[], cancel)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("cancelled"), "{error}");
    }

    #[test]
    fn account_client_debug_does_not_expose_endpoint_credentials() {
        let client = QuarkAccountClient {
            model: DEFAULT_MODEL_ID.to_string(),
            endpoint: "http://user:CANARY_QUARK_SECRET@127.0.0.1:9222".to_string(),
            generation_timeout: DEFAULT_GENERATION_TIMEOUT,
        };
        let debug = format!("{client:?}");
        assert!(!debug.contains("CANARY_QUARK_SECRET"), "{debug}");
        assert!(debug.contains("127.0.0.1:9222"), "{debug}");
    }

    #[tokio::test]
    #[ignore = "requires a user-enabled logged-in Quark browser on local CDP"]
    async fn imported_quark_model_completes_through_the_real_client_factory() {
        let mut config = CodeConfig {
            llm_api_timeout_ms: Some(60_000),
            ..CodeConfig::default()
        };
        import_local_account(&mut config).unwrap();
        let client = crate::llm::create_client_with_config(config.default_llm_config().unwrap());
        let response = client
            .complete(
                &[Message::user("请只回复：ASTER_QUARK_IMPORT_OK_20261128")],
                None,
                &[],
            )
            .await
            .unwrap();
        assert!(response.text().contains("ASTER_QUARK_IMPORT_OK_20261128"));
        assert_eq!(
            response.meta.unwrap().provider.as_deref(),
            Some(PROVIDER_NAME)
        );
    }

    #[tokio::test]
    #[ignore = "requires a user-enabled logged-in Quark browser on local CDP"]
    async fn imported_quark_model_returns_only_a_presented_tool_call() {
        let mut config = CodeConfig {
            llm_api_timeout_ms: Some(60_000),
            ..CodeConfig::default()
        };
        import_local_account(&mut config).unwrap();
        let client = crate::llm::create_client_with_config(config.default_llm_config().unwrap());
        let tools = [ToolDefinition {
            name: "lookup_weather".to_string(),
            description: "Look up the weather for a named city".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {"city": {"type": "string"}},
                "required": ["city"],
                "additionalProperties": false
            }),
        }];
        let response = client
            .complete(
                &[Message::user(
                    "调用 lookup_weather 查询杭州天气，不要直接回答",
                )],
                None,
                &tools,
            )
            .await
            .unwrap();
        let calls = response.tool_calls();
        assert_eq!(calls.len(), 1, "response was {}", response.text());
        assert_eq!(calls[0].name, "lookup_weather");
        assert_eq!(calls[0].args["city"], "杭州");
    }
}
