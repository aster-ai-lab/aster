#![cfg(test)]

use super::*;
use crate::unattended::result::RunProgress;
use unattended_turn::Attempt;

struct Fixture {
    _root: tempfile::TempDir,
    agent: AsterCodeAgent,
    session_id: acp::SessionId,
    session: Arc<AgentSession>,
}

impl Fixture {
    async fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let mut launch = tests::test_launch(Some(root.path().join("sessions")));
        launch.workspace = root.path().to_owned();
        let agent = AsterCodeAgent::new(launch);
        let opened = acp::Agent::new_session(&agent, acp::NewSessionRequest::new(root.path()))
            .await
            .unwrap();
        let session = agent.sessions.lock().await[opened.session_id.0.as_ref()]
            .session
            .clone();
        Self {
            _root: root,
            agent,
            session_id: opened.session_id,
            session,
        }
    }

    fn stream(
        &self,
        events: Vec<AgentEvent>,
        worker: JoinHandle<()>,
    ) -> aster_core::AgentRunStream {
        let (sender, receiver) = tokio::sync::mpsc::channel(events.len().max(1));
        for event in events {
            sender.try_send(event).unwrap();
        }
        drop(sender);
        aster_core::AgentRunStream {
            run: None,
            events: receiver,
            worker,
        }
    }

    async fn mark_cancelled(&self) {
        self.agent
            .cancelled_generation
            .lock()
            .await
            .insert(self.session_id.0.to_string(), 1);
    }

    async fn interactive(
        &self,
        stream: aster_core::AgentRunStream,
        cancelled: bool,
    ) -> acp::Result<acp::PromptResponse> {
        if cancelled {
            self.mark_cancelled().await;
        }
        let permit = Arc::new(tokio::sync::Mutex::new(())).lock_owned().await;
        self.agent
            .drain_turn(
                &self.session_id,
                &self.session,
                stream,
                None,
                1,
                permit,
                None,
            )
            .await
    }

    async fn unattended(
        &self,
        stream: aster_core::AgentRunStream,
        remaining: std::time::Duration,
        cancelled: bool,
    ) -> (
        anyhow::Result<Attempt>,
        RunProgress,
        Arc<tokio::sync::Mutex<()>>,
    ) {
        if cancelled {
            self.mark_cancelled().await;
        }
        let turn_lock = Arc::new(tokio::sync::Mutex::new(()));
        let permit = Arc::clone(&turn_lock).lock_owned().await;
        let mut progress = RunProgress::new();
        let attempt = self
            .agent
            .unattended_attempt(
                &self.session_id,
                &self.session,
                stream,
                1,
                permit,
                &mut progress,
                remaining,
            )
            .await;
        (attempt, progress, turn_lock)
    }
}

fn end() -> AgentEvent {
    AgentEvent::End {
        text: "done".into(),
        usage: Default::default(),
        verification_summary: Box::new(
            aster_core::verification::VerificationSummary::from_reports(&[]),
        ),
        completion_terminal: Default::default(),
        forced_stop: None,
        meta: None,
    }
}

fn panicking_worker() -> JoinHandle<()> {
    tokio::spawn(async { panic!("synthetic worker failure") })
}

#[tokio::test]
async fn interactive_and_unattended_entries_share_terminal_success() {
    let interactive = Fixture::new().await;
    let response = interactive
        .interactive(
            interactive.stream(vec![end()], tokio::spawn(async {})),
            false,
        )
        .await
        .unwrap();
    assert_eq!(response.stop_reason, acp::StopReason::EndTurn);

    let unattended = Fixture::new().await;
    let (attempt, progress, _) = unattended
        .unattended(
            unattended.stream(vec![end()], tokio::spawn(async {})),
            std::time::Duration::from_secs(30),
            false,
        )
        .await;
    match attempt.unwrap() {
        Attempt::Ended { join_failure, .. } => assert!(join_failure.is_none()),
        Attempt::Cancelled(_) => panic!("terminal run was cancelled"),
    }
    assert!(
        progress.terminal_event,
        "the synchronous note_event hook ran"
    );
}

#[tokio::test]
async fn interactive_and_unattended_entries_fail_closed_on_eof_and_worker_failure() {
    for worker_fails in [false, true] {
        let interactive = Fixture::new().await;
        let worker = if worker_fails {
            panicking_worker()
        } else {
            tokio::spawn(async {})
        };
        let error = interactive
            .interactive(interactive.stream(Vec::new(), worker), false)
            .await
            .unwrap_err();
        let message = error.data.unwrap().as_str().unwrap().to_owned();
        if worker_fails {
            assert!(message.contains("synthetic worker failure"), "{message}");
        } else {
            assert!(message.contains("without a terminal event"), "{message}");
        }

        let unattended = Fixture::new().await;
        let worker = if worker_fails {
            panicking_worker()
        } else {
            tokio::spawn(async {})
        };
        let (attempt, progress, _) = unattended
            .unattended(
                unattended.stream(Vec::new(), worker),
                std::time::Duration::from_secs(30),
                false,
            )
            .await;
        match attempt.unwrap() {
            Attempt::Ended {
                join_failure,
                facts,
                ..
            } => {
                assert_eq!(join_failure.is_some(), worker_fails);
                assert_eq!(facts.failed, worker_fails);
                if let Some(failure) = join_failure {
                    assert!(failure.contains("synthetic worker failure"), "{failure}");
                }
            }
            Attempt::Cancelled(_) => panic!("unexpected cancellation"),
        }
        assert!(!progress.terminal_event);
    }
}

#[tokio::test]
async fn interactive_and_unattended_entries_share_cancellation() {
    let interactive = Fixture::new().await;
    let response = interactive
        .interactive(interactive.stream(Vec::new(), tokio::spawn(async {})), true)
        .await
        .unwrap();
    assert_eq!(response.stop_reason, acp::StopReason::Cancelled);

    let unattended = Fixture::new().await;
    let (attempt, _, turn_lock) = unattended
        .unattended(
            unattended.stream(Vec::new(), tokio::spawn(async {})),
            std::time::Duration::from_secs(30),
            true,
        )
        .await;
    match attempt.unwrap() {
        Attempt::Cancelled(response) => {
            assert_eq!(response.unwrap().stop_reason, acp::StopReason::Cancelled);
        }
        Attempt::Ended { .. } => panic!("cancelled run reached normal settlement"),
    }
    assert!(turn_lock.try_lock().is_ok());
}

#[tokio::test]
async fn unattended_deadline_is_terminal_and_releases_the_permit() {
    let fixture = Fixture::new().await;
    let (attempt, _, turn_lock) = fixture
        .unattended(
            fixture.stream(Vec::new(), tokio::spawn(async {})),
            std::time::Duration::ZERO,
            false,
        )
        .await;
    let error = attempt.err().expect("zero deadline must fail closed");
    assert!(error.to_string().contains("deadline exceeded"), "{error:#}");
    assert!(turn_lock.try_lock().is_ok(), "deadline released the permit");
}

#[test]
fn attempt_settlement_preserves_specific_errors_and_recognizes_a_terminal() {
    let mut explicit = TurnState {
        turn_failure: Some("core failure".into()),
        ..TurnState::default()
    };
    attempt::settle_attempt_state(&mut explicit, Some("join failure"));
    assert_eq!(explicit.turn_failure.as_deref(), Some("core failure"));

    let mut eof = TurnState::default();
    attempt::settle_attempt_state(&mut eof, None);
    assert!(eof
        .turn_failure
        .as_deref()
        .is_some_and(|message| message.contains("without a terminal event")));

    let mut terminal = TurnState {
        end_text: Some(String::new()),
        ..TurnState::default()
    };
    attempt::settle_attempt_state(&mut terminal, None);
    assert!(terminal.turn_failure.is_none());
}
