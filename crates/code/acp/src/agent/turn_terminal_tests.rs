#![cfg(test)]

use super::*;
use std::future::Future;

#[tokio::test]
async fn text_chunks_distinguish_model_generation_from_controller_output() {
    use tokio::io::AsyncBufReadExt;
    use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

    tokio::task::LocalSet::new()
        .run_until(async {
            let root = tempfile::tempdir().unwrap();
            let agent = AsterCodeAgent::new(tests::test_launch(Some(root.path().join("sessions"))));
            let opened = acp::Agent::new_session(&agent, acp::NewSessionRequest::new(root.path()))
                .await
                .unwrap();
            let session = agent.sessions.lock().await[opened.session_id.0.as_ref()]
                .session
                .clone();
            let (client_end, agent_end) = tokio::io::duplex(64 * 1024);
            let (read, write) = tokio::io::split(agent_end);
            let (connection, io) = acp::AgentSideConnection::new(
                AsterCodeAgent::new(tests::test_launch(None)),
                write.compat_write(),
                read.compat(),
                |future| {
                    tokio::task::spawn_local(future);
                },
            );
            let transport = tokio::task::spawn_local(io);
            agent.set_client(Rc::new(connection));
            let publish = async {
                agent
                    .emit_text(&opened.session_id, "controller 甲", false)
                    .await;
                agent
                    .handle_event(
                        &opened.session_id,
                        &session,
                        &mut TurnState::default(),
                        AgentEvent::TextDelta {
                            text: "model 乙".into(),
                        },
                    )
                    .await;
                let mut buffered = end();
                if let AgentEvent::End { ref mut text, .. } = buffered {
                    *text = "buffered 丙".into();
                }
                agent
                    .handle_event(
                        &opened.session_id,
                        &session,
                        &mut TurnState::default(),
                        buffered,
                    )
                    .await;
            };
            let observe = async {
                let mut reader = tokio::io::BufReader::new(client_end);
                let mut chunks = Vec::new();
                while chunks.len() < 3 {
                    let mut line = String::new();
                    assert_ne!(reader.read_line(&mut line).await.unwrap(), 0);
                    let value: serde_json::Value = serde_json::from_str(&line).unwrap();
                    let update = &value["params"]["update"];
                    if update["sessionUpdate"] == "agent_message_chunk" {
                        chunks.push(update.clone());
                    }
                }
                chunks
            };
            let ((), chunks) = tokio::time::timeout(std::time::Duration::from_secs(2), async {
                tokio::join!(publish, observe)
            })
            .await
            .unwrap();
            assert_eq!(chunks[0]["content"]["text"], "controller 甲");
            assert_eq!(chunks[0]["_meta"]["asterGenerationDelta"], false);
            assert_eq!(chunks[1]["content"]["text"], "model 乙");
            assert_eq!(chunks[1]["_meta"]["asterGenerationDelta"], true);
            assert_eq!(chunks[2]["content"]["text"], "buffered 丙");
            assert_eq!(chunks[2]["_meta"]["asterGenerationDelta"], false);
            transport.abort();
        })
        .await;
}

#[tokio::test]
async fn permission_denial_closes_each_tool_row_once() {
    use tokio::io::AsyncBufReadExt;
    use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

    tokio::task::LocalSet::new()
        .run_until(async {
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
            let (client_end, agent_end) = tokio::io::duplex(64 * 1024);
            let (read, write) = tokio::io::split(agent_end);
            let (connection, io) = acp::AgentSideConnection::new(
                AsterCodeAgent::new(tests::test_launch(None)),
                write.compat_write(),
                read.compat(),
                |future| {
                    tokio::task::spawn_local(future);
                },
            );
            let transport = tokio::task::spawn_local(io);
            agent.set_client(Rc::new(connection));
            let publish = async {
                let mut state = TurnState::default();
                for (id, announced) in [("unannounced", false), ("announced", true)] {
                    let args = serde_json::json!({"file_path":"source 甲.txt"});
                    if announced {
                        agent
                            .handle_event(
                                &opened.session_id,
                                &session,
                                &mut state,
                                AgentEvent::ToolExecutionStart {
                                    id: id.into(),
                                    name: "read".into(),
                                    args: args.clone(),
                                },
                            )
                            .await;
                    }
                    agent
                        .handle_event(
                            &opened.session_id,
                            &session,
                            &mut state,
                            AgentEvent::PermissionDenied {
                                tool_id: id.into(),
                                tool_name: "read".into(),
                                args,
                                reason: "controlled refusal".into(),
                            },
                        )
                        .await;
                }
                agent
                    .emit_text(&opened.session_id, "all tool rows closed", false)
                    .await;
            };
            let observe = async {
                let mut reader = tokio::io::BufReader::new(client_end);
                let mut updates = Vec::new();
                loop {
                    let mut line = String::new();
                    assert_ne!(reader.read_line(&mut line).await.unwrap(), 0);
                    let value: serde_json::Value = serde_json::from_str(&line).unwrap();
                    let update = &value["params"]["update"];
                    if update["sessionUpdate"] == "agent_message_chunk" {
                        break;
                    }
                    if update["toolCallId"].is_string() {
                        updates.push(update.clone());
                    }
                }
                updates
            };
            let ((), updates) = tokio::time::timeout(std::time::Duration::from_secs(2), async {
                tokio::join!(publish, observe)
            })
            .await
            .unwrap();
            for id in ["unannounced", "announced"] {
                let rows: Vec<_> = updates
                    .iter()
                    .filter(|row| row["toolCallId"] == id)
                    .collect();
                assert_eq!(
                    rows.iter()
                        .filter(|row| row["sessionUpdate"] == "tool_call")
                        .count(),
                    1,
                    "{id}: no duplicate opening row"
                );
                assert_eq!(
                    rows.iter()
                        .filter(|row| row["sessionUpdate"] == "tool_call_update"
                            && row["status"] == "failed")
                        .count(),
                    1
                );
                assert_eq!(rows.last().unwrap()["status"], "failed");
            }
            session.close().await;
            transport.abort();
            let _ = transport.await;
        })
        .await;
}

#[tokio::test]
async fn send_now_cancels_before_waiting_for_the_active_turn_permit() {
    for send_now in [None, Some(false), Some(true)] {
        let root = tempfile::tempdir().unwrap();
        let mut launch = tests::test_launch(Some(root.path().join("sessions")));
        launch.workspace = root.path().to_owned();
        for provider in &mut launch.config.providers {
            provider.base_url = Some("http://127.0.0.1:9/v1".into());
        }
        let agent = AsterCodeAgent::new(launch);
        let opened = acp::Agent::new_session(&agent, acp::NewSessionRequest::new(root.path()))
            .await
            .unwrap();
        let session_key = opened.session_id.0.to_string();
        agent.turn_generation.lock().await.insert(
            session_key.clone(),
            TurnGeneration {
                number: 9,
                ..Default::default()
            },
        );
        let (queue, session) = {
            let sessions = agent.sessions.lock().await;
            let live = &sessions[&session_key];
            (Arc::clone(&live.turn_queue), Arc::clone(&live.session))
        };
        let held = queue.acquire_owned().await.unwrap();
        let mut meta = acp::Meta::new();
        if let Some(value) = send_now {
            meta.insert("sendNow".into(), serde_json::json!(value));
        }
        let mut prompt = Box::pin(acp::Agent::prompt(
            &agent,
            acp::PromptRequest::new(
                opened.session_id,
                vec![acp::ContentBlock::Text(acp::TextContent::new(
                    "queued request",
                ))],
            )
            .meta(Some(meta)),
        ));
        assert!(
            std::future::poll_fn(|cx| std::task::Poll::Ready(
                prompt.as_mut().poll(cx).is_pending()
            ))
            .await
        );
        assert_eq!(
            agent
                .cancelled_generation
                .lock()
                .await
                .get(&session_key)
                .copied(),
            send_now.filter(|value| *value).map(|_| 9)
        );
        drop(prompt);
        drop(held);
        session.close().await;
    }
}

#[tokio::test]
async fn malformed_send_now_refuses_without_cancelling_or_waiting_for_the_queue() {
    let root = tempfile::tempdir().unwrap();
    let mut launch = tests::test_launch(Some(root.path().join("sessions")));
    launch.workspace = root.path().to_owned();
    for provider in &mut launch.config.providers {
        provider.base_url = Some("http://127.0.0.1:9/v1".into());
    }
    let agent = AsterCodeAgent::new(launch);
    let opened = acp::Agent::new_session(&agent, acp::NewSessionRequest::new(root.path()))
        .await
        .unwrap();
    let session_key = opened.session_id.0.to_string();
    let (queue, session) = {
        let sessions = agent.sessions.lock().await;
        let live = &sessions[&session_key];
        (Arc::clone(&live.turn_queue), Arc::clone(&live.session))
    };
    let _held = queue.acquire_owned().await.unwrap();
    for value in [
        serde_json::json!("true"),
        serde_json::json!(1),
        serde_json::Value::Null,
    ] {
        let mut meta = acp::Meta::new();
        meta.insert("sendNow".into(), value);
        let error = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            acp::Agent::prompt(
                &agent,
                acp::PromptRequest::new(
                    opened.session_id.clone(),
                    vec![acp::ContentBlock::Text(acp::TextContent::new(
                        "must not execute",
                    ))],
                )
                .meta(Some(meta)),
            ),
        )
        .await
        .expect("invalid metadata must not enter the queue")
        .unwrap_err();
        assert_eq!(error.data.unwrap(), "sendNow must be a boolean");
        assert!(!agent
            .cancelled_generation
            .lock()
            .await
            .contains_key(&session_key));
    }
    session.close().await;
}

#[tokio::test]
async fn send_now_announces_the_running_prompt_before_its_text() {
    use tokio::io::AsyncBufReadExt;
    use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

    tokio::task::LocalSet::new()
        .run_until(async {
            let root = tempfile::tempdir().unwrap();
            let mut launch = tests::test_launch(Some(root.path().join("sessions")));
            launch.workspace = root.path().to_owned();
            for provider in &mut launch.config.providers {
                provider.base_url = Some("http://127.0.0.1:9/v1".into());
            }
            let agent = AsterCodeAgent::new(launch);
            let opened = acp::Agent::new_session(&agent, acp::NewSessionRequest::new(root.path()))
                .await
                .unwrap();
            let session = agent.sessions.lock().await[opened.session_id.0.as_ref()]
                .session
                .clone();
            let (client_end, agent_end) = tokio::io::duplex(64 * 1024);
            let (read, write) = tokio::io::split(agent_end);
            let (connection, io) = acp::AgentSideConnection::new(
                AsterCodeAgent::new(tests::test_launch(None)),
                write.compat_write(),
                read.compat(),
                |future| {
                    tokio::task::spawn_local(future);
                },
            );
            let transport = tokio::task::spawn_local(io);
            agent.set_client(Rc::new(connection));
            let mut meta = acp::Meta::new();
            meta.insert("sendNow".into(), serde_json::json!(true));
            meta.insert("promptId".into(), serde_json::json!("send-now-inspection"));
            let prompt = acp::Agent::prompt(
                &agent,
                acp::PromptRequest::new(
                    opened.session_id.clone(),
                    vec![acp::ContentBlock::Text(acp::TextContent::new("/tasks"))],
                )
                .meta(Some(meta)),
            );
            let observe = async {
                let mut reader = tokio::io::BufReader::new(client_end);
                let mut line = String::new();
                assert_ne!(reader.read_line(&mut line).await.unwrap(), 0);
                let first: serde_json::Value = serde_json::from_str(&line).unwrap();
                assert_eq!(first["method"], "_aster/queue/changed");
                assert_eq!(
                    first["params"]["sessionId"],
                    opened.session_id.0.to_string()
                );
                assert_eq!(first["params"]["runningPromptId"], "send-now-inspection");
                assert_eq!(first["params"]["runningText"], "/tasks");
                assert_eq!(first["params"]["runningKind"], "prompt");
            };
            let (response, ()) = tokio::time::timeout(std::time::Duration::from_secs(2), async {
                tokio::join!(prompt, observe)
            })
            .await
            .expect("the running identity must precede output");
            assert_eq!(response.unwrap().stop_reason, acp::StopReason::EndTurn);
            session.close().await;
            transport.abort();
            let _ = transport.await;
        })
        .await;
}

async fn drain(events: Vec<AgentEvent>, cancelled: bool) -> acp::Result<acp::PromptResponse> {
    drain_with_channel_lifetime(events, cancelled, false).await
}

async fn drain_with_channel_lifetime(
    events: Vec<AgentEvent>,
    cancelled: bool,
    keep_sender: bool,
) -> acp::Result<acp::PromptResponse> {
    let root = tempfile::tempdir().unwrap();
    let mut launch = tests::test_launch(Some(root.path().join("sessions")));
    launch.workspace = root.path().to_owned();
    let agent = AsterCodeAgent::new(launch);
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
    let _retained_sender = keep_sender.then_some(sender);
    let permit = Arc::new(tokio::sync::Mutex::new(())).lock_owned().await;
    let draining = agent.drain_turn(
        &opened.session_id,
        &session,
        aster_core::AgentRunStream {
            run: None,
            events: receiver,
            worker: tokio::spawn(async {}),
        },
        None,
        1,
        permit,
        None,
    );
    let result = if keep_sender {
        tokio::time::timeout(std::time::Duration::from_secs(1), draining)
            .await
            .expect("the original worker's result must not wait for a background sender")
    } else {
        draining.await
    };
    session.close().await;
    result
}

#[tokio::test]
async fn completed_worker_returns_even_when_a_background_sender_stays_open() {
    assert_eq!(
        drain_with_channel_lifetime(vec![end()], false, true)
            .await
            .unwrap()
            .stop_reason,
        acp::StopReason::EndTurn
    );
    let error = drain_with_channel_lifetime(Vec::new(), false, true)
        .await
        .unwrap_err();
    assert!(error
        .data
        .unwrap()
        .as_str()
        .unwrap()
        .contains("without a terminal event"));
}

fn end() -> AgentEvent {
    AgentEvent::End {
        text: "delivered answer".into(),
        usage: Default::default(),
        verification_summary: Box::new(
            aster_core::verification::VerificationSummary::from_reports(&[]),
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
async fn a_core_failure_after_visible_text_preserves_its_original_reason() {
    let reason = "goal.evidence_stale: source changed before terminal settlement";
    let error = drain(
        vec![
            AgentEvent::TextDelta {
                text: "Partial: the edits are ready for verification.".into(),
            },
            AgentEvent::Error {
                message: reason.into(),
            },
        ],
        false,
    )
    .await
    .unwrap_err();
    assert_eq!(error.data.unwrap(), reason);
}

#[tokio::test]
async fn forced_stop_is_not_a_successful_end_turn() {
    use aster_core::harness_loop::ForcedStop;
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
    let agent = AsterCodeAgent::new(launch);
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
        aster_core::AgentRunStream {
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

#[tokio::test]
async fn cancelled_prompt_forwards_a_child_terminal_that_arrives_after_the_response() {
    assert_late_child_terminal(true, true).await;
}

#[tokio::test]
async fn cancelled_prompt_observes_a_child_terminal_after_the_event_stream_closes() {
    assert_late_child_terminal(false, true).await;
}

#[tokio::test]
async fn completed_prompt_keeps_observing_its_live_child_after_the_response() {
    assert_late_child_terminal(true, false).await;
    assert_late_child_terminal(false, false).await;
}

async fn assert_late_child_terminal(via_event: bool, parent_cancelled: bool) {
    use tokio::io::AsyncBufReadExt;
    use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

    tokio::task::LocalSet::new()
        .run_until(async {
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
            let (client_end, agent_end) = tokio::io::duplex(64 * 1024);
            let (read, write) = tokio::io::split(agent_end);
            let (connection, io) = acp::AgentSideConnection::new(
                AsterCodeAgent::new(tests::test_launch(None)),
                write.compat_write(),
                read.compat(),
                |future| {
                    tokio::task::spawn_local(future);
                },
            );
            let transport = tokio::task::spawn_local(io);
            agent.set_client(Rc::new(connection));
            let start = AgentEvent::SubagentStart {
                task_id: "late-child".into(),
                session_id: "late-child".into(),
                parent_session_id: opened.session_id.0.to_string(),
                agent: "general".into(),
                description: "Late cleanup".into(),
                started_ms: 100,
            };
            let tracker = session.subagent_tracker();
            tracker.record_event(&start).await;
            let token = tokio_util::sync::CancellationToken::new();
            tracker
                .register_canceller("late-child", token.clone())
                .await;
            let background = AgentEvent::SubagentProgress {
                task_id: "late-child".into(),
                session_id: "late-child".into(),
                status: "started".into(),
                metadata: serde_json::json!({"background":true}),
            };
            let mut state = TurnState::default();
            if parent_cancelled {
                agent
                    .handle_event(&opened.session_id, &session, &mut state, start.clone())
                    .await;
                agent
                    .handle_event(&opened.session_id, &session, &mut state, background.clone())
                    .await;
                acp::Agent::cancel(
                    &agent,
                    acp::CancelNotification::new(opened.session_id.clone()),
                )
                .await
                .unwrap();
                assert!(token.is_cancelled());
            }
            let (sender, events) = tokio::sync::mpsc::channel(3);
            if !parent_cancelled {
                sender.send(start).await.unwrap();
                sender.send(background).await.unwrap();
                sender.send(end()).await.unwrap();
            }
            let sender = via_event.then_some(sender);
            let (release, waiting) = tokio::sync::oneshot::channel::<()>();
            let worker = tokio::spawn(async move {
                waiting.await.unwrap();
                let end = AgentEvent::SubagentEnd {
                    task_id: "late-child".into(),
                    session_id: "late-child".into(),
                    agent: "general".into(),
                    success: !parent_cancelled,
                    output: if parent_cancelled {
                        "cancelled"
                    } else {
                        "done"
                    }
                    .into(),
                    finished_ms: 700,
                };
                for _ in 0..6 {
                    tracker
                        .record_event(&AgentEvent::SubagentProgress {
                            task_id: "late-child".into(),
                            session_id: "late-child".into(),
                            status: "tool_completed".into(),
                            metadata: serde_json::json!({"tool":"read","exit_code":0}),
                        })
                        .await;
                }
                tracker
                    .record_event(&AgentEvent::SubagentProgress {
                        task_id: "late-child".into(),
                        session_id: "late-child".into(),
                        status: "turn_completed".into(),
                        metadata: serde_json::json!({"turn":7,"total_tokens":123}),
                    })
                    .await;
                tracker.record_event(&end).await;
                if let Some(sender) = sender {
                    sender
                        .send(end)
                        .await
                        .expect("the terminal receiver survives the prompt");
                }
            });
            let lock = Arc::new(tokio::sync::Mutex::new(()));
            let responding = async {
                if parent_cancelled {
                    agent
                        .leave_cancelled_turn(
                            &opened.session_id,
                            &session,
                            &mut state,
                            events,
                            worker,
                            lock.lock_owned().await,
                        )
                        .await
                } else {
                    drop(worker);
                    agent
                        .drain_turn(
                            &opened.session_id,
                            &session,
                            aster_core::AgentRunStream {
                                run: None,
                                events,
                                worker: tokio::spawn(async {}),
                            },
                            None,
                            1,
                            lock.lock_owned().await,
                            None,
                        )
                        .await
                }
            };
            let response = tokio::time::timeout(std::time::Duration::from_secs(1), responding)
                .await
                .expect("Stop returns before child cleanup")
                .unwrap();
            assert_eq!(
                response.stop_reason,
                if parent_cancelled {
                    acp::StopReason::Cancelled
                } else {
                    acp::StopReason::EndTurn
                }
            );
            assert!(session
                .subagent_task("late-child")
                .await
                .unwrap()
                .finished_ms
                .is_none());
            release.send(()).unwrap();
            let mut reader = tokio::io::BufReader::new(client_end);
            let finished = tokio::time::timeout(std::time::Duration::from_secs(2), async {
                let mut observed_background = false;
                loop {
                    let mut line = String::new();
                    assert_ne!(reader.read_line(&mut line).await.unwrap(), 0);
                    let message: serde_json::Value = serde_json::from_str(&line).unwrap();
                    let update = &message["params"]["update"];
                    if update["sessionUpdate"] == "subagent_progress"
                        && message["params"]["_meta"]["subagentBackground"] == true
                    {
                        observed_background = true;
                    }
                    if update["sessionUpdate"] == "subagent_finished" {
                        assert!(
                            observed_background,
                            "actual ACP wire must carry the typed background flag"
                        );
                        break update.clone();
                    }
                }
            })
            .await
            .expect("late terminal reaches the actual ACP connection");
            assert_eq!(finished["subagent_id"], "late-child");
            assert_eq!(
                finished["status"],
                if parent_cancelled {
                    "stopped"
                } else {
                    "completed"
                }
            );
            assert_eq!(finished["completedAt"], 700);
            assert_eq!(finished["duration_ms"], 600);
            assert_eq!(finished["tool_calls"], 6);
            assert_eq!(finished["turns"], 7);
            assert_eq!(finished["tokens_used"], 123);

            assert!(finished["error"].is_null());
            session.close().await;
            transport.abort();
        })
        .await;
}

async fn cancel_unanswered_event(event: AgentEvent, already_cancelled: bool) {
    use tokio::io::AsyncBufReadExt;
    use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

    tokio::task::LocalSet::new()
        .run_until(async {
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
            agent.turn_generation.lock().await.insert(
                opened.session_id.0.to_string(),
                TurnGeneration {
                    number: 1,
                    ..Default::default()
                },
            );
            let (client_end, agent_end) = tokio::io::duplex(64 * 1024);
            let (read, write) = tokio::io::split(agent_end);
            let (connection, io) = acp::AgentSideConnection::new(
                AsterCodeAgent::new(tests::test_launch(None)),
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
                        Some("session/request_permission" | "_aster/form/request")
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
                aster_core::AgentRunStream {
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
