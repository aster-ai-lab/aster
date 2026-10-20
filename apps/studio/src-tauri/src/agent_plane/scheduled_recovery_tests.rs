#![cfg(test)]

use super::*;

#[test]
fn scheduled_recovery_settles_interrupted_turn_and_run_after_restart() {
    for persisted in [false, true] {
        let dir = std::env::temp_dir().join(format!("a3s-scheduled-recovery-{}", new_id()));
        let root = persisted.then(|| dir.clone());
        let store = Arc::new(PlaneStore::open(root.clone()));
        let agents = Arc::new(AgentCatalog::new(Arc::clone(&store)));
        let sessions = Arc::new(SessionCatalog::new(Arc::clone(&store)));
        let agent = agents
            .create(&json!({"type":"a3s-code","name":"Helper","model":"openai::gpt"}))
            .unwrap();
        let session = sessions
            .create(&json!({"agentId":agent.id,"name":"Brief","workspace":{"type":"system"}}))
            .unwrap();
        let mut pending =
            super::tests::stored_line(&session.id, "assistant", "pending", "partial answer");
        let message_id = pending.id.clone();
        pending.runtime_resume_token = Some("old-runtime".into());
        store.save_message(pending).unwrap();
        store
            .save_runs(&[super::super::domain::TaskRun {
                id: new_id(),
                task_id: new_id(),
                session_id: Some(session.id.clone()),
                message_id: Some(message_id),
                started_at: now_iso(),
                finished_at: None,
                duration_ms: None,
                status: "running".into(),
                error: None,
                attempt: 1,
            }])
            .unwrap();
        let store = if persisted {
            Arc::new(PlaneStore::open(root.clone()))
        } else {
            store
        };
        let agents = Arc::new(AgentCatalog::new(Arc::clone(&store)));
        let sessions = Arc::new(SessionCatalog::new(Arc::clone(&store)));
        let hub = Arc::new(EventHub::new());
        let mut events = hub.subscribe();
        let config = Arc::new(PlaneConfig {
            root,
            credentials: Arc::new(|_| None),
            binary: None,
            workspace_documents: None,
            schedule_mcp: None,
            mail_mcp: None,
            slug: String::new(),
            personal_wiki: None,
            cloud_agent_runtime: None,
        });
        let turns = Arc::new(TurnService::new(
            Arc::clone(&store),
            Arc::clone(&agents),
            Arc::clone(&sessions),
            config,
            Arc::clone(&hub),
        ));
        let tasks = super::super::tasks::TaskService::new(
            Arc::clone(&store),
            agents,
            sessions,
            Arc::clone(&turns),
            hub,
        );
        tasks.tick();
        let rows = store.messages(&session.id);
        assert_eq!(
            rows[0].status, "error",
            "an interrupted scheduled turn must not stay pending"
        );
        assert!(rows[0].runtime_resume_token.is_none());
        assert_eq!(rows[0].parts()[0]["text"], "partial answer");
        assert_eq!(rows[0].parts().last().unwrap()["type"], "data-error");
        let runs = store.runs();
        assert_eq!(runs[0].status, "interrupted");
        assert!(runs[0].finished_at.is_some());
        assert!(
            runs[0]
                .error
                .as_deref()
                .is_some_and(|error| error.contains("执行已中断"))
        );
        assert_eq!(turns.attach(&session.id)["status"], "not-found");
        assert!(
            events.try_recv().is_ok(),
            "recovery must invalidate renderer data"
        );
        let settled = rows[0].clone();
        tasks.tick();
        assert_eq!(
            store.messages(&session.id)[0].updated_at,
            settled.updated_at
        );
        assert_eq!(
            store.messages(&session.id)[0].parts().len(),
            settled.parts().len()
        );
        if persisted {
            let reopened = PlaneStore::open(Some(dir.clone()));
            assert_eq!(reopened.messages(&session.id)[0].status, "error");
            assert_eq!(reopened.runs()[0].status, "interrupted");
            std::fs::remove_dir_all(&dir).unwrap();
        }
    }
}

#[test]
fn scheduled_recovery_preserves_live_and_other_session_turns() {
    let dir = std::env::temp_dir().join(format!("a3s-scheduled-live-{}", new_id()));
    let (turns, store, _) = super::tests::plane_fixture(&dir);
    store
        .save_message(super::tests::stored_line(
            "scheduled",
            "assistant",
            "pending",
            "",
        ))
        .unwrap();
    store
        .save_message(super::tests::stored_line(
            "other",
            "assistant",
            "pending",
            "",
        ))
        .unwrap();
    let (abort, _receiver) = watch::channel(false);
    let (steer_tx, _steer_rx) = mpsc::unbounded_channel();
    turns.lock_live().insert(
        "scheduled".into(),
        LiveTurn {
            acp_session: None,
            attempt_id: 1,
            execution_id: String::new(),
            owner: String::new(),
            status: "pending".into(),
            observed: None,
            buffered: Vec::new(),
            abort,
            approvals: HashMap::new(),
            steer_tx,
        },
    );
    turns.settle_orphan_pending("scheduled");
    assert_eq!(store.messages("scheduled")[0].status, "pending");
    assert_eq!(store.messages("other")[0].status, "pending");
    turns.lock_live().remove("scheduled");
    turns.settle_orphan_pending("scheduled");
    assert_eq!(store.messages("scheduled")[0].status, "error");
    assert_eq!(store.messages("other")[0].status, "pending");
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn scheduled_recovery_allows_manual_retry_without_waiting_for_ticker() {
    let dir = std::env::temp_dir().join(format!("a3s-scheduled-retry-{}", new_id()));
    let (turns, store, sessions) = super::tests::plane_fixture(&dir);
    let agent = turns
        .agents
        .create(&json!({"type":"a3s-code","name":"Retry","model":"openai::gpt"}))
        .unwrap();
    let tasks = super::super::tasks::TaskService::new(
        Arc::clone(&store),
        Arc::clone(&turns.agents),
        Arc::clone(&sessions),
        Arc::clone(&turns),
        Arc::clone(&turns.hub),
    );
    let task = tasks.create(&json!({"agentId":agent.id,"name":"Retry","prompt":"Retry fixture","trigger":{"kind":"once","at":chrono::Utc::now().timestamp_millis()+3600000},"workspace":{"type":"system"}})).unwrap();
    let session = sessions
        .create(&json!({"agentId":agent.id,"name":"Interrupted","workspace":{"type":"system"}}))
        .unwrap();
    let message = super::tests::stored_line(&session.id, "assistant", "pending", "");
    store.save_message(message.clone()).unwrap();
    store
        .save_runs(&[super::super::domain::TaskRun {
            id: new_id(),
            task_id: task["id"].as_str().unwrap().to_owned(),
            session_id: Some(session.id.clone()),
            message_id: Some(message.id),
            started_at: now_iso(),
            finished_at: None,
            duration_ms: None,
            status: "running".into(),
            error: None,
            attempt: 1,
        }])
        .unwrap();
    tasks
        .run(&agent.id, task["id"].as_str().unwrap())
        .expect("interrupted run must not block manual retry");
    assert_eq!(store.runs()[0].status, "interrupted");
    assert_eq!(store.messages(&session.id)[0].status, "error");
    assert_eq!(store.runs().len(), 2);
    let retry_session = store.runs()[1].session_id.clone().unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            tasks.tick();
            if store.runs()[1].status != "running" {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("failed credential preflight must settle");
    assert_eq!(store.runs()[1].status, "failed");
    assert_eq!(
        store.messages(&retry_session).last().unwrap().status,
        "error"
    );
    assert!(!turns.is_live(&retry_session));
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "uses the signed-in gateway and the real local ACP executable"]
async fn scheduled_recovery_live_scheduler_finishes_real_acp_turn() {
    assert_eq!(
        std::env::var("A3S_CLOUD_STUDIO_SCHEDULE_LIVE").as_deref(),
        Ok("1")
    );
    let home = PathBuf::from(std::env::var_os("HOME").unwrap());
    let app = home.join("Library/Application Support/dev.a3s.studio");
    let token = crate::member_session::MemberSessionStore::open_app(&app, "toyou")
        .token()
        .expect("signed-in member");
    let dir = std::env::temp_dir().join(format!("a3s-scheduled-live-acp-{}", new_id()));
    let root = dir.join("state");
    let store = Arc::new(PlaneStore::open(Some(root.clone())));
    let agents = Arc::new(AgentCatalog::new(Arc::clone(&store)));
    let sessions = Arc::new(SessionCatalog::new(Arc::clone(&store)));
    let hub = Arc::new(EventHub::new());
    let config = Arc::new(PlaneConfig {
        root: Some(root),
        credentials: Arc::new(move |provider| {
            crate::providers::signed_in_gateway_credentials(provider, Some(&token))
        }),
        binary: Some(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../target/debug/a3s-code-acp"),
        ),
        workspace_documents: None,
        schedule_mcp: None,
        mail_mcp: None,
        slug: String::new(),
        personal_wiki: None,
        cloud_agent_runtime: None,
    });
    let agent = agents.create(&json!({"type":"a3s-code","name":"Scheduler integration","model":"a3s::deepseek-v4-flash","configuration":{"permission_mode":"default"}})).unwrap();
    let turns = Arc::new(TurnService::new(
        Arc::clone(&store),
        Arc::clone(&agents),
        Arc::clone(&sessions),
        config,
        Arc::clone(&hub),
    ));
    let tasks = super::super::tasks::TaskService::new(
        Arc::clone(&store),
        agents,
        sessions,
        Arc::clone(&turns),
        hub,
    );
    let task = tasks.create(&json!({
        "agentId":agent.id,"name":"Scheduler integration","prompt":"只回复 SCHEDULE_OK。不要调用工具。",
        "trigger":{"kind":"once","at":chrono::Utc::now().timestamp_millis()-1000},
        "workspace":{"type":"system"},"timeoutMinutes":1
    })).unwrap();
    tasks.tick();
    let session_id = store.runs()[0].session_id.clone().unwrap();
    let completed = tokio::time::timeout(Duration::from_secs(90), async {
        loop {
            tasks.tick();
            if store.runs()[0].status != "running" {
                return;
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    })
    .await
    .is_ok();
    turns.abort(&session_id);
    turns.discard_runtime(&session_id).await;
    tokio::time::timeout(Duration::from_secs(5), async {
        while turns.is_live(&session_id) {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("owned turn cleanup");
    let run = store.runs()[0].clone();
    let reply = store
        .messages(&session_id)
        .into_iter()
        .find(|row| row.role == "assistant")
        .unwrap();
    let task_state = tasks.get(task["id"].as_str().unwrap()).unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
    assert!(
        completed,
        "real scheduled turn exceeded the bounded test deadline"
    );
    assert_eq!(run.status, "completed", "real scheduled turn must finish");
    assert_eq!(reply.status, "success");
    assert!(reply.searchable_text.contains("SCHEDULE_OK"));
    assert!(run.finished_at.is_some());
    assert_eq!(task_state["enabled"], false);
    assert_eq!(turns.attach(&session_id)["status"], "not-found");
    eprintln!(
        "SCHEDULE_LIVE model=deepseek-v4-flash status=completed reply=SCHEDULE_OK child=closed fixture=removed"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn interrupted_once_task_runs_again_without_a_manual_retry() {
    let dir = std::env::temp_dir().join(format!("a3s-scheduled-auto-retry-{}", new_id()));
    let (turns, store, sessions) = super::tests::plane_fixture(&dir);
    let agent = turns
        .agents
        .create(&json!({"type":"a3s-code","name":"Retry","model":"openai::gpt"}))
        .unwrap();
    let tasks = super::super::tasks::TaskService::new(
        Arc::clone(&store),
        Arc::clone(&turns.agents),
        Arc::clone(&sessions),
        Arc::clone(&turns),
        Arc::clone(&turns.hub),
    );
    let task = tasks.create(&json!({"agentId":agent.id,"name":"Brief","prompt":"Summarize","trigger":{"kind":"once","at":chrono::Utc::now().timestamp_millis()+3_600_000},"workspace":{"type":"system"}})).unwrap();
    let task_id = task["id"].as_str().unwrap().to_owned();
    let mut stored = store.tasks();
    stored[0].enabled = false;
    stored[0].fire_count = 1;
    stored[0].last_run = Some(now_iso());
    store.save_tasks(&stored).unwrap();
    let session = sessions
        .create(&json!({"agentId":agent.id,"name":"Interrupted","workspace":{"type":"system"}}))
        .unwrap();
    store
        .save_message(super::tests::stored_line(
            &session.id,
            "user",
            "success",
            "Summarize",
        ))
        .unwrap();
    let message = super::tests::stored_line(&session.id, "assistant", "pending", "");
    store.save_message(message.clone()).unwrap();
    store
        .save_runs(&[super::super::domain::TaskRun {
            id: new_id(),
            task_id: task_id.clone(),
            session_id: Some(session.id.clone()),
            message_id: Some(message.id),
            started_at: now_iso(),
            finished_at: None,
            duration_ms: None,
            status: "running".into(),
            error: None,
            attempt: 1,
        }])
        .unwrap();
    tasks.tick();
    let runs = store.runs();
    assert_eq!(runs.len(), 2, "an interrupted run starts one replacement");
    assert_eq!(runs[0].status, "interrupted");
    assert_eq!(runs[0].attempt, 1);
    assert_eq!(runs[1].session_id.as_deref(), Some(session.id.as_str()));
    let transcript = store.messages(&session.id);
    assert_eq!(transcript[0].role, "user");
    assert_eq!(transcript[1].role, "assistant");
    assert!(
        transcript[1]
            .searchable_text
            .contains(super::INTERRUPTED_RETRYING)
    );
    assert_eq!(transcript[2].role, "user");
    assert_eq!(transcript[2].searchable_text, "Summarize");
    assert!(
        runs[0]
            .error
            .as_deref()
            .is_some_and(|error| error.contains(super::INTERRUPTED_RETRYING))
    );
    let retry = runs[1].session_id.clone();
    if let Some(session_id) = &retry {
        turns.abort(session_id);
        turns.discard_runtime(session_id).await;
        let _ = tokio::time::timeout(Duration::from_secs(5), async {
            while turns.is_live(session_id) {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await;
    }
    tokio::time::sleep(Duration::from_millis(50)).await;
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_second_interruption_in_a_row_does_not_start_another_run() {
    let dir = std::env::temp_dir().join(format!("a3s-scheduled-retry-cap-{}", new_id()));
    let (turns, store, sessions) = super::tests::plane_fixture(&dir);
    let agent = turns
        .agents
        .create(&json!({"type":"a3s-code","name":"Retry","model":"openai::gpt"}))
        .unwrap();
    let tasks = super::super::tasks::TaskService::new(
        Arc::clone(&store),
        Arc::clone(&turns.agents),
        Arc::clone(&sessions),
        Arc::clone(&turns),
        Arc::clone(&turns.hub),
    );
    let task = tasks.create(&json!({"agentId":agent.id,"name":"Brief","prompt":"Summarize","trigger":{"kind":"once","at":chrono::Utc::now().timestamp_millis()+3_600_000},"workspace":{"type":"system"}})).unwrap();
    let task_id = task["id"].as_str().unwrap().to_owned();
    let session = sessions
        .create(&json!({"agentId":agent.id,"name":"Interrupted","workspace":{"type":"system"}}))
        .unwrap();
    let message = super::tests::stored_line(&session.id, "assistant", "pending", "");
    store.save_message(message.clone()).unwrap();
    store
        .save_runs(&[
            super::super::domain::TaskRun {
                id: new_id(),
                task_id: task_id.clone(),
                session_id: Some(session.id.clone()),
                message_id: None,
                started_at: now_iso(),
                finished_at: Some(now_iso()),
                duration_ms: Some(0),
                status: "interrupted".into(),
                error: Some(INTERRUPTED_RUN.to_owned()),
                attempt: 1,
            },
            super::super::domain::TaskRun {
                id: new_id(),
                task_id,
                session_id: Some(session.id.clone()),
                message_id: Some(message.id),
                started_at: now_iso(),
                finished_at: None,
                duration_ms: None,
                status: "running".into(),
                error: None,
                attempt: 1,
            },
        ])
        .unwrap();
    let next_run = store.tasks()[0].next_run.clone();
    tasks.tick();
    assert_eq!(store.runs().len(), 2);
    assert_eq!(store.runs()[1].status, "interrupted");
    assert_eq!(store.runs()[1].attempt, 2);
    assert!(store.tasks()[0].enabled);
    assert_eq!(store.tasks()[0].next_run, next_run);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn once_task_is_consumed_only_after_the_run_succeeds() {
    let dir = std::env::temp_dir().join(format!("a3s-scheduled-consume-{}", new_id()));
    let (turns, store, sessions) = super::tests::plane_fixture(&dir);
    let agent = turns
        .agents
        .create(&json!({"type":"a3s-code","name":"Once","model":"openai::gpt"}))
        .unwrap();
    let tasks = super::super::tasks::TaskService::new(
        Arc::clone(&store),
        Arc::clone(&turns.agents),
        Arc::clone(&sessions),
        Arc::clone(&turns),
        Arc::clone(&turns.hub),
    );
    let task = tasks.create(&json!({"agentId":agent.id,"name":"Once","prompt":"Go","trigger":{"kind":"once","at":chrono::Utc::now().timestamp_millis()+3_600_000},"workspace":{"type":"system"}})).unwrap();
    let task_id = task["id"].as_str().unwrap().to_owned();
    let session = sessions
        .create(&json!({"agentId":agent.id,"name":"Done","workspace":{"type":"system"}}))
        .unwrap();
    let message = super::tests::stored_line(&session.id, "assistant", "success", "done");
    store.save_message(message.clone()).unwrap();
    store
        .save_runs(&[super::super::domain::TaskRun {
            id: new_id(),
            task_id: task_id.clone(),
            session_id: Some(session.id),
            message_id: Some(message.id),
            started_at: now_iso(),
            finished_at: None,
            duration_ms: None,
            status: "running".into(),
            error: None,
            attempt: 1,
        }])
        .unwrap();
    tasks.tick();
    assert_eq!(store.runs().len(), 1);
    assert_eq!(store.runs()[0].status, "completed");
    assert!(!store.tasks()[0].enabled);
    assert!(store.tasks()[0].next_run.is_none());
    assert_eq!(store.tasks()[0].fire_count, 1);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_failed_once_run_is_not_started_again() {
    let dir = std::env::temp_dir().join(format!("a3s-scheduled-failed-once-{}", new_id()));
    let (turns, store, sessions) = super::tests::plane_fixture(&dir);
    let agent = turns
        .agents
        .create(&json!({"type":"a3s-code","name":"Once","model":"openai::gpt"}))
        .unwrap();
    let tasks = super::super::tasks::TaskService::new(
        Arc::clone(&store),
        Arc::clone(&turns.agents),
        Arc::clone(&sessions),
        Arc::clone(&turns),
        Arc::clone(&turns.hub),
    );
    let task = tasks.create(&json!({"agentId":agent.id,"name":"Once","prompt":"Go","trigger":{"kind":"once","at":chrono::Utc::now().timestamp_millis()-60_000},"workspace":{"type":"system"}})).unwrap();
    let task_id = task["id"].as_str().unwrap().to_owned();
    let session = sessions
        .create(&json!({"agentId":agent.id,"name":"Failed","workspace":{"type":"system"}}))
        .unwrap();
    let message = super::tests::stored_line(&session.id, "assistant", "error", "model down");
    store.save_message(message.clone()).unwrap();
    store
        .save_runs(&[super::super::domain::TaskRun {
            id: new_id(),
            task_id,
            session_id: Some(session.id),
            message_id: Some(message.id),
            started_at: now_iso(),
            finished_at: None,
            duration_ms: None,
            status: "running".into(),
            error: None,
            attempt: 1,
        }])
        .unwrap();
    tasks.tick();
    assert_eq!(store.runs().len(), 1);
    assert_eq!(store.runs()[0].status, "failed");
    assert!(store.tasks()[0].enabled);
    assert!(store.tasks()[0].next_run.is_none());
    tasks.tick();
    assert_eq!(store.runs().len(), 1);
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_stale_turn_records_the_interruption_without_waiting_for_the_ticker() {
    let dir = std::env::temp_dir().join(format!("a3s-scheduled-stale-{}", new_id()));
    let (turns, store, sessions) = super::tests::plane_fixture(&dir);
    let agent = turns
        .agents
        .create(&json!({"type":"a3s-code","name":"Once","model":"openai::gpt"}))
        .unwrap();
    let tasks = super::super::tasks::TaskService::new(
        Arc::clone(&store),
        Arc::clone(&turns.agents),
        Arc::clone(&sessions),
        Arc::clone(&turns),
        Arc::clone(&turns.hub),
    );
    let task = tasks.create(&json!({"agentId":agent.id,"name":"Once","prompt":"Go","trigger":{"kind":"once","at":chrono::Utc::now().timestamp_millis()+3_600_000},"workspace":{"type":"system"}})).unwrap();
    let task_id = task["id"].as_str().unwrap().to_owned();
    let session = sessions
        .create(&json!({"agentId":agent.id,"name":"Stale","workspace":{"type":"system"}}))
        .unwrap();
    let message = super::tests::stored_line(&session.id, "assistant", "pending", "");
    store.save_message(message.clone()).unwrap();
    store
        .save_runs(&[super::super::domain::TaskRun {
            id: new_id(),
            task_id,
            session_id: Some(session.id.clone()),
            message_id: Some(message.id.clone()),
            started_at: now_iso(),
            finished_at: None,
            duration_ms: None,
            status: "running".into(),
            error: None,
            attempt: 1,
        }])
        .unwrap();
    turns.record_process_interrupt(&session.id, &message.id);
    assert_eq!(store.messages(&session.id)[0].status, "error");
    assert!(
        store.messages(&session.id)[0]
            .searchable_text
            .contains(super::INTERRUPTED_RETRYING)
    );
    assert_eq!(store.runs()[0].status, "interrupted");
    assert_eq!(
        store.tasks()[0].resume_session_id.as_deref(),
        Some(session.id.as_str())
    );
    tasks.tick();
    assert_eq!(store.runs().len(), 2);
    assert_eq!(
        store.runs()[1].session_id.as_deref(),
        Some(session.id.as_str())
    );
    if let Some(session_id) = store.runs()[1].session_id.clone() {
        turns.abort(&session_id);
        turns.discard_runtime(&session_id).await;
    }
    let _ = std::fs::remove_dir_all(dir);
}
