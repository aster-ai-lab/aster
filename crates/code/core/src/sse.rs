//! Server-Sent Events parsing helpers.

/// Incrementally decode UTF-8 without replacing a code point split across
/// transport chunks.
///
/// HTTP byte streams make no promise that a chunk ends on a UTF-8 boundary.
/// Retaining the incomplete suffix keeps SSE JSON lossless while still
/// rejecting genuinely invalid byte sequences.
#[derive(Debug, Default)]
pub(crate) struct Utf8StreamDecoder {
    pending: Vec<u8>,
}

impl Utf8StreamDecoder {
    pub(crate) fn push_to(
        &mut self,
        chunk: &[u8],
        output: &mut String,
    ) -> Result<(), std::str::Utf8Error> {
        self.pending.extend_from_slice(chunk);
        match std::str::from_utf8(&self.pending) {
            Ok(text) => {
                output.push_str(text);
                self.pending.clear();
                Ok(())
            }
            Err(error) => {
                let valid_up_to = error.valid_up_to();
                if valid_up_to > 0 {
                    let text = std::str::from_utf8(&self.pending[..valid_up_to])
                        .expect("UTF-8 error valid prefix");
                    output.push_str(text);
                    self.pending.drain(..valid_up_to);
                }
                if error.error_len().is_some() {
                    Err(error)
                } else {
                    Ok(())
                }
            }
        }
    }

    pub(crate) fn finish(&self) -> Result<(), std::str::Utf8Error> {
        std::str::from_utf8(&self.pending).map(|_| ())
    }

    pub(crate) fn pending_bytes(&self) -> usize {
        self.pending.len()
    }
}

#[derive(Debug, Default)]
pub(crate) struct SseEventScan {
    index: usize,
    line_start: usize,
    event_start: usize,
}

/// Remove every complete SSE event from the front of `buffer`.
///
/// A blank line ends an event. The line ending may be LF, CRLF, or CR, so a
/// spec-shaped frame `data: ...\r\n\r\n` is one event. Searching for the two
/// bytes `\n\n` misses that frame: the newlines are separated by `\r`.
///
/// A trailing CR on a nonempty line waits for a possible LF. A blank CR line
/// already ends the event; a later LF is an empty line. `eof` flushes a final CR.
pub(crate) fn take_sse_events(buffer: &mut String, eof: bool) -> Vec<String> {
    take_sse_events_scanned(buffer, &mut SseEventScan::default(), eof)
}

/// Retain the scan position across chunks and compact the buffer once per batch.
/// Callers may only append between calls; this function owns buffer removal.
pub(crate) fn take_sse_events_scanned(
    buffer: &mut String,
    scan: &mut SseEventScan,
    eof: bool,
) -> Vec<String> {
    let mut events = Vec::new();
    let bytes = buffer.as_bytes();
    while scan.index < bytes.len() {
        while scan.index < bytes.len() && bytes[scan.index] != b'\n' && bytes[scan.index] != b'\r' {
            scan.index += 1;
        }
        if scan.index == bytes.len() {
            break;
        }
        let blank = scan.index == scan.line_start;
        let ending_len = if bytes[scan.index] == b'\r' {
            if scan.index + 1 == bytes.len() {
                if !eof && !blank {
                    break;
                }
                1
            } else if bytes[scan.index + 1] == b'\n' {
                2
            } else {
                1
            }
        } else {
            1
        };
        scan.index += ending_len;
        if blank {
            if scan.line_start > scan.event_start {
                events.push(buffer[scan.event_start..scan.line_start].to_owned());
            }
            scan.event_start = scan.index;
        }
        scan.line_start = scan.index;
    }
    let consumed = scan.event_start;
    buffer.drain(..consumed);
    scan.index -= consumed;
    scan.line_start -= consumed;
    scan.event_start = 0;
    events
}

/// Return the value of an SSE `data:` field.
///
/// The SSE field grammar allows `data:<value>` and `data: <value>`. When a
/// single space follows the colon, it is ignored by the parser.
pub(crate) fn data_field_value(line: &str) -> Option<&str> {
    let value = line.strip_prefix("data:")?;
    Some(value.strip_prefix(' ').unwrap_or(value))
}

#[cfg(test)]
#[path = "sse_tests.rs"]
mod tests;
