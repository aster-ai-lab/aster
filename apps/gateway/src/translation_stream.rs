//! Bounded SSE framing for cross-protocol response bodies.
//!
//! Complete events enter the shared translators; malformed, oversized or
//! unfinished streams fail the body. A protocol terminal releases the upstream.

use crate::response_body::ResponseBody;
use crate::sse::SseDecoder;
use crate::translation::{
    AnthropicToOpenaiStream, ChatToResponsesStream, OpenaiToAnthropicStream, ResponseTranslation,
    TranslationError,
};
use bytes::Bytes;
use futures_util::{Stream, StreamExt};
use http_body_util::{BodyExt, StreamBody};
use hyper::body::{Body, Frame, SizeHint};
use serde_json::Value;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

impl ResponseTranslation {
    /// Translate upstream SSE bytes with the same bounded codec used by Gateway.
    ///
    /// Malformed, oversized or unfinished streams yield an I/O error. Backpressure
    /// is preserved; a protocol terminal or dropping the stream releases upstream.
    /// `created_unix` supplies the timestamp for Anthropic-to-Chat output.
    pub fn translate_sse_stream<S>(
        self,
        upstream: S,
        created_unix: i64,
    ) -> impl Stream<Item = io::Result<Bytes>> + Send
    where
        S: Stream<Item = io::Result<Bytes>> + Send + 'static,
    {
        let body = ResponseBody::boxed(StreamBody::new(
            upstream.map(|chunk| chunk.map(Frame::data)),
        ));
        match self {
            Self::ToOpenai => TranslatedStreamBody::to_openai(body, created_unix),
            Self::ToAnthropic => TranslatedStreamBody::to_anthropic(body),
            Self::ResponsesFromChat => TranslatedStreamBody::to_responses(body),
        }
        .into_data_stream()
    }
}

#[expect(
    clippy::enum_variant_names,
    reason = "variants name the destination dialect"
)]
enum StreamMachine {
    ToOpenai(AnthropicToOpenaiStream),
    ToAnthropic(OpenaiToAnthropicStream),
    ToResponses(ChatToResponsesStream),
}

impl StreamMachine {
    fn push(&mut self, event: &Value) -> Result<Vec<Value>, TranslationError> {
        match self {
            Self::ToOpenai(machine) => machine.push(event),
            Self::ToAnthropic(machine) => machine.push(event),
            Self::ToResponses(machine) => machine.push_sse(event),
        }
    }

    fn is_finished(&self) -> bool {
        match self {
            Self::ToOpenai(machine) => machine.is_finished(),
            Self::ToAnthropic(machine) => machine.is_finished(),
            Self::ToResponses(machine) => machine.is_finished(),
        }
    }
}

pub(crate) struct TranslatedStreamBody {
    inner: Option<Pin<Box<ResponseBody>>>,
    machine: StreamMachine,
    decoder: SseDecoder,
    pending: std::collections::VecDeque<Bytes>,
    error: Option<io::Error>,
    sequence_number: u64,
}

impl TranslatedStreamBody {
    pub(crate) fn to_openai(inner: ResponseBody, created_unix: i64) -> Self {
        Self::with_machine(
            inner,
            StreamMachine::ToOpenai(AnthropicToOpenaiStream::new(created_unix)),
        )
    }

    pub(crate) fn to_anthropic(inner: ResponseBody) -> Self {
        Self::with_machine(
            inner,
            StreamMachine::ToAnthropic(OpenaiToAnthropicStream::new()),
        )
    }

    pub(crate) fn to_responses(inner: ResponseBody) -> Self {
        Self::with_machine(
            inner,
            StreamMachine::ToResponses(ChatToResponsesStream::new()),
        )
    }

    fn with_machine(inner: ResponseBody, machine: StreamMachine) -> Self {
        Self {
            inner: Some(Box::pin(inner)),
            machine,
            decoder: SseDecoder::default(),
            pending: std::collections::VecDeque::new(),
            error: None,
            sequence_number: 0,
        }
    }

    fn note_bytes(&mut self, mut bytes: &[u8]) -> io::Result<()> {
        while let Some(data) = self.decoder.next_event(&mut bytes)? {
            self.dispatch_event(&data)?;
            if self.inner.is_none() {
                break;
            }
        }
        Ok(())
    }

    fn dispatch_event(&mut self, data: &[u8]) -> io::Result<()> {
        let event = if data.trim_ascii() == b"[DONE]" {
            Value::String("[DONE]".into())
        } else {
            serde_json::from_slice(data)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?
        };
        let events = self
            .machine
            .push(&event)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        for mut event in events {
            if matches!(self.machine, StreamMachine::ToResponses(_)) {
                if let Some(object) = event.as_object_mut() {
                    object.insert("sequence_number".into(), Value::from(self.sequence_number));
                    self.sequence_number += 1;
                }
            }
            self.pending.push_back(frame_event(&event)?);
        }
        if self.machine.is_finished() {
            self.inner.take();
        }
        Ok(())
    }

    fn fail(&mut self, error: io::Error) {
        self.inner.take();
        self.decoder = SseDecoder::default();
        self.error = Some(error);
    }
}

impl Body for TranslatedStreamBody {
    type Data = Bytes;
    type Error = io::Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Option<io::Result<Frame<Bytes>>>> {
        let this = self.get_mut();
        loop {
            if let Some(frame) = this.pending.pop_front() {
                return Poll::Ready(Some(Ok(Frame::data(frame))));
            }
            if let Some(error) = this.error.take() {
                return Poll::Ready(Some(Err(error)));
            }
            let Some(inner) = this.inner.as_mut() else {
                return Poll::Ready(None);
            };
            match inner.as_mut().poll_frame(context) {
                Poll::Ready(Some(Ok(frame))) => {
                    if let Some(data) = frame.data_ref() {
                        if let Err(error) = this.note_bytes(data) {
                            this.fail(error);
                        }
                    }
                }
                Poll::Ready(Some(Err(error))) => this.fail(error),
                Poll::Ready(None) => this.fail(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "upstream SSE ended before its protocol terminal",
                )),
                Poll::Pending => return Poll::Pending,
            }
        }
    }

    fn is_end_stream(&self) -> bool {
        self.pending.is_empty() && self.error.is_none() && self.inner.is_none()
    }

    fn size_hint(&self) -> SizeHint {
        SizeHint::default()
    }
}

fn frame_event(event: &Value) -> io::Result<Bytes> {
    if let Some(sentinel) = event.as_str() {
        return Ok(Bytes::from(format!("data: {sentinel}\n\n")));
    }
    let payload = serde_json::to_string(event).map_err(io::Error::other)?;
    Ok(Bytes::from(
        match event.get("type").and_then(Value::as_str) {
            Some(name) => format!("event: {name}\ndata: {payload}\n\n"),
            None => format!("data: {payload}\n\n"),
        },
    ))
}

#[cfg(test)]
#[path = "translation_stream_tests.rs"]
mod tests;
