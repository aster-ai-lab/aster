//! Error markers shared by LLM clients and the agent retry loop.

use thiserror::Error;

/// A provider error that cannot succeed through an immediate retry.
///
/// The message must be safe to show directly to an end user. Provider clients
/// should use this only for precise terminal conditions, such as an exhausted
/// account quota, and not for ordinary transient rate limits.
const MAX_PROVIDER_ERROR_MESSAGE_BYTES: usize = 4 * 1024;

/// Transient provider exhaustion retained for the exact live Run.
///
/// Contains only status and attempt count, never response bodies or credentials.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryableProviderFailure {
    status: u16,
    attempts: u32,
}

impl RetryableProviderFailure {
    /// Classify a typed provider failure, including errors with anyhow context.
    /// Permanent account/quota refusals are not transient rate limits.
    pub fn from_error(error: &anyhow::Error) -> Option<Self> {
        let error = error.downcast_ref::<crate::retry::RetryExhaustedError>()?;
        matches!(error.status_code(), 408 | 429 | 500..=599).then_some(Self {
            status: error.status_code(),
            attempts: error.attempts(),
        })
    }

    pub fn status_code(self) -> u16 {
        self.status
    }

    pub fn attempts(self) -> u32 {
        self.attempts
    }
}

#[cfg(test)]
#[path = "provider_failure_tests.rs"]
mod provider_failure_tests;

#[derive(Debug, Error)]
#[error("{message}")]
pub struct NonRetryableLlmError {
    message: String,
    provider: Option<String>,
    status: Option<u16>,
    context_overflow: bool,
}

impl NonRetryableLlmError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: bound_message(message.into()),
            provider: None,
            status: None,
            context_overflow: false,
        }
    }

    /// Construct a terminal provider response without relying on rendered
    /// error text for retry decisions. The response body is bounded because
    /// provider gateways may return arbitrarily large diagnostic payloads.
    pub fn from_status(provider: impl Into<String>, status: u16, body: impl Into<String>) -> Self {
        let provider = provider.into();
        let body = body.into();
        let context_overflow = is_context_length_exceeded(status, &body);
        // Boyue reports an empty account as HTTP 401 "Invalid token". That is
        // no balance, not a missing child credential.
        let message = if boyue_account_has_no_balance(&provider, status, &body) {
            format!("{provider} API account has no balance (HTTP {status})")
        } else {
            format!("{provider} API returned HTTP {status}: {body}")
        };
        let mut error = Self::new(message);
        error.provider = Some(provider);
        error.status = Some(status);
        error.context_overflow = context_overflow;
        error
    }

    /// Provider label, when the error came from an HTTP response.
    pub fn provider(&self) -> Option<&str> {
        self.provider.as_deref()
    }

    /// HTTP status, when the error came from an HTTP response.
    pub fn status(&self) -> Option<u16> {
        self.status
    }

    /// True when the provider rejected the prompt for exceeding its context window.
    pub fn is_context_overflow(&self) -> bool {
        self.context_overflow
    }
}

fn is_context_length_exceeded(status: u16, body: &str) -> bool {
    if !matches!(status, 400 | 413 | 422) {
        return false;
    }
    let lower = body.to_ascii_lowercase();
    lower.contains("context_length_exceeded")
        || lower.contains("maximum context length")
        || lower.contains("prompt is too long")
        || lower.contains("input is too long")
        || lower.contains("too many tokens")
        || lower.contains("exceeds the context window")
        || lower.contains("exceeds the maximum number of tokens")
        || lower.contains("request_too_large")
        || lower.contains("token limit exceeded")
        || ((lower.contains("prompt_tokens") || lower.contains("input token count"))
            && lower.contains("exceed"))
}

fn boyue_account_has_no_balance(provider: &str, status: u16, body: &str) -> bool {
    if !provider.to_ascii_lowercase().contains("boyue") {
        return false;
    }
    if !matches!(status, 401..=403) {
        return false;
    }
    let body = body.to_ascii_lowercase();
    body.contains("invalid token")
        || body.contains("insufficient balance")
        || body.contains("no balance")
        || body.contains("quota")
}

fn bound_message(message: String) -> String {
    if message.len() <= MAX_PROVIDER_ERROR_MESSAGE_BYTES {
        return message;
    }
    let mut bounded = String::with_capacity(MAX_PROVIDER_ERROR_MESSAGE_BYTES);
    for character in message.chars() {
        if bounded.len() + character.len_utf8() + 3 > MAX_PROVIDER_ERROR_MESSAGE_BYTES {
            break;
        }
        bounded.push(character);
    }
    bounded.push('…');
    bounded
}

pub(crate) fn non_retryable_llm_error_message(error: &anyhow::Error) -> Option<&str> {
    if let Some(error) = error.downcast_ref::<NonRetryableLlmError>() {
        return Some(error.message.as_str());
    }
    error
        .downcast_ref::<crate::retry::RetryExhaustedError>()
        .map(crate::retry::RetryExhaustedError::non_retryable_message)
}

pub(crate) fn context_overflow_llm_error_message(error: &anyhow::Error) -> Option<&str> {
    error
        .downcast_ref::<NonRetryableLlmError>()
        .filter(|err| err.is_context_overflow())
        .map(|err| err.message.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_survives_anyhow_context() {
        let error = anyhow::Error::new(NonRetryableLlmError::new("quota exhausted"))
            .context("LLM call failed");

        assert_eq!(
            non_retryable_llm_error_message(&error),
            Some("quota exhausted")
        );
    }

    #[test]
    fn boyue_invalid_token_means_the_account_has_no_balance() {
        let error = NonRetryableLlmError::from_status("boyue", 401, "Invalid token");
        assert_eq!(error.provider(), Some("boyue"));
        assert_eq!(error.status(), Some(401));
        let message = error.to_string();
        assert!(message.contains("no balance"), "{message}");
        assert!(!message.to_ascii_lowercase().contains("invalid token"));
    }

    #[test]
    fn another_provider_invalid_token_stays_an_auth_error() {
        let error = NonRetryableLlmError::from_status("openai", 401, "Invalid token");
        assert!(error.to_string().contains("Invalid token"));
        assert!(!error.to_string().contains("no balance"));
    }

    #[test]
    fn provider_status_is_typed_and_message_is_bounded() {
        let error = NonRetryableLlmError::from_status("deepseek", 402, "x".repeat(10_000));
        assert_eq!(error.provider(), Some("deepseek"));
        assert_eq!(error.status(), Some(402));
        assert!(error.to_string().len() <= MAX_PROVIDER_ERROR_MESSAGE_BYTES);
        assert!(error.to_string().ends_with('…'));
    }

    #[test]
    fn retry_exhaustion_is_terminal_at_the_agent_boundary() {
        let error = anyhow::Error::new(crate::retry::RetryExhaustedError::new(
            3,
            reqwest::StatusCode::TOO_MANY_REQUESTS,
            "rate limited",
        ));
        let message = non_retryable_llm_error_message(&error)
            .expect("retry exhaustion must be terminal at the Agent boundary");
        assert!(message.contains("LLM API request failed after 3 attempts"));
        assert!(message.contains("429"));
        assert!(message.contains("rate limited"));
    }

    #[test]
    fn context_length_exceeded_status_is_typed_overflow_across_providers() {
        for (provider, status, body) in [
            (
                "openai",
                400,
                r#"{"error":{"code":"context_length_exceeded","message":"This model's maximum context length is 128000 tokens."}}"#,
            ),
            (
                "anthropic",
                400,
                r#"{"error":{"type":"invalid_request_error","message":"prompt is too long: 210000 tokens > 200000 maximum"}}"#,
            ),
            ("gemini", 413, "input is too long; request_too_large"),
            (
                "azure",
                422,
                "prompt_tokens count exceeds the context window limit",
            ),
        ] {
            let err = NonRetryableLlmError::from_status(provider, status, body);
            assert!(
                err.is_context_overflow(),
                "{provider} ({status}) must be classified as context overflow"
            );
            let boxed = anyhow::Error::new(err);
            assert!(context_overflow_llm_error_message(&boxed).is_some());
        }

        let ordinary = NonRetryableLlmError::from_status("openai", 400, "invalid tool schema");
        assert!(!ordinary.is_context_overflow());
        assert!(context_overflow_llm_error_message(&anyhow::Error::new(ordinary)).is_none());
    }
}
