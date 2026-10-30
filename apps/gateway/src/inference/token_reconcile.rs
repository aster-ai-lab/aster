//! Reconcile grant token budgets from observed OpenAI usage payloads.

use super::observed_tokens::{ObservedTokenUsage, ObservedTokens};
use super::InferenceAdmissionGuard;
use crate::response_body::ResponseBody;
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

/// Incomplete SSE/line remainder retained between body frames. OpenAI usage
/// objects fit in a few hundred bytes; a larger incomplete line is skipped.
const MAX_LINE_CARRY_BYTES: usize = 8 * 1024;

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
    track_attributed_token_budget_response(response, admission, observed_tokens, None)
}

/// [`track_token_budget_response`] that also records principal usage.
pub(crate) fn track_attributed_token_budget_response(
    response: Response<ResponseBody>,
    admission: Option<InferenceAdmissionGuard>,
    observed_tokens: Option<Arc<ObservedTokenUsage>>,
    attribution: Option<UsageAttribution>,
) -> Response<ResponseBody> {
    if admission.is_none() && observed_tokens.is_none() && attribution.is_none() {
        return response;
    }
    let status = response.status().as_u16();
    let (parts, body) = response.into_parts();
    let body = ResponseBody::boxed(TokenBudgetBody {
        inner: Box::pin(body),
        admission,
        observed_tokens,
        attribution,
        status,
        json_prefix: Vec::new(),
        json_truncated: false,
        json_tail: Vec::new(),
        line_carry: Vec::new(),
        skip_until_newline: false,
        sse_usage: None,
        sse_anthropic: None,
    });
    Response::from_parts(parts, body)
}

fn observe_json_usage(bytes: &[u8]) -> Option<ObservedUsage> {
    let value = serde_json::from_slice::<serde_json::Value>(bytes).ok()?;
    usage_from_value(&value)
}

fn usage_from_value(value: &serde_json::Value) -> Option<ObservedUsage> {
    let usage = value.get("usage")?;
    usage_from_object(usage)
}

fn u64_field(object: &serde_json::Value, key: &str) -> Option<u64> {
    object.get(key).and_then(serde_json::Value::as_u64)
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

/// OpenAI `prompt_tokens_details.cached_tokens`: the cache-served subset of
/// the prompt tokens.
fn openai_cached_input(usage: &serde_json::Value) -> Option<u64> {
    usage
        .get("prompt_tokens_details")?
        .get("cached_tokens")
        .and_then(serde_json::Value::as_u64)
}

/// OpenAI `completion_tokens_details.reasoning_tokens`.
fn openai_reasoning(usage: &serde_json::Value) -> Option<u64> {
    usage
        .get("completion_tokens_details")?
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
    if usage.get("prompt_tokens").is_some() || usage.get("total_tokens").is_some() {
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
        cached_input: u64_field(usage, "cache_read_input_tokens"),
        reasoning: None,
        total: sum_total(Some(prompt), completion),
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
    let mut search_from = 0;
    while let Some(rel) = text[search_from..].find("\"usage\"") {
        let key_at = search_from + rel;
        let after_key = &text[key_at + "\"usage\"".len()..];
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
                if let Some(usage) = usage_from_object(&parsed) {
                    last = Some(usage);
                }
            }
        }
        search_from = key_at + 1;
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

fn sse_data_value(line: &str) -> Option<serde_json::Value> {
    let data = line.strip_prefix("data:")?.trim();
    if data.is_empty() || data == "[DONE]" {
        return None;
    }
    serde_json::from_str::<serde_json::Value>(data).ok()
}

struct TokenBudgetBody {
    inner: Pin<Box<ResponseBody>>,
    admission: Option<InferenceAdmissionGuard>,
    observed_tokens: Option<Arc<ObservedTokenUsage>>,
    attribution: Option<UsageAttribution>,
    status: u16,
    json_prefix: Vec<u8>,
    json_truncated: bool,
    json_tail: Vec<u8>,
    line_carry: Vec<u8>,
    skip_until_newline: bool,
    sse_usage: Option<ObservedUsage>,
    /// Anthropic stream: input from `message_start`, cumulative output from
    /// `message_delta`.
    sse_anthropic: Option<ObservedUsage>,
}

impl TokenBudgetBody {
    fn note_sse_line(&mut self, line: &str) {
        let Some(value) = sse_data_value(line) else {
            return;
        };
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

    fn note_sse_lines(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }

        let mut offset = 0;
        if self.skip_until_newline {
            if let Some(newline) = bytes.iter().position(|&b| b == b'\n') {
                self.skip_until_newline = false;
                offset = newline + 1;
            } else {
                return;
            }
        }

        self.line_carry.extend_from_slice(&bytes[offset..]);
        while let Some(newline) = self.line_carry.iter().position(|&b| b == b'\n') {
            let mut line = self.line_carry.drain(..=newline).collect::<Vec<u8>>();
            if line.last() == Some(&b'\n') {
                line.pop();
            }
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            if let Ok(text) = std::str::from_utf8(&line) {
                self.note_sse_line(text);
            }
        }

        if self.line_carry.len() > MAX_LINE_CARRY_BYTES {
            self.line_carry.clear();
            self.skip_until_newline = true;
        }
    }

    fn note_data(&mut self, bytes: &[u8]) {
        self.note_json_prefix(bytes);
        self.note_sse_lines(bytes);
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
        if !self.line_carry.is_empty() {
            let carry = std::mem::take(&mut self.line_carry);
            if let Ok(text) = std::str::from_utf8(&carry) {
                self.note_sse_line(text.trim_end_matches(['\r', '\n']));
            }
        }

        let usage = self
            .observed_sse_usage()
            .or_else(|| self.observed_json_usage());
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
                target: "a3s_gateway::usage",
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
mod tests {
    use super::*;
    use crate::inference::LifecycleTokenBreakdown;

    fn empty_body() -> TokenBudgetBody {
        TokenBudgetBody {
            inner: Box::pin(ResponseBody::full(Bytes::new())),
            admission: None,
            observed_tokens: None,
            attribution: None,
            status: 200,
            json_prefix: Vec::new(),
            json_truncated: false,
            json_tail: Vec::new(),
            line_carry: Vec::new(),
            skip_until_newline: false,
            sse_usage: None,
            sse_anthropic: None,
        }
    }

    fn observe_usage_from_bytes(bytes: &[u8]) -> Option<ObservedUsage> {
        let mut body = empty_body();
        body.note_data(bytes);
        body.finish();
        body.observed_sse_usage()
            .or_else(|| body.observed_json_usage())
    }

    fn observe_total_tokens_from_bytes(bytes: &[u8]) -> Option<u64> {
        observe_usage_from_bytes(bytes).and_then(|usage| usage.resolved_total())
    }

    #[test]
    fn parses_anthropic_json_usage() {
        let body = br#"{"type":"message","usage":{"input_tokens":10,"cache_read_input_tokens":4,"output_tokens":6}}"#;
        assert_eq!(
            observe_usage_from_bytes(body),
            Some(ObservedUsage {
                prompt: Some(14),
                completion: Some(6),
                cached_input: Some(4),
                reasoning: None,
                total: Some(20),
            })
        );
    }

    #[test]
    fn parses_anthropic_sse_usage_across_start_and_delta() {
        let body = b"event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":25,\"output_tokens\":1}}}\n\n\
event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"delta\":{\"text\":\"hi\"}}\n\n\
event: message_delta\ndata: {\"type\":\"message_delta\",\"usage\":{\"output_tokens\":15}}\n\n";
        assert_eq!(
            observe_usage_from_bytes(body),
            Some(ObservedUsage {
                prompt: Some(25),
                completion: Some(15),
                cached_input: None,
                reasoning: None,
                total: Some(40),
            })
        );
    }

    #[test]
    fn parses_openai_cache_and_reasoning_details() {
        let body = br#"{"id":"1","usage":{"prompt_tokens":100,"prompt_tokens_details":{"cached_tokens":60},"completion_tokens":7,"completion_tokens_details":{"reasoning_tokens":5},"total_tokens":107}}"#;
        assert_eq!(
            observe_usage_from_bytes(body),
            Some(ObservedUsage {
                prompt: Some(100),
                completion: Some(7),
                cached_input: Some(60),
                reasoning: Some(5),
                total: Some(107),
            })
        );
    }

    #[test]
    fn total_only_usage_leaves_split_lanes_unknown() {
        let body = br#"{"usage":{"total_tokens":42}}"#;
        assert_eq!(
            observe_usage_from_bytes(body),
            Some(ObservedUsage {
                prompt: None,
                completion: None,
                cached_input: None,
                reasoning: None,
                total: Some(42),
            })
        );
    }

    #[test]
    fn parses_json_usage_total_tokens() {
        let body =
            br#"{"id":"1","usage":{"prompt_tokens":3,"completion_tokens":7,"total_tokens":10}}"#;
        assert_eq!(observe_total_tokens_from_bytes(body), Some(10));
    }

    #[test]
    fn parses_json_usage_without_total_field() {
        let body = br#"{"usage":{"prompt_tokens":2,"completion_tokens":5}}"#;
        assert_eq!(observe_total_tokens_from_bytes(body), Some(7));
    }

    #[test]
    fn parses_sse_usage_from_last_data_line() {
        let body = b"data: {\"choices\":[]}\n\ndata: {\"usage\":{\"total_tokens\":42}}\n\ndata: [DONE]\n\n";
        assert_eq!(observe_total_tokens_from_bytes(body), Some(42));
    }

    #[test]
    fn observes_sse_usage_after_prefix_larger_than_json_buffer() {
        let sink = std::sync::Arc::new(ObservedTokenUsage::new());
        let mut body = empty_body();
        body.observed_tokens = Some(sink.clone());

        let padding_line = b"data: {\"choices\":[{\"delta\":{\"content\":\"x\"}}]}\n\n";
        let mut padded = 0usize;
        while padded <= MAX_JSON_PREFIX_BYTES {
            body.note_data(padding_line);
            padded += padding_line.len();
        }
        assert!(body.json_truncated);
        body.note_data(b"data: {\"usage\":{\"total_tokens\":99}}\n\ndata: [DONE]\n\n");
        body.finish();
        assert_eq!(sink.total(), Some(99));
    }

    #[test]
    fn observes_sse_usage_split_across_frames() {
        let sink = std::sync::Arc::new(ObservedTokenUsage::new());
        let mut body = empty_body();
        body.observed_tokens = Some(sink.clone());
        body.note_data(b"data: {\"usage\":{\"total_tok");
        body.note_data(b"ens\":17}}\n\ndata: [DONE]\n\n");
        body.finish();
        assert_eq!(sink.total(), Some(17));
    }

    #[test]
    fn observes_json_usage_after_body_exceeds_prefix_budget() {
        let sink = std::sync::Arc::new(ObservedTokenUsage::new());
        let mut body = empty_body();
        body.observed_tokens = Some(sink.clone());

        body.note_data(b"{\"id\":\"1\",\"choices\":[{\"message\":{\"content\":\"");
        let mut padded = body.json_prefix.len();
        let chunk = b"x".repeat(4096);
        while padded <= MAX_JSON_PREFIX_BYTES {
            body.note_data(&chunk);
            padded += chunk.len();
        }
        assert!(body.json_truncated);
        body.note_data(
            b"\"}}],\"usage\":{\"prompt_tokens\":2,\"completion_tokens\":3,\"total_tokens\":88}}",
        );
        body.finish();
        assert_eq!(sink.total(), Some(88));
    }

    #[test]
    fn sink_records_the_full_token_split() {
        let sink = std::sync::Arc::new(ObservedTokenUsage::new());
        let mut body = empty_body();
        body.observed_tokens = Some(sink.clone());
        body.note_data(
            b"data: {\"usage\":{\"prompt_tokens\":9,\"completion_tokens\":4,\"total_tokens\":13}}\n\n",
        );
        body.finish();
        assert_eq!(sink.total(), Some(13));
        assert_eq!(
            sink.breakdown(),
            Some(LifecycleTokenBreakdown {
                prompt_tokens: Some(9),
                completion_tokens: Some(4),
                cached_input_tokens: None,
                reasoning_tokens: None,
            })
        );
    }
}
