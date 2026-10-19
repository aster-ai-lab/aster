#![cfg(test)]

use super::*;
use std::future::Future;

async fn drain(events: Vec<AgentEvent>, cancelled: bool) -> acp::Result<acp::PromptResponse> {
    let root = tempfile::tempdir().unwrap();
    let mut launch = tests::test_launch(Some(root.path().join("sessions")));
    launch.workspace = root.path().to_owned();
    let agent = A3sCodeAgent::new(launch);
    let opened = acp::Agent::new_session(&agent, acp::NewSessionRequest::new(root.path()))
        .await
        .unwrap();
    let session = agent
        .sessions
        .lock()
        .await
        .get(opened.session_id.0.as_ref())
        .unwrap()
        .session
        .clone();
    if cancelled {
        agent
            .cancelled_generation
            .lock()
            .await
            .insert(opened.session_id.0.to_string(), 1);
    }
    let (sender, receiver) = tokio::sync::mpsc::channel(events.len().max(1));
    for event in events {
        sender.send(event).await.unwrap();
    }
    drop(sender);
    let permit = Arc::new(tokio::sync::Mutex::new(())).lock_owned().await;
    let result = agent
        .drain_turn(
            &opened.session_id,
            &session,
            a3s_code_core::AgentRunStream {
                run: None,
                events: receiver,
                worker: tokio::spawn(async {}),
            },
            None,
            1,
            permit,
            None,
        )
        .await;
    session.close().await;
    result
}

fn end() -> AgentEvent {
    AgentEvent::End {
        text: "delivered answer".into(),
        usage: Default::default(),
        verification_summary: Box::new(
            a3s_code_core::verification::VerificationSummary::from_reports(&[]),
        ),
        completion_terminal: Default::default(),
        forced_stop: None,
        meta: None,
    }
}

#[tokio::test]
async fn a_worker_return_without_a_terminal_is_a_failed_turn() {
    for events in [
        Vec::new(),
        vec![AgentEvent::TextDelta {
            text: "partial answer".into(),
        }],
    ] {
        let error = drain(events, false).await.unwrap_err();
        assert!(error
            .data
            .unwrap()
            .as_str()
            .unwrap()
            .contains("without a terminal event"));
    }
}

#[tokio::test]
async fn an_explicit_terminal_preserves_success_failure_and_cancellation() {
    assert_eq!(
        drain(vec![end()], false).await.unwrap().stop_reason,
        acp::StopReason::EndTurn
    );
    let error = drain(
        vec![AgentEvent::Error {
            message: "native_process_lifetime_unqualified".into(),
        }],
        false,
    )
    .await
    .unwrap_err();
    assert_eq!(error.data.unwrap(), "native_process_lifetime_unqualified");
    assert_eq!(
        drain(Vec::new(), true).await.unwrap().stop_reason,
        acp::StopReason::Cancelled
    );
}

#[tokio::test]
async fn forced_stop_is_not_a_successful_end_turn() {
    use a3s_code_core::harness_loop::ForcedStop;
    for (forced, expected) in [
        (ForcedStop::ResponseTruncated, acp::StopReason::MaxTokens),
        (ForcedStop::ToolBudget, acp::StopReason::MaxTurnRequests),
        (
            ForcedStop::ReadOnlyEvidence,
            acp::StopReason::MaxTurnRequests,
        ),
    ] {
        let mut event = end();
        if let AgentEvent::End { forced_stop, .. } = &mut event {
            *forced_stop = Some(forced);
        }
        assert_eq!(
            drain(vec![event], false).await.unwrap().stop_reason,
            expected
        );
    }
}

#[tokio::test]
async fn cancellation_after_event_eof_returns_while_retaining_the_live_worker_permit() {
    let root = tempfile::tempdir().unwrap();
    let mut launch = tests::test_launch(Some(root.path().join("sessions")));
    launch.workspace = root.path().to_owned();
    let agent = A3sCodeAgent::new(launch);
    let opened = acp::Agent::new_session(&agent, acp::NewSessionRequest::new(root.path()))
        .await
        .unwrap();
    let session = agent
        .sessions
        .lock()
        .await
        .get(opened.session_id.0.as_ref())
        .unwrap()
        .session
        .clone();
    let (sender, events) = tokio::sync::mpsc::channel(1);
    drop(sender);
    let (release, waiting) = tokio::sync::oneshot::channel::<()>();
    let worker = tokio::spawn(async move {
        let _ = waiting.await;
    });
    let turn_lock = Arc::new(tokio::sync::Mutex::new(()));
    let permit = Arc::clone(&turn_lock).lock_owned().await;
    let mut draining = Box::pin(agent.drain_turn(
        &opened.session_id,
        &session,
        a3s_code_core::AgentRunStream {
            run: None,
            events,
            worker,
        },
        None,
        1,
        permit,
        None,
    ));
    assert!(
        std::future::poll_fn(|cx| std::task::Poll::Ready(draining.as_mut().poll(cx).is_pending()))
            .await
    );
    agent
        .cancelled_generation
        .lock()
        .await
        .insert(opened.session_id.0.to_string(), 1);
    agent.turn_cancel.notify_waiters();
    let response = tokio::time::timeout(std::time::Duration::from_secs(1), draining)
        .await
        .expect("cancellation must remain observable after event EOF")
        .unwrap();
    assert_eq!(response.stop_reason, acp::StopReason::Cancelled);
    assert!(
        turn_lock.try_lock().is_err(),
        "a cancelled observer cannot release a live worker's turn lock"
    );
    release.send(()).unwrap();
    let _held = tokio::time::timeout(std::time::Duration::from_secs(1), turn_lock.lock())
        .await
        .expect("the worker returned and released its retained permit");
    session.close().await;
}

async fn cancel_unanswered_event(event: AgentEvent, already_cancelled: bool) {
    use tokio::io::AsyncBufReadExt;
    use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

    tokio::task::LocalSet::new()
        .run_until(async {
            let root = tempfile::tempdir().unwrap();
            let mut launch = tests::test_launch(Some(root.path().join("sessions")));
            launch.workspace = root.path().to_owned();
            let agent = A3sCodeAgent::new(launch);
            let opened = acp::Agent::new_session(&agent, acp::NewSessionRequest::new(root.path()))
                .await
                .unwrap();
            let session = agent.sessions.lock().await[opened.session_id.0.as_ref()]
                .session
                .clone();
            agent
                .turn_generation
                .lock()
                .await
                .insert(opened.session_id.0.to_string(), 1);
            let (client_end, agent_end) = tokio::io::duplex(64 * 1024);
            let (read, write) = tokio::io::split(agent_end);
            let (connection, io) = acp::AgentSideConnection::new(
                A3sCodeAgent::new(tests::test_launch(None)),
                write.compat_write(),
                read.compat(),
                |future| {
                    tokio::task::spawn_local(future);
                },
            );
            let transport = tokio::task::spawn_local(io);
            agent.set_client(Rc::new(connection));
            let (sender, events) = tokio::sync::mpsc::channel(1);
            sender.send(event).await.unwrap();
            drop(sender);
            if already_cancelled {
                acp::Agent::cancel(
                    &agent,
                    acp::CancelNotification::new(opened.session_id.clone()),
                )
                .await
                .unwrap();
            }
            let cancellation = async {
                let mut reader = tokio::io::BufReader::new(client_end);
                loop {
                    let mut line = String::new();
                    assert_ne!(reader.read_line(&mut line).await.unwrap(), 0);
                    let request: serde_json::Value = serde_json::from_str(&line).unwrap();
                    if matches!(
                        request["method"].as_str(),
                        Some("session/request_permission" | "_a3s/form/request")
                    ) {
                        assert!(
                            !already_cancelled,
                            "a cancelled turn must not open a new ask"
                        );
                        // Another session's Stop must not interrupt this ask.
                        acp::Agent::cancel(
                            &agent,
                            acp::CancelNotification::new(acp::SessionId::new("other-session")),
                        )
                        .await
                        .unwrap();
                        tokio::task::yield_now().await;
                        acp::Agent::cancel(
                            &agent,
                            acp::CancelNotification::new(opened.session_id.clone()),
                        )
                        .await
                        .unwrap();
                        // Keep the connection live and leave the ask unanswered.
                        std::future::pending::<()>().await;
                    }
                }
            };
            let lock = Arc::new(tokio::sync::Mutex::new(()));
            let draining = agent.drain_turn(
                &opened.session_id,
                &session,
                a3s_code_core::AgentRunStream {
                    run: None,
                    events,
                    worker: tokio::spawn(async {}),
                },
                None,
                1,
                lock.lock_owned().await,
                None,
            );
            tokio::pin!(cancellation);
            let response = tokio::time::timeout(std::time::Duration::from_secs(2), async {
                tokio::select! {
                    response = draining => response,
                    _ = &mut cancellation => std::future::pending().await,
                }
            })
            .await
            .expect("an unanswered host interaction must not block Stop")
            .unwrap();
            assert_eq!(response.stop_reason, acp::StopReason::Cancelled);
            transport.abort();
            session.close().await;
        })
        .await;
}

#[tokio::test]
async fn stop_interrupts_unanswered_permissions_questions_and_forms() {
    let permission = AgentEvent::ConfirmationRequired {
        tool_id: "pending-tool".into(),
        tool_name: "bash".into(),
        args: serde_json::json!({"command": "true"}),
        timeout_ms: 60_000,
    };
    let question = AgentEvent::UserQuestion {
        question_id: "pending-question".into(),
        question: "Choose one".into(),
        options: vec!["left".into(), "right".into()],
        allow_free_text: false,
        form: None,
    };
    let form = AgentEvent::UserQuestion {
        question_id: "pending-form".into(),
        question: "Fill in the form".into(),
        options: Vec::new(),
        allow_free_text: true,
        form: Some("root = Form()".into()),
    };
    for event in [permission, question, form] {
        cancel_unanswered_event(event.clone(), false).await;
        cancel_unanswered_event(event, true).await;
    }
}
