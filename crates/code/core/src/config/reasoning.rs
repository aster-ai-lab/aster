//! Declared reasoning controls shared by discovery, ACP and provider requests.

use super::{ProviderProtocol, ReasoningEffort};
use serde::{Deserialize, Serialize};

/// Native request control; absence of a declaration means unknown support.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReasoningMode {
    Unknown,
    Unsupported,
    Preset,
    Effort,
    Adaptive,
    OutputConfig,
    Budget,
    ThinkingLevel,
}

/// Validated model controls. Reset is an action, never a native effort level.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Declaration")]
pub struct ReasoningCapabilities {
    mode: ReasoningMode,
    levels: Vec<ReasoningEffort>,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_level: Option<ReasoningEffort>,
    #[serde(skip)]
    source: &'static str,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Declaration {
    mode: ReasoningMode,
    #[serde(default)]
    levels: Vec<ReasoningEffort>,
    #[serde(default)]
    default_level: Option<ReasoningEffort>,
}

impl TryFrom<Declaration> for ReasoningCapabilities {
    type Error = String;

    fn try_from(value: Declaration) -> Result<Self, Self::Error> {
        let caps = Self::new(value.mode, value.levels)?;
        match value.default_level {
            Some(level) => caps.with_default_level(level),
            None => Ok(caps),
        }
    }
}

impl ReasoningMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Unsupported => "unsupported",
            Self::Preset => "preset",
            Self::Effort => "effort",
            Self::Adaptive => "adaptive",
            Self::OutputConfig => "output-config",
            Self::Budget => "budget",
            Self::ThinkingLevel => "thinking-level",
        }
    }
}

impl ReasoningCapabilities {
    pub fn new(mode: ReasoningMode, levels: Vec<ReasoningEffort>) -> Result<Self, String> {
        let selectable = matches!(
            mode,
            ReasoningMode::Effort
                | ReasoningMode::Adaptive
                | ReasoningMode::OutputConfig
                | ReasoningMode::ThinkingLevel
        );
        if selectable == levels.is_empty() {
            return Err("reasoning controls require levels only for effort, adaptive or thinking-level mode".into());
        }
        for (index, level) in levels.iter().enumerate() {
            if (matches!(mode, ReasoningMode::Adaptive | ReasoningMode::OutputConfig)
                && matches!(level, ReasoningEffort::None | ReasoningEffort::Minimal))
                || (mode == ReasoningMode::ThinkingLevel
                    && matches!(
                        level,
                        ReasoningEffort::None | ReasoningEffort::Xhigh | ReasoningEffort::Max
                    ))
                || matches!(level, ReasoningEffort::Default | ReasoningEffort::Ultracode)
                || levels[..index].contains(level)
            {
                return Err("reasoning levels must be distinct native values: none, minimal, low, medium, high, xhigh, max".into());
            }
        }
        Ok(Self {
            mode,
            levels,
            default_level: None,
            source: "declared",
        })
    }

    pub fn unknown() -> Self {
        Self {
            mode: ReasoningMode::Unknown,
            levels: Vec::new(),
            default_level: None,
            source: "declared",
        }
    }

    pub fn validate_protocol(&self, protocol: ProviderProtocol) -> Result<(), String> {
        let supported = match self.mode {
            ReasoningMode::Unknown | ReasoningMode::Unsupported => true,
            ReasoningMode::Preset => protocol == ProviderProtocol::Cursor,
            ReasoningMode::Effort => matches!(
                protocol,
                ProviderProtocol::Openai
                    | ProviderProtocol::OpenaiResponses
                    | ProviderProtocol::Codex
                    | ProviderProtocol::Glm
                    | ProviderProtocol::Anthropic
            ),
            ReasoningMode::Adaptive | ReasoningMode::OutputConfig => {
                protocol == ProviderProtocol::Anthropic
            }
            ReasoningMode::Budget => matches!(
                protocol,
                ProviderProtocol::Anthropic | ProviderProtocol::Gemini
            ),
            ReasoningMode::ThinkingLevel => protocol == ProviderProtocol::Gemini,
        };
        if supported {
            Ok(())
        } else {
            Err(format!(
                "reasoning mode {} is incompatible with protocol {}",
                self.mode.as_str(),
                protocol.as_str()
            ))
        }
    }

    pub fn preset() -> Self {
        Self {
            mode: ReasoningMode::Preset,
            levels: Vec::new(),
            default_level: None,
            source: "account",
        }
    }

    pub fn mode(&self) -> ReasoningMode {
        self.mode
    }

    pub fn default_level(&self) -> Option<ReasoningEffort> {
        self.default_level
    }

    pub fn with_default_level(mut self, level: ReasoningEffort) -> Result<Self, String> {
        if !self.levels.contains(&level) {
            return Err("model default must be an offered native effort level".into());
        }
        self.default_level = Some(level);
        Ok(self)
    }

    pub fn levels(&self) -> &[ReasoningEffort] {
        &self.levels
    }

    pub fn source(&self) -> &'static str {
        self.source
    }

    pub fn with_account_source(mut self) -> Self {
        self.source = "account";
        self
    }

    pub(crate) fn with_provider_source(mut self) -> Self {
        self.source = "provider";
        self
    }

    pub(crate) fn with_source(mut self, source: &'static str) -> Self {
        self.source = source;
        self
    }

    pub fn supports(&self, token: &str) -> bool {
        self.levels.iter().any(|level| level.as_str() == token)
    }

    /// Reject explicit unsupported selections before contacting a provider.
    pub fn validate_effort(&self, token: &str) -> Result<(), String> {
        if self.supports(token) {
            return Ok(());
        }
        Err(format!(
            "unsupported reasoning effort {token:?}; offered levels: {}",
            self.levels
                .iter()
                .map(|level| level.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ))
    }
}

/// Custom gateways need an explicit declaration; a compatible protocol alone does not declare vendor extensions.
pub(crate) fn documented_at(
    protocol: ProviderProtocol,
    model: &str,
    base_url: Option<&str>,
) -> Option<ReasoningCapabilities> {
    if let Some(base_url) = base_url {
        let url = url::Url::parse(base_url).ok()?;
        let host = url.host_str()?;
        let official = match protocol {
            ProviderProtocol::Openai | ProviderProtocol::OpenaiResponses => matches!(
                host,
                "api.openai.com"
                    | "api.deepseek.com"
                    | "open.bigmodel.cn"
                    | "api.z.ai"
                    | "api.kimi.com"
                    | "api.kimi.ai"
            ),
            ProviderProtocol::Anthropic => matches!(
                host,
                "api.anthropic.com" | "open.bigmodel.cn" | "api.z.ai" | "api.deepseek.com"
            ),
            ProviderProtocol::Glm => matches!(host, "open.bigmodel.cn" | "api.z.ai"),
            ProviderProtocol::Gemini => host == "generativelanguage.googleapis.com",
            ProviderProtocol::Codex => host == "chatgpt.com",
            ProviderProtocol::Cursor => false,
        };
        if !official {
            return None;
        }
    }
    documented_reasoning(protocol, model)
}

/// Exact canonical IDs only. New catalog entries remain unknown until declared.
pub fn documented_reasoning(
    protocol: ProviderProtocol,
    model: &str,
) -> Option<ReasoningCapabilities> {
    let (mode, levels) = documented_spec(protocol, model)?;
    // Every token in the source-owned table belongs to the closed effort enum.
    let levels = levels
        .iter()
        .map(|level| {
            ReasoningEffort::parse(level)
                .expect("documented catalog contains canonical effort values")
        })
        .collect();
    Some(ReasoningCapabilities {
        mode,
        levels,
        default_level: match model {
            "glm-5.2" | "glm-5.3" | "glm-5.3-flash" | "glm-5.3-flashx" | "kimi-for-coding" => {
                Some(ReasoningEffort::Max)
            }
            "k3.1-pro" | "k3.1-pro-highspeed" | "k3" | "k3-256k" | "claude-opus-4-6"
            | "claude-sonnet-4-6" | "claude-sonnet-5-5" | "deepseek-flash" | "deepseek-v4-pro" => {
                Some(ReasoningEffort::High)
            }
            "gpt-5.4" => Some(ReasoningEffort::None),
            "gpt-6.1-sol" | "gpt-6-sol" | "gpt-6-luna" | "claude-opus-5-5" | "claude-haiku-5-5" => {
                Some(ReasoningEffort::Medium)
            }
            _ => None,
        },
        source: "documented",
    })
}

pub(crate) fn documented_spec(
    protocol: ProviderProtocol,
    model: &str,
) -> Option<(ReasoningMode, &'static [&'static str])> {
    use ProviderProtocol as Protocol;
    use ReasoningMode as Mode;
    let (mode, levels): (Mode, &[&str]) = match (protocol, model) {
        (Protocol::Openai | Protocol::OpenaiResponses | Protocol::Codex, "gpt-5.4") => {
            (Mode::Effort, &["none", "low", "medium", "high", "xhigh"])
        }
        (
            Protocol::Openai | Protocol::OpenaiResponses | Protocol::Codex,
            "gpt-6-astra" | "gpt-6.1-sol",
        ) => (Mode::Effort, &["low", "medium", "high", "xhigh", "max"]),
        (
            Protocol::Openai | Protocol::OpenaiResponses | Protocol::Codex,
            "gpt-6-sol" | "gpt-6-luna",
        ) => (
            Mode::Effort,
            &["none", "low", "medium", "high", "xhigh", "max"],
        ),
        (Protocol::Openai | Protocol::OpenaiResponses, "gpt-4o" | "gpt-4o-mini") => {
            (Mode::Unsupported, &[])
        }
        (
            Protocol::Openai | Protocol::Anthropic | Protocol::Glm,
            "glm-5.3" | "glm-5.3-flash" | "glm-5.3-flashx",
        ) => (Mode::Effort, &["low", "high", "max"]),
        (Protocol::Openai | Protocol::OpenaiResponses, "deepseek-flash" | "deepseek-v4-pro") => {
            (Mode::Effort, &["none", "low", "high", "max"])
        }
        (
            Protocol::Anthropic,
            "claude-opus-5-5" | "claude-sonnet-5-5" | "claude-haiku-5-5" | "claude-opus-4-7"
            | "claude-opus-4-8" | "claude-opus-5" | "claude-sonnet-5",
        ) => (Mode::Adaptive, &["low", "medium", "high", "xhigh", "max"]),
        (Protocol::Anthropic, "claude-opus-4-6" | "claude-sonnet-4-6") => {
            (Mode::Adaptive, &["low", "medium", "high", "max"])
        }
        (
            Protocol::Anthropic,
            "claude-sonnet-4-20250514" | "claude-opus-4-20250514" | "claude-3-7-sonnet-20250219",
        ) => (Mode::Budget, &[]),
        (Protocol::Gemini, "gemini-3-pro-preview") => (Mode::ThinkingLevel, &["low", "high"]),
        (Protocol::Gemini, "gemini-3.1-pro-preview" | "gemini-3.8-flash" | "gemini-4-argon") => {
            (Mode::ThinkingLevel, &["low", "medium", "high"])
        }
        (
            Protocol::Gemini,
            "gemini-3-flash-preview" | "gemini-3.6-flash" | "gemini-3.5-flash-lite",
        ) => (Mode::ThinkingLevel, &["minimal", "low", "medium", "high"]),
        (Protocol::Gemini, "gemini-2.5-pro" | "gemini-2.5-flash" | "gemini-2.5-flash-lite") => {
            (Mode::Budget, &[])
        }
        (
            Protocol::Openai | Protocol::OpenaiResponses,
            "k3.1-pro" | "k3.1-pro-highspeed" | "k3" | "k3-256k" | "kimi-for-coding",
        ) => (Mode::Effort, &["low", "high", "max"]),
        (Protocol::Openai | Protocol::OpenaiResponses, "kimi-for-coding-highspeed") => {
            (Mode::Unsupported, &[])
        }
        (Protocol::Openai | Protocol::Anthropic | Protocol::Glm, "glm-5.2") => {
            (Mode::Effort, &["none", "high", "max"])
        }
        _ => return None,
    };
    Some((mode, levels))
}

#[cfg(test)]
#[path = "reasoning_tests.rs"]
mod tests;
