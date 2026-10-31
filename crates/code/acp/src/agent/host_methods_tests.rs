use super::*;
use aster_core::llm::{
    LlmClient, LlmResponse, Message, StreamEvent, TokenUsage, ToolDefinition,
};
use aster_core::store::{MemorySessionStore, TranscriptEntryKind};
use agent_client_protocol::{self as acp, Agent as _};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use tokio::sync::{broadcast, mpsc, Notify};

#[test]
fn every_host_method_parses_back_from_its_name() {
    for method in HostMethod::ALL {
        let wire = match method {
            HostMethod::GoalCommand => "aster/goal/state",
            HostMethod::Review => "aster/review/target",
            other => other.as_str(),
        };
        assert_eq!(HostMethod::parse(wire), Some(method), "{wire}");
    }
}

#[test]
fn protocol_and_unknown_methods_are_outside_the_host_set() {
    for wire in [
        "session/prompt",
        "aster/goal",
        "aster/goal/unknown",
        "aster/review",
        "aster/session/transcript/page",
        "aster/session/fork/prefix",
        "aster/session/tooling",
        "aster/session",
        "aster/session/infos",
        "aster/permission_posture/set",
        "_meta/unknown",
        "",
    ] {
        assert_eq!(HostMethod::parse(wire), None, "{wire}");
    }
    assert_eq!(
        HostMethod::parse("aster/compact_conversation/start"),
        Some(HostMethod::CompactConversation)
    );
    assert_eq!(
        HostMethod::parse("aster/interject/cancel"),
        Some(HostMethod::Interject)
    );
}

struct WireModel {
    calls: AtomicUsize,
    block_third: AtomicBool,
    started: Notify,
    release: broadcast::Sender<()>,
}

impl Default for WireModel {
    fn default() -> Self {
        Self {
            calls: AtomicUsize::new(0),
            block_third: AtomicBool::new(false),
            started: Notify::new(),
            release: broadcast::channel(1).0,
        }
    }
}

#[async_trait::async_trait]
impl LlmClient for WireModel {
    async fn complete(
        &self,
        _messages: &[Message],
        _system: Option<&str>,
        _tools: &[ToolDefinition],
    ) -> anyhow::Result<LlmResponse> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        if call >= 2 && self.block_third.load(Ordering::SeqCst) {
            self.started.notify_one();
            let _ = self.release.subscribe().recv().await;
        }
        Ok(LlmResponse {
            message: Message::assistant(format!("answer-{}", call + 1).as_str()),
            usage: TokenUsage::default(),
            stop_reason: None,
            token_logprobs: Vec::new(),
            meta: None,
        })
    }

    async fn complete_streaming(
        &self,
        _messages: &[Message],
        _system: Option<&str>,
        _tools: &[ToolDefinition],
        _cancel: tokio_util::sync::CancellationToken,
    ) -> anyhow::Result<mpsc::Receiver<StreamEvent>> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        let (sender, receiver) = mpsc::channel(2);
        let blocking = call >= 2 && self.block_third.load(Ordering::SeqCst);
        let mut release = self.release.subscribe();
        tokio::spawn(async move {
            if blocking {
                let _ = release.recv().await;
            }
            let _ = sender
                .send(StreamEvent::Done(LlmResponse {
                    message: Message::assistant(format!("answer-{}", call + 1).as_str()),
                    usage: TokenUsage::default(),
                    stop_reason: None,
                    token_logprobs: Vec::new(),
                    meta: None,
                }))
                .await;
        });
        if blocking {
            self.started.notify_one();
        }
        Ok(receiver)
    }
}

struct WireFixture {
    root: tempfile::TempDir,
    host: AsterCodeAgent,
    session_id: String,
    source: Arc<AgentSession>,
    model: Arc<WireModel>,
}

async fn wire_fixture() -> WireFixture {
    wire_fixture_with_policy(None).await
}

async fn wire_fixture_with_policy(work_policy: Option<aster_core::WorkPolicyV1>) -> WireFixture {
    let root = tempfile::tempdir().unwrap();
    let mut launch = test_launch(Some(root.path().join("sessions")));
    launch.workspace = root.path().to_path_buf();
    let host = AsterCodeAgent::new(launch.clone());
    let opened = host
        .new_session(
            acp::NewSessionRequest::new(root.path()).meta(
                json!({"asterEngineering":{"policy":"none"}})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await
        .unwrap();
    let session_id = opened.session_id.0.to_string();
    let core = Agent::from_config(launch.config).await.unwrap();
    let model = Arc::new(WireModel::default());
    let mut options = SessionOptions::new()
        .with_session_id(session_id.clone())
        .with_session_store(Arc::new(MemorySessionStore::new()))
        .with_llm_client(model.clone())
        .with_auto_save(true)
        .with_planning_mode(aster_core::PlanningMode::Disabled)
        .with_continuation(false);
    if let Some(policy) = work_policy {
        options = options.with_work_policy(policy);
    }
    let source = core
        .session_async(root.path().display().to_string(), Some(options))
        .await
        .unwrap();
    let source = Arc::new(source);
    let original = {
        let mut sessions = host.sessions.lock().await;
        std::mem::replace(
            &mut sessions.get_mut(&session_id).unwrap().session,
            Arc::clone(&source),
        )
    };
    original.close().await;
    *host.core_agent.lock().await = Some(core);
    source.send("question-1", None).await.unwrap();
    source.send("question-2", None).await.unwrap();
    source.save().await.unwrap();
    WireFixture {
        root,
        host,
        session_id,
        source,
        model,
    }
}

async fn ext(host: &AsterCodeAgent, method: &str, params: Value) -> acp::Result<Value> {
    let raw = serde_json::value::to_raw_value(&params).unwrap();
    let response = host
        .ext_method(acp::ExtRequest::new(method, raw.into()))
        .await?;
    Ok(serde_json::from_str(response.0.get()).unwrap())
}

fn page_params(fixture: &WireFixture, limit: usize, cursor: Value) -> Value {
    json!({
        "schema": TRANSCRIPT_PAGE_SCHEMA,
        "sessionId": fixture.session_id,
        "operationId": "page-operation",
        "cwd": fixture.root.path(),
        "limit": limit,
        "cursor": cursor,
    })
}

#[tokio::test]
async fn workspace_search_uses_current_session_sources_and_governed_permissions_without_a_model() {
    let fixture = wire_fixture().await;
    let foreign = tempfile::tempdir().unwrap();
    let source = fixture.root.path().join("evidence.rs");
    std::fs::write(&source, "pub fn workspace_evidence_marker() {}\n").unwrap();
    std::fs::write(
        fixture.root.path().join(".env"),
        "secret_qxv_marker=private\n",
    )
    .unwrap();
    let calls = fixture.model.calls.load(Ordering::SeqCst);
    let params = json!({
        "schema":WORKSPACE_SEARCH_SCHEMA,"sessionId":fixture.session_id,
        "operationId":"workspace-search","cwd":fixture.root.path(),
        "query":"workspace_evidence_marker",
    });
    assert!(ext(
        &fixture.host,
        HostMethod::WorkspaceSearch.as_str(),
        params.clone()
    )
    .await
    .is_err());
    fixture
        .source
        .set_permission_policy(aster_core::permissions::PermissionPolicy::new().allow("search"))
        .unwrap();
    let result = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            let result = ext(
                &fixture.host,
                HostMethod::WorkspaceSearch.as_str(),
                params.clone(),
            )
            .await
            .unwrap();
            if result["output"]
                .as_str()
                .unwrap()
                .contains("workspace_evidence_marker")
            {
                break result;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(result["schema"], WORKSPACE_SEARCH_SCHEMA);
    assert_eq!(result["sessionId"], fixture.session_id);
    assert_eq!(result["operationId"], "workspace-search");
    assert_eq!(result["cwd"], fixture.root.path().to_str().unwrap());
    assert!(result["metadata"]["results"]
        .as_array()
        .unwrap()
        .iter()
        .all(|hit| hit["digest_verified"] == true));
    std::fs::write(&source, "pub fn changed_evidence_marker() {}\n").unwrap();
    let changed = ext(
        &fixture.host,
        HostMethod::WorkspaceSearch.as_str(),
        params.clone(),
    )
    .await
    .unwrap();
    assert!(!changed["output"]
        .as_str()
        .unwrap()
        .contains("workspace_evidence_marker"));
    let mut secret_params = params.clone();
    secret_params["query"] = json!("secret_qxv_marker");
    let secret = ext(
        &fixture.host,
        HostMethod::WorkspaceSearch.as_str(),
        secret_params,
    )
    .await;
    assert!(!secret.unwrap()["output"]
        .as_str()
        .unwrap()
        .contains("secret_qxv_marker"));
    for (key, value) in [
        ("cwd", json!(foreign.path())),
        ("sessionId", json!("unknown-session")),
        ("schema", json!("unknown-schema")),
        ("query", json!("")),
        ("query", json!("invalid\u{0000}query")),
        ("query", json!(42)),
        ("query", json!("x".repeat(2049))),
        ("operationId", json!("")),
        ("path", json!("../outside")),
    ] {
        let mut invalid = params.clone();
        invalid[key] = value;
        assert!(
            ext(&fixture.host, HostMethod::WorkspaceSearch.as_str(), invalid)
                .await
                .is_err()
        );
    }
    fixture
        .source
        .set_permission_policy(aster_core::permissions::PermissionPolicy::new().deny("search"))
        .unwrap();
    assert!(ext(
        &fixture.host,
        HostMethod::WorkspaceSearch.as_str(),
        params.clone()
    )
    .await
    .is_err());
    assert_eq!(fixture.model.calls.load(Ordering::SeqCst), calls);
    assert_eq!(
        std::fs::read_to_string(&source).unwrap(),
        "pub fn changed_evidence_marker() {}\n"
    );
    assert_eq!(
        std::fs::read_to_string(fixture.root.path().join(".env")).unwrap(),
        "secret_qxv_marker=private\n"
    );
    fixture.source.close().await;
    assert!(
        ext(&fixture.host, HostMethod::WorkspaceSearch.as_str(), params)
            .await
            .is_err()
    );
}

fn fork_params(fixture: &WireFixture, target: &str, page: &Value, boundary: &str) -> Value {
    json!({
        "schema": PREFIX_FORK_SCHEMA,
        "sessionId": fixture.session_id,
        "operationId": format!("fork-{target}"),
        "request": {
            "boundary_entry_id": boundary,
            "expected_cut": page["page"]["source_cut"],
            "target_session_id": target,
            "expected_workspace": fixture.root.path(),
        }
    })
}

#[tokio::test]
async fn transcript_page_and_tooling_use_core_identity_and_actual_definitions() {
    let fixture = wire_fixture().await;
    let first = ext(
        &fixture.host,
        HostMethod::SessionTranscriptPage.as_str(),
        page_params(&fixture, 2, Value::Null),
    )
    .await
    .unwrap();
    assert_eq!(first["schema"], TRANSCRIPT_PAGE_SCHEMA);
    assert_eq!(first["operationId"], "page-operation");
    assert_eq!(first["page"]["entries"].as_array().unwrap().len(), 2);
    let cursor = first["page"]["next_cursor"].clone();
    assert!(cursor.is_object());
    let second = ext(
        &fixture.host,
        HostMethod::SessionTranscriptPage.as_str(),
        page_params(&fixture, 2, cursor.clone()),
    )
    .await
    .unwrap();
    let entries = second["page"]["entries"].as_array().unwrap();
    assert_eq!(entries[0]["kind"], "turn_complete");
    assert_eq!(entries[1]["kind"], "user");

    let tooling = ext(
        &fixture.host,
        HostMethod::SessionTooling.as_str(),
        json!({
            "schema": TOOLING_SCHEMA,
            "sessionId": fixture.session_id,
            "operationId": "tooling-operation",
            "cwd": fixture.root.path(),
        }),
    )
    .await
    .unwrap();
    assert_eq!(tooling["schema"], TOOLING_SCHEMA);
    assert_eq!(
        tooling["tools"],
        serde_json::to_value(fixture.source.tool_definitions()).unwrap()
    );
    assert!(tooling["tools"].as_array().unwrap().len() > 5);
    assert!(tooling["capabilityStamp"]["generation"].is_u64());
    assert!(tooling["capabilityStamp"]["digest"]
        .as_str()
        .is_some_and(|digest| digest.starts_with("sha256:")));
    for retired_duplicate in ["models", "modes", "commands"] {
        assert!(tooling.get(retired_duplicate).is_none());
    }

    fixture.source.send("question-3", None).await.unwrap();
    let error = ext(
        &fixture.host,
        HostMethod::SessionTranscriptPage.as_str(),
        page_params(&fixture, 2, cursor),
    )
    .await
    .unwrap_err();
    assert!(error
        .data
        .and_then(|data| data.as_str().map(str::to_string))
        .is_some_and(|data| data.contains("source changed since cursor")));
}

#[tokio::test]
async fn prefix_fork_preserves_source_and_returns_only_the_verified_child_prefix() {
    let policy = crate::engineering::named_policy("engineering-v1").unwrap();
    let fixture = wire_fixture_with_policy(Some(policy.clone())).await;
    let page = ext(
        &fixture.host,
        HostMethod::SessionTranscriptPage.as_str(),
        page_params(&fixture, 50, Value::Null),
    )
    .await
    .unwrap();
    let boundary = page["page"]["entries"][1]["entry_id"].as_str().unwrap();
    assert_eq!(page["page"]["entries"][1]["kind"], "assistant");
    let fork = ext(
        &fixture.host,
        HostMethod::SessionPrefixFork.as_str(),
        fork_params(&fixture, "wire-child", &page, boundary),
    )
    .await
    .unwrap();
    assert_eq!(fork["schema"], PREFIX_FORK_SCHEMA);
    assert_eq!(fork["childId"], "wire-child");
    assert_eq!(
        fork["lineage"]["sourceSessionId"],
        json!(fixture.session_id)
    );
    assert_eq!(fork["lineage"]["childSessionId"], "wire-child");
    assert_eq!(fork["lineage"]["boundaryEntryId"], boundary);
    assert_eq!(fork["sourceCut"], page["page"]["source_cut"]);
    let sessions = fixture.host.sessions.lock().await;
    let child = &sessions["wire-child"];
    assert!(child.host.work_policy.is_none());
    assert_eq!(child.session.work_policy().as_ref(), Some(&policy));
    drop(sessions);
    assert!(
        fork["prefixCut"]["seq"].as_u64().unwrap() < fork["sourceCut"]["seq"].as_u64().unwrap()
    );

    let source_after = fixture
        .source
        .transcript_page(aster_core::store::TranscriptPageRequest {
            cursor: None,
            limit: 50,
        })
        .unwrap();
    assert_eq!(
        serde_json::to_value(source_after.source_cut).unwrap(),
        page["page"]["source_cut"]
    );
    assert_eq!(source_after.entries.len(), 6);
    let child_page = ext(
        &fixture.host,
        HostMethod::SessionTranscriptPage.as_str(),
        json!({
            "schema": TRANSCRIPT_PAGE_SCHEMA,
            "sessionId": "wire-child",
            "operationId": "child-page",
            "cwd": fixture.root.path(),
            "limit": 50,
            "cursor": null,
        }),
    )
    .await
    .unwrap();
    let child_entries = child_page["page"]["entries"].as_array().unwrap();
    assert_eq!(child_entries.len(), 2);
    assert_eq!(child_entries[0]["payload"]["text"], "question-1");
    assert_eq!(child_entries[1]["kind"], "assistant");
    assert_eq!(child_entries[1]["payload"]["text"], "answer-1");
}

#[tokio::test]
async fn prefix_fork_child_load_replaces_the_registered_owner_and_preserves_history() {
    let fixture = wire_fixture().await;
    let page = ext(
        &fixture.host,
        HostMethod::SessionTranscriptPage.as_str(),
        page_params(&fixture, 50, Value::Null),
    )
    .await
    .unwrap();
    let boundary = page["page"]["entries"].as_array().unwrap().last().unwrap()["entry_id"]
        .as_str()
        .unwrap();
    ext(
        &fixture.host,
        HostMethod::SessionPrefixFork.as_str(),
        fork_params(&fixture, "reload-child", &page, boundary),
    )
    .await
    .unwrap();
    let (child, model, queue) = {
        let sessions = fixture.host.sessions.lock().await;
        let live = &sessions["reload-child"];
        (
            Arc::clone(&live.session),
            live.model_id.clone(),
            Arc::clone(&live.turn_queue),
        )
    };
    let before = child.transcript_page(Default::default()).unwrap();
    *fixture.host.model_id.lock().await = "missing/unrelated-selection".into();
    let permit = queue.acquire_owned().await.unwrap();
    let mut load = Box::pin(fixture.host.load_session(acp::LoadSessionRequest::new(
        "reload-child",
        fixture.root.path(),
    )));
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(50), &mut load)
            .await
            .is_err()
    );
    drop(permit);
    let loaded = tokio::time::timeout(std::time::Duration::from_secs(10), load)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(loaded.models.unwrap().current_model_id.0.as_ref(), model);
    let sessions = fixture.host.sessions.lock().await;
    let live = &sessions["reload-child"];
    assert!(!Arc::ptr_eq(&live.session, &child));
    assert!(Arc::ptr_eq(&live.turn_queue, &queue));
    assert_eq!(
        live.session.transcript_page(Default::default()).unwrap(),
        before
    );
    assert_eq!(
        fixture
            .source
            .transcript_page(Default::default())
            .unwrap()
            .source_cut,
        serde_json::from_value(page["page"]["source_cut"].clone()).unwrap()
    );
}

#[tokio::test]
async fn prefix_fork_leaves_active_source_admission_to_core_and_refuses() {
    let fixture = wire_fixture().await;
    let page = ext(
        &fixture.host,
        HostMethod::SessionTranscriptPage.as_str(),
        page_params(&fixture, 50, Value::Null),
    )
    .await
    .unwrap();
    let boundary = page["page"]["entries"][1]["entry_id"].as_str().unwrap();
    fixture.model.block_third.store(true, Ordering::SeqCst);
    let stream = fixture
        .source
        .stream_with_run("question-3", None)
        .await
        .unwrap();
    let mut events = stream.events;
    let running = stream.worker;
    tokio::time::timeout(
        std::time::Duration::from_secs(10),
        fixture.model.started.notified(),
    )
    .await
    .expect("the third Core run reached the blocking model");
    let attempted = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        ext(
            &fixture.host,
            HostMethod::SessionPrefixFork.as_str(),
            fork_params(&fixture, "active-child", &page, boundary),
        ),
    )
    .await;
    fixture.model.release.send(()).unwrap();
    let error = attempted
        .expect("prefix fork refused the active source before the deadline")
        .unwrap_err();
    assert!(error
        .data
        .and_then(|data| data.as_str().map(str::to_string))
        .is_some_and(|data| data.contains("source has unfinished execution")));
    assert!(!fixture
        .host
        .sessions
        .lock()
        .await
        .contains_key("active-child"));
    while events.recv().await.is_some() {}
    running.await.unwrap();
    assert_eq!(
        fixture
            .source
            .transcript_page(aster_core::store::TranscriptPageRequest {
                cursor: None,
                limit: 50,
            })
            .unwrap()
            .entries
            .last()
            .unwrap()
            .kind,
        TranscriptEntryKind::TurnComplete
    );
}

#[tokio::test]
async fn background_task_snapshot_and_stop_use_owner_lifecycle() {
    let fixture = wire_fixture().await;
    let tracker = fixture.source.subagent_tracker();
    for event in [
        AgentEvent::SubagentStart {
            task_id: "background-1".to_string(),
            session_id: "background-1".to_string(),
            parent_session_id: fixture.session_id.clone(),
            agent: "local_bash".to_string(),
            description: "Watch build".to_string(),
            started_ms: 100,
        },
        AgentEvent::SubagentProgress {
            task_id: "background-1".to_string(),
            session_id: "background-1".to_string(),
            status: "started".to_string(),
            metadata: json!({"command":"sleep 30","disposition":"in_progress"}),
        },
        AgentEvent::SubagentStart {
            task_id: "background-2".to_string(),
            session_id: "background-2".to_string(),
            parent_session_id: fixture.session_id.clone(),
            agent: "local_bash".to_string(),
            description: "Failing command".to_string(),
            started_ms: 200,
        },
        AgentEvent::SubagentProgress {
            task_id: "background-2".to_string(),
            session_id: "background-2".to_string(),
            status: "finished".to_string(),
            metadata: json!({"command":"exit 7","disposition":"failed","exitCode":7}),
        },
        AgentEvent::SubagentEnd {
            task_id: "background-2".to_string(),
            session_id: "background-2".to_string(),
            agent: "local_bash".to_string(),
            output: "exit status 7".to_string(),
            success: false,
            finished_ms: 400,
        },
    ] {
        tracker.record_event(&event).await;
    }
    let cancellation = tokio_util::sync::CancellationToken::new();
    tracker
        .register_canceller("background-1", cancellation.clone())
        .await;

    let snapshot = ext(
        &fixture.host,
        HostMethod::SessionBackgroundTasks.as_str(),
        json!({"schema":BACKGROUND_TASKS_SCHEMA,"sessionId":fixture.session_id}),
    )
    .await
    .unwrap();
    let shell_tasks: Vec<_> = snapshot["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|task| task["type"] == "local_bash")
        .collect();
    assert_eq!(shell_tasks.len(), 2);
    assert_eq!(snapshot["tasks"][0]["status"], "in_progress");
    assert_eq!(snapshot["tasks"][0]["command"], "sleep 30");
    assert_eq!(snapshot["tasks"][0]["startedAt"], 100);
    assert_eq!(snapshot["tasks"][1]["status"], "error");
    assert_eq!(snapshot["tasks"][1]["exitCode"], 7);
    assert_eq!(snapshot["tasks"][1]["completedAt"], 400);
    assert_eq!(snapshot["tasks"][1]["durationMs"], 200);

    let stopped = ext(
        &fixture.host,
        HostMethod::SessionBackgroundTaskStop.as_str(),
        json!({"schema":BACKGROUND_TASKS_SCHEMA,"sessionId":fixture.session_id,"taskId":"background-1"}),
    )
    .await
    .unwrap();
    assert_eq!(stopped, Value::Bool(true));
    assert!(cancellation.is_cancelled());
    let after = ext(
        &fixture.host,
        HostMethod::SessionBackgroundTasks.as_str(),
        json!({"schema":BACKGROUND_TASKS_SCHEMA,"sessionId":fixture.session_id}),
    )
    .await
    .unwrap();
    assert_eq!(after["tasks"][0]["status"], "stopped");

    let missing = ext(
        &fixture.host,
        HostMethod::SessionBackgroundTaskStop.as_str(),
        json!({"schema":BACKGROUND_TASKS_SCHEMA,"sessionId":fixture.session_id,"taskId":"missing"}),
    )
    .await
    .unwrap();
    assert_eq!(missing, Value::Bool(false));
    assert!(ext(
        &fixture.host,
        HostMethod::SessionBackgroundTaskStop.as_str(),
        json!({"schema":BACKGROUND_TASKS_SCHEMA,"sessionId":"other","taskId":"background-1"}),
    )
    .await
    .is_err());
}

#[test]
fn memory_extraction_snapshots_keep_terminal_outcomes_and_scope_private() {
    use aster_core::memory::{MemoryExtractionStatus as Status, MemoryExtractionTask};
    for (state, expected) in [
        (Status::Pending, "pending"),
        (Status::Running, "in_progress"),
        (Status::Completed, "completed"),
        (Status::Failed, "error"),
        (Status::Cancelled, "stopped"),
    ] {
        let value = memory_task_snapshot(MemoryExtractionTask {
            id: "original-ticket".into(),
            session_id: "private-core-session".into(),
            status: state,
            created_ms: 100,
            updated_ms: 300,
            completed_ms: Some(300),
            summary: Some("no_items".into()),
        });
        assert_eq!(value["id"], "original-ticket");
        assert_eq!(value["type"], "memory_extraction");
        assert_eq!(value["status"], expected);
        assert_eq!(value["durationMs"], 200);
        assert_eq!(value["summary"], "no_items");
        assert_eq!(value["error"].is_string(), expected == "error");
        assert!(!value.to_string().contains("private-core-session"));
    }
}

#[tokio::test]
async fn pager_subagent_cancel_is_session_scoped_and_reports_observed_outcomes() {
    let fixture = wire_fixture().await;
    let calls_before = fixture.model.calls.load(Ordering::SeqCst);
    let tracker = fixture.source.subagent_tracker();
    let token = tokio_util::sync::CancellationToken::new();
    tracker
        .record_event(&AgentEvent::SubagentStart {
            task_id: "child-stop".into(),
            session_id: "child-stop".into(),
            parent_session_id: fixture.session_id.clone(),
            agent: "general".into(),
            description: "Stop child".into(),
            started_ms: 1,
        })
        .await;
    tracker
        .register_canceller("child-stop", token.clone())
        .await;
    let method = HostMethod::SubagentCancel.as_str();
    assert!(ext(
        &fixture.host,
        method,
        json!({"sessionId":"foreign","subagentId":"child-stop"})
    )
    .await
    .is_err());
    assert!(!token.is_cancelled());
    let result = ext(
        &fixture.host,
        method,
        json!({"sessionId":fixture.session_id,"subagentId":"child-stop"}),
    )
    .await
    .unwrap();
    assert_eq!(result["result"]["outcome"]["kind"], "cancelled");
    assert_eq!(result["result"]["cancelled"], true);
    assert!(token.is_cancelled());
    let again = ext(
        &fixture.host,
        method,
        json!({"sessionId":fixture.session_id,"subagentId":"child-stop"}),
    )
    .await
    .unwrap();
    assert_eq!(
        again["result"]["outcome"],
        json!({"kind":"already_finished","status":"cancelled"})
    );
    let missing = ext(
        &fixture.host,
        method,
        json!({"sessionId":fixture.session_id,"subagentId":"unknown"}),
    )
    .await
    .unwrap();
    assert_eq!(missing["result"]["outcome"]["kind"], "not_found");
    tracker
        .record_event(&AgentEvent::SubagentStart {
            task_id: "orphan".into(),
            session_id: "orphan".into(),
            parent_session_id: fixture.session_id.clone(),
            agent: "general".into(),
            description: "Orphan".into(),
            started_ms: 2,
        })
        .await;
    assert!(ext(
        &fixture.host,
        method,
        json!({"sessionId":fixture.session_id,"subagentId":"orphan"})
    )
    .await
    .is_err());
    assert_eq!(fixture.model.calls.load(Ordering::SeqCst), calls_before);
}

#[tokio::test]
async fn text_only_model_refuses_images_before_a_model_or_run_is_admitted() {
    let mut fixture = wire_fixture().await;
    let calls_before = fixture.model.calls.load(Ordering::SeqCst);
    let id = fixture.host.sessions.lock().await[&fixture.session_id]
        .model_id
        .clone();
    let mut configured = false;
    for provider in &mut fixture.host.launch.config.providers {
        for model in &mut provider.models {
            if format!("{}/{}", provider.name, model.id) == id {
                model.modalities.input = vec!["text".into()];
                configured = true;
            }
        }
    }
    assert!(configured);
    let error = fixture
        .host
        .prompt(acp::PromptRequest::new(
            acp::SessionId::new(fixture.session_id.clone()),
            vec![acp::ContentBlock::Image(acp::ImageContent::new(
                "AQID",
                "image/png",
            ))],
        ))
        .await
        .unwrap_err();
    assert!(error
        .data
        .unwrap()
        .to_string()
        .contains("does not support image input"));
    assert_eq!(fixture.model.calls.load(Ordering::SeqCst), calls_before);
}
