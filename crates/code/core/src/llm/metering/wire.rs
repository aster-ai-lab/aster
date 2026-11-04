//! Usage extracted before legacy provider counters replace absent fields with zero.

use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{InputAccounting, ObservedTokens};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsageProtocol {
    OpenaiChat,
    OpenaiCompletions,
    OpenaiEmbeddings,
    OpenaiResponses,
    AnthropicMessages,
    /// Gemini GenerateContent JSON and SSE, including tools and thought-token usage.
    Gemini,
}

impl UsageProtocol {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "openai_chat" => Ok(Self::OpenaiChat),
            "openai_completions" => Ok(Self::OpenaiCompletions),
            "openai_embeddings" => Ok(Self::OpenaiEmbeddings),
            "openai_responses" => Ok(Self::OpenaiResponses),
            "anthropic_messages" => Ok(Self::AnthropicMessages),
            "gemini" => Ok(Self::Gemini),
            _ => bail!("unsupported model usage protocol: {value}"),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::OpenaiChat => "openai_chat",
            Self::OpenaiCompletions => "openai_completions",
            Self::OpenaiEmbeddings => "openai_embeddings",
            Self::OpenaiResponses => "openai_responses",
            Self::AnthropicMessages => "anthropic_messages",
            Self::Gemini => "gemini",
        }
    }

    /// Fixed message endpoint, or the unchanged Gemini base for model-aware dispatch.
    pub fn endpoint(self, base_url: &str) -> String {
        match self {
            Self::OpenaiChat => crate::llm::http::join_chat_completions_url(
                &crate::llm::http::normalize_base_url(base_url),
                "/v1/chat/completions",
            ),
            Self::OpenaiCompletions | Self::OpenaiEmbeddings => {
                crate::llm::http::join_chat_completions_url(
                    &crate::llm::http::normalize_base_url(base_url),
                    if self == Self::OpenaiCompletions {
                        "/v1/completions"
                    } else {
                        "/v1/embeddings"
                    },
                )
            }
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
            // Gemini selects a model and method in the URL at dispatch time.
            Self::Gemini => base_url.to_owned(),
        }
    }
}

/// Observe provider counters without retaining message content.
#[derive(Clone)]
pub struct WireMeter {
    protocol: UsageProtocol,
    input: Option<u64>,
    output: Option<u64>,
    cache_read: Option<u64>,
    cache_write: Option<u64>,
    gemini_tool_input: Option<u64>,
    gemini_thoughts: Option<u64>,
    gemini_total: Option<u64>,
    gemini_saw_usage: bool,
    gemini_saw_finish: bool,
    gemini_open_candidates: u8,
    pub usage_complete: bool,
    pub finished: bool,
    pub failed: bool,
    pub response_model: Option<String>,
}

impl std::fmt::Debug for WireMeter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("WireMeter")
            .field("protocol", &self.protocol)
            .field("usage_complete", &self.usage_complete)
            .field("finished", &self.finished)
            .field("failed", &self.failed)
            .finish_non_exhaustive()
    }
}

impl WireMeter {
    pub fn new(protocol: UsageProtocol) -> Self {
        Self {
            protocol,
            input: None,
            output: (protocol == UsageProtocol::OpenaiEmbeddings).then_some(0),
            cache_read: None,
            cache_write: None,
            gemini_tool_input: None,
            gemini_thoughts: None,
            gemini_total: None,
            gemini_saw_usage: false,
            gemini_saw_finish: false,
            gemini_open_candidates: 0,
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
        let usage_key = if self.protocol == UsageProtocol::Gemini {
            "usageMetadata"
        } else {
            "usage"
        };
        if let Some(usage) = value.get(usage_key).filter(|usage| !usage.is_null()) {
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
                UsageProtocol::OpenaiChat | UsageProtocol::OpenaiCompletions => {
                    self.finished = true;
                    return Ok(false);
                }
                UsageProtocol::OpenaiResponses if self.finished => return Ok(false),
                _ => bail!("unexpected stream terminator"),
            }
        }
        let value: Value = serde_json::from_str(data).context("invalid usage event JSON")?;
        match self.protocol {
            UsageProtocol::OpenaiChat | UsageProtocol::OpenaiCompletions => {
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
                            (self.protocol == UsageProtocol::OpenaiCompletions
                                && choice
                                    .get("text")
                                    .and_then(Value::as_str)
                                    .is_some_and(|text| !text.is_empty()))
                                || choice.get("delta").is_some_and(|delta| {
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
            UsageProtocol::Gemini => self.gemini_event(&value),
            UsageProtocol::OpenaiEmbeddings => bail!("embeddings do not support streaming"),
        }
    }

    pub fn usage(&self) -> Result<ObservedTokens> {
        if self.protocol == UsageProtocol::Gemini {
            let mut input = self
                .input
                .zip(self.gemini_tool_input)
                .map(|(prompt, tools)| {
                    prompt
                        .checked_add(tools)
                        .context("Gemini input token overflow")
                })
                .transpose()?;
            let mut output = self
                .output
                .zip(self.gemini_thoughts)
                .map(|(candidates, thoughts)| {
                    candidates
                        .checked_add(thoughts)
                        .context("Gemini output token overflow")
                })
                .transpose()?;
            match (self.gemini_total, input, output) {
                (Some(total), Some(known), None) => {
                    output = Some(
                        total
                            .checked_sub(known)
                            .context("Gemini total is below input")?,
                    )
                }
                (Some(total), None, Some(known)) => {
                    input = Some(
                        total
                            .checked_sub(known)
                            .context("Gemini total is below output")?,
                    )
                }
                (Some(0), None, None) => {
                    input = Some(0);
                    output = Some(0);
                }
                _ => {}
            }
            for (aggregate, components) in [
                (input, [self.input, self.gemini_tool_input]),
                (output, [self.output, self.gemini_thoughts]),
            ] {
                if let Some(aggregate) = aggregate {
                    ensure!(
                        components
                            .into_iter()
                            .flatten()
                            .all(|component| component <= aggregate),
                        "Gemini component exceeds aggregate"
                    );
                }
            }
            if let Some((total, (input, output))) = self.gemini_total.zip(input.zip(output)) {
                ensure!(
                    input.checked_add(output) == Some(total),
                    "Gemini usage total disagrees"
                );
            }
            let prompt = self.input.or_else(|| {
                input
                    .zip(self.gemini_tool_input)
                    .and_then(|(input, tools)| input.checked_sub(tools))
            });
            if let Some((cached, prompt)) = self.cache_read.zip(prompt) {
                ensure!(cached <= prompt, "Gemini cache exceeds prompt input");
            }
            return ObservedTokens::from_reported(
                input,
                output,
                self.cache_read,
                self.cache_write,
                InputAccounting::IncludesCache,
            );
        }
        ObservedTokens::from_reported(
            self.input,
            self.output,
            self.cache_read,
            self.cache_write,
            match self.protocol {
                UsageProtocol::OpenaiChat
                | UsageProtocol::OpenaiCompletions
                | UsageProtocol::OpenaiEmbeddings
                | UsageProtocol::OpenaiResponses
                | UsageProtocol::Gemini => InputAccounting::IncludesCache,
                UsageProtocol::AnthropicMessages => InputAccounting::ExcludesCache,
            },
        )
    }

    fn model(&mut self, value: &Value) -> Result<()> {
        let key = if self.protocol == UsageProtocol::Gemini {
            "modelVersion"
        } else {
            "model"
        };
        if let Some(model) = value.get(key).filter(|value| !value.is_null()) {
            let model = model.as_str().context("response model must be a string")?;
            if model.trim().is_empty() || model.len() > 1024 {
                bail!("invalid response model");
            }
            self.response_model = Some(model.to_owned());
        }
        Ok(())
    }

    fn update(&mut self, usage: &Value) -> Result<()> {
        // A malformed later snapshot must not overwrite already observed consumption.
        let mut next = self.clone();
        next.update_counters(usage)?;
        *self = next;
        Ok(())
    }

    fn update_counters(&mut self, usage: &Value) -> Result<()> {
        let usage = usage.as_object().context("usage must be an object")?;
        if self.protocol == UsageProtocol::Gemini {
            for (key, target, default_zero) in [
                ("promptTokenCount", &mut self.input, false),
                ("candidatesTokenCount", &mut self.output, false),
                ("cachedContentTokenCount", &mut self.cache_read, true),
                ("toolUsePromptTokenCount", &mut self.gemini_tool_input, true),
                ("thoughtsTokenCount", &mut self.gemini_thoughts, true),
                ("totalTokenCount", &mut self.gemini_total, false),
            ] {
                if let Some(value) = usage.get(key) {
                    replace_counter(target, value)?;
                } else if !self.gemini_saw_usage && default_zero {
                    // Gemini's omitted additive proto counters are zero; explicit null stays unknown.
                    *target = Some(0);
                }
            }
            // GenerateContent reads caches; cache creation is a separate API operation.
            self.cache_write = Some(0);
            self.gemini_saw_usage = true;
            self.usage()?;
            return Ok(());
        }
        if self.protocol == UsageProtocol::OpenaiEmbeddings {
            if let Some(value) = usage.get("prompt_tokens") {
                replace_counter(&mut self.input, value)?;
            }
            if let Some(value) = usage.get("total_tokens") {
                let mut total = None;
                replace_counter(&mut total, value)?;
                if let (Some(input), Some(total)) = (self.input, total) {
                    ensure!(input == total, "embedding token counters disagree");
                }
                if self.input.is_none() {
                    self.input = total;
                }
            }
        }
        let (input, output, read, write, details) = match self.protocol {
            UsageProtocol::OpenaiChat
            | UsageProtocol::OpenaiCompletions
            | UsageProtocol::OpenaiEmbeddings => (
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
            UsageProtocol::Gemini => unreachable!("Gemini usage returned above"),
        };
        for (key, target) in [(input, &mut self.input), (output, &mut self.output)] {
            if let Some(value) = usage.get(key) {
                replace_counter(target, value)?;
            }
        }
        if self.protocol == UsageProtocol::OpenaiEmbeddings {
            ensure!(
                self.output.is_none_or(|output| output == 0),
                "embedding output token counter is nonzero"
            );
            self.output = Some(0);
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

    fn gemini_event(&mut self, value: &Value) -> Result<bool> {
        self.model(value)?;
        self.failed |= value.get("error").is_some_and(|value| !value.is_null());
        if let Some(usage) = value.get("usageMetadata").filter(|value| !value.is_null()) {
            self.update(usage)?;
        }
        let mut has_output = false;
        if let Some(candidates) = value.get("candidates") {
            for candidate in candidates
                .as_array()
                .context("Gemini candidates must be an array")?
            {
                let index = candidate
                    .get("index")
                    .map(|value| value.as_u64().context("invalid Gemini candidate index"))
                    .transpose()?
                    .unwrap_or(0);
                let shift = u32::try_from(index).context("invalid Gemini candidate index")?;
                let candidate_mask = 1_u8
                    .checked_shl(shift)
                    .context("Gemini supports at most eight candidates")?;
                if let Some(parts) = candidate
                    .get("content")
                    .and_then(|value| value.get("parts"))
                {
                    let parts = parts.as_array().context("Gemini parts must be an array")?;
                    has_output |= parts
                        .iter()
                        .any(|part| part.as_object().is_some_and(|part| !part.is_empty()));
                }
                self.gemini_open_candidates |= candidate_mask;
                if let Some(reason) = candidate
                    .get("finishReason")
                    .filter(|value| !value.is_null())
                {
                    let reason = reason.as_str().context("invalid Gemini finish reason")?;
                    if !reason.is_empty() && reason != "FINISH_REASON_UNSPECIFIED" {
                        self.gemini_open_candidates &= !candidate_mask;
                        self.gemini_saw_finish = true;
                    }
                }
            }
        }
        if let Some(reason) = value
            .get("promptFeedback")
            .and_then(|value| value.get("blockReason"))
        {
            let reason = reason.as_str().context("invalid Gemini block reason")?;
            self.gemini_saw_finish |= !reason.is_empty() && reason != "BLOCK_REASON_UNSPECIFIED";
        }
        Ok(has_output)
    }

    fn finish_stream(&mut self) {
        if self.protocol == UsageProtocol::Gemini && !self.failed {
            // Gemini has no message-stop event; EOF also lets trailing usage and other candidates arrive.
            self.finished = self.gemini_saw_finish && self.gemini_open_candidates == 0;
            self.usage_complete = self.finished && self.gemini_saw_usage;
        }
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

/// Maximum raw bytes retained for one incomplete SSE frame, including fields
/// and delimiters. A following LF after a committed blank CR line is not retained.
pub const MAX_SSE_FRAME_BYTES: usize = 16 * 1024 * 1024;

/// Incremental usage observation with a bounded raw SSE frame buffer.
pub struct SseMeter {
    pub meter: WireMeter,
    decoder: crate::sse::Utf8StreamDecoder,
    buffer: String,
    scan: crate::sse::SseEventScan,
}

impl std::fmt::Debug for SseMeter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SseMeter")
            .field("buffered_bytes", &self.buffered_bytes())
            .field("finished", &self.meter.finished)
            .field("failed", &self.meter.failed)
            .finish_non_exhaustive()
    }
}

impl SseMeter {
    pub fn new(protocol: UsageProtocol) -> Self {
        Self {
            meter: WireMeter::new(protocol),
            decoder: Default::default(),
            buffer: String::new(),
            scan: Default::default(),
        }
    }

    /// Observe complete events, independent of transport chunk boundaries.
    /// Invalid UTF-8, event data or counters, and oversized frames return an error;
    /// previously observed counters survive, but usage is marked incomplete.
    pub fn push(&mut self, chunk: &[u8]) -> Result<Vec<ObservedFrame>> {
        let result = (|| {
            let mut frames = Vec::new();
            let mut remaining = chunk;
            while !remaining.is_empty() {
                let room = MAX_SSE_FRAME_BYTES - self.buffered_bytes();
                ensure!(room > 0, "model SSE frame exceeds byte limit");
                let (next, rest) = remaining.split_at(remaining.len().min(room));
                self.decoder.push_to(next, &mut self.buffer)?;
                frames.extend(self.frames(false)?);
                remaining = rest;
            }
            Ok(frames)
        })();
        if result.is_err() {
            self.meter.failed = true;
            self.meter.usage_complete = false;
        }
        result
    }

    /// Raw bytes awaiting a frame delimiter, including an incomplete UTF-8 suffix.
    pub fn buffered_bytes(&self) -> usize {
        self.buffer.len() + self.decoder.pending_bytes()
    }

    /// Flush a final CR delimiter and reject incomplete UTF-8. Unterminated
    /// events remain buffered; callers must also check `meter.finished` at EOF.
    /// Gemini completes here after all seen candidates finish, retaining trailing usage.
    pub fn finish(&mut self) -> Result<Vec<ObservedFrame>> {
        let result = (|| {
            self.decoder.finish()?;
            let frames = self.frames(true)?;
            self.meter.finish_stream();
            Ok(frames)
        })();
        if result.is_err() {
            self.meter.failed = true;
            self.meter.usage_complete = false;
        }
        result
    }

    fn frames(&mut self, eof: bool) -> Result<Vec<ObservedFrame>> {
        crate::sse::take_sse_events_scanned(&mut self.buffer, &mut self.scan, eof)
            .into_iter()
            .map(|frame| {
                let data = frame
                    .split(['\r', '\n'])
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

#[cfg(test)]
#[path = "wire/gemini_tests.rs"]
mod gemini_tests;
