#![cfg(test)]

use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{Value, json};

use super::catalog::SessionCatalog;
use super::session_query::SessionQuery;
use super::store::PlaneStore;
use super::tests::{TOKEN, request, start_in_memory};

#[test]
fn archiving_a_live_task_keeps_the_same_prompt_available_for_continuation() {
    use std::time::{Duration, Instant};

    let root = std::env::temp_dir().join(format!("a3s-archive-live-{}", uuid::Uuid::now_v7()));
    std::fs::create_dir_all(&root).unwrap();
    let binary = root.join("fake-acp");
    std::fs::write(&binary, super::fake_acp::SCRIPT).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let base = super::tests::start_with_binary(root.join("state"), binary);
    let call = |method, path: &str, body: Option<&Value>| {
        let text = body.map(Value::to_string);
        let (status, body) = request(&base, method, path, Some(TOKEN), text.as_deref());
        assert!(
            (200..300).contains(&status),
            "{method} {path}: {status} {body}"
        );
        serde_json::from_str::<Value>(&body).unwrap()
    };
    let agent = call(
        "POST",
        "/api/agents",
        Some(&json!({
            "type": "a3s-code", "name": "Archive continuity", "model": "openai::fixture"
        })),
    );
    let session = call(
        "POST",
        "/api/agent-sessions",
        Some(&json!({
            "agentId": agent["id"], "name": "归档不停止", "workspace": { "type": "system" }
        })),
    );
    let id = session["id"].as_str().unwrap();
    let turns = format!("/api/agent-sessions/{id}/turns");
    let started = call(
        "POST",
        &turns,
        Some(&json!({
            "userMessageParts": [{ "type": "text", "text": "hang" }]
        })),
    );
    assert_eq!(started["mode"], "started");
    let anchor = started["reservedMessages"][1]["id"].as_str().unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let trace = std::fs::read(root.join("turn-trace.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok());
        if trace
            .as_ref()
            .and_then(|trace| trace["prompts"].as_array())
            .is_some_and(|rows| !rows.is_empty())
        {
            break;
        }
        assert!(Instant::now() < deadline, "ACP prompt was not admitted");
        std::thread::sleep(Duration::from_millis(10));
    }
    call(
        "POST",
        "/api/agent-sessions/archive",
        Some(&json!({ "sessionIds": [id] })),
    );
    call("POST", &format!("/api/agent-sessions/{id}/restore"), None);
    assert_eq!(
        call(
            "POST",
            &turns,
            Some(&json!({
                "userMessageParts": [{ "type": "text", "text": "continue" }]
            }))
        )["mode"],
        "injected"
    );
    loop {
        let messages = call("GET", &format!("/api/agent-sessions/{id}/messages"), None);
        if messages["items"].as_array().unwrap().iter().any(|message| {
            message["role"] == "assistant"
                && message["status"] == "success"
                && message.to_string().contains("inserted continue")
        }) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "Original prompt did not complete after continuation"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let trace: Value =
        serde_json::from_slice(&std::fs::read(root.join("turn-trace.json")).unwrap()).unwrap();
    assert_eq!(trace["prompts"].as_array().unwrap().len(), 1);
    loop {
        let query = call(
            "GET",
            &format!("/api/agent-sessions?ids={id}&status=done"),
            None,
        );
        if query["items"][0]["execution"]["status"] == "done" {
            assert_eq!(query["items"][0]["execution"]["anchorMessageId"], anchor);
            break;
        }
        assert!(
            Instant::now() < deadline,
            "Execution terminal was not queryable"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn archived_tasks_are_listed_and_restored_through_the_real_http_routes() {
    let base = start_in_memory();
    let (status, body) = request(
        &base,
        "POST",
        "/api/agent-sessions",
        Some(TOKEN),
        Some(r#"{"agentId":"archive-agent","name":"归档报告","workspace":{"type":"system"}}"#),
    );
    assert_eq!(status, 201, "{body}");
    let created: Value = serde_json::from_str(&body).unwrap();
    let id = created["id"].as_str().unwrap();
    let archive = json!({ "sessionIds": [id] }).to_string();
    assert_eq!(
        request(
            &base,
            "POST",
            "/api/agent-sessions/archive",
            Some(TOKEN),
            Some(&archive)
        )
        .0,
        200
    );

    let (status, body) = request(
        &base,
        "GET",
        "/api/agent-sessions?inTrash=true",
        Some(TOKEN),
        None,
    );
    assert_eq!(status, 200, "{body}");
    let archived: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(archived["items"][0]["id"], id);
    assert!(archived["items"][0]["deletedAt"].is_string());

    let restore = format!("/api/agent-sessions/{id}/restore");
    assert_eq!(request(&base, "POST", &restore, None, Some("{}")).0, 403);
    let (status, body) = request(&base, "POST", &restore, Some(TOKEN), Some("{}"));
    assert_eq!(status, 200, "{body}");
    let restored: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(restored["id"], id);
    assert!(restored.get("deletedAt").is_none());
    let (_, body) = request(
        &base,
        "GET",
        "/api/agent-sessions?inTrash=false",
        Some(TOKEN),
        None,
    );
    let active: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(active["items"][0]["id"], id);
    let (_, body) = request(
        &base,
        "GET",
        "/api/agent-sessions?inTrash=true",
        Some(TOKEN),
        None,
    );
    assert!(
        serde_json::from_str::<Value>(&body).unwrap()["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        request(
            &base,
            "GET",
            "/api/agent-sessions?inTrash=invalid",
            Some(TOKEN),
            None
        )
        .0,
        400
    );
    assert_eq!(
        request(
            &base,
            "POST",
            "/api/agent-sessions/missing/restore",
            Some(TOKEN),
            Some("{}")
        )
        .0,
        404
    );
}

fn archive_restore_campaign(root: Option<PathBuf>) {
    let store = Arc::new(PlaneStore::open(root.clone()));
    let sessions = SessionCatalog::new(Arc::clone(&store));
    let session = sessions
        .create(&json!({
            "agentId": "archive-agent",
            "name": "保留交付文件",
            "workspace": { "type": "system" }
        }))
        .unwrap();
    let path = PathBuf::from(&session.workspace.path).join("report.md");
    std::fs::write(&path, "保留原始成果\n").unwrap();
    sessions
        .archive(&json!({ "sessionIds": [&session.id] }))
        .unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "保留原始成果\n");
    let archived = sessions
        .list(
            &SessionQuery {
                in_trash: true,
                ..SessionQuery::default()
            },
            None,
            20,
        )
        .unwrap();
    assert_eq!(archived["items"][0]["id"], session.id);

    let restored = sessions.restore(&session.id).unwrap();
    assert_eq!(restored.workspace.path, session.workspace.path);
    assert_eq!(
        sessions.restore(&session.id).unwrap().updated_at,
        restored.updated_at
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "保留原始成果\n");
    if let Some(root) = &root {
        let reopened = SessionCatalog::new(Arc::new(PlaneStore::open(Some(root.clone()))));
        assert!(reopened.get(&session.id).unwrap().deleted_at.is_none());
        assert!(
            reopened
                .list(
                    &SessionQuery {
                        in_trash: true,
                        ..SessionQuery::default()
                    },
                    None,
                    20
                )
                .unwrap()["items"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }
    sessions
        .destroy(&json!({ "sessionIds": [&session.id] }))
        .unwrap();
    assert!(sessions.restore(&session.id).is_err());
    assert!(!path.exists());
    if let Some(root) = root {
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn archive_restore_preserves_files_in_memory_and_after_disk_restart() {
    archive_restore_campaign(None);
    archive_restore_campaign(Some(
        std::env::temp_dir().join(format!("a3s-task-archive-{}", uuid::Uuid::now_v7())),
    ));
}

#[test]
#[ignore = "owned native HTTP fixture for the task archive browser E2E"]
fn serve_archive_e2e() {
    use std::io::Read;
    use std::net::{Ipv4Addr, TcpListener};

    let descriptor =
        PathBuf::from(std::env::var_os("A3S_CLOUD_TEST_ARCHIVE_RUNTIME").expect("runtime path"));
    let root = descriptor.parent().unwrap().join("state");
    let store = Arc::new(PlaneStore::open(Some(root.clone())));
    let agents = super::catalog::AgentCatalog::new(Arc::clone(&store));
    let agent = agents
        .create(&json!({
            "type": "a3s-code", "name": "Archive E2E", "model": "openai::fixture"
        }))
        .unwrap();
    let sessions = SessionCatalog::new(store);
    let archived = sessions
        .create(&json!({
            "agentId": agent.id, "name": "待恢复的报告", "workspace": { "type": "system" }
        }))
        .unwrap();
    let active = sessions
        .create(&json!({
            "agentId": agent.id, "name": "正在使用的报告", "workspace": { "type": "system" }
        }))
        .unwrap();
    sessions
        .archive(&json!({ "sessionIds": [archived.id] }))
        .unwrap();

    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    listener.set_nonblocking(true).unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let token = format!(
        "{}{}",
        uuid::Uuid::now_v7().simple(),
        uuid::Uuid::now_v7().simple()
    );
    let app = super::application(
        super::PlaneConfig {
            root: Some(root.clone()),
            credentials: Arc::new(|_| None),
            binary: None,
            workspace_documents: None,
            schedule_mcp: None,
            mail_mcp: None,
            slug: String::new(),
            personal_wiki: None,
            cloud_agent_runtime: None,
        },
        token.clone(),
    )
    .unwrap();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    std::thread::spawn(move || runtime.block_on(super::serve(app, listener)));
    std::fs::write(
        &descriptor,
        serde_json::to_vec(&json!({
            "baseUrl": base, "token": token, "homePath": root,
            "archivedId": archived.id, "activeId": active.id
        }))
        .unwrap(),
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&descriptor, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    let mut stop = [0u8; 1];
    let _ = std::io::stdin().read(&mut stop);
    std::fs::remove_file(descriptor).unwrap();
}
