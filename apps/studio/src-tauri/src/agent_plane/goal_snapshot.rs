//! Read-only ACP goal projection. Stored chat prose is never execution authority.

use super::*;
use serde::{Deserialize, Serialize};

const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Deserialize)]
struct AcpGoalState {
    #[serde(deserialize_with = "Option::<Definition>::deserialize")]
    definition: Option<Definition>,
    // Required even when null: an older agent cannot prove there is no live goal.
    #[serde(deserialize_with = "Option::<Tick>::deserialize")]
    tick: Option<Tick>,
}

#[derive(Deserialize)]
struct Definition {
    binding: Binding,
    state: State,
    goal: Objective,
    goal_sequence: u64,
    #[serde(deserialize_with = "Option::<u32>::deserialize")]
    plan_revision: Option<u32>,
}

#[derive(Deserialize)]
struct Binding {
    session_id: String,
    goal_id: String,
    goal_revision: u32,
}

#[derive(Deserialize)]
struct Objective {
    description: String,
}

// These are protocol closed sets, not labels inferred from assistant text.
macro_rules! wire_set {
    ($name:ident { $($variant:ident => $wire:literal),+ $(,)? }) => {
        #[derive(Clone, Copy, PartialEq)]
        enum $name { $($variant),+ }
        impl $name {
            fn parse(value: &str) -> Result<Self, String> {
                match value {
                    $($wire => Ok(Self::$variant),)+
                    _ => Err(format!("unknown native goal {}: {value}", stringify!($name))),
                }
            }
            fn as_str(self) -> &'static str {
                match self { $(Self::$variant => $wire),+ }
            }
        }
        impl Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(self.as_str())
            }
        }
        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                Self::parse(&String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
            }
        }
    };
}

wire_set!(State {
    Active => "active", Paused => "paused", Waiting => "waiting",
    Completed => "completed", Cancelled => "cancelled",
});
wire_set!(Tick {
    Advance => "advance", Wait => "wait", NeedsInput => "needs_input", Paused => "paused",
    BudgetExhausted => "budget_exhausted", Reconcile => "reconcile", Complete => "complete",
});

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Goal {
    goal_id: String,
    goal_revision: u32,
    plan_revision: Option<u32>,
    goal_sequence: u64,
    state: State,
    objective: String,
    tick: Option<Tick>,
}

wire_set!(Unavailable {
    RuntimeUnavailable => "runtime_unavailable", InvalidSnapshot => "invalid_snapshot",
    RequestFailed => "request_failed", StaleBinding => "stale_binding",
});

fn unavailable(
    session: &str,
    acp_session: Option<&str>,
    execution: Option<&str>,
    reason: Unavailable,
) -> Value {
    json!({"status":"unavailable","sessionId":session,"acpSessionId":acp_session,
        "executionId":execution,"goal":null,"reason":reason})
}

fn project(value: Value, session: &str, acp_session: &str, execution: Option<&str>) -> Value {
    let parsed = serde_json::from_value::<AcpGoalState>(value).ok();
    let goal = parsed.and_then(|parsed| match parsed.definition {
        None if parsed.tick.is_none() => Some(None),
        Some(definition)
            if definition.binding.session_id == acp_session
                && !definition.binding.goal_id.is_empty()
                && definition.binding.goal_revision > 0
                && definition.goal_sequence <= MAX_SAFE_INTEGER
                && definition.goal_sequence > 0
                && definition.plan_revision.is_none_or(|revision| revision > 0)
                && !definition.goal.description.trim().is_empty()
                && ((definition.state == State::Cancelled && parsed.tick.is_none())
                    || (definition.state != State::Cancelled && parsed.tick.is_some())) =>
        {
            Some(Some(Goal {
                goal_id: definition.binding.goal_id,
                goal_revision: definition.binding.goal_revision,
                plan_revision: definition.plan_revision,
                goal_sequence: definition.goal_sequence,
                state: definition.state,
                objective: definition.goal.description,
                tick: parsed.tick,
            }))
        }
        _ => None,
    });
    match goal {
        Some(goal) => json!({"status":"known","sessionId":session,"acpSessionId":acp_session,
            "executionId":execution,"goal":goal}),
        None => unavailable(
            session,
            Some(acp_session),
            execution,
            Unavailable::InvalidSnapshot,
        ),
    }
}

impl TurnService {
    /// Point-in-time inspection of the already-bound native runtime; this starts no agent or Run.
    pub async fn goal_snapshot(&self, session_id: &str) -> Result<Value, PlaneError> {
        self.sessions.get(session_id)?;
        let owner = self
            .lock_live()
            .get(session_id)
            .map(|live| (live.execution_id.clone(), live.acp_session.clone()));
        let execution = owner.as_ref().map(|(execution, _)| execution.as_str());
        let deadline = tokio::time::Instant::now() + Duration::from_secs(1);
        let Ok(runtimes) = tokio::time::timeout_at(deadline, self.runtimes.lock()).await else {
            return Ok(unavailable(
                session_id,
                None,
                execution,
                Unavailable::RequestFailed,
            ));
        };
        let runtime = runtimes.get(session_id).cloned();
        drop(runtimes);
        let Some(runtime) = runtime else {
            return Ok(unavailable(
                session_id,
                None,
                execution,
                Unavailable::RuntimeUnavailable,
            ));
        };
        let Ok(_read) = tokio::time::timeout_at(deadline, runtime.goal_read.lock()).await else {
            return Ok(unavailable(
                session_id,
                Some(&runtime.acp_session),
                execution,
                Unavailable::RequestFailed,
            ));
        };
        let cached = self
            .goal_snapshots
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(session_id)
            .cloned();
        let admission = cached
            .as_ref()
            .and_then(|snapshot| snapshot["executionId"].as_str())
            .map(str::to_owned);
        let bound = match &owner {
            Some((_, binding)) => binding.as_deref() == Some(runtime.acp_session.as_str()),
            // The most recently admitted non-local turn withdraws an older local binding.
            None => cached.as_ref().is_some_and(|snapshot| {
                snapshot["acpSessionId"].as_str() == Some(runtime.acp_session.as_str())
            }),
        };
        if !bound {
            return Ok(unavailable(
                session_id,
                None,
                execution,
                Unavailable::RuntimeUnavailable,
            ));
        }
        let result = tokio::time::timeout_at(
            deadline,
            runtime
                .client
                .request("_a3s/goal/state", json!({"session_id":runtime.acp_session})),
        )
        .await;
        let snapshot = match result {
            Ok(Ok(value)) => project(value, session_id, &runtime.acp_session, execution),
            _ => unavailable(
                session_id,
                Some(&runtime.acp_session),
                execution,
                Unavailable::RequestFailed,
            ),
        };
        // A response from a replaced process or turn cannot overwrite the new owner's snapshot.
        let Ok(runtimes) = tokio::time::timeout_at(deadline, self.runtimes.lock()).await else {
            return Ok(unavailable(
                session_id,
                Some(&runtime.acp_session),
                execution,
                Unavailable::RequestFailed,
            ));
        };
        let current = runtimes
            .get(session_id)
            .is_some_and(|current| Arc::ptr_eq(current, &runtime));
        let live = self.lock_live();
        if !current
            || live
                .get(session_id)
                .map(|live| (live.execution_id.clone(), live.acp_session.clone()))
                != owner
        {
            return Ok(unavailable(
                session_id,
                Some(&runtime.acp_session),
                execution,
                Unavailable::StaleBinding,
            ));
        }
        let mut snapshots = self
            .goal_snapshots
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if snapshots
            .get(session_id)
            .and_then(|snapshot| snapshot["executionId"].as_str())
            != admission.as_deref()
        {
            return Ok(unavailable(
                session_id,
                Some(&runtime.acp_session),
                execution,
                Unavailable::StaleBinding,
            ));
        }
        // Preserve the last admitted execution as an ABA stamp even for idle GETs.
        let mut cached_snapshot = snapshot.clone();
        cached_snapshot["executionId"] = json!(admission);
        snapshots.insert(session_id.to_owned(), cached_snapshot);
        drop(snapshots);
        if let Some(execution) = execution {
            self.hub.refresh_goal_snapshot(
                &format!("agent-session:{session_id}"),
                execution,
                snapshot.clone(),
            );
        }
        Ok(snapshot)
    }

    pub(super) fn reset_goal_snapshot(&self, execution: &Execution) {
        self.goal_snapshots
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .insert(
                execution.session_id.clone(),
                unavailable(
                    &execution.session_id,
                    None,
                    Some(&execution.execution_id),
                    Unavailable::RuntimeUnavailable,
                ),
            );
    }

    pub(super) fn withdraw_goal_runtime(&self, session_id: &str) {
        let mut snapshots = self
            .goal_snapshots
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if let Some(snapshot) = snapshots.get_mut(session_id) {
            let execution = snapshot["executionId"].as_str().map(str::to_owned);
            *snapshot = unavailable(
                session_id,
                None,
                execution.as_deref(),
                Unavailable::RuntimeUnavailable,
            );
        }
    }

    pub(super) fn bind_goal_runtime(&self, execution: &Execution, acp_session: &str) -> bool {
        let mut live = self.lock_live();
        let Some(turn) = live.get_mut(&execution.session_id).filter(|turn| {
            turn.owner == execution.anchor_id
                && turn.execution_id == execution.execution_id
                && !*turn.abort.borrow()
        }) else {
            return false;
        };
        turn.acp_session = Some(acp_session.to_owned());
        true
    }

    pub(super) fn cached_goal_snapshot(&self, session_id: &str, execution_id: &str) -> Value {
        self.goal_snapshots
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(session_id)
            .filter(|snapshot| snapshot["executionId"].as_str() == Some(execution_id))
            .cloned()
            .unwrap_or_else(|| {
                unavailable(
                    session_id,
                    None,
                    Some(execution_id),
                    Unavailable::RuntimeUnavailable,
                )
            })
    }
}

#[cfg(test)]
#[path = "goal_snapshot_tests.rs"]
mod tests;
