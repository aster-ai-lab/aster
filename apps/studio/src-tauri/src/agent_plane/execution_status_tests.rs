#![cfg(test)]

use super::*;

#[test]
fn orphan_recovery_fails_the_original_execution_after_restart() {
    for status in [
        ExecutionStatus::Pending,
        ExecutionStatus::Streaming,
        ExecutionStatus::AwaitingApproval,
        ExecutionStatus::AwaitingForm,
    ] {
        for already_interrupted in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let (old_turns, store, sessions) = super::tests::plane_fixture(root.path());
            let session = sessions
                .create(&json!({"name":"Interrupted","workspace":{"type":"system"}}))
                .unwrap();
            let mut message = super::tests::stored_line(
                &session.id,
                "assistant",
                if already_interrupted {
                    "error"
                } else {
                    "pending"
                },
                "partial answer",
            );
            message.runtime_resume_token = Some("retired-runtime".into());
            let anchor = message.id.clone();
            store.save_message(message).unwrap();
            store
                .save_session_execution(
                    &session.id,
                    SessionExecution {
                        status,
                        execution_id: "original-execution".into(),
                        anchor_message_id: anchor.clone(),
                        observed_at: "2026-10-05T00:00:00.000Z".into(),
                    },
                    true,
                )
                .unwrap();
            let reopened = Arc::new(PlaneStore::open(Some(root.path().join("state"))));
            let turns = TurnService::new(
                Arc::clone(&reopened),
                Arc::new(AgentCatalog::new(Arc::clone(&reopened))),
                Arc::new(SessionCatalog::new(Arc::clone(&reopened))),
                Arc::clone(&old_turns.config),
                Arc::new(EventHub::new()),
            );
            turns.settle_orphan_pending(&session.id);
            let summary = reopened.session_executions()[&session.id].clone();
            assert_eq!(summary.status, ExecutionStatus::Error);
            assert_eq!(summary.execution_id, "original-execution");
            assert_eq!(summary.anchor_message_id, anchor);
            assert_ne!(summary.observed_at, "2026-10-05T00:00:00.000Z");
            let settled = reopened.messages(&session.id);
            assert_eq!(settled[0].status, "error");
            assert_eq!(settled[0].parts()[0]["text"], "partial answer");
            turns.settle_orphan_pending(&session.id);
            assert_eq!(
                reopened.session_executions()[&session.id].observed_at,
                summary.observed_at,
                "repeated recovery must preserve the original failure observation"
            );
            let disk = PlaneStore::open(Some(root.path().join("state")));
            assert_eq!(
                disk.session_executions()[&session.id].status,
                ExecutionStatus::Error
            );
        }
    }
}

#[test]
fn orphan_recovery_preserves_live_and_terminal_execution_ownership() {
    let root = tempfile::tempdir().unwrap();
    let (turns, store, sessions) = super::tests::plane_fixture(root.path());
    let mut ids = Vec::new();
    for (name, status) in [
        ("live", ExecutionStatus::AwaitingApproval),
        ("terminal", ExecutionStatus::Done),
        ("other", ExecutionStatus::Streaming),
    ] {
        let session = sessions
            .create(&json!({"name":name,"workspace":{"type":"system"}}))
            .unwrap();
        let message = super::tests::stored_line(
            &session.id,
            "assistant",
            if status.is_live() {
                "pending"
            } else {
                "success"
            },
            "retained answer",
        );
        store.save_message(message.clone()).unwrap();
        store
            .save_session_execution(
                &session.id,
                SessionExecution {
                    status,
                    execution_id: format!("{name}-execution"),
                    anchor_message_id: message.id.clone(),
                    observed_at: "2026-10-05T00:00:00.000Z".into(),
                },
                true,
            )
            .unwrap();
        ids.push((session.id, message.id));
    }
    let (abort, _) = watch::channel(false);
    let (steer_tx, _) = mpsc::unbounded_channel();
    turns.lock_live().insert(
        ids[0].0.clone(),
        LiveTurn {
            acp_session: None,
            attempt_id: 1,
            owner: ids[0].1.clone(),
            execution_id: "live-execution".into(),
            status: "awaiting-approval".into(),
            observed: None,
            buffered: vec![],
            abort,
            approvals: HashMap::new(),
            steer_tx,
        },
    );
    turns.settle_orphan_pending(&ids[0].0);
    turns.settle_orphan_pending(&ids[1].0);
    let summaries = store.session_executions();
    assert_eq!(
        summaries[&ids[0].0].status,
        ExecutionStatus::AwaitingApproval
    );
    assert_eq!(store.messages(&ids[0].0)[0].status, "pending");
    assert_eq!(summaries[&ids[1].0].status, ExecutionStatus::Done);
    assert_eq!(store.messages(&ids[1].0)[0].status, "success");
    assert_eq!(summaries[&ids[2].0].status, ExecutionStatus::Streaming);
    assert_eq!(store.messages(&ids[2].0)[0].status, "pending");
}

#[test]
fn orphan_execution_write_failure_remains_unknown_and_can_retry() {
    let root = tempfile::tempdir().unwrap();
    let (turns, store, sessions) = super::tests::plane_fixture(root.path());
    let session = sessions
        .create(&json!({"name":"Interrupted","workspace":{"type":"system"}}))
        .unwrap();
    let message = super::tests::stored_line(&session.id, "assistant", "pending", "partial");
    store.save_message(message.clone()).unwrap();
    store
        .save_session_execution(
            &session.id,
            SessionExecution {
                status: ExecutionStatus::AwaitingApproval,
                execution_id: "retired-execution".into(),
                anchor_message_id: message.id,
                observed_at: "2026-10-05T00:00:00.000Z".into(),
            },
            true,
        )
        .unwrap();
    let file = root.path().join("state/session-executions.json");
    let backup = root.path().join("state/execution-backup");
    std::fs::rename(&file, &backup).unwrap();
    std::fs::create_dir(&file).unwrap();
    turns.settle_orphan_pending(&session.id);
    assert_eq!(store.messages(&session.id)[0].status, "error");
    assert!(
        !store.session_executions().contains_key(&session.id),
        "an uncommitted terminal is not authoritative"
    );
    std::fs::remove_dir(&file).unwrap();
    std::fs::rename(&backup, &file).unwrap();
    turns.settle_orphan_pending(&session.id);
    assert_eq!(
        store.session_executions()[&session.id].status,
        ExecutionStatus::Error
    );
    assert_eq!(
        PlaneStore::open(Some(root.path().join("state"))).session_executions()[&session.id].status,
        ExecutionStatus::Error
    );
}

// Catch a retired assistant anchor revoking a newer Run's UI observation.
#[tokio::test]
async fn status_publication_requires_the_current_live_owner_and_withdraws_at_settlement() {
    let root = std::env::temp_dir().join(format!("a3s-status-owner-{}", new_id()));
    let (turns, _, _) = super::tests::plane_fixture(&root);
    let mut observer = super::super::execution_observation::Observer::new("acp", "prompt");
    let observation = observer
        .accept(
            &json!({"protocolVersion":1,"sessionId":"acp","promptId":"prompt",
            "coreSessionId":"core","runId":"run","sequence":1,"kind":"run-opened",
            "turn":null,"status":null}),
        )
        .expect("native observation")
        .expect("Run open");
    let (abort, _) = watch::channel(false);
    let (steer_tx, _) = mpsc::unbounded_channel();
    turns.lock_live().insert(
        "session".into(),
        LiveTurn {
            acp_session: None,
            attempt_id: 1,
            execution_id: "execution".into(),
            owner: "current".into(),
            status: "pending".into(),
            observed: Some(observation),
            buffered: vec![],
            abort,
            approvals: HashMap::new(),
            steer_tx,
        },
    );
    let current = Execution {
        session_id: "session".into(),
        topic_id: "agent-session:session".into(),
        execution_id: "execution".into(),
        attempt_id: 1,
        anchor_id: "current".into(),
    };
    let mut events = turns.hub.subscribe();
    turns.status(&current, "streaming", false);
    assert_eq!(
        events.recv().await.unwrap()["payload"]["status"]["nativeExecution"]["runId"],
        "run"
    );
    let mut retired = current.clone();
    retired.anchor_id = "retired".into();
    turns.status(&retired, "done", false);
    assert!(matches!(
        events.try_recv(),
        Err(broadcast::error::TryRecvError::Empty)
    ));
    let mut reconnected = super::super::events::stream(turns.hub.clone());
    use futures_util::StreamExt;
    let snapshot = String::from_utf8(reconnected.next().await.unwrap().unwrap().encode()).unwrap();
    assert!(snapshot.contains("\"runId\":\"run\""));
    turns.lock_live().remove("session");
    turns.status(&current, "done", false);
    let settled = events.recv().await.unwrap();
    assert!(
        settled["payload"]["status"]
            .get("nativeExecution")
            .is_none()
    );
    let mut reconnected = super::super::events::stream(turns.hub.clone());
    let snapshot = String::from_utf8(reconnected.next().await.unwrap().unwrap().encode()).unwrap();
    assert!(snapshot.contains("\"topics\":[]"));
    if root.exists() {
        std::fs::remove_dir_all(root).unwrap();
    }
}
