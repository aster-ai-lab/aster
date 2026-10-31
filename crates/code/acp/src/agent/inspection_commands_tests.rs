use super::*;
use agent_client_protocol::Agent as _;

#[test]
fn commands_parse_mechanical_syntax_and_refuse_invalid_mutations() {
    for text in ["/memory-extra", "explain /memory", "/tasks-extra"] {
        assert!(parse(text).is_none());
    }
    for text in [
        "/memory delete id",
        "/memory delete id stale",
        "/memory read",
        "/memory unknown",
        "/tasks stop",
        "/tasks delete id",
    ] {
        assert!(parse(text).unwrap().is_err(), "{text}");
    }
    let InspectionCommand::Memory(MemoryManagementRequest::Read { id }) =
        parse(" /memory\tread id with spaces ").unwrap().unwrap()
    else {
        panic!("read command");
    };
    assert_eq!(id, "id with spaces");
    let cursor = r#"{"generation":1,"filter":"opaque","offset":50}"#;
    let InspectionCommand::Memory(MemoryManagementRequest::List {
        cursor: next,
        query,
        ..
    }) = parse(&format!("/memory next {cursor} Rust workflow"))
        .unwrap()
        .unwrap()
    else {
        panic!("next command");
    };
    assert_eq!(next.as_deref(), Some(cursor));
    assert_eq!(query, "Rust workflow");
}

#[test]
fn a_filtered_next_page_command_keeps_the_real_cursor_and_filter() {
    use aster_core::durable_memory_vec::{MemoryRecord, VecMemoryStore};
    let root = tempfile::tempdir().unwrap();
    let store = VecMemoryStore::open(root.path()).unwrap();
    for index in 0..51 {
        store
            .put(&MemoryRecord {
                id: format!("note-{index:02}"),
                content: format!("Rust workspace conventions, note {index}"),
                tags: vec![],
                importance: 0.5,
                memory_type: "\"procedural\"".into(),
                timestamp: "2026-10-24T00:00:00Z".into(),
                meta: "{}".into(),
            })
            .unwrap();
    }
    let page = store
        .manage(parse_memory("list Rust workspace").unwrap())
        .unwrap();
    let output = format_memory(page, "Rust workspace").unwrap();
    let next = output
        .lines()
        .find_map(|line| line.strip_prefix("Next page: "))
        .unwrap();
    let InspectionCommand::Memory(request) = parse(next).unwrap().unwrap() else {
        panic!("memory page");
    };
    let MemoryManagementResponse::List {
        items,
        total,
        next_cursor,
    } = store.manage(request).unwrap()
    else {
        panic!("list response");
    };
    assert_eq!(total, 51);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].id, "note-50");
    assert!(next_cursor.is_none());
}

#[tokio::test]
async fn terminal_memory_commands_use_revision_guards_and_do_not_call_the_model() {
    let root = tempfile::tempdir().unwrap();
    let workspace = root.path().canonicalize().unwrap();
    let mut launch = test_launch(Some(root.path().join("sessions")));
    launch.workspace = workspace.clone();
    for provider in &mut launch.config.providers {
        provider.base_url = Some("http://127.0.0.1:9/v1".into());
    }
    launch.config.memory_dir = Some(root.path().join("memory"));
    let host = AsterCodeAgent::new(launch);
    let id = host
        .new_session(acp::NewSessionRequest::new(&workspace))
        .await
        .unwrap()
        .session_id;
    let session = host.sessions.lock().await[id.0.as_ref()].session.clone();
    let memory = session.memory().unwrap();
    let record = memory
        .remember_success_item("Use the pinned Rust toolchain", &[], "fixture")
        .await
        .unwrap();
    let run = |text: &str| parse(text).unwrap();
    let listed = host
        .handle_inspection_command(&session, run("/memory list Rust"))
        .await
        .unwrap();
    assert!(listed.contains("Stored memories: 1"), "{listed}");
    assert!(listed.contains(&record.id), "{listed}");
    let read = host
        .handle_inspection_command(&session, run(&format!("/memory read {}", record.id)))
        .await
        .unwrap();
    let revision = read
        .lines()
        .find_map(|line| line.strip_prefix("Revision: "))
        .unwrap();
    let stale = host
        .handle_inspection_command(
            &session,
            run(&format!("/memory delete {} {}", record.id, "0".repeat(64))),
        )
        .await
        .unwrap_err();
    assert!(stale.data.unwrap().as_str().unwrap().contains("conflict"));
    assert!(memory.store().retrieve(&record.id).await.unwrap().is_some());
    host.handle_inspection_command(
        &session,
        run(&format!("/memory delete {} {revision}", record.id)),
    )
    .await
    .unwrap();
    assert!(memory.store().retrieve(&record.id).await.unwrap().is_none());
    // The fixture has no reachable model endpoint: a dispatch fallthrough fails here.
    for text in ["/memory", "/tasks"] {
        let response = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            host.prompt(acp::PromptRequest::new(
                id.clone(),
                vec![acp::ContentBlock::Text(acp::TextContent::new(text))],
            )),
        )
        .await
        .expect("inspection must not wait for a model")
        .unwrap();
        assert_eq!(response.stop_reason, acp::StopReason::EndTurn);
    }
    session.close().await;
}

#[tokio::test]
async fn terminal_tasks_stop_only_the_session_owned_canceller() {
    let root = tempfile::tempdir().unwrap();
    let mut launch = test_launch(None);
    launch.workspace = root.path().into();
    let host = AsterCodeAgent::new(launch);
    let id = host
        .new_session(acp::NewSessionRequest::new(root.path()))
        .await
        .unwrap()
        .session_id;
    let session = host.sessions.lock().await[id.0.as_ref()].session.clone();
    let tracker = session.subagent_tracker();
    tracker
        .record_event(&AgentEvent::SubagentStart {
            task_id: "owned-task".into(),
            session_id: "owned-child".into(),
            parent_session_id: session.session_id().into(),
            agent: "test-worker".into(),
            description: "Fixture task".into(),
            started_ms: 100,
        })
        .await;
    let token = tokio_util::sync::CancellationToken::new();
    tracker
        .register_canceller("owned-task", token.clone())
        .await;
    let text = host
        .handle_inspection_command(&session, parse("/tasks").unwrap())
        .await
        .unwrap();
    assert!(
        text.contains("owned-task") && text.contains("in_progress"),
        "{text}"
    );
    assert!(host
        .handle_inspection_command(&session, parse("/tasks stop foreign-task").unwrap())
        .await
        .is_err());
    assert!(!token.is_cancelled());
    assert_eq!(
        host.handle_inspection_command(&session, parse("/tasks stop owned-task").unwrap())
            .await
            .unwrap(),
        "Task stopped."
    );
    assert!(token.is_cancelled());
    session.close().await;
}

#[tokio::test]
async fn memory_commands_follow_the_session_workspace_without_widening_unbound_access() {
    let root = tempfile::tempdir().unwrap();
    let a = root.path().join("project-a");
    let b = root.path().join("project-b");
    std::fs::create_dir(&a).unwrap();
    std::fs::create_dir(&b).unwrap();
    let a = a.canonicalize().unwrap();
    let b = b.canonicalize().unwrap();
    let mut launch = test_launch(Some(root.path().join("sessions")));
    launch.workspace = a.clone();
    assert!(launch.config.memory_dir.is_none());
    let host = AsterCodeAgent::new(launch);
    let mut live = Vec::new();
    for (workspace, note) in [(&a, "scope-alpha"), (&a, "scope-gamma"), (&b, "scope-beta")] {
        let id = host
            .new_session(acp::NewSessionRequest::new(workspace))
            .await
            .unwrap()
            .session_id;
        let session = host.sessions.lock().await[id.0.as_ref()].session.clone();
        session
            .memory()
            .unwrap()
            .remember_success_item(note, &[], "fixture")
            .await
            .unwrap();
        live.push((id, session));
    }
    let listed = host
        .handle_inspection_command(&live[2].1, parse("/memory").unwrap())
        .await
        .unwrap();
    assert!(listed.contains("scope-beta"), "{listed}");
    assert!(
        !listed.contains("scope-alpha") && !listed.contains("scope-gamma"),
        "{listed}"
    );
    let body = |method, value: serde_json::Value| {
        acp::ExtRequest::new(
            method,
            serde_json::value::to_raw_value(&value).unwrap().into(),
        )
    };
    let query = serde_json::json!({"operation":"list","query":"","memory_type":null,"cursor":null});
    for session in [None, Some(live[0].0 .0.to_string()), Some("unknown".into())] {
        let request = body(
            "aster/memory/manage",
            serde_json::json!({"workspace_root":b,"session_id":session,"request":query}),
        );
        assert!(host.handle_memory_management(&request).await.is_err());
    }
    for (session, count) in [(None, 2), (Some(live[0].0 .0.to_string()), 1)] {
        let request = body(
            "aster/memory/snapshot",
            serde_json::json!({"workspace_root":a,"session_id":session,"layer":"session"}),
        );
        let reply = host.handle_memory_snapshot(&request).await.unwrap();
        let value: serde_json::Value = serde_json::from_str(reply.0.get()).unwrap();
        assert_eq!(value["items"].as_array().unwrap().len(), count);
        if session.is_some() {
            assert!(value["items"][0]["content"]
                .as_str()
                .unwrap()
                .contains("scope-alpha"));
        }
    }
    let count = body(
        "aster/memory/snapshot",
        serde_json::json!({"workspace_root":b,"session_id":live[2].0.0,"layer":"durable","count_only":true}),
    );
    let reply = host.handle_memory_snapshot(&count).await.unwrap();
    let value: serde_json::Value = serde_json::from_str(reply.0.get()).unwrap();
    assert_eq!(value["count"], 1);
    // Stop memory maintenance before replacing the workspace directory.
    for (_, session) in &live {
        session.close().await;
    }
    #[cfg(unix)]
    {
        let original = root.path().join("project-b-original");
        std::fs::rename(&b, &original).unwrap();
        std::os::unix::fs::symlink(&a, &b).unwrap();
        let request = body(
            "aster/memory/manage",
            serde_json::json!({"workspace_root":a,"session_id":live[2].0.0,"request":query}),
        );
        assert!(
            host.handle_memory_management(&request).await.is_err(),
            "retargeting a directory must not rebind a session's memory scope"
        );
        std::fs::remove_file(&b).unwrap();
        std::fs::rename(&original, &b).unwrap();
    }
}
