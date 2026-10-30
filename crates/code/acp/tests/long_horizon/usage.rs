#![cfg(test)]

use serde::{Deserialize, Serialize};

const EVENT_LIMIT: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    #[default]
    Incomplete,
    Missing,
    Invalid,
    Present,
}

impl State {
    pub fn parse(value: &str) -> Result<Self, &'static str> {
        match value {
            "incomplete" => Ok(Self::Incomplete),
            "missing" => Ok(Self::Missing),
            "invalid" => Ok(Self::Invalid),
            "present" => Ok(Self::Present),
            _ => Err("unknown raw usage state; expected incomplete, missing, invalid, present"),
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Incomplete => "incomplete",
            Self::Missing => "missing",
            Self::Invalid => "invalid",
            Self::Present => "present",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Counts {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    pub cache_read_tokens: Option<u64>,
    pub cache_miss_tokens: Option<u64>,
    pub reasoning_tokens: Option<u64>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Observation {
    pub http_status: Option<u16>,
    pub state: State,
    pub usage: Option<Counts>,
}

#[derive(Deserialize)]
struct Envelope {
    usage: Option<WireUsage>,
}

#[derive(Deserialize)]
struct UsageShape {
    usage: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct WireUsage {
    prompt_tokens: u64,
    completion_tokens: u64,
    total_tokens: u64,
    prompt_tokens_details: Option<PromptDetails>,
    completion_tokens_details: Option<CompletionDetails>,
    prompt_cache_hit_tokens: Option<u64>,
    prompt_cache_miss_tokens: Option<u64>,
}

#[derive(Deserialize)]
struct PromptDetails {
    cached_tokens: Option<u64>,
}

#[derive(Deserialize)]
struct CompletionDetails {
    reasoning_tokens: Option<u64>,
}

// Deserialize only known numeric fields; response and reasoning text are ignored.
fn counts(bytes: &[u8]) -> Result<Option<Counts>, ()> {
    if bytes.trim_ascii_start().first() != Some(&b'{') {
        return Err(());
    }
    let envelope: Envelope = serde_json::from_slice(bytes).map_err(|_| ())?;
    let Some(usage) = envelope.usage else {
        return Ok(None);
    };
    // Typed decoding rejects duplicate counters; the shape check rejects positional arrays.
    let raw = serde_json::from_slice::<UsageShape>(bytes)
        .map_err(|_| ())?
        .usage
        .ok_or(())?;
    if !raw.is_object()
        || ["prompt_tokens_details", "completion_tokens_details"]
            .iter()
            .any(|key| {
                raw.get(key)
                    .is_some_and(|value| !value.is_null() && !value.is_object())
            })
    {
        return Err(());
    }
    let cached = usage
        .prompt_tokens_details
        .and_then(|value| value.cached_tokens);
    let hit = usage.prompt_cache_hit_tokens;
    if cached.zip(hit).is_some_and(|(a, b)| a != b)
        || usage.prompt_tokens.checked_add(usage.completion_tokens) != Some(usage.total_tokens)
    {
        return Err(());
    }
    let cache_read_tokens = cached.or(hit);
    let miss = usage.prompt_cache_miss_tokens;
    let reasoning_tokens = usage
        .completion_tokens_details
        .and_then(|value| value.reasoning_tokens);
    if cache_read_tokens.is_some_and(|value| value > usage.prompt_tokens)
        || miss.is_some_and(|value| value > usage.prompt_tokens)
        || cache_read_tokens
            .zip(miss)
            .is_some_and(|(a, b)| a.checked_add(b) != Some(usage.prompt_tokens))
        || reasoning_tokens.is_some_and(|value| value > usage.completion_tokens)
    {
        return Err(());
    }
    Ok(Some(Counts {
        prompt_tokens: usage.prompt_tokens,
        completion_tokens: usage.completion_tokens,
        total_tokens: usage.total_tokens,
        cache_read_tokens,
        cache_miss_tokens: miss,
        reasoning_tokens,
    }))
}

impl Observation {
    pub fn json(status: u16, bytes: &[u8]) -> Self {
        let (state, usage) = match counts(bytes) {
            Ok(Some(usage)) => (State::Present, Some(usage)),
            Ok(None) => (State::Missing, None),
            Err(()) => (State::Invalid, None),
        };
        Self {
            http_status: Some(status),
            state,
            usage,
        }
    }
}

// This bounded observer never changes transport bytes or repairs a provider frame.
#[derive(Default)]
pub struct Sse {
    line: Vec<u8>,
    event: Vec<u8>,
    usage: Option<Counts>,
    done: bool,
    invalid: bool,
}

impl Sse {
    pub fn feed(&mut self, bytes: &[u8]) {
        if self.invalid {
            return;
        }
        for &byte in bytes {
            if byte == b'\n' {
                let mut line = std::mem::take(&mut self.line);
                if line.last() == Some(&b'\r') {
                    line.pop();
                }
                self.line(&line);
                line.clear();
                self.line = line;
            } else if self.line.len() < EVENT_LIMIT {
                self.line.push(byte);
            } else {
                self.invalid = true;
            }
            if self.invalid {
                self.line.clear();
                self.event.clear();
                return;
            }
        }
    }

    fn line(&mut self, line: &[u8]) {
        if line.is_empty() {
            if self.event.is_empty() {
                return;
            }
            let data = self.event.trim_ascii();
            if self.done {
                self.invalid = true;
            } else if data == b"[DONE]" {
                self.done = true;
            } else {
                match counts(data) {
                    Ok(Some(usage)) => {
                        if self.usage.as_ref().is_some_and(|old| old != &usage) {
                            self.invalid = true;
                        } else {
                            self.usage = Some(usage);
                        }
                    }
                    Ok(None) => {}
                    Err(()) => self.invalid = true,
                }
            }
            self.event.clear();
        } else if let Some(data) = line.strip_prefix(b"data:") {
            let data = data.strip_prefix(b" ").unwrap_or(data);
            if self
                .event
                .len()
                .saturating_add(data.len())
                .saturating_add(1)
                > EVENT_LIMIT
            {
                self.invalid = true;
                return;
            }
            if !self.event.is_empty() {
                self.event.push(b'\n');
            }
            self.event.extend_from_slice(data);
        }
    }

    pub fn finish(self, status: u16) -> Observation {
        let state = if self.invalid {
            State::Invalid
        } else if !self.done || !self.line.is_empty() || !self.event.is_empty() {
            State::Incomplete
        } else if self.usage.is_some() {
            State::Present
        } else {
            State::Missing
        };
        Observation {
            http_status: Some(status),
            state,
            usage: if state == State::Present {
                self.usage
            } else {
                None
            },
        }
    }
}

#[path = "usage_tests.rs"]
mod tests;
