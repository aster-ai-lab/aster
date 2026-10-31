use crate::llm::LlmConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Provider Configuration
// ============================================================================

/// Model cost information (per million tokens)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ModelCost {
    /// Input token cost
    #[serde(default)]
    pub input: f64,
    /// Output token cost
    #[serde(default)]
    pub output: f64,
    /// Cache read cost
    #[serde(default)]
    pub cache_read: f64,
    /// Cache write cost
    #[serde(default)]
    pub cache_write: f64,
}

/// Model limits
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelLimit {
    /// Maximum context tokens
    #[serde(default)]
    pub context: u32,
    /// Maximum output tokens
    #[serde(default)]
    pub output: u32,
}

/// Model modalities (input/output types)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelModalities {
    /// Supported input types
    #[serde(default)]
    pub input: Vec<String>,
    /// Supported output types
    #[serde(default)]
    pub output: Vec<String>,
}

/// Model configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelConfig {
    /// Model ID (e.g., "claude-sonnet-4-20250514")
    pub id: String,
    /// Display name
    #[serde(default)]
    pub name: String,
    /// Model family (e.g., "claude-sonnet")
    #[serde(default)]
    pub family: String,
    /// Per-model API key override
    #[serde(default)]
    pub api_key: Option<String>,
    /// Per-model base URL override
    #[serde(default)]
    pub base_url: Option<String>,
    /// Static HTTP headers for this model
    #[serde(default)]
    pub headers: HashMap<String, String>,
    /// Header name to receive the runtime session ID
    #[serde(default)]
    pub session_id_header: Option<String>,
    /// Supports file attachments
    #[serde(default)]
    pub attachment: bool,
    /// Supports reasoning/thinking
    #[serde(default)]
    pub reasoning: bool,
    /// Supports tool calling
    #[serde(default = "default_true")]
    pub tool_call: bool,
    /// Supports temperature setting
    #[serde(default = "default_true")]
    pub temperature: bool,
    /// Release date
    #[serde(default)]
    pub release_date: Option<String>,
    /// Input/output modalities
    #[serde(default)]
    pub modalities: ModelModalities,
    /// Cost information
    #[serde(default)]
    pub cost: ModelCost,
    /// Token limits
    #[serde(default)]
    pub limit: ModelLimit,
}

pub(crate) fn default_true() -> bool {
    true
}

/// Wire protocol a provider's HTTP client must speak.
///
/// This is independent of [`ProviderConfig::name`], which is only the account
/// / picker identity (`glm-01`, `work-kimi`, …).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderProtocol {
    /// OpenAI Chat Completions (and compatible gateways). Default when unset.
    #[default]
    Openai,
    /// Anthropic Messages (`/v1/messages`), including BigModel's Anthropic gateway.
    Anthropic,
    /// Zhipu / BigModel PaaS Chat Completions (`/api/paas/v4/...`).
    Glm,
    /// Cursor account Connect RPC.
    Cursor,
    /// Codex / ChatGPT plan account Responses session.
    Codex,
    /// OpenAI Responses API (non-account).
    OpenaiResponses,
}

impl ProviderProtocol {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim() {
            "openai" | "kimi" => Ok(Self::Openai),
            "anthropic" | "claude" => Ok(Self::Anthropic),
            "glm" | "zhipu" | "bigmodel" => Ok(Self::Glm),
            "cursor" => Ok(Self::Cursor),
            "codex" => Ok(Self::Codex),
            "openai-responses" => Ok(Self::OpenaiResponses),
            other => Err(format!(
                "unknown provider protocol {other:?} (closed set: openai, anthropic, glm, kimi, cursor, codex, openai-responses)"
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Openai => "openai",
            Self::Anthropic => "anthropic",
            Self::Glm => "glm",
            Self::Cursor => "cursor",
            Self::Codex => "codex",
            Self::OpenaiResponses => "openai-responses",
        }
    }

    /// Resolve the wire protocol for a provider block.
    ///
    /// Explicit `protocol = "..."` wins. Otherwise known provider names keep
    /// their historical mapping; every other name defaults to OpenAI-compatible.
    pub fn resolve(explicit: Option<Self>, provider_name: &str, base_url: Option<&str>) -> Self {
        if let Some(protocol) = explicit {
            return protocol;
        }
        let name = provider_name.trim();
        if matches!(name, "anthropic" | "claude" | "cc-switch") || name.starts_with("cc-switch-") {
            return Self::Anthropic;
        }
        if matches!(name, "glm" | "zhipu" | "bigmodel") {
            return Self::Glm;
        }
        if name == "cursor" {
            return Self::Cursor;
        }
        if name == "codex" {
            return Self::Codex;
        }
        if name == "openai-responses" {
            return Self::OpenaiResponses;
        }
        if base_url_looks_like_anthropic_messages(base_url) {
            return Self::Anthropic;
        }
        Self::Openai
    }
}

fn base_url_looks_like_anthropic_messages(base_url: Option<&str>) -> bool {
    let Some(raw) = base_url.map(str::trim).filter(|value| !value.is_empty()) else {
        return false;
    };
    if let Ok(url) = url::Url::parse(raw) {
        let host = url.host_str().unwrap_or("").to_ascii_lowercase();
        if host == "api.anthropic.com" {
            return true;
        }
        let path = url.path().trim_end_matches('/').to_ascii_lowercase();
        return path.ends_with("/anthropic") || path.contains("/anthropic/");
    }
    let lower = raw.to_ascii_lowercase();
    lower.contains("api.anthropic.com") || lower.contains("/anthropic")
}

/// Provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConfig {
    /// Provider name (e.g., "anthropic", "openai")
    pub name: String,
    /// Wire protocol. `None` means unset in ACL (resolved at client build).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol: Option<ProviderProtocol>,
    /// API key for this provider
    #[serde(default)]
    pub api_key: Option<String>,
    /// Base URL for the API
    #[serde(default)]
    pub base_url: Option<String>,
    /// Static HTTP headers for this provider
    #[serde(default)]
    pub headers: HashMap<String, String>,
    /// Header name to receive the runtime session ID
    #[serde(default)]
    pub session_id_header: Option<String>,
    /// Available models
    #[serde(default)]
    pub models: Vec<ModelConfig>,
}

/// Apply model capability flags to an LlmConfig.
///
/// - `temperature = false` → omit temperature (model ignores it, e.g. o1)
/// - `reasoning = true` + `thinking_budget` set → pass budget to client
/// - `limit.output > 0` → use as max_tokens
pub(crate) fn apply_model_caps(
    mut config: LlmConfig,
    model: &ModelConfig,
    thinking_budget: Option<usize>,
) -> LlmConfig {
    // reasoning=true + thinking_budget set → pass budget to client (Anthropic only)
    if model.reasoning {
        if let Some(budget) = thinking_budget {
            config = config.with_thinking_budget(budget);
        }
    }

    // limit.output > 0 → use as max_tokens cap
    if model.limit.output > 0 {
        config = config.with_max_tokens(model.limit.output as usize);
    }

    // temperature=false models (e.g. o1) must not receive a temperature param.
    // Store the flag so the LLM client can gate it at call time.
    if !model.temperature {
        config.disable_temperature = true;
    }

    // Declared input modalities without "image" → never send image parts.
    // An undeclared list keeps the old behavior.
    let input = &model.modalities.input;
    config.text_only_input = !input.is_empty() && !input.iter().any(|kind| kind == "image");

    config
}

impl ProviderConfig {
    /// Effective wire protocol for this provider.
    pub fn resolved_protocol(&self) -> ProviderProtocol {
        ProviderProtocol::resolve(self.protocol, &self.name, self.base_url.as_deref())
    }

    /// Find a model by ID
    pub fn find_model(&self, model_id: &str) -> Option<&ModelConfig> {
        self.models.iter().find(|m| m.id == model_id)
    }

    /// Get the effective API key for a model (model override or provider default)
    pub fn get_api_key<'a>(&'a self, model: &'a ModelConfig) -> Option<&'a str> {
        model.api_key.as_deref().or(self.api_key.as_deref())
    }

    /// Get the effective base URL for a model (model override or provider default)
    pub fn get_base_url<'a>(&'a self, model: &'a ModelConfig) -> Option<&'a str> {
        model.base_url.as_deref().or(self.base_url.as_deref())
    }

    /// Get the effective static headers for a model (provider defaults with model overrides)
    pub fn get_headers(&self, model: &ModelConfig) -> HashMap<String, String> {
        let mut headers = self.headers.clone();
        headers.extend(model.headers.clone());
        headers
    }

    /// Get the header name that should carry the runtime session ID.
    pub fn get_session_id_header<'a>(&'a self, model: &'a ModelConfig) -> Option<&'a str> {
        model
            .session_id_header
            .as_deref()
            .or(self.session_id_header.as_deref())
    }
}

#[cfg(test)]
mod protocol_resolve_tests {
    use super::ProviderProtocol;

    #[test]
    fn explicit_protocol_wins_over_name_and_url() {
        assert_eq!(
            ProviderProtocol::resolve(
                Some(ProviderProtocol::Openai),
                "anthropic",
                Some("https://api.anthropic.com")
            ),
            ProviderProtocol::Openai
        );
        assert_eq!(
            ProviderProtocol::resolve(
                Some(ProviderProtocol::Anthropic),
                "glm-01",
                Some("https://open.bigmodel.cn/api/paas/v4")
            ),
            ProviderProtocol::Anthropic
        );
    }

    #[test]
    fn unset_defaults_to_openai_compat_except_known_wires() {
        assert_eq!(
            ProviderProtocol::resolve(None, "work-kimi", None),
            ProviderProtocol::Openai
        );
        assert_eq!(
            ProviderProtocol::resolve(None, "glm", None),
            ProviderProtocol::Glm
        );
        assert_eq!(
            ProviderProtocol::resolve(
                None,
                "glm-01",
                Some("https://open.bigmodel.cn/api/anthropic")
            ),
            ProviderProtocol::Anthropic
        );
    }
}
