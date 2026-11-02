//! Canonical effort levels accepted by the host's ACL configuration.

use crate::error::{CodeError, Result};
use serde::{Deserialize, Serialize};

/// Host effort levels supported by the ACP session budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReasoningEffort {
    Default,
    None,
    Minimal,
    Low,
    Medium,
    High,
    Xhigh,
    Max,
    Ultracode,
}

impl ReasoningEffort {
    /// Parse a canonical level, returning `CodeError::Config` for an unsupported value.
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "default" => Ok(Self::Default),
            "none" => Ok(Self::None),
            "minimal" => Ok(Self::Minimal),
            "low" => Ok(Self::Low),
            "medium" => Ok(Self::Medium),
            "high" => Ok(Self::High),
            "xhigh" => Ok(Self::Xhigh),
            "max" => Ok(Self::Max),
            "ultracode" => Ok(Self::Ultracode),
            _ => Err(CodeError::Config(
                "reasoning_effort must be one of default, none, minimal, low, medium, high, xhigh, max, ultracode".into()
            )),
        }
    }

    /// Return the canonical ACL and ACP token.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::None => "none",
            Self::Minimal => "minimal",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Xhigh => "xhigh",
            Self::Max => "max",
            Self::Ultracode => "ultracode",
        }
    }
}
