//! Account-scoped model metadata. Discovery never samples a model or runs tools.

use super::{http, LlmConfig};
use crate::config::{ProviderProtocol, ReasoningCapabilities, ReasoningEffort, ReasoningMode};
use serde_json::Value;
use std::{sync::Arc, time::Duration};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Default)]
pub struct ModelDiscovery {
    pub context_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
    pub reasoning: Option<ReasoningCapabilities>,
    pub source: &'static str,
}

#[derive(Debug, thiserror::Error)]
pub enum DiscoveryError {
    #[error("model catalog unavailable")]
    Unavailable,
    #[error("model catalog returned HTTP {0}")]
    Http(u16),
    #[error("invalid model catalog")]
    Invalid,
    #[error("model catalog timed out")]
    Timeout,
    #[error("model catalog transport failed")]
    Transport(#[source] anyhow::Error),
}

/// Read the selected model's own account/catalog metadata with bounded work.
pub async fn discover(config: &LlmConfig) -> Result<ModelDiscovery, DiscoveryError> {
    let cancel = CancellationToken::new();
    let _cancel_on_drop = cancel.clone().drop_guard();
    tokio::time::timeout(Duration::from_secs(10), read_catalog(config, cancel))
        .await
        .map_err(|_| DiscoveryError::Timeout)?
}

async fn read_catalog(
    config: &LlmConfig,
    cancel: CancellationToken,
) -> Result<ModelDiscovery, DiscoveryError> {
    if config.protocol == ProviderProtocol::Codex {
        let config = config.clone();
        return tokio::task::spawn_blocking(move || {
            let model = super::codex_login::CodexLoginClient::catalog_model(&config)
                .map_err(DiscoveryError::Transport)?
                .ok_or(DiscoveryError::Unavailable)?;
            Ok(ModelDiscovery {
                context_tokens: (model.limit.context > 0).then_some(model.limit.context),
                output_tokens: (model.limit.output > 0).then_some(model.limit.output),
                reasoning: model.reasoning_control,
                source: "account",
            })
        })
        .await
        .map_err(|_| DiscoveryError::Unavailable)?;
    }
    let http = match &config.http_override {
        Some(http) => Arc::clone(http),
        None => Arc::new(
            http::ReqwestHttpClient::without_redirects().map_err(DiscoveryError::Transport)?,
        ),
    };
    let value = if config.protocol == ProviderProtocol::Cursor {
        let mut client = super::cursor_account::CursorAccountClient::new(
            config.api_key.expose().into(),
            config.model.clone(),
        )
        .with_http_client(http);
        if let Some(base) = &config.base_url {
            client = client.with_base_url(base.clone());
        }
        client = client.with_headers(config.resolved_headers());
        client
            .model_catalog(cancel)
            .await
            .map_err(DiscoveryError::Transport)?
    } else {
        let endpoint = endpoint(config, &http)?;
        let mut headers = config.resolved_headers().into_iter().collect::<Vec<_>>();
        let auth = match config.protocol {
            ProviderProtocol::Anthropic => ("x-api-key", config.api_key.expose().to_string()),
            ProviderProtocol::Gemini => ("x-goog-api-key", config.api_key.expose().to_string()),
            _ => (
                "Authorization",
                format!("Bearer {}", config.api_key.expose()),
            ),
        };
        if !headers
            .iter()
            .any(|(key, _)| key.eq_ignore_ascii_case(auth.0))
        {
            headers.push((auth.0.into(), auth.1));
        }
        if config.protocol == ProviderProtocol::Anthropic
            && !headers
                .iter()
                .any(|(key, _)| key.eq_ignore_ascii_case("anthropic-version"))
        {
            headers.push(("anthropic-version".into(), "2023-06-01".into()));
        }
        let headers = headers
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .collect();
        let response = http
            .get(&endpoint, headers, cancel)
            .await
            .map_err(DiscoveryError::Transport)?;
        if !(200..300).contains(&response.status) {
            return Err(DiscoveryError::Http(response.status));
        }
        serde_json::from_str(&response.body).map_err(|_| DiscoveryError::Invalid)?
    };
    let mut found = parse(config.protocol, &config.model, &value)?;
    // DeepSeek lists enabled-thinking levels separately from its documented off switch.
    if super::effort::is_deepseek_model(&config.model)
        && crate::config::documented_at(config.protocol, &config.model, config.base_url.as_deref())
            .is_some_and(|caps| caps.supports("none"))
    {
        if let Some(caps) = found
            .reasoning
            .as_ref()
            .filter(|caps| caps.mode() == ReasoningMode::Effort && !caps.supports("none"))
        {
            let mut levels = caps.levels().to_vec();
            levels.insert(0, ReasoningEffort::None);
            let mut controls = ReasoningCapabilities::new(ReasoningMode::Effort, levels)
                .map_err(|_| DiscoveryError::Invalid)?;
            if let Some(default) = caps.default_level() {
                controls = controls
                    .with_default_level(default)
                    .map_err(|_| DiscoveryError::Invalid)?;
            }
            found.reasoning = Some(controls.with_source("provider+documented"));
        }
    }
    Ok(found)
}

fn endpoint(
    config: &LlmConfig,
    http: &Arc<dyn super::HttpClient>,
) -> Result<String, DiscoveryError> {
    if config.protocol == ProviderProtocol::Gemini {
        let client = super::gemini::GeminiClient::new(
            config.clone(),
            config.resolved_headers(),
            Arc::clone(http),
        );
        let mut url = client
            .endpoint(false)
            .map_err(|_| DiscoveryError::Invalid)?;
        url.path_segments_mut()
            .map_err(|_| DiscoveryError::Invalid)?
            .pop()
            .push(&config.model);
        return Ok(url.to_string());
    }
    let (base, suffix) = match config.protocol {
        ProviderProtocol::Anthropic => (
            http::normalize_base_url(
                config
                    .base_url
                    .as_deref()
                    .unwrap_or("https://api.anthropic.com"),
            ),
            "/v1/models",
        ),
        ProviderProtocol::Gemini => (
            config
                .base_url
                .as_deref()
                .unwrap_or("https://generativelanguage.googleapis.com/v1beta")
                .trim_end_matches('/')
                .to_string(),
            "/models",
        ),
        ProviderProtocol::Glm => (
            http::join_chat_completions_url(
                config
                    .base_url
                    .as_deref()
                    .unwrap_or("https://open.bigmodel.cn"),
                "/api/paas/v4/chat/completions",
            )
            .trim_end_matches("/chat/completions")
            .to_string(),
            "/models",
        ),
        ProviderProtocol::OpenaiResponses => (
            super::metering::wire::UsageProtocol::OpenaiResponses
                .endpoint(
                    config
                        .base_url
                        .as_deref()
                        .unwrap_or("https://api.openai.com"),
                )
                .trim_end_matches("/responses")
                .to_string(),
            "/models",
        ),
        ProviderProtocol::Openai => (
            super::metering::wire::UsageProtocol::OpenaiChat
                .endpoint(
                    config
                        .base_url
                        .as_deref()
                        .unwrap_or("https://api.openai.com"),
                )
                .trim_end_matches("/chat/completions")
                .to_string(),
            "/models",
        ),
        _ => return Err(DiscoveryError::Unavailable),
    };
    let mut url =
        url::Url::parse(&format!("{base}{suffix}")).map_err(|_| DiscoveryError::Invalid)?;
    if matches!(
        config.protocol,
        ProviderProtocol::Anthropic | ProviderProtocol::Gemini
    ) {
        url.path_segments_mut()
            .map_err(|_| DiscoveryError::Invalid)?
            .push(&config.model);
    }
    Ok(url.to_string())
}

fn positive(value: Option<&Value>) -> Result<Option<u32>, DiscoveryError> {
    match value.filter(|value| !value.is_null()) {
        None => Ok(None),
        Some(value) if value.as_u64() == Some(0) => Ok(None),
        Some(value) => value
            .as_u64()
            .and_then(|value| u32::try_from(value).ok())
            .filter(|value| *value > 0)
            .map(Some)
            .ok_or(DiscoveryError::Invalid),
    }
}

fn parse(
    protocol: ProviderProtocol,
    model: &str,
    value: &Value,
) -> Result<ModelDiscovery, DiscoveryError> {
    let entries = value
        .get("data")
        .or_else(|| value.get("models"))
        .and_then(Value::as_array);
    let entry = match entries {
        Some(entries) => entries
            .iter()
            .find(|entry| {
                ["id", "slug", "modelId", "displayModelId"]
                    .iter()
                    .any(|key| entry.get(key).and_then(Value::as_str) == Some(model))
                    || entry
                        .get("aliases")
                        .and_then(Value::as_array)
                        .is_some_and(|aliases| {
                            aliases.iter().any(|alias| alias.as_str() == Some(model))
                        })
            })
            .ok_or(DiscoveryError::Unavailable)?,
        None => {
            let identity = value.get("id").and_then(Value::as_str) == Some(model)
                || value.get("name").and_then(Value::as_str)
                    == Some(format!("models/{model}").as_str());
            if !identity {
                return Err(DiscoveryError::Invalid);
            }
            value
        }
    };
    let mut found = ModelDiscovery {
        source: "provider",
        ..Default::default()
    };
    found.context_tokens = positive(
        entry
            .get("context_window")
            .or_else(|| entry.get("context_length"))
            .or_else(|| entry.get("max_model_len"))
            .or_else(|| entry.get("max_input_tokens"))
            .or_else(|| entry.get("inputTokenLimit"))
            .or_else(|| entry.get("contextWindow")),
    )?;
    found.output_tokens = positive(
        entry
            .get("max_output_tokens")
            .or_else(|| entry.get("outputTokenLimit"))
            .or_else(|| entry.pointer("/top_provider/max_completion_tokens")),
    )?;
    if protocol == ProviderProtocol::Anthropic {
        found.output_tokens = positive(entry.get("max_tokens"))?.or(found.output_tokens);
    }
    if protocol == ProviderProtocol::Cursor {
        found.reasoning = Some(ReasoningCapabilities::preset());
    } else if let Some(levels) = entry
        .pointer("/effort/supported_levels")
        .and_then(Value::as_array)
    {
        let levels = levels
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .ok_or(DiscoveryError::Invalid)
                    .and_then(|token| {
                        ReasoningEffort::parse(token).map_err(|_| DiscoveryError::Invalid)
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        found.reasoning = Some(
            ReasoningCapabilities::new(
                if protocol == ProviderProtocol::Anthropic {
                    ReasoningMode::OutputConfig
                } else if protocol == ProviderProtocol::Gemini {
                    ReasoningMode::ThinkingLevel
                } else {
                    ReasoningMode::Effort
                },
                levels,
            )
            .map_err(|_| DiscoveryError::Invalid)?
            .with_provider_source(),
        );
    } else if let Some(effort) = entry.pointer("/capabilities/effort") {
        if effort.get("supported").and_then(Value::as_bool) == Some(false) {
            found.reasoning = Some(
                ReasoningCapabilities::new(
                    if entry
                        .pointer("/capabilities/thinking/types/enabled/supported")
                        .and_then(Value::as_bool)
                        == Some(true)
                    {
                        ReasoningMode::Budget
                    } else {
                        ReasoningMode::Unsupported
                    },
                    Vec::new(),
                )
                .map_err(|_| DiscoveryError::Invalid)?
                .with_provider_source(),
            );
        } else if effort.get("supported").and_then(Value::as_bool) == Some(true) {
            let levels = [
                ReasoningEffort::None,
                ReasoningEffort::Minimal,
                ReasoningEffort::Low,
                ReasoningEffort::Medium,
                ReasoningEffort::High,
                ReasoningEffort::Xhigh,
                ReasoningEffort::Max,
            ]
            .into_iter()
            .filter(|level| {
                effort
                    .get(level.as_str())
                    .and_then(|value| value.get("supported"))
                    .and_then(Value::as_bool)
                    == Some(true)
            })
            .collect();
            let adaptive = entry
                .pointer("/capabilities/thinking/types/adaptive/supported")
                .and_then(Value::as_bool)
                == Some(true);
            found.reasoning = Some(
                ReasoningCapabilities::new(
                    if protocol != ProviderProtocol::Anthropic {
                        ReasoningMode::Effort
                    } else if adaptive {
                        ReasoningMode::Adaptive
                    } else {
                        ReasoningMode::OutputConfig
                    },
                    levels,
                )
                .map_err(|_| DiscoveryError::Invalid)?
                .with_provider_source(),
            );
        }
    }
    if protocol == ProviderProtocol::Gemini
        && entry.get("thinking").and_then(Value::as_bool) == Some(false)
    {
        found.reasoning = Some(
            ReasoningCapabilities::new(ReasoningMode::Unsupported, Vec::new())
                .map_err(|_| DiscoveryError::Invalid)?
                .with_provider_source(),
        );
    }
    if let Some(default) = entry
        .pointer("/effort/default_level")
        .and_then(Value::as_str)
    {
        let level = ReasoningEffort::parse(default).map_err(|_| DiscoveryError::Invalid)?;
        if let Some(caps) = found.reasoning.take() {
            found.reasoning = Some(
                caps.with_default_level(level)
                    .map_err(|_| DiscoveryError::Invalid)?,
            );
        }
    }
    Ok(found)
}

#[cfg(test)]
#[path = "model_discovery_tests.rs"]
mod tests;
