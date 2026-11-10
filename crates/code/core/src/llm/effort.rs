//! Provider effort parameters.
//!
//! Effort selects how deeply one completion thinks. It is not a tool-round
//! or step budget. Each provider receives its own parameter. `max_tokens`
//! defaults leave room for the visible answer; an explicit output cap wins.

/// Output window used at every effort level.
///
/// Effort changes how deeply the model thinks. It does not shrink the answer.
/// Anthropic counts thinking toward `max_tokens`, so a short window truncates
/// the turn once reasoning starts. A coding turn also needs room for a patch.
pub(crate) const HIGH_EFFORT_OUTPUT_TOKENS: usize = 65_536;

/// Host safety ceiling for tool rounds. Every effort level uses this same cap.
pub const TOOL_ROUND_SAFETY_CEILING: usize = 3_200;

pub(crate) fn validate_selection(
    protocol: crate::config::ProviderProtocol,
    capabilities: Option<&crate::config::ReasoningCapabilities>,
    token: Option<&str>,
) -> anyhow::Result<()> {
    if let Some(capabilities) = capabilities {
        capabilities
            .validate_protocol(protocol)
            .map_err(super::NonRetryableLlmError::new)?;
    }
    let Some(token) = token.filter(|token| *token != "default") else {
        return Ok(());
    };
    let capabilities = capabilities.ok_or_else(|| {
        super::NonRetryableLlmError::new("reasoning controls are unknown for this model")
    })?;
    capabilities
        .validate_effort(token)
        .map_err(|error| super::NonRetryableLlmError::new(error).into())
}

/// Default output-window floor; an explicit `max_tokens` may be smaller.
pub(crate) fn output_token_floor(level: &str) -> usize {
    match level {
        "none" | "minimal" | "low" | "medium" | "high" | "xhigh" | "max" => {
            HIGH_EFFORT_OUTPUT_TOKENS
        }
        _ => 0,
    }
}

pub(crate) fn resolved_max_tokens(explicit: Option<usize>, default: usize, level: &str) -> usize {
    explicit.unwrap_or_else(|| default.max(output_token_floor(level)))
}

pub(crate) fn is_glm_model(model: &str) -> bool {
    matches!(model, "glm-5.3" | "glm-5.3-flash" | "glm-5.3-flashx")
}

/// Sampling value DeepSeek applies while thinking is enabled.
///
/// Thinking mode ignores temperature. `top_p` is the parameter that still
/// takes effect, and a value below 0.95 is raised to 0.95. `0.95` is the
/// documented sample. Disabled thinking does not send it.
pub fn deepseek_thinking_top_p(level: &str) -> Option<f64> {
    match level {
        "low" | "high" | "max" => Some(0.95),
        _ => None,
    }
}

pub(crate) fn is_deepseek_model(model: &str) -> bool {
    matches!(model, "deepseek-flash" | "deepseek-v4-pro")
}

/// Legacy ID-only SDK view. Custom accounts must supply model capabilities.
pub fn native_effort_menu(model_id: &str) -> &'static [&'static str] {
    let Some((provider, model)) = model_id.split_once('/') else {
        return &[];
    };
    let protocol = match provider {
        "anthropic" | "claude" => crate::config::ProviderProtocol::Anthropic,
        "glm" | "zhipu" | "bigmodel" => crate::config::ProviderProtocol::Glm,
        "openai" | "deepseek" => crate::config::ProviderProtocol::Openai,
        "codex" => crate::config::ProviderProtocol::Codex,
        _ => return &[],
    };
    crate::config::documented_spec(protocol, model).map_or(&[], |(_, levels)| levels)
}

pub fn model_sends_native_effort(model_id: &str) -> bool {
    !native_effort_menu(model_id).is_empty()
}

#[cfg(test)]
#[path = "effort_tests.rs"]
mod tests;
