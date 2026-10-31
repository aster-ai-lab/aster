#![cfg(test)]

use super::*;
use aster_core::llm::{
    ContentBlock, LlmClient, LlmResponse, Message, StreamEvent, TokenUsage, ToolDefinition,
};
use aster_core::store::{MemorySessionStore, SessionStore};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::mpsc;
use tokio::time::{timeout, Duration};
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

#[derive(Default)]
struct PlanModel {
    calls: AtomicUsize,
}

#[async_trait::async_trait]
impl LlmClient for PlanModel {
    async fn complete(
        &self,
        _messages: &[Message],
        _: Option<&str>,
        tools: &[ToolDefinition],
    ) -> anyhow::Result<LlmResponse> {
        if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
            assert!(tools.iter().any(|tool| tool.name == "update_plan"));
            return Ok(LlmResponse {
                message: Message {
                    content: vec![ContentBlock::ToolUse {
                        id: "plan-call".into(),
                        name: "update_plan".into(),
                        input: json!({"plan":[
                            {"id":"pending","step":"Pending","status":"pending"},
                            {"id":"active","step":"Active","status":"in_progress"},
                            {"id":"done","step":"Done","status":"completed"},
                            {"id":"failed","step":"Failed","status":"failed"},
                            {"id":"skipped","step":"Skipped","status":"skipped"},
                            {"id":"cancelled","step":"Cancelled","status":"cancelled"}
                        ]}),
                    }],
                    ..Message::assistant("")
                },
                usage: TokenUsage::default(),
                stop_reason: None,
                token_logprobs: vec![],
                meta: None,
            });
        }
        Ok(LlmResponse {
            message: Message::assistant("Plan recorded"),
            usage: TokenUsage::default(),
            stop_reason: None,
            token_logprobs: vec![],
            meta: None,
        })
    }

    async fn complete_streaming(
        &self,
        _: &[Message],
        _: Option<&str>,
        _: &[ToolDefinition],
        _: tokio_util::sync::CancellationToken,
    ) -> anyhow::Result<mpsc::Receiver<StreamEvent>> {
        anyhow::bail!("non-streaming plan fixture")
    }
}

struct Host(mpsc::UnboundedSender<Value>);

#[async_trait::async_trait(?Send)]
impl acp::Client for Host {
    async fn request_permission(
        &self,
        _: acp::RequestPermissionRequest,
    ) -> acp::Result<acp::RequestPermissionResponse> {
        Err(acp::Error::internal_error())
    }

    async fn session_notification(
        &self,
        notification: acp::SessionNotification,
    ) -> acp::Result<()> {
        self.0
            .send(serde_json::to_value(notification).unwrap())
            .unwrap();
        Ok(())
    }
}

#[tokio::test]
async fn load_replay_publishes_the_resumed_typed_plan() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let root = tempfile::tempdir().unwrap();
            let acp_root = tempfile::tempdir().unwrap();
            let launch = crate::agent::tests::test_launch(Some(root.path().join("sessions")));
            let store = Arc::new(MemorySessionStore::new());
            let core = Agent::from_config(aster_core::CodeConfig::default())
                .await
                .unwrap();
            let session = core
                .session_async(
                    root.path().display().to_string(),
                    Some(
                        SessionOptions::new()
                            .with_session_store(store.clone())
                            .with_auto_save(true)
                            .with_llm_client(Arc::new(PlanModel::default()))
                            .with_planning_mode(aster_core::PlanningMode::Disabled)
                            .with_permission_policy(
                                aster_core::permissions::PermissionPolicy::new()
                                    .allow("update_plan"),
                            ),
                    ),
                )
                .await
                .unwrap();
            let session = Arc::new(session);
            let core_session_id = session.session_id().to_string();
            let agent = Rc::new(AsterCodeAgent::new(launch));
            let (left, right) = tokio::io::duplex(64 * 1024);
            let (agent_read, agent_write) = tokio::io::split(left);
            let (client_read, client_write) = tokio::io::split(right);
            let (connection, agent_io) = acp::AgentSideConnection::new(
                agent.clone(),
                agent_write.compat_write(),
                agent_read.compat(),
                |future| {
                    tokio::task::spawn_local(future);
                },
            );
            agent.set_client(Rc::new(connection));
            let (sender, mut packets) = mpsc::unbounded_channel();
            let (client, client_io) = acp::ClientSideConnection::new(
                Host(sender),
                client_write.compat_write(),
                client_read.compat(),
                |future| {
                    tokio::task::spawn_local(future);
                },
            );
            let io = vec![
                tokio::task::spawn_local(async move {
                    let _ = agent_io.await;
                }),
                tokio::task::spawn_local(async move {
                    let _ = client_io.await;
                }),
            ];
            let client = Rc::new(client);
            let opened = acp::Agent::new_session(
                client.as_ref(),
                acp::NewSessionRequest::new(acp_root.path()),
            )
            .await
            .unwrap();
            let session_id = opened.session_id;
            let original = {
                let mut sessions = agent.sessions.lock().await;
                let live = sessions.get_mut(session_id.0.as_ref()).unwrap();
                std::mem::replace(&mut live.session, session.clone())
            };
            original.close().await;
            while packets.try_recv().is_ok() {}

            agent
                .replay_loaded_transcript(&session_id, &session)
                .await
                .unwrap();
            assert!(packets.try_recv().is_err(), "empty plans stay silent");
            let mut commits = store.watch_commits().await.unwrap();
            acp::Agent::prompt(
                client.as_ref(),
                acp::PromptRequest::new(
                    session_id.clone(),
                    vec![acp::ContentBlock::Text(acp::TextContent::new(
                        "Record the plan",
                    ))],
                ),
            )
            .await
            .unwrap();
            let mut live = Vec::new();
            while let Ok(notification) = packets.try_recv() {
                live.push(notification);
            }
            let live_plan = live
                .iter()
                .find(|notification| notification["update"]["sessionUpdate"] == "plan")
                .expect("the real ACP run publishes its typed plan");
            assert_eq!(live_plan["update"]["entries"].as_array().unwrap().len(), 6);
            let current_session = agent
                .sessions
                .lock()
                .await
                .get(session_id.0.as_ref())
                .unwrap()
                .session
                .clone();
            assert!(Arc::ptr_eq(&current_session, &session));
            assert_eq!(current_session.plan_snapshot().await.unwrap().len(), 6);
            current_session.save().await.unwrap();
            let saved = timeout(Duration::from_secs(5), async {
                loop {
                    let saved = store.load(&core_session_id).await.unwrap().unwrap();
                    if saved.tasks.len() == 6 {
                        break saved;
                    }
                    let commit = commits.recv().await.unwrap();
                    assert_eq!(commit.session_id, core_session_id);
                }
            })
            .await
            .expect("the runtime plan reaches a durable generation");
            assert_eq!(saved.tasks.len(), 6);
            assert_eq!(session.plan_snapshot().await.unwrap().len(), 6);
            current_session.close().await;
            drop(current_session);
            drop(session);
            let resume_agent = Agent::from_config(aster_core::CodeConfig::default())
                .await
                .unwrap();
            // Keep resume's nested runtime independent of ACP's LocalSet; the
            // in-LocalSet equivalent overflows the test thread's stack.
            let resumed = tokio::task::spawn_blocking(move || {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap()
                    .block_on(
                        resume_agent.resume_session_async(
                            &core_session_id,
                            SessionOptions::new()
                                .with_session_store(store)
                                .with_llm_client(Arc::new(PlanModel::default())),
                        ),
                    )
            })
            .await
            .unwrap()
            .unwrap();

            agent
                .replay_loaded_transcript(&session_id, &resumed)
                .await
                .unwrap();
            let mut replayed = Vec::new();
            loop {
                let notification = timeout(Duration::from_secs(5), packets.recv())
                    .await
                    .expect("the resumed transcript and plan are published")
                    .unwrap();
                let is_plan = notification["update"]["sessionUpdate"] == "plan";
                replayed.push(notification);
                if is_plan {
                    break;
                }
                assert!(replayed.len() <= 8, "unbounded replay fixture");
            }
            assert!(replayed.len() >= 3, "transcript precedes the plan");
            assert!(replayed[..replayed.len() - 1].iter().all(|notification| {
                matches!(
                    notification["update"]["sessionUpdate"].as_str(),
                    Some(
                        "user_message_chunk"
                            | "agent_message_chunk"
                            | "tool_call"
                            | "tool_call_update"
                    )
                )
            }));
            let notification = replayed.last().unwrap();
            assert_eq!(notification["update"]["sessionUpdate"], "plan");
            assert_eq!(notification["_meta"]["isReplay"], true);
            assert_eq!(notification["_meta"]["origin"], "core.plan_projection");
            assert!(notification["_meta"]["entryId"]
                .as_str()
                .is_some_and(|id| id.starts_with("plan:")));
            assert!(notification["_meta"]["digest"].as_str().is_some());
            let entries = notification["update"]["entries"].as_array().unwrap();
            assert_eq!(entries.len(), 6);
            for (entry, (id, status)) in entries.iter().zip([
                ("pending", "pending"),
                ("active", "in_progress"),
                ("done", "completed"),
                ("failed", "failed"),
                ("skipped", "skipped"),
                ("cancelled", "cancelled"),
            ]) {
                assert_eq!(entry["_meta"]["id"], id);
                let actual = entry["_meta"]["asterStatus"]
                    .as_str()
                    .unwrap_or_else(|| entry["status"].as_str().unwrap());
                assert_eq!(actual, status);
            }
            assert!(timeout(Duration::from_millis(50), packets.recv())
                .await
                .is_err());

            resumed.close().await;
            for task in io {
                task.abort();
            }
        })
        .await;
}

fn replay_entry(
    payload: aster_core::store::TranscriptEntryPayload,
) -> aster_core::store::TranscriptEntry {
    use aster_core::store::{TranscriptEntryKind, TranscriptEntryPayload as Payload};
    let kind = match payload {
        Payload::User { .. } => TranscriptEntryKind::User,
        Payload::Assistant { .. } => TranscriptEntryKind::Assistant,
        Payload::Tool { .. } => TranscriptEntryKind::Tool,
        Payload::Compaction { .. } => TranscriptEntryKind::Compaction,
        Payload::TurnComplete { .. } => TranscriptEntryKind::TurnComplete,
        Payload::LegacyHistory { .. } => TranscriptEntryKind::LegacyHistory,
    };
    aster_core::store::TranscriptEntry {
        entry_id: "fact:7:digest".into(),
        kind,
        fact_seq: 7,
        source_span: aster_core::store::TranscriptSourceSpan {
            first_seq: 7,
            last_seq: 8,
        },
        digest: "entry-digest".into(),
        payload,
    }
}

fn replay_json(notification: ReplayNotification) -> Value {
    match notification {
        ReplayNotification::Acp(notification) => serde_json::to_value(notification).unwrap(),
        ReplayNotification::Typed(notification) => json!({
            "method": format!("_{}", notification.method),
            "params": serde_json::from_str::<Value>(notification.params.get()).unwrap(),
        }),
    }
}

#[test]
fn replay_maps_tool_parts_without_dropping_text_or_entry_identity() {
    let entry = replay_entry(aster_core::store::TranscriptEntryPayload::Tool {
        calls: vec![aster_core::store::TranscriptToolCall {
            id: "call-1".into(),
            name: "read".into(),
            args: json!({"file_path":"lib.rs"}),
            text: Some("Reading".into()),
            reasoning: Some("Need source".into()),
        }],
        results: vec![aster_core::store::TranscriptToolResult {
            tool_call_id: "call-1".into(),
            ok: true,
            output: "contents".into(),
        }],
        complete: true,
    });
    let notifications = transcript_replay_notifications(&acp::SessionId::new("s"), &entry)
        .unwrap()
        .into_iter()
        .map(replay_json)
        .collect::<Vec<_>>();
    assert_eq!(notifications.len(), 4);
    assert_eq!(
        notifications[0]["update"]["sessionUpdate"],
        "agent_message_chunk"
    );
    assert_eq!(
        notifications[1]["update"]["sessionUpdate"],
        "agent_thought_chunk"
    );
    assert_eq!(notifications[2]["update"]["sessionUpdate"], "tool_call");
    assert_eq!(
        notifications[2]["update"]["rawInput"]["file_path"],
        "lib.rs"
    );
    assert_eq!(
        notifications[3]["update"]["sessionUpdate"],
        "tool_call_update"
    );
    for notification in notifications {
        assert_eq!(notification["_meta"]["entryId"], "fact:7:digest");
        assert_eq!(notification["_meta"]["factSeq"], 7);
        assert_eq!(notification["_meta"]["origin"], "core.transcript_page");
        assert_eq!(notification["_meta"]["digest"], "entry-digest");
    }
}

#[test]
fn replay_exposes_compaction_turn_completion_and_legacy_as_typed_updates() {
    for (payload, expected) in [
        (
            aster_core::store::TranscriptEntryPayload::Compaction {
                summary: "summary".into(),
                kept_lines: 2,
            },
            "compaction",
        ),
        (
            aster_core::store::TranscriptEntryPayload::TurnComplete {
                terminal: "narrative".into(),
                effect_digest: None,
                forced_stop: None,
                log_cut: aster_core::store::TranscriptSourceCut {
                    seq: 6,
                    digest: "cut".into(),
                },
            },
            "turn_complete",
        ),
        (
            aster_core::store::TranscriptEntryPayload::LegacyHistory {
                source_kind: "user.message".into(),
                fact_key: "user:rich".into(),
                reason: "rich user parts require legacy_history".into(),
                display: None,
            },
            "legacy_history",
        ),
    ] {
        let entry = replay_entry(payload);
        let notifications = transcript_replay_notifications(&acp::SessionId::new("s"), &entry)
            .unwrap()
            .into_iter()
            .map(replay_json)
            .collect::<Vec<_>>();
        assert_eq!(notifications.len(), 1);
        assert_eq!(
            notifications[0]["method"],
            "_aster/session/transcript/update"
        );
        assert_eq!(
            notifications[0]["params"]["schema"],
            TRANSCRIPT_UPDATE_SCHEMA
        );
        assert_eq!(
            notifications[0]["params"]["update"]["sessionUpdate"],
            expected
        );
        assert_eq!(
            notifications[0]["params"]["_meta"]["entryId"],
            "fact:7:digest"
        );
        if expected == "legacy_history" {
            assert_eq!(
                notifications[0]["params"]["update"]["reason"],
                "rich user parts require legacy_history"
            );
        }
    }
}

#[test]
fn replay_preserves_legacy_display_without_creating_an_executable_message() {
    let display = aster_core::store::TranscriptLegacyHistoryDisplay {
        source_entry_id: "historical-entry".into(),
        role: aster_core::store::LegacyStudioRole::User,
        text: "A stored historical message.".into(),
        attachments: vec![],
    };
    let entry = replay_entry(aster_core::store::TranscriptEntryPayload::LegacyHistory {
        source_kind: "legacy.studio.entry".into(),
        fact_key: "historical-fact".into(),
        reason: "Display-only imported history".into(),
        display: Some(display.clone()),
    });
    let notifications = transcript_replay_notifications(&acp::SessionId::new("s"), &entry)
        .unwrap()
        .into_iter()
        .map(replay_json)
        .collect::<Vec<_>>();
    assert_eq!(notifications.len(), 1);
    assert_eq!(
        notifications[0]["method"],
        "_aster/session/transcript/update"
    );
    assert_eq!(
        notifications[0]["params"]["update"]["sessionUpdate"],
        "legacy_history"
    );
    assert_eq!(
        notifications[0]["params"]["update"]["display"],
        json!(display)
    );
}
