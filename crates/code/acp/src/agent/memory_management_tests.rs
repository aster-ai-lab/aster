use super::*;
use aster_core::durable_memory_vec::management::MemoryManagementResponse;
use std::path::Path;

#[tokio::test]
async fn live_snapshot_reads_actual_working_and_session_memory_and_refuses_foreign_scope() {
    use acp::Agent as _;
    let root = tempfile::tempdir().unwrap();
    let workspace = root.path().canonicalize().unwrap();
    let mut launch = test_launch(Some(root.path().join("sessions")));
    launch.workspace = workspace.clone();
    launch.config.memory_dir = Some(root.path().join("memory"));
    let host = AsterCodeAgent::new(launch);
    let id = host
        .new_session(acp::NewSessionRequest::new(&workspace))
        .await
        .unwrap()
        .session_id;
    let session = host.sessions.lock().await[&id.0.to_string()]
        .session
        .clone();
    let memory = session.memory().unwrap();
    let persistent = memory
        .remember_success_item("session history memory", &[], "fixture")
        .await
        .unwrap();
    let mut working = persistent.clone();
    working.id = "working-context".into();
    working.content = "active working context".into();
    memory.add_to_working(working).await.unwrap();
    let request = |workspace: &Path, layer| {
        acp::ExtRequest::new(
            "aster/memory/snapshot",
            serde_json::value::to_raw_value(
                &serde_json::json!({"workspace_root": workspace, "layer": layer}),
            )
            .unwrap()
            .into(),
        )
    };
    for (layer, expected) in [
        ("working", "active working context"),
        ("session", persistent.content.as_str()),
    ] {
        let response = host
            .handle_memory_snapshot(&request(&workspace, layer))
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_str(response.0.get()).unwrap();
        assert_eq!(body["layer"], layer);
        assert_eq!(body["items"].as_array().unwrap().len(), 1);
        assert_eq!(body["items"][0]["content"], expected);
        let count_request = acp::ExtRequest::new(
            "aster/memory/snapshot",
            serde_json::value::to_raw_value(
                &serde_json::json!({"workspace_root":workspace,"layer":layer,"count_only":true}),
            )
            .unwrap()
            .into(),
        );
        let count_response = host.handle_memory_snapshot(&count_request).await.unwrap();
        let count: serde_json::Value = serde_json::from_str(count_response.0.get()).unwrap();
        assert_eq!(count["count"], 1);
        assert!(
            count.get("items").is_none(),
            "hover counts must not copy memory bodies"
        );
        assert_eq!(count["workspace_root"], serde_json::json!(workspace));
        assert_eq!(
            body["items"][0]["metadata"]["aster.session_id"],
            id.0.to_string()
        );
    }
    assert_eq!(
        memory
            .store()
            .retrieve(&persistent.id)
            .await
            .unwrap()
            .unwrap()
            .content,
        persistent.content
    );
    assert!(host
        .handle_memory_snapshot(&request(Path::new("/another-workspace"), "working"))
        .await
        .is_err());
    assert!(host
        .handle_memory_snapshot(&request(&workspace, "durable"))
        .await
        .is_err());
    host.sessions.lock().await.remove(&id.0.to_string());
    let response = host
        .handle_memory_snapshot(&request(&workspace, "session"))
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_str(response.0.get()).unwrap();
    assert!(body["items"].as_array().unwrap().is_empty());
    session.close().await;
}

#[tokio::test]
async fn management_works_without_model_or_session_and_refuses_another_workspace() {
    let root = tempfile::tempdir().unwrap();
    let mut launch = test_launch(None);
    launch.workspace = root.path().canonicalize().unwrap();
    launch.model_id.clear();
    launch.config = Default::default();
    launch.config.memory_dir = Some(root.path().join("private-memory"));
    let store = aster_core::durable_memory_vec::VecMemoryStore::open(
        launch.config.memory_dir.as_ref().unwrap(),
    )
    .unwrap();
    store
        .put(&aster_core::durable_memory_vec::MemoryRecord {
            id: "fixture".into(),
            content: "provider-free inspection".into(),
            tags: vec![],
            importance: 0.6,
            memory_type: "\"semantic\"".into(),
            timestamp: "2026-10-24T00:00:00Z".into(),
            meta: "{}".into(),
        })
        .unwrap();
    drop(store);
    let host = AsterCodeAgent::new(launch);
    let request = |workspace: &std::path::Path, body: serde_json::Value| {
        acp::ExtRequest::new(
            "aster/memory/manage",
            serde_json::value::to_raw_value(
                &serde_json::json!({ "workspace_root": workspace, "request": body }),
            )
            .unwrap()
            .into(),
        )
    };
    let query = serde_json::json!({ "operation": "list", "query": "", "memory_type": null, "cursor": null });
    let workspace = root.path().canonicalize().unwrap();
    let answer = host
        .handle_memory_management(&request(&workspace, query.clone()))
        .await
        .unwrap();
    let value: serde_json::Value = serde_json::from_str(answer.0.get()).unwrap();
    let response: MemoryManagementResponse =
        serde_json::from_value(value["outcome"]["Ok"].clone()).unwrap();
    assert!(matches!(
        response,
        MemoryManagementResponse::List { total: 1, .. }
    ));
    let answer = host
        .handle_memory_management(&request(
            &workspace,
            serde_json::json!({ "operation": "read", "id": "fixture" }),
        ))
        .await
        .unwrap();
    let value: serde_json::Value = serde_json::from_str(answer.0.get()).unwrap();
    assert_eq!(
        value["outcome"]["Ok"]["item"]["content"],
        "provider-free inspection"
    );
    let revision = &value["outcome"]["Ok"]["item"]["revision"];
    let answer = host
        .handle_memory_management(&request(
            &workspace,
            serde_json::json!({ "operation": "delete", "id": "fixture", "revision": revision }),
        ))
        .await
        .unwrap();
    let value: serde_json::Value = serde_json::from_str(answer.0.get()).unwrap();
    assert_eq!(value["outcome"]["Ok"]["id"], "fixture");
    assert!(host.sessions.lock().await.is_empty());
    assert!(host.core_agent.lock().await.is_none());
    let elsewhere = tempfile::tempdir().unwrap();
    assert!(host
        .handle_memory_management(&request(elsewhere.path(), query))
        .await
        .is_err());
    assert!(host
        .handle_memory_management(&request(
            root.path(),
            serde_json::json!({ "operation": "read", "id": "a", "memory_directory": "/other" })
        ))
        .await
        .is_err());
}
