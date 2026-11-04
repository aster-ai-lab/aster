//! Numeric Messages observations for the existing bounded SSE reader.

use super::{Counts, Observation, State};
use serde::Deserialize;

#[derive(Default, Deserialize)]
struct Usage {
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    cache_read_input_tokens: Option<u64>,
    cache_creation_input_tokens: Option<u64>,
}

impl Usage {
    fn counts(&self) -> Result<Option<Counts>, ()> {
        let (Some(input), Some(output)) = (self.input_tokens, self.output_tokens) else {
            return Ok(None);
        };
        let prompt = input
            .checked_add(self.cache_read_input_tokens.unwrap_or(0))
            .ok_or(())?
            .checked_add(self.cache_creation_input_tokens.unwrap_or(0))
            .ok_or(())?;
        Ok(Some(Counts {
            prompt_tokens: prompt,
            completion_tokens: output,
            total_tokens: prompt.checked_add(output).ok_or(())?,
            cache_read_tokens: self.cache_read_input_tokens,
            cache_write_tokens: self.cache_creation_input_tokens,
            cache_miss_tokens: None,
            reasoning_tokens: None,
        }))
    }

    fn merge(&mut self, update: Self) -> Result<(), ()> {
        if update
            .output_tokens
            .zip(self.output_tokens)
            .is_some_and(|(new, old)| new < old)
        {
            return Err(());
        }
        if update.input_tokens.is_some() {
            self.input_tokens = update.input_tokens;
        }
        if update.output_tokens.is_some() {
            self.output_tokens = update.output_tokens;
        }
        if update.cache_read_input_tokens.is_some() {
            self.cache_read_input_tokens = update.cache_read_input_tokens;
        }
        if update.cache_creation_input_tokens.is_some() {
            self.cache_creation_input_tokens = update.cache_creation_input_tokens;
        }
        self.counts()?;
        Ok(())
    }
}

#[derive(Deserialize)]
struct Message {
    usage: Option<Usage>,
    stop_reason: Option<String>,
}

#[derive(Deserialize)]
struct Delta {
    stop_reason: Option<String>,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Frame {
    MessageStart { message: Message },
    MessageDelta { delta: Delta, usage: Option<Usage> },
    MessageStop,
    ContentBlockStart,
    ContentBlockDelta,
    ContentBlockStop,
    Ping,
    Error,
}

fn object_shapes(bytes: &[u8]) -> Result<(), ()> {
    let value: serde_json::Value = serde_json::from_slice(bytes).map_err(|_| ())?;
    if !value.is_object() {
        return Err(());
    }
    for object in [&value, value.get("message").unwrap_or(&value)] {
        if !object.is_object()
            || object
                .get("usage")
                .is_some_and(|usage| !usage.is_null() && !usage.is_object())
        {
            return Err(());
        }
    }
    if value.get("delta").is_some_and(|delta| !delta.is_object()) {
        return Err(());
    }
    Ok(())
}

pub(super) fn json(status: u16, bytes: &[u8]) -> Observation {
    let parsed = (|| {
        object_shapes(bytes)?;
        let message: Message = serde_json::from_slice(bytes).map_err(|_| ())?;
        let counts = message
            .usage
            .as_ref()
            .map(Usage::counts)
            .transpose()?
            .flatten();
        Ok::<_, ()>((counts, message.stop_reason))
    })();
    match parsed {
        Ok((usage, stop_reason)) => Observation {
            http_status: Some(status),
            state: if usage.is_some() {
                State::Present
            } else {
                State::Missing
            },
            usage,
            stop_reason,
        },
        Err(()) => Observation {
            http_status: Some(status),
            state: State::Invalid,
            ..Default::default()
        },
    }
}

#[derive(Default)]
pub(super) struct Stream {
    started: bool,
    usage: Usage,
    final_usage: bool,
    pub(super) stop_reason: Option<String>,
}

impl Stream {
    pub(super) fn observe(&mut self, bytes: &[u8]) -> Result<bool, ()> {
        object_shapes(bytes)?;
        let frame: Frame = serde_json::from_slice(bytes).map_err(|_| ())?;
        match frame {
            Frame::MessageStart { message } => {
                if self.started {
                    return Err(());
                }
                self.started = true;
                if let Some(usage) = message.usage {
                    self.usage.merge(usage)?;
                }
            }
            Frame::MessageDelta { delta, usage } => {
                if !self.started {
                    return Err(());
                }
                if let Some(usage) = usage {
                    self.final_usage |= usage.output_tokens.is_some();
                    self.usage.merge(usage)?;
                }
                if delta.stop_reason.is_some() {
                    self.stop_reason = delta.stop_reason;
                }
            }
            Frame::MessageStop => {
                if !self.started {
                    return Err(());
                }
                return Ok(true);
            }
            Frame::Error => return Err(()),
            Frame::ContentBlockStart | Frame::ContentBlockDelta | Frame::ContentBlockStop => {
                if !self.started {
                    return Err(());
                }
            }
            Frame::Ping => {}
        }
        Ok(false)
    }

    pub(super) fn counts(&self) -> Result<Option<Counts>, ()> {
        if !self.final_usage {
            return Ok(None);
        }
        self.usage.counts()
    }
}
