use super::{state::field, GeminiClient, GeminiError, GeminiState, MAX_BODY_BYTES};
use crate::llm::{
    metering::wire::{SseMeter, UsageProtocol, WireMeter},
    structured, ContentBlock, LlmClient, LlmResponse, LlmResponseMeta, Message,
    ModelGenerationPool, StreamEvent, TokenUsage, ToolDefinition,
};
use crate::retry::AttemptOutcome;
use anyhow::Result;
use async_trait::async_trait;
use futures::StreamExt;
use serde_json::Value;
use std::time::Instant;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

#[async_trait]
impl LlmClient for GeminiClient {
    fn with_output_cap(&self, max_tokens: u32) -> Option<std::sync::Arc<dyn LlmClient>> {
        let max = self.max_tokens().min(max_tokens as usize);
        if max == 0 {
            return None;
        }
        let mut client = self.clone();
        client.config.max_tokens = Some(max);
        Some(std::sync::Arc::new(client))
    }

    fn model_generation_pool(&self) -> Option<ModelGenerationPool> {
        ModelGenerationPool::for_endpoint(
            &self.config.provider,
            &self.config.model,
            self.config
                .base_url
                .as_deref()
                .unwrap_or("https://generativelanguage.googleapis.com"),
            self.model_generation_concurrency(),
        )
        .ok()
    }

    fn native_structured_support(&self) -> structured::NativeStructuredSupport {
        self.config
            .native_structured_support
            .unwrap_or(structured::NativeStructuredSupport::JsonSchema)
    }
    fn has_distinct_non_streaming_transport(&self) -> bool {
        true
    }

    async fn complete(
        &self,
        messages: &[Message],
        system: Option<&str>,
        tools: &[ToolDefinition],
    ) -> Result<LlmResponse> {
        self.complete_native(messages, system, tools, None).await
    }

    async fn complete_structured(
        &self,
        messages: &[Message],
        system: Option<&str>,
        tools: &[ToolDefinition],
        directive: &structured::StructuredDirective,
    ) -> Result<LlmResponse> {
        self.complete_native(messages, system, tools, Some(directive))
            .await
    }

    async fn complete_streaming(
        &self,
        messages: &[Message],
        system: Option<&str>,
        tools: &[ToolDefinition],
        cancel: CancellationToken,
    ) -> Result<mpsc::Receiver<StreamEvent>> {
        self.stream_native(messages, system, tools, None, cancel)
            .await
    }

    async fn complete_streaming_structured(
        &self,
        messages: &[Message],
        system: Option<&str>,
        tools: &[ToolDefinition],
        directive: &structured::StructuredDirective,
        cancel: CancellationToken,
    ) -> Result<mpsc::Receiver<StreamEvent>> {
        self.stream_native(messages, system, tools, Some(directive), cancel)
            .await
    }
}

impl GeminiClient {
    async fn complete_native(
        &self,
        messages: &[Message],
        system: Option<&str>,
        tools: &[ToolDefinition],
        directive: Option<&structured::StructuredDirective>,
    ) -> Result<LlmResponse> {
        let body = self
            .request(messages, system, tools, directive)
            .map_err(request_error)?;
        let url = self.endpoint(false).map_err(request_error)?;
        let started = Instant::now();
        let cancel = CancellationToken::new();
        let _guard = cancel.clone().drop_guard();
        let mut response = self.send(&url, &body, cancel.clone()).await?;
        let mut bytes = Vec::new();
        while let Some(chunk) = response.byte_stream.next().await {
            let chunk = chunk?;
            if bytes.len().saturating_add(chunk.len()) > MAX_BODY_BYTES {
                return Err(GeminiError::Invalid("Gemini response exceeds byte limit").into());
            }
            bytes.extend_from_slice(&chunk);
        }
        let value: Value = serde_json::from_slice(&bytes)?;
        let mut meter = WireMeter::new(UsageProtocol::Gemini);
        meter.response(&value).map_err(GeminiError::Metering)?;
        let mut assembly = Assembly::default();
        assembly.push(&value, self)?;
        assembly.finish(self, &meter, url, started, None)
    }

    async fn stream_native(
        &self,
        messages: &[Message],
        system: Option<&str>,
        tools: &[ToolDefinition],
        directive: Option<&structured::StructuredDirective>,
        cancel: CancellationToken,
    ) -> Result<mpsc::Receiver<StreamEvent>> {
        let cancel = cancel.child_token();
        let guard = cancel.clone().drop_guard();
        let body = self
            .request(messages, system, tools, directive)
            .map_err(request_error)?;
        let url = self.endpoint(true).map_err(request_error)?;
        let started = Instant::now();
        let response = self.send(&url, &body, cancel.clone()).await?;
        let client = self.clone();
        let (sender, receiver) = mpsc::channel(64);
        tokio::spawn(async move {
            let _guard = guard;
            let mut stream = response.byte_stream;
            let mut meter = SseMeter::new(UsageProtocol::Gemini);
            let mut assembly = Assembly::default();
            let mut first_token_ms = None;
            loop {
                let chunk = tokio::select! {
                    biased;
                    _=cancel.cancelled()=>return,
                    _=sender.closed()=>return,
                    chunk=stream.next()=>chunk,
                };
                let eof = chunk.is_none();
                let frames = match chunk {
                    Some(Ok(bytes)) => meter.push(&bytes),
                    Some(Err(_)) => return,
                    None => meter.finish(),
                };
                let Ok(frames) = frames else {
                    return;
                };
                for frame in frames {
                    if frame.failed {
                        return;
                    }
                    let data = frame
                        .frame
                        .split(['\r', '\n'])
                        .filter_map(crate::sse::data_field_value)
                        .collect::<Vec<_>>()
                        .join("\n");
                    if data.is_empty() {
                        continue;
                    }
                    let Ok(value) = serde_json::from_str::<Value>(&data) else {
                        return;
                    };
                    let Ok(events) = assembly.push(&value, &client) else {
                        return;
                    };
                    for event in events {
                        first_token_ms.get_or_insert_with(|| started.elapsed().as_millis() as u64);
                        tokio::select! {
                            biased;
                            _=cancel.cancelled()=>return,
                            _=sender.closed()=>return,
                            sent=sender.send(event)=>if sent.is_err() { return; },
                        }
                    }
                }
                if eof {
                    if meter.buffered_bytes() != 0 {
                        return;
                    }
                    if let Ok(response) =
                        assembly.finish(&client, &meter.meter, url, started, first_token_ms)
                    {
                        tokio::select! {
                            biased;
                            _ = cancel.cancelled() => {},
                            _ = sender.closed() => {},
                            _ = sender.send(StreamEvent::Done(response)) => {},
                        }
                    }
                    return;
                }
            }
        });
        Ok(receiver)
    }

    async fn send(
        &self,
        url: &url::Url,
        body: &Value,
        cancel: CancellationToken,
    ) -> Result<crate::llm::StreamingHttpResponse> {
        let retry = self.config.retry_config.clone().unwrap_or_default();
        let mut headers = self
            .config
            .headers
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect::<Vec<_>>();
        if !headers
            .iter()
            .any(|(key, _)| key.eq_ignore_ascii_case("x-goog-api-key"))
            && !self.config.api_key.expose().is_empty()
        {
            headers.push(("x-goog-api-key".into(), self.config.api_key.expose().into()));
        }
        let headers = crate::llm::http::invocation_headers(headers);
        crate::retry::with_retry_cancellable(&retry, &cancel, |_| {
            let headers = headers
                .iter()
                .map(|(key, value)| (key.as_str(), value.as_str()))
                .collect();
            let cancel = cancel.clone();
            let retry = &retry;
            async move {
                    let response=tokio::select! {
                        biased;
                        _ = cancel.cancelled() => return AttemptOutcome::Fatal(anyhow::Error::new(crate::llm::HttpClientError::cancelled("Gemini request"))),
                        response = self.http.post_streaming(url.as_str(), headers, body, cancel.clone()) => response,
                    };
                    match response {
                    Ok(response) if (200..300).contains(&response.status) => {
                        AttemptOutcome::Success(response)
                    }
                    Ok(response) => {
                        let Ok(status) = reqwest::StatusCode::from_u16(response.status) else {
                            return AttemptOutcome::Fatal(
                                GeminiError::Http {
                                    status: response.status,
                                }
                                .into(),
                            );
                        };
                        if retry.is_retryable_status(status) {
                            AttemptOutcome::Retryable {
                                status,
                                body: "Gemini request failed".into(),
                                retry_after: crate::retry::RetryConfig::parse_retry_after(
                                    response.retry_after.as_deref(),
                                ),
                            }
                        } else {
                            AttemptOutcome::Fatal(anyhow::Error::new(
                                crate::llm::NonRetryableLlmError::from_status(
                                    &self.config.provider,
                                    response.status,
                                    "Gemini request failed",
                                ),
                            ))
                        }
                    }
                    Err(error) if crate::llm::http::is_retryable_http_failure(&error) => {
                        AttemptOutcome::Retryable {
                            status: reqwest::StatusCode::SERVICE_UNAVAILABLE,
                            body: "Gemini transport failed".into(),
                            retry_after: None,
                        }
                    }
                    Err(error) => AttemptOutcome::Fatal(error),
                }
            }
        })
        .await
    }
}

#[derive(Default)]
struct Assembly {
    parts: Vec<Value>,
    ids: Vec<String>,
    parts_bytes: usize,
    finish_reason: Option<String>,
    response_id: Option<String>,
}

impl Assembly {
    fn push(
        &mut self,
        value: &Value,
        client: &GeminiClient,
    ) -> Result<Vec<StreamEvent>, GeminiError> {
        if value.get("error").is_some_and(|value| !value.is_null()) {
            return Err(GeminiError::Invalid("Gemini returned an error"));
        }
        if let Some(id) = field(value, "responseId", "response_id")? {
            if self
                .response_id
                .as_deref()
                .is_some_and(|previous| Some(previous) != id.as_str())
            {
                return Err(GeminiError::Invalid("conflicting Gemini response ids"));
            }
            self.response_id = Some(
                id.as_str()
                    .ok_or(GeminiError::Invalid("invalid Gemini response id"))?
                    .into(),
            );
        }
        if value
            .pointer("/promptFeedback/blockReason")
            .and_then(Value::as_str)
            .is_some_and(|reason| !reason.is_empty() && reason != "BLOCK_REASON_UNSPECIFIED")
        {
            return Err(GeminiError::Blocked);
        }
        let Some(candidates) = value.get("candidates") else {
            return Ok(Vec::new());
        };
        let candidates = candidates
            .as_array()
            .ok_or(GeminiError::Invalid("invalid Gemini candidates"))?;
        if candidates.len() > 1 {
            return Err(GeminiError::Invalid(
                "Gemini client requested exactly one candidate",
            ));
        }
        let Some(candidate) = candidates.first() else {
            return Ok(Vec::new());
        };
        if candidate
            .get("index")
            .is_some_and(|index| index.as_u64() != Some(0))
        {
            return Err(GeminiError::Invalid("unexpected Gemini candidate index"));
        }
        let mut events = Vec::new();
        if candidate
            .pointer("/content/role")
            .is_some_and(|role| !matches!(role.as_str(), Some("model" | "")))
        {
            return Err(GeminiError::Invalid("invalid Gemini response role"));
        }
        if let Some(parts) = candidate.pointer("/content/parts") {
            let parts = parts
                .as_array()
                .ok_or(GeminiError::Invalid("invalid Gemini parts"))?;
            if !parts.is_empty() {
                if self.finish_reason.is_some() {
                    return Err(GeminiError::Invalid("Gemini content arrived after finish"));
                }
                self.parts_bytes = self
                    .parts_bytes
                    .saturating_add(serde_json::to_vec(parts)?.len());
                if self.parts_bytes > MAX_BODY_BYTES {
                    return Err(GeminiError::Invalid(
                        "Gemini accumulated parts exceed byte limit",
                    ));
                }
                let message = GeminiState::message(
                    client.provider_identity(),
                    &client.config.model,
                    parts.clone(),
                )?;
                if let Some(reasoning) = message.reasoning_content {
                    if !reasoning.is_empty() {
                        events.push(StreamEvent::ReasoningDelta(reasoning));
                    }
                }
                for block in message.content {
                    match block {
                        ContentBlock::Text { text } if !text.is_empty() => {
                            events.push(StreamEvent::TextDelta(text))
                        }
                        ContentBlock::ToolUse { id, name, input } => {
                            self.ids.push(id.clone());
                            events.push(StreamEvent::ToolUseStart {
                                id: id.clone(),
                                name,
                            });
                            events.push(StreamEvent::ToolUseInputDelta {
                                id: Some(id),
                                delta: serde_json::to_string(&input)?,
                            });
                        }
                        _ => {}
                    }
                }
                self.parts.extend_from_slice(parts);
            }
        }
        if let Some(reason) = field(candidate, "finishReason", "finish_reason")? {
            let reason = reason
                .as_str()
                .ok_or(GeminiError::Invalid("invalid Gemini finish reason"))?;
            if reason != "FINISH_REASON_UNSPECIFIED" && !reason.is_empty() {
                if !matches!(
                    reason,
                    "STOP"
                        | "MAX_TOKENS"
                        | "SAFETY"
                        | "RECITATION"
                        | "OTHER"
                        | "BLOCKLIST"
                        | "PROHIBITED_CONTENT"
                        | "SPII"
                        | "MALFORMED_FUNCTION_CALL"
                        | "IMAGE_SAFETY"
                        | "UNEXPECTED_TOOL_CALL"
                        | "IMAGE_PROHIBITED_CONTENT"
                        | "NO_IMAGE"
                        | "IMAGE_OTHER"
                        | "IMAGE_RECITATION"
                ) {
                    return Err(GeminiError::Invalid("unsupported Gemini finish reason"));
                }
                if self
                    .finish_reason
                    .as_ref()
                    .is_some_and(|previous| previous != reason)
                {
                    return Err(GeminiError::Invalid("conflicting Gemini finish reasons"));
                }
                self.finish_reason = Some(reason.into());
            }
        }
        Ok(events)
    }

    fn finish(
        self,
        client: &GeminiClient,
        meter: &WireMeter,
        mut url: url::Url,
        started: Instant,
        first_token_ms: Option<u64>,
    ) -> Result<LlmResponse> {
        if meter.failed || !meter.finished || !meter.usage_complete {
            return Err(GeminiError::Invalid("incomplete Gemini response or usage").into());
        }
        let reason = self.finish_reason.ok_or(GeminiError::Invalid(
            "Gemini response lacks a finish reason",
        ))?;
        let measured = meter.usage().map_err(GeminiError::Metering)?;
        let token = |value: Option<u64>| -> Result<usize, GeminiError> {
            value
                .and_then(|value| usize::try_from(value).ok())
                .ok_or(GeminiError::Invalid(
                    "Gemini usage is missing or exceeds this target",
                ))
        };
        let message = if self.parts.is_empty() {
            Message::assistant("")
        } else {
            GeminiState::message_with_ids(
                client.provider_identity(),
                &client.config.model,
                self.parts,
                self.ids,
            )?
        };
        let usage = TokenUsage {
            prompt_tokens: token(measured.input_tokens)?,
            completion_tokens: token(measured.output_tokens)?,
            total_tokens: token(measured.total_tokens().map_err(GeminiError::Metering)?)?,
            cache_read_tokens: measured
                .cache_read_tokens
                .map(|value| token(Some(value)))
                .transpose()?,
            cache_write_tokens: measured
                .cache_write_tokens
                .map(|value| token(Some(value)))
                .transpose()?,
        };
        let stop_reason = if !message.tool_calls().is_empty() {
            "tool_use"
        } else {
            match reason.as_str() {
                "STOP" => "stop",
                "MAX_TOKENS" => "max_tokens",
                _ => reason.as_str(),
            }
        }
        .to_owned();
        crate::telemetry::record_llm_usage(
            usage.prompt_tokens,
            usage.completion_tokens,
            usage.total_tokens,
            Some(&stop_reason),
        );
        url.set_query(None);
        Ok(LlmResponse {
            message,
            usage,
            stop_reason: Some(stop_reason),
            token_logprobs: Vec::new(),
            meta: Some(LlmResponseMeta {
                provider: Some(client.config.provider.clone()),
                request_model: Some(client.config.model.clone()),
                request_url: Some(url.to_string()),
                response_id: self.response_id,
                response_model: meter.response_model.clone(),
                response_object: Some("gemini.generateContent".into()),
                first_token_ms,
                duration_ms: Some(started.elapsed().as_millis() as u64),
            }),
        })
    }
}

fn request_error(error: GeminiError) -> anyhow::Error {
    let message = error.to_string();
    anyhow::Error::new(error).context(crate::llm::NonRetryableLlmError::new(message))
}
