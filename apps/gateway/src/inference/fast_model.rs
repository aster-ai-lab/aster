//! Fast model-alias extraction that avoids full JSON parsing when the
//! request can be forwarded as-is (proxy-only, no model rewriting).
//!
//! The common path for an OpenAI-compatible proxy request only needs the
//! `model` field for routing. A full `serde_json::Map` parse is needed
//! only when managed inference rewrites the model or the native wire
//! stage consumes the document — those callers fall back to the full
//! `collect_json_body` path.

use bytes::Bytes;

/// Extract the `model` string from an OpenAI JSON body without building
/// a full `serde_json::Map`. Returns `None` when the body is not a JSON
/// object or the model field is not a string — callers fall back to the
/// full parse path for error reporting.

pub(crate) fn extract_model_alias(body: &[u8]) -> Option<String> {
    let value: serde_json::Value = serde_json::from_slice(body).ok()?;
    value.get("model")?.as_str().map(str::to_owned)
}

/// A validated OpenAI request body with the model extracted lazily.
/// The common proxy path stores only the raw bytes and the model alias;
/// the full JSON document is parsed only when a managed stage needs it.

pub(crate) struct LightweightInferenceRequest {
    pub(crate) body: Bytes,
    pub(crate) model_alias: String,
}

impl LightweightInferenceRequest {
    /// Parse just enough to route: the model alias. Returns None on
    /// invalid JSON or missing model — the caller uses the full
    /// `collect_json_body` path which produces proper error details.
    pub(crate) fn try_from_body(body: Bytes) -> Option<Self> {
        let model_alias = extract_model_alias(&body)?;
        Some(Self { body, model_alias })
    }

    /// Fall back to the full parse when a managed stage needs the
    /// complete document (model rewriting, native wire, etc.).
    pub(crate) fn into_full(self) -> Result<super::OpenAiJsonRequest, super::OpenAiRequestError> {
        let document = match serde_json::from_slice::<serde_json::Value>(&self.body) {
            Ok(serde_json::Value::Object(map)) => map,
            _ => return Err(super::OpenAiRequestError::InvalidBodyShape),
        };
        Ok(super::OpenAiJsonRequest {
            body: self.body,
            document,
            path_model: None,
        })
    }
}

/// SSE chunk forwarding: pass through each `data:` event with zero
/// transformation. The proxy already streams; this documents the
/// invariant and provides a bounded chunk assembler for callers that
/// need to reconstruct events from partial network reads.

pub(crate) struct SseChunkAssembler {
    buffer: Vec<u8>,
}

impl SseChunkAssembler {
    pub(crate) fn new() -> Self {
        Self { buffer: Vec::new() }
    }

    /// Feed raw bytes; returns any complete SSE events (terminated by
    /// a blank line) without retaining them in the buffer.
    pub(crate) fn feed(&mut self, chunk: &[u8]) -> Vec<Vec<u8>> {
        self.buffer.extend_from_slice(chunk);
        let mut events = Vec::new();
        loop {
            let pos = find_double_newline(&self.buffer);
            let Some(end) = pos else { break };
            let event = self.buffer[..end].to_vec();
            // Skip the separator: LF LF (2 bytes) or CRLF CRLF (4 bytes)
            let skip = if self.buffer.get(end) == Some(&0x0d) {
                4
            } else {
                2
            };
            self.buffer.drain(..end + skip);
            if !event.is_empty() {
                events.push(event);
            }
        }
        events
    }

    pub(crate) fn remaining(&self) -> &[u8] {
        &self.buffer
    }
}

fn find_double_newline(data: &[u8]) -> Option<usize> {
    // SSE event terminator: LF LF or CRLF CRLF (blank line).
    // CRLF alone is a line terminator, NOT an event separator.
    for i in 0..data.len().saturating_sub(1) {
        if data[i] == 0x0a && data[i + 1] == 0x0a {
            return Some(i); // LF LF
        }
        if data[i] == 0x0d
            && i + 3 < data.len()
            && data[i + 1] == 0x0a
            && data[i + 2] == 0x0d
            && data[i + 3] == 0x0a
        {
            return Some(i); // CRLF CRLF
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_model_from_chat_completion() {
        let body =
            br#"{"model":"gpt-4o","messages":[{"role":"user","content":"Hello"}],"stream":true}"#;
        assert_eq!(extract_model_alias(body).as_deref(), Some("gpt-4o"));
    }

    #[test]
    fn returns_none_on_invalid_json() {
        assert!(extract_model_alias(b"not json").is_none());
    }

    #[test]
    fn returns_none_when_model_missing() {
        assert!(extract_model_alias(br#"{"messages":[]}"#).is_none());
    }

    #[test]
    fn returns_none_when_model_not_string() {
        assert!(extract_model_alias(br#"{"model":42}"#).is_none());
    }

    #[test]
    fn lightweight_request_round_trips_to_full() {
        let body = Bytes::from_static(
            br#"{"model":"claude","messages":[{"role":"user","content":"Hi"}]}"#,
        );
        let light = LightweightInferenceRequest::try_from_body(body).unwrap();
        assert_eq!(light.model_alias, "claude");
        let full = light.into_full().unwrap();
        assert_eq!(full.document.get("model").unwrap().as_str(), Some("claude"));
    }

    #[test]
    fn sse_assembler_yields_complete_events() {
        let mut sse = SseChunkAssembler::new();
        assert!(sse.feed(b"data: {\"delta\":\"He").is_empty()); // partial
        let events = sse.feed(b"llo\"}\n\ndata: {\"delta\":\"!\"}\n\n");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0], b"data: {\"delta\":\"Hello\"}");
        assert_eq!(events[1], b"data: {\"delta\":\"!\"}");
        assert!(sse.remaining().is_empty());
    }

    #[test]
    fn sse_assembler_handles_crlf() {
        let mut sse = SseChunkAssembler::new();
        let events = sse.feed(b"data: hello\r\ndata: world\r\n\r\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0], b"data: hello\r\ndata: world");
    }
}
