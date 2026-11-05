//! Reconcile grant token budgets from observed OpenAI usage payloads.

use super::observed_tokens::{ObservedTokenUsage, ObservedTokens};
use super::InferenceAdmissionGuard;
use crate::response_body::ResponseBody;
use crate::sse::SseDecoder;
use bytes::Bytes;
use http::Response;
use hyper::body::{Body, Frame, SizeHint};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

/// Upper bound for buffering a non-SSE JSON response while hunting for usage.
const MAX_JSON_PREFIX_BYTES: usize = 256 * 1024;

/// Rolling tail retained after the JSON prefix truncates. OpenAI places `usage`
/// after `choices`; a bounded tail is enough to recover the object without
/// keeping the whole body.
const MAX_JSON_TAIL_BYTES: usize = 16 * 1024;

const GEMINI_USAGE_FIELDS: [&str; 6] = [
    "promptTokenCount",
    "toolUsePromptTokenCount",
    "candidatesTokenCount",
    "thoughtsTokenCount",
    "cachedContentTokenCount",
    "totalTokenCount",
];

/// Signed-in caller a finished response is attributed to.
#[derive(Debug, Clone)]
pub(crate) struct UsageAttribution {
    pub(crate) subject: super::PrincipalSubject,
    pub(crate) model: String,
    pub(crate) endpoint: crate::config::InferenceEndpoint,
    pub(crate) request_id: uuid::Uuid,
}

/// Observed token usage of one response. Unknown lanes stay `None`; they are
/// never invented as zero.
type ObservedUsage = ObservedTokens;

/// Hold the admission guard until the response body ends, then reconcile the
/// token budget from any observed upstream `usage`.
pub(crate) fn track_token_budget_response(
    response: Response<ResponseBody>,
    admission: Option<InferenceAdmissionGuard>,
    observed_tokens: Option<Arc<ObservedTokenUsage>>,
) -> Response<ResponseBody> {
    track_attributed_token_budget_response(response, admission, observed_tokens, None, None)
}

/// [`track_token_budget_response`] that also records principal usage.
pub(crate) fn track_attributed_token_budget_response(
    response: Response<ResponseBody>,
    admission: Option<InferenceAdmissionGuard>,
    observed_tokens: Option<Arc<ObservedTokenUsage>>,
    attribution: Option<UsageAttribution>,
    upstream_tokens: Option<Arc<ObservedTokenUsage>>,
) -> Response<ResponseBody> {
    if admission.is_none() && observed_tokens.is_none() && attribution.is_none() {
        return response;
    }
    let sse_response = response
        .headers()
        .get(http::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            value.split(';').next().is_some_and(|media_type| {
                media_type.trim().eq_ignore_ascii_case("text/event-stream")
            })
        });
    let status = response.status().as_u16();
    let (parts, body) = response.into_parts();
    let body = ResponseBody::boxed(TokenBudgetBody {
        inner: Box::pin(body),
        admission,
        observed_tokens,
        attribution,
        upstream_tokens,
        status,
        sse_response,
        json_prefix: Vec::new(),
        json_truncated: false,
        json_tail: Vec::new(),
        sse_decoder: Some(SseDecoder::default()),
        sse_usage: None,
        sse_anthropic: None,
        sse_gemini: serde_json::Map::new(),
    });
    Response::from_parts(parts, body)
}

fn observe_json_usage(bytes: &[u8]) -> Option<ObservedUsage> {
    let value = serde_json::from_slice::<serde_json::Value>(bytes).ok()?;
    usage_from_value(&value)
}

fn usage_from_value(value: &serde_json::Value) -> Option<ObservedUsage> {
    if let Some(usage) = value.get("usageMetadata") {
        return gemini_usage(usage);
    }
    let value = match value.get("type").and_then(serde_json::Value::as_str) {
        Some("response.completed" | "response.incomplete" | "response.failed") => {
            value.get("response")?
        }
        _ => value,
    };
    let usage = value.get("usage")?;
    usage_from_object(usage)
}

fn u64_field(object: &serde_json::Value, key: &str) -> Option<u64> {
    object.get(key).and_then(serde_json::Value::as_u64)
}

fn gemini_usage(usage: &serde_json::Value) -> Option<ObservedUsage> {
    usage.as_object()?;
    for name in GEMINI_USAGE_FIELDS {
        if usage
            .get(name)
            .is_some_and(|value| !value.is_null() && value.as_u64().is_none())
        {
            return None;
        }
    }
    let additive = |name| match usage.get(name) {
        None => Some(0),
        Some(value) => value.as_u64(),
    };
    let mut prompt =
        match u64_field(usage, "promptTokenCount").zip(additive("toolUsePromptTokenCount")) {
            Some((base, extra)) => Some(base.checked_add(extra)?),
            None => None,
        };
    let reasoning = additive("thoughtsTokenCount");
    let mut completion = match u64_field(usage, "candidatesTokenCount").zip(reasoning) {
        Some((base, extra)) => Some(base.checked_add(extra)?),
        None => None,
    };
    let cached_input = additive("cachedContentTokenCount");
    let total = u64_field(usage, "totalTokenCount");
    if let Some(total) = total {
        match (prompt, completion) {
            (Some(input), Some(output)) if input.checked_add(output)? != total => return None,
            (Some(input), None) => completion = Some(total.checked_sub(input)?),
            (None, Some(output)) => prompt = Some(total.checked_sub(output)?),
            (None, None) if total == 0 => {
                prompt = Some(0);
                completion = Some(0);
            }
            _ => {}
        }
    }
    if cached_input
        .zip(u64_field(usage, "promptTokenCount"))
        .is_some_and(|(cached, prompt)| cached > prompt)
        || [
            "promptTokenCount",
            "toolUsePromptTokenCount",
            "cachedContentTokenCount",
        ]
        .iter()
        .any(|name| {
            u64_field(usage, name)
                .zip(prompt.or(total))
                .is_some_and(|(component, input)| component > input)
        })
        || ["candidatesTokenCount", "thoughtsTokenCount"]
            .iter()
            .any(|name| {
                u64_field(usage, name)
                    .zip(completion.or(total))
                    .is_some_and(|(component, output)| component > output)
            })
        || cached_input
            .zip(prompt)
            .is_some_and(|(cached, input)| cached > input)
        || reasoning
            .zip(completion)
            .is_some_and(|(thoughts, output)| thoughts > output)
    {
        return None;
    }
    let sum = match prompt.zip(completion) {
        Some((input, output)) => Some(input.checked_add(output)?),
        None => None,
    };
    Some(ObservedUsage {
        prompt,
        completion,
        cached_input,
        reasoning,
        total: total.or(sum),
    })
}

/// Anthropic input including prompt-cache reads and writes.
fn anthropic_input(usage: &serde_json::Value) -> Option<u64> {
    let input = u64_field(usage, "input_tokens")?;
    Some(
        input
            .saturating_add(u64_field(usage, "cache_creation_input_tokens").unwrap_or(0))
            .saturating_add(u64_field(usage, "cache_read_input_tokens").unwrap_or(0)),
    )
}

/// Cache-served input in Chat Completions or Responses usage.
fn openai_cached_input(usage: &serde_json::Value) -> Option<u64> {
    usage
        .get("prompt_tokens_details")
        .or_else(|| usage.get("input_tokens_details"))?
        .get("cached_tokens")
        .and_then(serde_json::Value::as_u64)
}

/// Reasoning output in Chat Completions or Responses usage.
fn openai_reasoning(usage: &serde_json::Value) -> Option<u64> {
    usage
        .get("completion_tokens_details")
        .or_else(|| usage.get("output_tokens_details"))?
        .get("reasoning_tokens")
        .and_then(serde_json::Value::as_u64)
}

fn sum_total(prompt: Option<u64>, completion: Option<u64>) -> Option<u64> {
    match (prompt, completion) {
        (Some(prompt), Some(completion)) => Some(prompt.saturating_add(completion)),
        _ => None,
    }
}

fn usage_from_object(usage: &serde_json::Value) -> Option<ObservedUsage> {
    if usage.get("prompt_tokens").is_some()
        || (usage.get("input_tokens").is_none() && usage.get("total_tokens").is_some())
    {
        let prompt = u64_field(usage, "prompt_tokens");
        let completion = u64_field(usage, "completion_tokens");
        return Some(ObservedUsage {
            prompt,
            completion,
            cached_input: openai_cached_input(usage),
            reasoning: openai_reasoning(usage),
            total: u64_field(usage, "total_tokens").or_else(|| sum_total(prompt, completion)),
        });
    }
    let prompt = anthropic_input(usage)?;
    let completion = u64_field(usage, "output_tokens");
    Some(ObservedUsage {
        prompt: Some(prompt),
        completion,
        // Anthropic reports cache reads separately; they are already inside
        // the summed prompt lane above.
        cached_input: u64_field(usage, "cache_read_input_tokens")
            .or_else(|| openai_cached_input(usage)),
        reasoning: openai_reasoning(usage),
        total: u64_field(usage, "total_tokens").or_else(|| sum_total(Some(prompt), completion)),
    })
}

/// Recover `usage` from a truncated JSON body by scanning the last `"usage"`
/// object (brace-balanced), without requiring a complete document parse.
fn observe_json_usage_from_tail(bytes: &[u8]) -> Option<ObservedUsage> {
    if let Some(usage) = observe_json_usage(bytes) {
        return Some(usage);
    }
    let text = std::str::from_utf8(bytes).ok()?;
    let mut last = None;
    for key in ["\"usage\"", "\"usageMetadata\""] {
        let mut search_from = 0;
        while let Some(rel) = text[search_from..].find(key) {
            let key_at = search_from + rel;
            let after_key = &text[key_at + key.len()..];
            let trimmed = after_key.trim_start();
            if !trimmed.starts_with(':') {
                search_from = key_at + 1;
                continue;
            }
            let after_colon = &trimmed[1..];
            let value = after_colon.trim_start();
            if !value.starts_with('{') {
                search_from = key_at + 1;
                continue;
            }
            if let Some(object) = extract_balanced_json_object(value) {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(object) {
                    if let Some(usage) = if key == "\"usageMetadata\"" {
                        gemini_usage(&parsed)
                    } else {
                        usage_from_object(&parsed)
                    } {
                        last = Some(usage);
                    }
                }
            }
            search_from = key_at + 1;
        }
    }
    last
}

fn extract_balanced_json_object(input: &str) -> Option<&str> {
    let bytes = input.as_bytes();
    if bytes.first() != Some(&b'{') {
        return None;
    }
    let mut depth = 0_i32;
    let mut in_string = false;
    let mut escape = false;
    for (index, &byte) in bytes.iter().enumerate() {
        if in_string {
            if escape {
                escape = false;
            } else if byte == b'\\' {
                escape = true;
            } else if byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&input[..=index]);
                }
            }
            _ => {}
        }
    }
    None
}

struct TokenBudgetBody {
    inner: Pin<Box<ResponseBody>>,
    admission: Option<InferenceAdmissionGuard>,
    observed_tokens: Option<Arc<ObservedTokenUsage>>,
    attribution: Option<UsageAttribution>,
    upstream_tokens: Option<Arc<ObservedTokenUsage>>,
    status: u16,
    sse_response: bool,
    json_prefix: Vec<u8>,
    json_truncated: bool,
    json_tail: Vec<u8>,
    sse_decoder: Option<SseDecoder>,
    sse_usage: Option<ObservedUsage>,
    /// Anthropic stream: input from `message_start`, cumulative output from
    /// `message_delta`.
    sse_anthropic: Option<ObservedUsage>,
    sse_gemini: serde_json::Map<String, serde_json::Value>,
}

impl TokenBudgetBody {
    fn note_sse_event(&mut self, data: &[u8]) {
        let Ok(value) = serde_json::from_slice::<serde_json::Value>(data) else {
            return;
        };
        if let Some(metadata) = value.get("usageMetadata") {
            self.note_gemini_usage(metadata);
            return;
        }
        match value.get("type").and_then(serde_json::Value::as_str) {
            Some("message_start") => {
                if let Some(usage) = value
                    .get("message")
                    .and_then(|message| message.get("usage"))
                {
                    let anthropic = self.sse_anthropic.get_or_insert_with(Default::default);
                    if let Some(input) = anthropic_input(usage) {
                        anthropic.prompt = Some(input);
                    }
                    if let Some(output) = u64_field(usage, "output_tokens") {
                        anthropic.completion = Some(output);
                    }
                    if let Some(cached) = u64_field(usage, "cache_read_input_tokens") {
                        anthropic.cached_input = Some(cached);
                    }
                }
            }
            Some("message_delta") => {
                if let Some(usage) = value.get("usage") {
                    let anthropic = self.sse_anthropic.get_or_insert_with(Default::default);
                    if let Some(input) = anthropic_input(usage) {
                        anthropic.prompt = Some(input);
                    }
                    if let Some(output) = u64_field(usage, "output_tokens") {
                        anthropic.completion = Some(output);
                    }
                    if let Some(cached) = u64_field(usage, "cache_read_input_tokens") {
                        anthropic.cached_input = Some(cached);
                    }
                }
            }
            _ => {
                if let Some(usage) = usage_from_value(&value) {
                    self.sse_usage = Some(usage);
                }
            }
        }
    }

    fn note_gemini_usage(&mut self, metadata: &serde_json::Value) {
        let Some(metadata) = metadata.as_object() else {
            return;
        };
        let mut snapshot = self.sse_gemini.clone();
        for name in GEMINI_USAGE_FIELDS {
            let Some(value) = metadata.get(name) else {
                continue;
            };
            if value.is_null() {
                continue;
            }
            let Some(counter) = value.as_u64() else {
                return;
            };
            if snapshot
                .get(name)
                .and_then(serde_json::Value::as_u64)
                .is_some_and(|previous| counter < previous)
            {
                return;
            }
            snapshot.insert(name.into(), value.clone());
        }
        if !metadata.contains_key("totalTokenCount") || metadata["totalTokenCount"].is_null() {
            snapshot.remove("totalTokenCount");
        }
        let Some(mut usage) = gemini_usage(&serde_json::Value::Object(snapshot.clone())) else {
            return;
        };
        if let Some(previous) = self.sse_usage {
            usage.prompt = usage.prompt.or(previous.prompt);
            usage.completion = usage.completion.or(previous.completion);
            if [
                (previous.prompt, usage.prompt),
                (previous.completion, usage.completion),
                (previous.cached_input, usage.cached_input),
                (previous.reasoning, usage.reasoning),
                (previous.total, usage.total),
            ]
            .iter()
            .any(|(old, new)| old.zip(*new).is_some_and(|(old, new)| new < old))
            {
                return;
            }
        }
        if let Some((input, output)) = usage.prompt.zip(usage.completion) {
            let Some(total) = input.checked_add(output) else {
                return;
            };
            if usage.total.is_some_and(|reported| reported != total) {
                return;
            }
            usage.total = Some(total);
        }
        if [
            (usage.cached_input, usage.prompt),
            (usage.reasoning, usage.completion),
        ]
        .iter()
        .any(|(component, lane)| {
            component
                .zip(*lane)
                .is_some_and(|(component, lane)| component > lane)
        }) {
            return;
        }
        self.sse_gemini = snapshot;
        self.sse_usage = Some(usage);
    }

    fn observed_sse_usage(&self) -> Option<ObservedUsage> {
        self.sse_anthropic
            .map(|mut usage| {
                usage.total = usage.resolved_total();
                usage
            })
            .or(self.sse_usage)
    }

    fn note_json_prefix(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        self.json_tail.extend_from_slice(bytes);
        if self.json_tail.len() > MAX_JSON_TAIL_BYTES {
            let excess = self.json_tail.len() - MAX_JSON_TAIL_BYTES;
            self.json_tail.drain(..excess);
        }

        if self.json_truncated {
            return;
        }
        let remaining = MAX_JSON_PREFIX_BYTES.saturating_sub(self.json_prefix.len());
        if remaining == 0 {
            self.json_truncated = true;
            return;
        }
        let take = bytes.len().min(remaining);
        self.json_prefix.extend_from_slice(&bytes[..take]);
        if take < bytes.len() {
            self.json_truncated = true;
        }
    }

    fn note_sse_events(&mut self, mut bytes: &[u8]) {
        while let Some(decoder) = self.sse_decoder.as_mut() {
            match decoder.next_event(&mut bytes) {
                Ok(Some(data)) => self.note_sse_event(&data),
                Ok(None) => break,
                Err(error) => {
                    tracing::warn!(%error, "SSE usage observation stopped; unknown usage keeps its reservation");
                    self.sse_decoder = None;
                }
            }
        }
    }

    fn note_data(&mut self, bytes: &[u8]) {
        if self.upstream_tokens.is_some() {
            return;
        }
        if self.sse_response {
            self.note_sse_events(bytes);
        } else {
            self.note_json_prefix(bytes);
        }
    }

    fn observed_json_usage(&self) -> Option<ObservedUsage> {
        if !self.json_truncated {
            if let Some(usage) = observe_json_usage(&self.json_prefix) {
                return Some(usage);
            }
        }
        observe_json_usage_from_tail(&self.json_tail)
            .or_else(|| observe_json_usage_from_tail(&self.json_prefix))
    }

    fn finish(&mut self) {
        // A partial SSE stream may still contain provisional usage in
        // response.created; JSON tail recovery must not settle from it.
        let usage = if let Some(upstream) = &self.upstream_tokens {
            upstream.observation()
        } else {
            self.observed_sse_usage().or_else(|| {
                if self.sse_response {
                    None
                } else {
                    self.observed_json_usage()
                }
            })
        };
        if let Some(usage) = usage.as_ref() {
            if let Some(sink) = self.observed_tokens.as_ref() {
                sink.record(usage);
            }
            if let Some(admission) = self.admission.take() {
                // A total that cannot be resolved from the upstream usage
                // keeps the reservation — reconcile never under-charges.
                match usage.resolved_total() {
                    Some(_) => admission.reconcile_usage(usage),
                    None => drop(admission),
                }
            }
        } else if let Some(admission) = self.admission.take() {
            drop(admission);
        }
        if let Some(attribution) = self.attribution.take() {
            let usage = usage.unwrap_or_default();
            tracing::info!(
                target: "aster_gateway::usage",
                subject = %attribution.subject.subject,
                org = attribution.subject.org.as_deref().unwrap_or(""),
                model = %attribution.model,
                endpoint = attribution.endpoint.as_str(),
                request_id = %attribution.request_id,
                status = self.status,
                prompt_tokens = ?usage.prompt,
                completion_tokens = ?usage.completion,
                cached_input_tokens = ?usage.cached_input,
                reasoning_tokens = ?usage.reasoning,
                total_tokens = ?usage.total,
                "Principal inference usage"
            );
            super::usage_report::report_principal_usage(
                &attribution,
                usage.resolved_total().unwrap_or(0),
                self.status,
                usage.prompt,
                usage.completion,
            );
        }
        self.observed_tokens = None;
    }
}

impl Body for TokenBudgetBody {
    type Data = Bytes;
    type Error = std::io::Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let this = self.get_mut();
        let result = this.inner.as_mut().poll_frame(context);
        let inner_ended = this.inner.is_end_stream();
        match &result {
            Poll::Ready(Some(Ok(frame))) => {
                if let Some(data) = frame.data_ref() {
                    this.note_data(data);
                }
                if inner_ended {
                    this.finish();
                }
            }
            Poll::Ready(Some(Err(_))) | Poll::Ready(None) => this.finish(),
            Poll::Pending => {}
        }
        result
    }

    fn is_end_stream(&self) -> bool {
        self.admission.is_none()
            && self.observed_tokens.is_none()
            && self.attribution.is_none()
            && self.inner.is_end_stream()
    }

    fn size_hint(&self) -> SizeHint {
        self.inner.size_hint()
    }
}

impl Drop for TokenBudgetBody {
    fn drop(&mut self) {
        self.finish();
    }
}

#[cfg(test)]
#[path = "token_reconcile_tests.rs"]
mod tests;
