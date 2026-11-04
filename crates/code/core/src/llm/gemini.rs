//! Native Gemini GenerateContent transport and durable tool-round context.

mod state;
pub use state::GeminiState;
mod request;
mod transport;

use super::{HttpClient, LlmConfig};
use std::sync::Arc;

const MAX_BODY_BYTES: usize = 8 * 1024 * 1024;

/// Native Gemini request/response failures without provider payload disclosure.
#[derive(Debug, thiserror::Error)]
pub enum GeminiError {
    #[error("{0}")]
    Invalid(&'static str),
    #[error("Gemini native context no longer matches this message or model")]
    StateMismatch,
    #[error("Gemini JSON encoding failed")]
    Json(#[from] serde_json::Error),
    #[error("Gemini context identity could not be computed")]
    Identity(#[source] crate::session_checkpoint::SessionCheckpointError),
    #[error("Gemini API returned HTTP {status}")]
    Http { status: u16 },
    #[error("Gemini rejected the prompt")]
    Blocked,
    #[error("Gemini usage could not be read")]
    Metering(#[source] anyhow::Error),
}

#[derive(Clone)]
pub(crate) struct GeminiClient {
    config: LlmConfig,
    http: Arc<dyn HttpClient>,
}

impl GeminiClient {
    pub(crate) fn new(
        mut config: LlmConfig,
        headers: std::collections::HashMap<String, String>,
        http: Arc<dyn HttpClient>,
    ) -> Self {
        config.headers = headers;
        config.http_override = None;
        Self { config, http }
    }

    fn max_tokens(&self) -> usize {
        self.config.max_tokens.unwrap_or(8192)
    }

    fn provider_identity(&self) -> &str {
        self.config
            .headers
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case("x-aster-provider-id"))
            .map_or(self.config.provider.as_str(), |(_, value)| value.as_str())
    }
}

pub(crate) fn refuse_foreign_state(messages: &[super::Message]) -> anyhow::Result<()> {
    if messages
        .iter()
        .flat_map(|message| &message.content)
        .any(|block| matches!(block, super::ContentBlock::GeminiState { .. }))
    {
        return Err(anyhow::Error::new(GeminiError::StateMismatch).context(
            super::NonRetryableLlmError::new(
                "Native Gemini context cannot be sent to another provider",
            ),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
