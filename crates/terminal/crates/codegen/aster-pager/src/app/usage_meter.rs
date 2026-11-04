//! Session accounting and output throughput for the prompt footer.

use std::time::Instant;

use agent_client_protocol as acp;

#[derive(Debug, Default)]
pub(crate) struct UsageMeter {
    total: Option<u64>,
    last_call: Option<(Option<String>, u64)>,
    stream_prompt: Option<String>,
    stream_started: Option<Instant>,
    stream_bytes: u64,
    measured_rate: Option<f64>,
}

impl UsageMeter {
    pub(crate) fn observe(
        &mut self,
        update: &acp::SessionUpdate,
        prompt: Option<&str>,
        now: Instant,
    ) {
        match update {
            acp::SessionUpdate::AgentMessageChunk(chunk)
            | acp::SessionUpdate::AgentThoughtChunk(chunk) => {
                if chunk
                    .meta
                    .as_ref()
                    .and_then(|meta| meta.get("asterGenerationDelta"))
                    .and_then(|value| value.as_bool())
                    == Some(false)
                {
                    return;
                }
                if let acp::ContentBlock::Text(text) = &chunk.content
                    && !text.text.is_empty()
                {
                    if self.stream_prompt.as_deref() != prompt {
                        self.pause();
                        self.stream_prompt = prompt.map(str::to_owned);
                    }
                    self.stream_started.get_or_insert(now);
                    self.stream_bytes = self.stream_bytes.saturating_add(text.text.len() as u64);
                    self.measured_rate = None;
                }
            }
            acp::SessionUpdate::UsageUpdate(usage) => {
                let Some(meta) = &usage.meta else { return };
                if let Some(total) = meta.get("sessionTotalTokens").and_then(|v| v.as_u64()) {
                    self.reconcile(Some(total), None, None);
                } else if let (Some(sequence), Some(tokens)) = (
                    meta.get("callSequence").and_then(|v| v.as_u64()),
                    meta.get("totalTokens").and_then(|v| v.as_u64()),
                ) {
                    let key = (prompt.map(str::to_owned), sequence);
                    if self.last_call.as_ref() != Some(&key) {
                        self.total = self.total.map(|total| total.saturating_add(tokens));
                        self.last_call = Some(key);
                    }
                }
                self.pause();
            }
            acp::SessionUpdate::ToolCall(_) | acp::SessionUpdate::ToolCallUpdate(_) => self.pause(),
            acp::SessionUpdate::UserMessageChunk(_) => {
                self.pause();
                self.measured_rate = None;
            }
            _ => {}
        }
    }

    pub(crate) fn reconcile(
        &mut self,
        total: Option<u64>,
        output: Option<u64>,
        duration_ms: Option<u64>,
    ) {
        if let Some(total) = total {
            self.total = Some(self.total.map_or(total, |current| current.max(total)));
        }
        if output.is_none() && duration_ms.is_none() {
            return;
        }
        self.pause();
        self.measured_rate = output
            .zip(duration_ms)
            .filter(|(_, ms)| *ms > 0)
            .map(|(tokens, ms)| tokens as f64 * 1000.0 / ms as f64);
    }

    fn pause(&mut self) {
        self.stream_started = None;
        self.stream_bytes = 0;
    }

    pub(crate) fn caption(&self, now: Instant, running: bool) -> String {
        let total =
            self.total.map_or_else(
                || "—".into(),
                |tokens| {
                    crate::views::agent_status::format_tokens_compact(
                        tokens.min(i64::MAX as u64) as i64
                    )
                },
            );
        let rate = if let Some(start) = self.stream_started.filter(|_| running) {
            let seconds = now.saturating_duration_since(start).as_secs_f64();
            // shortcut: bytes/4 estimates visible output only; replace when providers stream token counters.
            if seconds >= 0.25 {
                format!(
                    "≈{:.1}",
                    self.stream_bytes as f64
                        / aster_token_estimation::BYTES_PER_TOKEN as f64
                        / seconds
                )
            } else {
                "—".into()
            }
        } else if running {
            "—".into()
        } else {
            self.measured_rate
                .map_or_else(|| "—".into(), |rate| format!("{rate:.1}"))
        };
        format!("Σ {total} tokens · {rate} tok/s")
    }
}

#[cfg(test)]
#[path = "usage_meter_tests.rs"]
mod tests;
