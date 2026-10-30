//! Usage extracted before legacy provider counters replace absent fields with zero.

use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{InputAccounting, ObservedTokens};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsageProtocol {
    OpenaiChat,
    OpenaiResponses,
    AnthropicMessages,
}

impl UsageProtocol {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "openai_chat" => Ok(Self::OpenaiChat),
            "openai_responses" => Ok(Self::OpenaiResponses),
            "anthropic_messages" => Ok(Self::AnthropicMessages),
            _ => bail!("unsupported model usage protocol: {value}"),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::OpenaiChat => "openai_chat",
            Self::OpenaiResponses => "openai_responses",
            Self::AnthropicMessages => "anthropic_messages",
        }
    }

    pub fn endpoint(self, base_url: &str) -> String {
        match self {
            Self::OpenaiChat => crate::llm::http::join_chat_completions_url(
                &crate::llm::http::normalize_base_url(base_url),
                "/v1/chat/completions",
            ),
            Self::OpenaiResponses => {
                let base = base_url.trim_end_matches('/');
                if base.ends_with("/responses") {
                    return base.to_owned();
                }
                crate::llm::http::join_chat_completions_url(
                    &crate::llm::http::normalize_base_url(
                        base.trim_end_matches("/chat/completions"),
                    ),
                    "/v1/responses",
                )
            }
            Self::AnthropicMessages => format!(
                "{}/v1/messages",
                crate::llm::http::normalize_base_url(base_url)
            ),
        }
    }
}

pub struct WireMeter {
    protocol: UsageProtocol,
    input: Option<u64>,
    output: Option<u64>,
    cache_read: Option<u64>,
    cache_write: Option<u64>,
    pub usage_complete: bool,
    pub finished: bool,
    pub failed: bool,
    pub response_model: Option<String>,
}

impl WireMeter {
    pub fn new(protocol: UsageProtocol) -> Self {
        Self {
            protocol,
            input: None,
            output: None,
            cache_read: None,
            cache_write: None,
            usage_complete: false,
            finished: false,
            failed: false,
            response_model: None,
        }
    }

    pub fn response(&mut self, value: &Value) -> Result<()> {
        let result = self.read_response(value);
        if result.is_err() {
            self.usage_complete = false;
            self.failed = true;
        }
        result
    }

    fn read_response(&mut self, value: &Value) -> Result<()> {
        self.failed = value.get("error").is_some_and(|error| !error.is_null());
        if self.protocol == UsageProtocol::OpenaiResponses {
            self.failed |= matches!(
                value.get("status").and_then(Value::as_str),
                Some("failed" | "cancelled")
            );
        }
        self.model(value)?;
        if let Some(usage) = value.get("usage").filter(|usage| !usage.is_null()) {
            self.update(usage)?;
            self.usage_complete = true;
        }
        self.finished = true;
        Ok(())
    }

    /// Returns whether this event carries generated content, including tools or reasoning.
    pub fn event(&mut self, data: &str) -> Result<bool> {
        let result = self.read_event(data);
        if result.is_err() {
            self.usage_complete = false;
            self.failed = true;
        }
        result
    }

    fn read_event(&mut self, data: &str) -> Result<bool> {
        if data == "[DONE]" {
            match self.protocol {
                UsageProtocol::OpenaiChat => {
                    self.finished = true;
                    return Ok(false);
                }
                UsageProtocol::OpenaiResponses if self.finished => return Ok(false),
                _ => bail!("unexpected stream terminator"),
            }
        }
        let value: Value = serde_json::from_str(data).context("invalid usage event JSON")?;
        match self.protocol {
            UsageProtocol::OpenaiChat => {
                self.model(&value)?;
                self.failed |= value.get("error").is_some_and(|error| !error.is_null());
                if let Some(usage) = value.get("usage").filter(|usage| !usage.is_null()) {
                    self.update(usage)?;
                    // Chat completion usage chunks report the whole request, even if DONE is lost.
                    self.usage_complete = true;
                }
                Ok(value
                    .get("choices")
                    .and_then(Value::as_array)
                    .is_some_and(|choices| {
                        choices.iter().any(|choice| {
                            choice.get("delta").is_some_and(|delta| {
                                ["content", "reasoning_content"].iter().any(|key| {
                                    delta
                                        .get(key)
                                        .and_then(Value::as_str)
                                        .is_some_and(|text| !text.is_empty())
                                }) || delta
                                    .get("tool_calls")
                                    .and_then(Value::as_array)
                                    .is_some_and(|tools| !tools.is_empty())
                            })
                        })
                    }))
            }
            UsageProtocol::AnthropicMessages => {
                match value.get("type").and_then(Value::as_str) {
                    Some("message_start") => {
                        let message = value.get("message").context("usage event lacks message")?;
                        self.model(message)?;
                        if let Some(usage) = message.get("usage") {
                            self.update(usage)?;
                        }
                    }
                    Some("message_delta") => {
                        if let Some(usage) = value.get("usage") {
                            self.update(usage)?;
                        }
                    }
                    Some("message_stop") => {
                        self.finished = true;
                        self.usage_complete = true;
                    }
                    Some("error") => self.failed = true,
                    Some("content_block_delta") => {
                        return Ok(value.get("delta").is_some_and(|delta| {
                            ["text", "thinking", "partial_json"].iter().any(|key| {
                                delta
                                    .get(key)
                                    .and_then(Value::as_str)
                                    .is_some_and(|text| !text.is_empty())
                            })
                        }))
                    }
                    Some("content_block_start") => {
                        return Ok(value.get("content_block").is_some_and(|block| {
                            block.get("type").and_then(Value::as_str) == Some("tool_use")
                                || block
                                    .get("text")
                                    .and_then(Value::as_str)
                                    .is_some_and(|text| !text.is_empty())
                        }))
                    }
                    _ => {}
                }
                Ok(false)
            }
            UsageProtocol::OpenaiResponses => {
                let kind = value
                    .get("type")
                    .and_then(Value::as_str)
                    .context("Responses event lacks type")?;
                match kind {
                    "response.created"
                    | "response.in_progress"
                    | "response.completed"
                    | "response.incomplete"
                    | "response.failed" => {
                        let response = value
                            .get("response")
                            .context("Responses event lacks response")?;
                        self.model(response)?;
                        let terminal = matches!(
                            kind,
                            "response.completed" | "response.incomplete" | "response.failed"
                        );
                        if let Some(usage) = response.get("usage").filter(|usage| !usage.is_null())
                        {
                            self.update(usage)?;
                            self.usage_complete = terminal;
                        }
                        self.finished = terminal;
                        self.failed |= kind == "response.failed"
                            || response.get("error").is_some_and(|error| !error.is_null());
                        Ok(terminal
                            && response
                                .get("output")
                                .and_then(Value::as_array)
                                .is_some_and(|items| !items.is_empty()))
                    }
                    "error" => {
                        self.failed = true;
                        Ok(false)
                    }
                    "response.output_item.added" | "response.output_item.done" => {
                        Ok(value.get("item").is_some_and(|item| item.is_object()))
                    }
                    "response.output_text.delta"
                    | "response.function_call_arguments.delta"
                    | "response.reasoning_summary_text.delta"
                    | "response.reasoning_text.delta"
                    | "response.refusal.delta" => Ok(value
                        .get("delta")
                        .and_then(Value::as_str)
                        .is_some_and(|text| !text.is_empty())),
                    _ => Ok(false),
                }
            }
        }
    }

    pub fn usage(&self) -> Result<ObservedTokens> {
        ObservedTokens::from_reported(
            self.input,
            self.output,
            self.cache_read,
            self.cache_write,
            match self.protocol {
                UsageProtocol::OpenaiChat | UsageProtocol::OpenaiResponses => {
                    InputAccounting::IncludesCache
                }
                UsageProtocol::AnthropicMessages => InputAccounting::ExcludesCache,
            },
        )
    }

    fn model(&mut self, value: &Value) -> Result<()> {
        if let Some(model) = value.get("model").filter(|value| !value.is_null()) {
            let model = model.as_str().context("response model must be a string")?;
            if model.trim().is_empty() || model.len() > 1024 {
                bail!("invalid response model");
            }
            self.response_model = Some(model.to_owned());
        }
        Ok(())
    }

    fn update(&mut self, usage: &Value) -> Result<()> {
        let usage = usage.as_object().context("usage must be an object")?;
        let (input, output, read, write, details) = match self.protocol {
            UsageProtocol::OpenaiChat => (
                "prompt_tokens",
                "completion_tokens",
                "cached_tokens",
                "cache_write_tokens",
                true,
            ),
            UsageProtocol::AnthropicMessages => (
                "input_tokens",
                "output_tokens",
                "cache_read_input_tokens",
                "cache_creation_input_tokens",
                false,
            ),
            UsageProtocol::OpenaiResponses => (
                "input_tokens",
                "output_tokens",
                "cached_tokens",
                "cache_write_tokens",
                true,
            ),
        };
        for (key, target) in [(input, &mut self.input), (output, &mut self.output)] {
            if let Some(value) = usage.get(key) {
                replace_counter(target, value)?;
            }
        }
        let cache = if details {
            match usage
                .get(if self.protocol == UsageProtocol::OpenaiResponses {
                    "input_tokens_details"
                } else {
                    "prompt_tokens_details"
                })
                .filter(|value| !value.is_null())
            {
                Some(value) => Some(
                    value
                        .as_object()
                        .context("prompt token details must be an object")?,
                ),
                None => None,
            }
        } else {
            Some(usage)
        };
        if let Some(cache) = cache {
            for (key, target) in [(read, &mut self.cache_read), (write, &mut self.cache_write)] {
                if let Some(value) = cache.get(key) {
                    replace_counter(target, value)?;
                }
            }
        }
        self.usage()?;
        Ok(())
    }
}

fn replace_counter(target: &mut Option<u64>, value: &Value) -> Result<()> {
    if value.is_null() {
        *target = None;
        return Ok(());
    }
    let value = value
        .as_u64()
        .context("usage counter must be a nonnegative integer")?;
    ensure!(
        target.is_none_or(|previous| value >= previous),
        "cumulative usage counter decreased"
    );
    *target = Some(value);
    Ok(())
}

pub struct ObservedFrame {
    pub frame: String,
    pub has_output: bool,
    pub failed: bool,
}

pub struct SseMeter {
    pub meter: WireMeter,
    decoder: crate::sse::Utf8StreamDecoder,
    buffer: String,
}

impl SseMeter {
    pub fn new(protocol: UsageProtocol) -> Self {
        Self {
            meter: WireMeter::new(protocol),
            decoder: Default::default(),
            buffer: String::new(),
        }
    }

    pub fn push(&mut self, chunk: &[u8]) -> Result<Vec<ObservedFrame>> {
        let result = (|| {
            ensure!(
                self.buffer.len().saturating_add(chunk.len()) <= 1024 * 1024,
                "model SSE frame exceeds byte limit"
            );
            self.decoder.push_to(chunk, &mut self.buffer)?;
            self.frames(false)
        })();
        if result.is_err() {
            self.meter.failed = true;
            self.meter.usage_complete = false;
        }
        result
    }

    pub fn finish(&mut self) -> Result<Vec<ObservedFrame>> {
        let result = (|| {
            self.decoder.finish()?;
            self.frames(true)
        })();
        if result.is_err() {
            self.meter.failed = true;
            self.meter.usage_complete = false;
        }
        result
    }

    fn frames(&mut self, eof: bool) -> Result<Vec<ObservedFrame>> {
        crate::sse::take_sse_events(&mut self.buffer, eof)
            .into_iter()
            .map(|frame| {
                let data = frame
                    .lines()
                    .filter_map(crate::sse::data_field_value)
                    .collect::<Vec<_>>()
                    .join("\n");
                let has_output = !data.is_empty() && self.meter.event(&data)?;
                Ok(ObservedFrame {
                    frame,
                    has_output,
                    failed: self.meter.failed,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "wire/responses_tests.rs"]
mod responses_tests;
