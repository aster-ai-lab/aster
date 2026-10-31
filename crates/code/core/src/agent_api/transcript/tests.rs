use super::*;
use crate::agent_api::tests::test_config;
use crate::store::{MemorySessionStore, SessionStore};
use aster_effect::NewFact;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Default)]
struct CountingClient {
    calls: AtomicUsize,
}

#[async_trait::async_trait]
impl LlmClient for CountingClient {
    async fn complete(
        &self,
        _messages: &[Message],
        _system: Option<&str>,
        _tools: &[crate::llm::ToolDefinition],
    ) -> anyhow::Result<crate::llm::LlmResponse> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(crate::llm::LlmResponse {
            message: Message::assistant("unexpected model call"),
            usage: Default::default(),
            stop_reason: None,
            token_logprobs: Vec::new(),
            meta: None,
        })
    }

    async fn complete_streaming(
        &self,
        _messages: &[Message],
        _system: Option<&str>,
        _tools: &[crate::llm::ToolDefinition],
        _cancel_token: tokio_util::sync::CancellationToken,
    ) -> anyhow::Result<mpsc::Receiver<crate::llm::StreamEvent>> {
        anyhow::bail!("unused")
    }
}

struct Fixture {
    root: tempfile::TempDir,
    agent: Agent,
    session: AgentSession,
    store: Arc<MemorySessionStore>,
    client: Arc<CountingClient>,
}

fn options(
    id: &str,
    store: Arc<MemorySessionStore>,
    client: Arc<CountingClient>,
) -> SessionOptions {
    SessionOptions::new()
        .with_session_id(id)
        .with_session_store(store)
        .with_llm_client(client)
        .with_planning_mode(crate::PlanningMode::Disabled)
        .with_auto_delegation_enabled(false)
        .with_manual_delegation_enabled(false)
        .with_continuation(false)
}

async fn fixture(id: &str) -> Fixture {
    let root = tempfile::tempdir().unwrap();
    let agent = Agent::from_config(test_config()).await.unwrap();
    let store = Arc::new(MemorySessionStore::new());
    let client = Arc::new(CountingClient::default());
    let session = agent
        .session_async(
            root.path().display().to_string(),
            Some(options(id, store.clone(), client.clone())),
        )
        .await
        .unwrap();
    Fixture {
        root,
        agent,
        session,
        store,
        client,
    }
}

fn log(fixture: &Fixture) -> aster_effect::FileLog {
    aster_effect::FileLog::open(crate::session_folder::effect_log_dir(
        fixture.root.path(),
        fixture.session.session_id(),
    ))
    .unwrap()
}

fn append(fixture: &Fixture, kind: &str, key: &str, payload: serde_json::Value) {
    log(fixture)
        .append(
            &crate::fact_control::thread_for_session(fixture.session.session_id()),
            &[NewFact {
                kind: kind.into(),
                key: key.into(),
                payload,
            }],
            None,
        )
        .unwrap();
}

#[tokio::test]
async fn prompt_count_reuses_the_session_writer_and_refuses_a_foreign_writer() {
    let fixture = fixture("prompt-count-writer").await;
    append(
        &fixture,
        "user.message",
        "first",
        serde_json::json!({"text":"first"}),
    );
    append(
        &fixture,
        "model.turn",
        "reply",
        serde_json::json!({"kind":"text","text":"reply"}),
    );
    append(
        &fixture,
        "question.answered",
        "answer",
        serde_json::json!({"text":"answer"}),
    );
    append(
        &fixture,
        "compaction.done",
        "compact",
        serde_json::json!({"summary":"earlier context","kept":[]}),
    );
    append(
        &fixture,
        "user.message",
        "second",
        serde_json::json!({"text":"second"}),
    );
    assert_eq!(fixture.session.user_prompt_count().unwrap(), 2);
    let owned = fixture
        .session
        .run_admission
        .fact_log_slot()
        .lock()
        .unwrap()
        .clone()
        .unwrap();
    let writer = owned.try_writer_fence().unwrap();
    assert_eq!(fixture.session.user_prompt_count().unwrap(), 2);
    drop(writer);
    let foreign = log(&fixture);
    let foreign_writer = foreign.try_writer_fence().unwrap();
    let error = fixture.session.user_prompt_count().unwrap_err();
    assert!(
        error
            .to_string()
            .contains("fact log is busy in another instance"),
        "{error}"
    );
    drop((foreign_writer, foreign));
    assert_eq!(fixture.session.user_prompt_count().unwrap(), 2);
    assert_eq!(fixture.client.calls.load(Ordering::SeqCst), 0);
    fixture.session.close().await;
    fixture.agent.close().await;
}

fn append_complete(fixture: &Fixture, key: &str) {
    let log = log(fixture);
    let thread = crate::fact_control::thread_for_session(fixture.session.session_id());
    let cut = aster_effect::cut_log(&log.read(&thread).unwrap());
    log.append(
        &thread,
        &[NewFact {
            kind: "turn.complete".into(),
            key: key.into(),
            payload: serde_json::json!({
                "terminal": "narrative",
                "effect_digest": null,
                "forced_stop": null,
                "receipt_keys": [],
                "background_receipt_keys": [],
                "log_cut": {"seq": cut.seq, "digest": cut.digest},
            }),
        }],
        None,
    )
    .unwrap();
}

fn append_text_turn(fixture: &Fixture, turn: usize) {
    append(
        fixture,
        "user.message",
        &format!("user-{turn}"),
        serde_json::json!({"text": format!("question-{turn}")}),
    );
    append(
        fixture,
        "model.turn",
        &format!("model-{turn}"),
        serde_json::json!({"kind":"text", "text": format!("answer-{turn}")}),
    );
    append_complete(fixture, &format!("complete-{turn}"));
}

async fn two_turn_fixture(id: &str) -> Fixture {
    let fixture = fixture(id).await;
    append_text_turn(&fixture, 1);
    append_text_turn(&fixture, 2);
    fixture.session.save().await.unwrap();
    fixture
}

fn page(fixture: &Fixture) -> TranscriptPage {
    fixture
        .session
        .transcript_page(TranscriptPageRequest {
            cursor: None,
            limit: 50,
        })
        .unwrap()
}

fn source_cut(fixture: &Fixture) -> TranscriptSourceCut {
    let facts = crate::fact_control::read_workspace_facts(
        fixture.root.path(),
        &crate::fact_control::thread_for_session(fixture.session.id()),
    )
    .unwrap();
    TranscriptSourceCut::from(aster_effect::cut_log(&facts))
}

fn fork_request(fixture: &Fixture, boundary_entry_id: String, target: &str) -> PrefixForkRequest {
    PrefixForkRequest {
        boundary_entry_id,
        expected_cut: source_cut(fixture),
        target_session_id: target.into(),
        expected_workspace: fixture.root.path().to_path_buf(),
    }
}

async fn close(fixture: Fixture) {
    fixture.session.close().await;
    fixture.agent.close().await;
}

#[tokio::test]
async fn transcript_ids_sequences_and_cursors_are_stable_and_strict() {
    let fixture = two_turn_fixture("transcript-pages").await;
    let first = fixture
        .session
        .transcript_page(TranscriptPageRequest {
            cursor: None,
            limit: 2,
        })
        .unwrap();
    assert_eq!(first.entries.len(), 2);
    assert_eq!(first.entries[0].fact_seq, 1);
    assert_eq!(first.entries[1].fact_seq, 2);
    let ids = first
        .entries
        .iter()
        .map(|entry| entry.entry_id.clone())
        .collect::<Vec<_>>();
    let reread = fixture
        .session
        .transcript_page(TranscriptPageRequest {
            cursor: None,
            limit: 2,
        })
        .unwrap();
    assert_eq!(
        reread
            .entries
            .iter()
            .map(|entry| entry.entry_id.clone())
            .collect::<Vec<_>>(),
        ids
    );
    let second = fixture
        .session
        .transcript_page(TranscriptPageRequest {
            cursor: first.next_cursor.clone(),
            limit: 2,
        })
        .unwrap();
    assert_eq!(second.entries[0].kind, TranscriptEntryKind::TurnComplete);
    assert_eq!(second.entries[1].kind, TranscriptEntryKind::User);
    let third = fixture
        .session
        .transcript_page(TranscriptPageRequest {
            cursor: second.next_cursor,
            limit: 2,
        })
        .unwrap();
    assert_eq!(third.entries.len(), 2);
    assert!(third.next_cursor.is_none());

    let mut foreign = first.next_cursor.clone().unwrap();
    foreign.session_id = "other-session".into();
    let error = fixture
        .session
        .transcript_page(TranscriptPageRequest {
            cursor: Some(foreign),
            limit: 2,
        })
        .unwrap_err();
    assert!(error.to_string().contains("cursor/session mismatch"));
    append(
        &fixture,
        "user.message",
        "later-user",
        serde_json::json!({"text":"later"}),
    );
    let error = fixture
        .session
        .transcript_page(TranscriptPageRequest {
            cursor: first.next_cursor,
            limit: 2,
        })
        .unwrap_err();
    assert!(error.to_string().contains("source changed"));
    let plan = fixture.session.plan_projection().await.unwrap();
    assert!(plan.revision.is_none());
    assert!(!plan.digest.is_empty());
    close(fixture).await;
}

#[tokio::test]
async fn transcript_preserves_pre_compaction_history_and_aggregates_typed_tools() {
    let fixture = fixture("transcript-compaction-tools").await;
    append_text_turn(&fixture, 1);
    append(
        &fixture,
        "compaction.done",
        "compact-1",
        serde_json::json!({"summary":"older history summary", "kept":[]}),
    );
    append(
        &fixture,
        "user.message",
        "user-tool",
        serde_json::json!({"text":"run a tool"}),
    );
    append(
        &fixture,
        "model.turn",
        "model-tool",
        serde_json::json!({"kind":"tool", "call":{
            "id":"call-1", "name":"read", "args":{"path":"a.txt"},
            "needs_confirmation":false, "text":"reading", "reasoning":"check"
        }, "also":[]}),
    );
    append(
        &fixture,
        "confirmation.answered",
        "confirmation-1",
        serde_json::json!({"tool_call_id":"call-1", "approved":true}),
    );
    append(
        &fixture,
        "tool.prepared",
        "prepared-1",
        serde_json::json!({"prepared":{"opaque":"execution metadata"}}),
    );
    append(
        &fixture,
        "tool.result",
        "result-1",
        serde_json::json!({"toolCallId":"call-1", "ok":true, "output":"file text"}),
    );
    append_complete(&fixture, "complete-tool");
    let transcript = page(&fixture);
    assert_eq!(transcript.entries[0].kind, TranscriptEntryKind::User);
    assert!(transcript.entries.iter().any(|entry| matches!(
        &entry.payload,
        TranscriptEntryPayload::Compaction { summary, .. } if summary == "older history summary"
    )));
    let tool = transcript
        .entries
        .iter()
        .find(|entry| entry.kind == TranscriptEntryKind::Tool)
        .unwrap();
    let TranscriptEntryPayload::Tool {
        calls,
        results,
        complete,
    } = &tool.payload
    else {
        panic!("expected tool payload")
    };
    assert!(complete);
    assert_eq!(calls[0].name, "read");
    assert_eq!(results[0].output, "file text");
    assert!(tool.source_span.last_seq > tool.source_span.first_seq + 1);

    append(
        &fixture,
        "user.message",
        "rich-user",
        serde_json::json!({"text":"image", "images":[{"media_type":"image/png","data":"AA=="}]}),
    );
    assert!(page(&fixture).entries.iter().any(|entry| matches!(
        &entry.payload,
        TranscriptEntryPayload::LegacyHistory { source_kind, .. } if source_kind == "user.message"
    )));
    close(fixture).await;
}

#[tokio::test]
async fn transcript_refuses_unknown_corrupt_and_unreadable_sources() {
    let fixture = fixture("transcript-errors").await;
    append(&fixture, "tool.future", "future-1", serde_json::json!({}));
    assert!(fixture
        .session
        .transcript_page(TranscriptPageRequest::default())
        .unwrap_err()
        .to_string()
        .contains("unknown required fact kind"));
    let directory =
        crate::session_folder::effect_log_dir(fixture.root.path(), fixture.session.id());
    let file = directory.join(format!(
        "{}.jsonl",
        crate::fact_control::thread_for_session(fixture.session.id())
    ));
    std::fs::write(&file, "not-json\n").unwrap();
    assert!(fixture
        .session
        .transcript_page(TranscriptPageRequest::default())
        .is_err());
    std::fs::remove_dir_all(&directory).unwrap();
    *crate::error::write_or_recover(&fixture.session.history) = vec![Message::user("not empty")];
    let error = fixture
        .session
        .transcript_page(TranscriptPageRequest::default())
        .unwrap_err();
    assert!(error.to_string().contains("source is missing"));
    close(fixture).await;
}

#[tokio::test]
async fn prefix_fork_is_non_destructive_and_reopens_only_the_completed_prefix() {
    let fixture = two_turn_fixture("fork-source").await;
    let page = page(&fixture);
    let assistant = page
        .entries
        .iter()
        .find(|entry| entry.kind == TranscriptEntryKind::Assistant)
        .unwrap();
    let request = fork_request(&fixture, assistant.entry_id.clone(), "fork-child");
    let source_snapshot = SessionPersistenceContext::from_session(&fixture.session)
        .capture_snapshot()
        .await
        .unwrap();
    let source_facts = crate::fact_control::read_workspace_facts(
        fixture.root.path(),
        &crate::fact_control::thread_for_session(fixture.session.id()),
    )
    .unwrap();
    let selected = prepare_prefix_snapshot(&source_snapshot, &source_facts, &request).unwrap();
    assert_eq!(selected.boundary_cut.seq, 3);
    assert_eq!(
        selected.boundary_cut,
        TranscriptSourceCut::from(aster_effect::cut_log(&source_facts[..3]))
    );
    let before = crate::fact_control::read_workspace_facts(
        fixture.root.path(),
        &crate::fact_control::thread_for_session(fixture.session.id()),
    )
    .unwrap();
    let child = fixture
        .agent
        .fork_session_prefix_async(
            &fixture.session,
            request,
            options("fork-child", fixture.store.clone(), fixture.client.clone()),
        )
        .await
        .unwrap();
    assert_eq!(fixture.client.calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        crate::fact_control::read_workspace_facts(
            fixture.root.path(),
            &crate::fact_control::thread_for_session(fixture.session.id()),
        )
        .unwrap(),
        before
    );
    let child_page = child
        .transcript_page(TranscriptPageRequest::default())
        .unwrap();
    assert_eq!(child_page.entries.len(), 2);
    assert!(child_page.entries.iter().all(|entry| !matches!(
        &entry.payload,
        TranscriptEntryPayload::User { text } if text == "question-2"
    )));
    assert!(child.runs().await.is_empty());
    assert!(child.pending_confirmations().await.is_empty());
    assert!(child.plan_snapshot().await.unwrap().is_empty());
    let saved = fixture
        .store
        .load_snapshot("fork-child")
        .await
        .unwrap()
        .unwrap();
    assert!(saved.run_records.is_empty());
    assert!(saved.subagent_tasks.is_empty());
    child.close().await;
    drop(child);
    fixture.session.close().await;
    drop(fixture.session);

    let reopened = fixture
        .agent
        .resume_session_async(
            "fork-child",
            options("fork-child", fixture.store.clone(), fixture.client.clone()),
        )
        .await
        .unwrap();
    let reopened_page = reopened
        .transcript_page(TranscriptPageRequest::default())
        .unwrap();
    assert_eq!(reopened_page.entries.len(), 2);
    assert!(reopened.runs().await.is_empty());
    reopened.close().await;
    fixture.agent.close().await;
}

#[tokio::test]
async fn prefix_fork_rejects_the_closed_refusal_set_without_source_changes() {
    let fixture = two_turn_fixture("fork-refusals").await;
    let boundary = page(&fixture)
        .entries
        .into_iter()
        .find(|entry| entry.kind == TranscriptEntryKind::Assistant)
        .unwrap()
        .entry_id;
    let attempt = |request: PrefixForkRequest| {
        let fixture = &fixture;
        let boundary = boundary.clone();
        async move {
            fixture
                .agent
                .fork_session_prefix_async(
                    &fixture.session,
                    PrefixForkRequest {
                        boundary_entry_id: boundary,
                        ..request
                    },
                    options("fork-target", fixture.store.clone(), fixture.client.clone()),
                )
                .await
                .map(|_| ())
        }
    };

    let lease = fixture
        .session
        .run_admission
        .try_acquire(fixture.session.id())
        .unwrap();
    assert!(
        attempt(fork_request(&fixture, boundary.clone(), "fork-target"))
            .await
            .is_err()
    );
    drop(lease);

    fixture
        .session
        .subagent_tasks
        .replace_snapshots(vec![crate::subagent_task_tracker::SubagentTaskSnapshot {
            task_id: "active-child".into(),
            parent_session_id: fixture.session.id().into(),
            child_session_id: "child".into(),
            agent: "worker".into(),
            description: "active".into(),
            status: crate::subagent_task_tracker::SubagentStatus::Running,
            started_ms: 1,
            updated_ms: 1,
            finished_ms: None,
            output: None,
            success: None,
            source_anchors: vec![],
            progress: vec![],
        }])
        .await;
    assert!(
        attempt(fork_request(&fixture, boundary.clone(), "fork-target"))
            .await
            .is_err()
    );
    fixture
        .session
        .subagent_tasks
        .replace_snapshots(Vec::new())
        .await;

    let mut mismatch = fork_request(&fixture, boundary.clone(), "fork-target");
    mismatch.expected_workspace = tempfile::tempdir().unwrap().path().to_path_buf();
    assert!(attempt(mismatch).await.is_err());

    let mut stale = fork_request(&fixture, boundary.clone(), "fork-target");
    stale.expected_cut.digest = "stale".into();
    assert!(attempt(stale).await.is_err());

    let existing = fixture
        .agent
        .session_async(
            fixture.root.path().display().to_string(),
            Some(options(
                "fork-target",
                fixture.store.clone(),
                fixture.client.clone(),
            )),
        )
        .await
        .unwrap();
    assert!(
        attempt(fork_request(&fixture, boundary.clone(), "fork-target"))
            .await
            .is_err()
    );
    existing.close().await;
    drop(existing);

    append(
        &fixture,
        "model.turn",
        "pending-form",
        serde_json::json!({"kind":"question", "question_id":"q", "question":"pending", "allow_free_text":true, "options":[]}),
    );
    let before_pending = crate::fact_control::read_workspace_facts(
        fixture.root.path(),
        &crate::fact_control::thread_for_session(fixture.session.id()),
    )
    .unwrap();
    assert!(
        attempt(fork_request(&fixture, boundary.clone(), "fork-target"))
            .await
            .is_err()
    );
    assert_eq!(
        crate::fact_control::read_workspace_facts(
            fixture.root.path(),
            &crate::fact_control::thread_for_session(fixture.session.id()),
        )
        .unwrap(),
        before_pending
    );
    assert!(!fixture.store.exists("fork-target").await.unwrap());
    assert_eq!(fixture.client.calls.load(Ordering::SeqCst), 0);
    close(fixture).await;
}

#[tokio::test]
async fn prefix_fork_rejects_pending_confirmation_and_unsettled_effects() {
    for scenario in ["confirmation", "effects"] {
        let fixture = two_turn_fixture(&format!("fork-refuse-{scenario}")).await;
        let boundary = page(&fixture)
            .entries
            .into_iter()
            .find(|entry| entry.kind == TranscriptEntryKind::Assistant)
            .unwrap()
            .entry_id;
        match scenario {
            "confirmation" => append(
                &fixture,
                "model.turn",
                "pending-confirmation",
                serde_json::json!({"kind":"tool", "call":{
                    "id":"needs-approval", "name":"write", "args":{},
                    "needs_confirmation":true
                }, "also":[]}),
            ),
            _ => append(
                &fixture,
                "tool.result",
                "orphan-effect",
                serde_json::json!({"toolCallId":"unknown", "ok":false, "output":"unsettled", "unresolvedLegacy":true}),
            ),
        }
        let error = fixture
            .agent
            .fork_session_prefix_async(
                &fixture.session,
                fork_request(&fixture, boundary, "fork-target"),
                options("fork-target", fixture.store.clone(), fixture.client.clone()),
            )
            .await
            .unwrap_err();
        assert!(
            error.to_string().contains("session fork:"),
            "{scenario}: {error}"
        );
        assert!(!fixture.store.exists("fork-target").await.unwrap());
        close(fixture).await;
    }
}

#[tokio::test]
async fn prefix_fork_rejects_unverified_assistant_boundary() {
    let fixture = fixture("fork-unverified").await;
    append(
        &fixture,
        "user.message",
        "user",
        serde_json::json!({"text":"question"}),
    );
    append(
        &fixture,
        "model.turn",
        "model",
        serde_json::json!({"kind":"text", "text":"answer"}),
    );
    append(
        &fixture,
        "turn.complete",
        "unverified",
        serde_json::json!({"terminal":"narrative", "log_cut":{"seq":0,"digest":"wrong"}}),
    );
    let assistant = page(&fixture)
        .entries
        .into_iter()
        .find(|entry| entry.kind == TranscriptEntryKind::Assistant)
        .unwrap()
        .entry_id;
    let error = fixture
        .agent
        .fork_session_prefix_async(
            &fixture.session,
            fork_request(&fixture, assistant, "fork-child"),
            options("fork-child", fixture.store.clone(), fixture.client.clone()),
        )
        .await
        .unwrap_err();
    assert!(error.to_string().contains("not verified"));
    close(fixture).await;
}

struct CorruptStore;

#[async_trait::async_trait]
impl SessionStore for CorruptStore {
    async fn save(&self, _session: &crate::store::SessionData) -> anyhow::Result<()> {
        Ok(())
    }
    async fn load(&self, _id: &str) -> anyhow::Result<Option<crate::store::SessionData>> {
        Ok(None)
    }
    async fn delete(&self, _id: &str) -> anyhow::Result<()> {
        Ok(())
    }
    async fn list(&self) -> anyhow::Result<Vec<String>> {
        Ok(Vec::new())
    }
    async fn exists(&self, _id: &str) -> anyhow::Result<bool> {
        Ok(true)
    }
    async fn load_snapshot(
        &self,
        _id: &str,
    ) -> anyhow::Result<Option<crate::store::SessionSnapshotV1>> {
        anyhow::bail!("corrupt snapshot")
    }
}

#[tokio::test]
async fn strict_resume_refuses_missing_corrupt_and_foreign_workspace_without_replacement() {
    let fixture = fixture("strict-resume").await;
    fixture.session.save().await.unwrap();
    fixture.session.close().await;
    drop(fixture.session);

    let missing = fixture
        .agent
        .resume_session_async(
            "missing",
            SessionOptions::new().with_session_store(fixture.store.clone()),
        )
        .await;
    assert!(missing.is_err());
    let corrupt = fixture
        .agent
        .resume_session_async(
            "corrupt",
            SessionOptions::new().with_session_store(Arc::new(CorruptStore)),
        )
        .await;
    assert!(corrupt
        .unwrap_err()
        .to_string()
        .contains("corrupt snapshot"));
    let foreign = tempfile::tempdir().unwrap();
    let reopened = fixture
        .agent
        .open_protocol_session_async(
            foreign.path().display().to_string(),
            SessionOptions::new()
                .with_session_id("strict-resume")
                .with_session_store(fixture.store.clone())
                .with_llm_client(fixture.client.clone()),
            false,
        )
        .await;
    assert!(reopened
        .unwrap_err()
        .to_string()
        .contains("foreign workspace"));
    assert!(!fixture
        .agent
        .list_sessions()
        .await
        .contains(&"strict-resume".to_string()));
    assert!(!fixture.store.exists("missing").await.unwrap());
    assert!(!fixture.store.exists("corrupt").await.unwrap());
    fixture.agent.close().await;
}
