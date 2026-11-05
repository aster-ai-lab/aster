//! Bounded SSE data-event framing shared by translation and usage observation.

use bytes::Bytes;
use std::io;

const MAX_EVENT_BYTES: usize = 8 * 1024 * 1024;

#[derive(Default)]
pub(crate) struct SseDecoder {
    line: Vec<u8>,
    data: Vec<u8>,
    skip_lf: bool,
}

impl SseDecoder {
    /// Consume through one complete data event, leaving subsequent wire bytes
    /// untouched so a protocol terminal can release its upstream immediately.
    pub(crate) fn next_event(&mut self, input: &mut &[u8]) -> io::Result<Option<Bytes>> {
        while !input.is_empty() {
            if self.skip_lf {
                self.skip_lf = false;
                if input.first() == Some(&b'\n') {
                    *input = &input[1..];
                    continue;
                }
            }
            let end = input
                .iter()
                .position(|byte| matches!(*byte, b'\r' | b'\n'))
                .map_or(input.len(), |index| index + 1);
            let part = &input[..end];
            *input = &input[end..];
            if part.len() > MAX_EVENT_BYTES.saturating_sub(self.line.len()) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "SSE line exceeds the 8 MiB bound",
                ));
            }
            self.line.extend_from_slice(part);
            if !part
                .last()
                .is_some_and(|byte| matches!(*byte, b'\r' | b'\n'))
            {
                continue;
            }
            self.skip_lf = part.last() == Some(&b'\r');
            let line = std::mem::take(&mut self.line);
            let line = std::str::from_utf8(&line[..line.len() - 1])
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
            if line.is_empty() {
                if !self.data.is_empty() {
                    return Ok(Some(Bytes::from(std::mem::take(&mut self.data))));
                }
                continue;
            }
            let (field, value) = line.split_once(':').unwrap_or((line, ""));
            if field != "data" {
                continue;
            }
            let value = value.strip_prefix(' ').unwrap_or(value);
            if value.len().saturating_add(1) > MAX_EVENT_BYTES.saturating_sub(self.data.len()) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "SSE event exceeds the 8 MiB bound",
                ));
            }
            self.data.extend_from_slice(value.as_bytes());
            self.data.push(b'\n');
        }
        Ok(None)
    }
}

#[cfg(test)]
#[path = "sse_tests.rs"]
mod tests;
