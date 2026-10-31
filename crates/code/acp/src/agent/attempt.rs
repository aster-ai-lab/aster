//! Drive one admitted Core Run for either interactive or unattended attendance.
//! Admission, host retries and response mapping stay with the caller.

use super::*;

pub(super) struct AttemptCompletion {
    pub(super) state: TurnState,
    pub(super) join_failure: Option<String>,
    pub(super) deadline_exceeded: bool,
    pub(super) turn_permit: OwnedMutexGuard<()>,
    pub(super) events_drained: std::time::Duration,
    pub(super) drain_started: std::time::Instant,
}

pub(super) enum AttemptDrive {
    Ended(AttemptCompletion),
    Cancelled(acp::Result<acp::PromptResponse>),
}

impl AsterCodeAgent {
    /// Drain and join an already admitted Run. The synchronous event hook is
    /// for attendance-specific accounting; event delivery itself stays shared.
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn drive_attempt<F>(
        &self,
        session_id: &acp::SessionId,
        session: &AgentSession,
        stream: aster_core::AgentRunStream,
        generation: u64,
        turn_permit: OwnedMutexGuard<()>,
        observation: Option<&crate::run_observation::Binding>,
        remaining: Option<std::time::Duration>,
        mut note_event: F,
    ) -> AttemptDrive
    where
        F: FnMut(&AgentEvent),
    {
        let mut observer = stream.run.and_then(|run| {
            observation.map(|binding| {
                crate::run_observation::Observer::new(run, session_id.0.to_string(), binding)
            })
        });
        self.emit_run_observation(observer.as_mut().and_then(|observer| observer.opened()))
            .await;
        let mut state = TurnState {
            observer,
            ..TurnState::default()
        };
        let mut rx = stream.events;
        let mut join = stream.worker;
        let session_key = session_id.0.to_string();
        let drain_started = std::time::Instant::now();
        let immediate_deadline = remaining.is_some_and(|duration| duration.is_zero());
        let mut deadline = remaining.map(|duration| Box::pin(tokio::time::sleep(duration)));
        let mut deadline_exceeded = immediate_deadline;
        if immediate_deadline {
            tracing::warn!("run deadline reached before event delivery; cancelling the core run");
            session.cancel().await;
        }
        loop {
            // Register before the map read. `notify_waiters` stores no permit.
            let notified = self.turn_cancel.notified();
            if self.turn_was_cancelled(&session_key, generation).await {
                return AttemptDrive::Cancelled(
                    self.leave_cancelled_turn(
                        session_id,
                        session,
                        &mut state,
                        &mut rx,
                        join,
                        turn_permit,
                    )
                    .await,
                );
            }
            tokio::select! {
                biased;
                _ = notified => continue,
                _ = async {
                    match deadline.as_mut() {
                        Some(deadline) => deadline.await,
                        None => std::future::pending::<()>().await,
                    }
                }, if deadline.is_some() && !deadline_exceeded => {
                    deadline_exceeded = true;
                    tracing::warn!("run deadline reached mid-attempt; cancelling the core run");
                    session.cancel().await;
                }
                event = rx.recv() => {
                    match event {
                        Some(event) => {
                            note_event(&event);
                            tokio::select! {
                                biased;
                                _ = self.wait_for_turn_cancel(&session_key, generation) => {},
                                _ = self.handle_event(session_id, session, &mut state, event) => {},
                            }
                        }
                        None => break,
                    }
                }
            }
        }
        let events_drained = drain_started.elapsed();
        // A worker that died without emitting a terminal event closed the
        // channel above; the join result is the only remaining evidence.
        let joined = loop {
            let notified = self.turn_cancel.notified();
            if self.turn_was_cancelled(&session_key, generation).await {
                return AttemptDrive::Cancelled(
                    self.leave_cancelled_turn(
                        session_id,
                        session,
                        &mut state,
                        &mut rx,
                        join,
                        turn_permit,
                    )
                    .await,
                );
            }
            tokio::select! {
                biased;
                _ = notified => continue,
                result = &mut join => break result,
            }
        };
        let join_failure = joined.err().map(worker_join_failure);
        if let (Some(message), None) = (&join_failure, &state.turn_failure) {
            tracing::error!(
                session = %session_id.0,
                detail = %message,
                "turn worker died without a terminal event"
            );
        }
        settle_attempt_state(&mut state, join_failure.as_deref());
        AttemptDrive::Ended(AttemptCompletion {
            state,
            join_failure,
            deadline_exceeded,
            turn_permit,
            events_drained,
            drain_started,
        })
    }

    /// Publish the terminal payload at the caller's existing ordering point.
    pub(super) async fn publish_attempt_outcome(
        &self,
        session_id: &acp::SessionId,
        state: &mut TurnState,
    ) {
        if let Some(outcome) = state.end_outcome.take() {
            self.emit_turn_outcome(session_id, outcome).await;
        }
    }
}

/// Settle EOF and worker failure without treating either as a clean EndTurn.
/// An explicit Core error remains more specific than the subsequent join error.
pub(super) fn settle_attempt_state(state: &mut TurnState, join_failure: Option<&str>) {
    if let (Some(message), None) = (join_failure, &state.turn_failure) {
        state.turn_failure = Some(message.to_owned());
    }
    if state.turn_failure.is_none() && state.end_text.is_none() {
        state.turn_failure = Some("turn worker exited without a terminal event".into());
    }
}
