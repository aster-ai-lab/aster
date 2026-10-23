//! Pass-through, cancellation-owned terminal observations for prefix routing.
//! No task, prompt log, client-supplied usage, or unbounded response buffer.

use super::prefix_affinity::{PrefixLease, PrefixTiming};
use crate::response_body::ResponseBody;
use bytes::Bytes;
use hyper::body::{Body, Frame, SizeHint};
use serde_json::Value;
use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
    time::Instant,
};

const MAX_EVENT_BYTES: usize = 64 * 1024;
const MAX_JSON_BYTES: usize = 256 * 1024;

pub(crate) fn track_prefix_body(
    body: ResponseBody,
    lease: Option<PrefixLease>,
    success: bool,
    sse: bool,
) -> ResponseBody {
    let Some(lease) = lease.filter(|_| success) else {
        return body;
    };
    ResponseBody::boxed(FeedbackBody {
        inner: Box::pin(body),
        lease: Some(lease),
        observation: Observation::new(sse),
    })
}

struct FeedbackBody {
    inner: Pin<Box<ResponseBody>>,
    lease: Option<PrefixLease>,
    observation: Observation,
}

impl Body for FeedbackBody {
    type Data = Bytes;
    type Error = io::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, io::Error>>> {
        match self.inner.as_mut().poll_frame(cx) {
            Poll::Ready(Some(Ok(frame))) => {
                if let Some(bytes) = frame.data_ref() {
                    self.observation.push(bytes);
                }
                // Full/known-length bodies can report EOF on the last frame;
                // hyper need not poll them again after that frame is delivered.
                if self.inner.is_end_stream() {
                    self.finish();
                }
                Poll::Ready(Some(Ok(frame)))
            }
            Poll::Ready(None) => {
                self.finish();
                Poll::Ready(None)
            }
            Poll::Ready(Some(Err(error))) => {
                self.lease.take();
                Poll::Ready(Some(Err(error)))
            }
            Poll::Pending => Poll::Pending,
        }
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }
    fn size_hint(&self) -> SizeHint {
        self.inner.size_hint()
    }
}

impl FeedbackBody {
    fn finish(&mut self) {
        if let Some(lease) = self.lease.take() {
            if self.observation.complete() {
                lease.complete(self.observation.timing.take(), Instant::now());
            }
        }
    }
}

struct Observation {
    sse: bool,
    buffer: Vec<u8>,
    event: Vec<u8>,
    invalid: bool,
    finished: bool,
    done: bool,
    timing: Option<PrefixTiming>,
}

impl Observation {
    fn new(sse: bool) -> Self {
        Self {
            sse,
            buffer: Vec::new(),
            event: Vec::new(),
            invalid: false,
            finished: false,
            done: false,
            timing: None,
        }
    }

    fn push(&mut self, bytes: &[u8]) {
        if self.invalid {
            return;
        }
        if !self.sse {
            if self.buffer.len().saturating_add(bytes.len()) > MAX_JSON_BYTES {
                self.invalid = true;
                self.buffer.clear();
            } else {
                self.buffer.extend_from_slice(bytes);
            }
            return;
        }
        for part in bytes.split_inclusive(|byte| *byte == b'\n') {
            if self.buffer.len().saturating_add(part.len()) > MAX_EVENT_BYTES {
                self.invalid = true;
                break;
            }
            self.buffer.extend_from_slice(part);
            if part.last() != Some(&b'\n') {
                continue;
            }
            let mut line = std::mem::take(&mut self.buffer);
            line.pop();
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            if line.is_empty() {
                self.flush_event();
            } else if let Some(data) = line.strip_prefix(b"data:") {
                let data = data.strip_prefix(b" ").unwrap_or(data);
                if self
                    .event
                    .len()
                    .saturating_add(data.len())
                    .saturating_add(1)
                    > MAX_EVENT_BYTES
                {
                    self.invalid = true;
                    break;
                }
                if !self.event.is_empty() {
                    self.event.push(b'\n');
                }
                self.event.extend_from_slice(data);
            } else if line.starts_with(b"event: error") {
                self.invalid = true;
            }
        }
        if self.invalid {
            self.buffer.clear();
            self.event.clear();
            self.timing = None;
        }
    }

    fn flush_event(&mut self) {
        if self.event.is_empty() {
            return;
        }
        let event = std::mem::take(&mut self.event);
        if self.done {
            self.invalid = true;
            return;
        }
        if event == b"[DONE]" {
            self.done = true;
            return;
        }
        match serde_json::from_slice::<Value>(&event) {
            Ok(value) => self.observe(&value),
            Err(_) => self.invalid = true,
        }
    }

    fn observe(&mut self, value: &Value) {
        if value.get("error").is_some() {
            self.invalid = true;
            return;
        }
        if let Some(choices) = value.get("choices").and_then(Value::as_array) {
            // Multi-choice telemetry does not define one cache owner or one
            // work curve. It still passes through byte-for-byte.
            if choices.len() > 1 {
                self.invalid = true;
                return;
            }
            for choice in choices {
                if let Some(reason) = choice
                    .get("finish_reason")
                    .filter(|reason| !reason.is_null())
                {
                    match reason.as_str() {
                        Some(
                            "stop" | "length" | "tool_calls" | "function_call" | "content_filter",
                        ) => self.finished = true,
                        _ => self.invalid = true,
                    }
                }
            }
        }
        // The exact upstream extension is distinct from untrusted message
        // replay annotations. Internal estimates are not accounting facts.
        let Some(upstream) = value.get("prism_upstream") else {
            return;
        };
        if upstream.get("source").and_then(Value::as_str) != Some("upstream-reported") {
            return;
        }
        self.timing = parse_timing(upstream);
    }

    fn complete(&mut self) -> bool {
        if self.invalid {
            return false;
        }
        if self.sse {
            // A final blank event delimiter and DONE are required. Truncated
            // UTF-8, an incomplete line, or missing terminal event stays cold.
            self.buffer.is_empty() && self.event.is_empty() && self.done && self.finished
        } else {
            match serde_json::from_slice::<Value>(&self.buffer) {
                Ok(value) => {
                    self.observe(&value);
                    !self.invalid && self.finished
                }
                Err(_) => false,
            }
        }
    }
}

fn parse_timing(upstream: &Value) -> Option<PrefixTiming> {
    let timings = upstream.get("timings")?;
    let usage = upstream.get("usage")?;
    let prompt = usage.get("prompt_tokens")?.as_u64()?;
    let output = usage.get("completion_tokens")?.as_u64()?;
    let cached = timings.get("cache_n")?.as_u64()?;
    if cached.checked_add(timings.get("prompt_n")?.as_u64()?)? != prompt
        || timings.get("predicted_n")?.as_u64()? != output
        || usage.get("total_tokens")?.as_u64()? != prompt.checked_add(output)?
    {
        return None;
    }
    Some(PrefixTiming {
        prompt,
        cached,
        output,
        prefill_ms: timings.get("prompt_ms")?.as_f64()?,
        decode_ms: timings.get("predicted_ms")?.as_f64()?,
    })
}

#[cfg(test)]
mod tests;
