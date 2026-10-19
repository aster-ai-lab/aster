//! Prompt-correlated, read-only observations from the admitted Code stream.

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub(super) const METHOD: &str = "_a3s/execution/observe";
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Kind {
    RunOpened,
    ModelTurnOpened,
    ModelTurnClosed,
    RunClosed,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Terminal {
    Completed,
    Failed,
    Cancelled,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Packet {
    protocol_version: u32,
    session_id: String,
    prompt_id: String,
    core_session_id: String,
    run_id: String,
    sequence: u64,
    kind: Kind,
    turn: Option<u64>,
    status: Option<Terminal>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Snapshot {
    protocol_version: u32,
    pub(super) prompt_id: String,
    pub(super) core_session_id: String,
    pub(super) run_id: String,
    sequence: u64,
    pub(super) model_turn: Option<u64>,
}

pub(super) struct Observer {
    session: String,
    prompt: String,
    current: Option<Snapshot>,
    last_turn: u64,
    retired: bool,
}

impl Observer {
    pub(super) fn new(session: &str, prompt: &str) -> Self {
        Self {
            session: session.to_owned(),
            prompt: prompt.to_owned(),
            current: None,
            last_turn: 0,
            retired: false,
        }
    }

    pub(super) fn accept(&mut self, params: &Value) -> Option<Option<Snapshot>> {
        if self.retired
            || params.get("sessionId").and_then(Value::as_str) != Some(&self.session)
            || params.get("promptId").and_then(Value::as_str) != Some(&self.prompt)
        {
            return None;
        }
        let Some(packet) = serde_json::from_value::<Packet>(params.clone()).ok() else {
            return self.refuse();
        };
        if packet.session_id != self.session
            || packet.protocol_version != 1
            || !valid_id(&packet.run_id)
            || !valid_id(&packet.core_session_id)
            || packet.sequence == 0
            || packet.sequence > MAX_SAFE_INTEGER
            || packet
                .turn
                .is_some_and(|turn| turn == 0 || turn > MAX_SAFE_INTEGER)
        {
            return self.refuse();
        }
        match &self.current {
            None if !matches!(packet.kind, Kind::RunOpened) || packet.sequence != 1 => {
                return self.refuse();
            }
            Some(current)
                if packet.run_id != current.run_id
                    || packet.core_session_id != current.core_session_id
                    || packet.sequence != current.sequence + 1 =>
            {
                return self.refuse();
            }
            _ => {}
        }
        match packet.kind {
            Kind::RunOpened
                if self.current.is_none() && packet.turn.is_none() && packet.status.is_none() => {}
            Kind::ModelTurnOpened
                if packet.status.is_none()
                    && self
                        .current
                        .as_ref()
                        .is_some_and(|run| run.model_turn.is_none())
                    && packet.turn.is_some_and(|turn| turn > self.last_turn) =>
            {
                self.last_turn = packet.turn?;
            }
            Kind::ModelTurnClosed
                if packet.status.is_none()
                    && self.current.as_ref().is_some_and(|run| {
                        run.model_turn.is_some() && run.model_turn == packet.turn
                    }) => {}
            Kind::RunClosed if packet.turn.is_none() && packet.status.is_some() => {
                self.current = None;
                self.retired = true;
                return Some(None);
            }
            _ => return self.refuse(),
        }
        self.current = Some(Snapshot {
            protocol_version: 1,
            prompt_id: packet.prompt_id,
            core_session_id: packet.core_session_id,
            run_id: packet.run_id,
            sequence: packet.sequence,
            model_turn: if matches!(packet.kind, Kind::ModelTurnClosed) {
                None
            } else {
                packet.turn
            },
        });
        Some(self.current.clone())
    }

    fn refuse(&mut self) -> Option<Option<Snapshot>> {
        self.current = None;
        self.retired = true;
        Some(None)
    }
}

fn valid_id(id: &str) -> bool {
    !id.trim().is_empty() && id.len() <= 128 && !id.chars().any(char::is_control)
}

#[cfg(test)]
#[path = "execution_observation_tests.rs"]
mod tests;
