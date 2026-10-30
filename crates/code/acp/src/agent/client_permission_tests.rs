use super::*;
use acp::Agent as _;
use serde_json::json;

async fn configured(root: &std::path::Path, rules: serde_json::Value) -> A3sCodeAgent {
    let mut launch = tests::test_launch(Some(root.join("store")));
    launch.workspace = root.into();
    let agent = A3sCodeAgent::new(launch);
    agent
        .initialize(
            acp::InitializeRequest::new(acp::ProtocolVersion::V1)
                .meta(json!({"permissionRules": rules}).as_object().cloned()),
        )
        .await
        .unwrap();
    agent
}

async fn write(agent: &A3sCodeAgent, id: &acp::SessionId, file: &str) -> i32 {
    let session = agent.sessions.lock().await[id.0.as_ref()].session.clone();
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        session.governed_tool("write", json!({"file_path":file,"content":"changed"})),
    )
    .await
    .expect("CLI rules must settle without a permission prompt")
    .unwrap()
    .exit_code
}

#[tokio::test]
async fn cli_permission_deny_survives_yolo_mode_and_model_replacement() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("protected.txt");
    std::fs::write(&file, "original").unwrap();
    let agent = configured(root.path(), json!({"deny":["Write(*)"]})).await;
    let id = agent
        .new_session(acp::NewSessionRequest::new(root.path()))
        .await
        .unwrap()
        .session_id;
    for mode in ["yolo", "plan", "auto", "default", "yolo"] {
        agent
            .set_session_mode(acp::SetSessionModeRequest::new(id.clone(), mode))
            .await
            .unwrap();
        assert_ne!(write(&agent, &id, "protected.txt").await, 0, "mode={mode}");
    }
    agent
        .set_session_model(acp::SetSessionModelRequest::new(
            id.clone(),
            "openai/gpt-4o",
        ))
        .await
        .unwrap();
    assert_ne!(write(&agent, &id, "protected.txt").await, 0);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "original");
}

#[tokio::test]
async fn cli_permission_allow_cannot_relax_plan_or_research_and_deny_stays_binding() {
    let root = tempfile::tempdir().unwrap();
    let agent = configured(
        root.path(),
        json!({"allow":["Write(*)"],"deny":["Write(protected.txt)"]}),
    )
    .await;
    let id = agent
        .new_session(acp::NewSessionRequest::new(root.path()))
        .await
        .unwrap()
        .session_id;
    assert_eq!(write(&agent, &id, "allowed.txt").await, 0);
    assert_ne!(write(&agent, &id, "protected.txt").await, 0);
    agent
        .set_session_mode(acp::SetSessionModeRequest::new(id.clone(), "plan"))
        .await
        .unwrap();
    assert_ne!(write(&agent, &id, "plan.txt").await, 0);
    let gate = ResearchGate::enter(&agent.sessions.lock().await[id.0.as_ref()]).unwrap();
    agent
        .set_session_mode(acp::SetSessionModeRequest::new(id.clone(), "yolo"))
        .await
        .unwrap();
    assert_ne!(write(&agent, &id, "research.txt").await, 0);
    drop(gate);
    assert_eq!(write(&agent, &id, "after-research.txt").await, 0);
    assert_ne!(write(&agent, &id, "protected.txt").await, 0);
    for file in ["protected.txt", "plan.txt", "research.txt"] {
        assert!(
            !root.path().join(file).exists(),
            "{file} must not be written"
        );
    }
}

#[tokio::test]
async fn cli_permission_deny_is_applied_during_research_admission() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("private.txt"), "private").unwrap();
    let agent = configured(root.path(), json!({"deny":["Read(private.txt)"]})).await;
    let id = agent
        .new_session(acp::NewSessionRequest::new(root.path()))
        .await
        .unwrap()
        .session_id;
    let (gate, session) = {
        let sessions = agent.sessions.lock().await;
        let live = &sessions[id.0.as_ref()];
        (ResearchGate::enter(live).unwrap(), live.session.clone())
    };
    let result = session
        .governed_tool("read", json!({"file_path":"private.txt"}))
        .await
        .unwrap();
    assert_ne!(result.exit_code, 0);
    assert!(!result.output.contains("private\n"));
    drop(gate);
}

#[tokio::test]
async fn cli_permission_invalid_metadata_is_refused_before_state_changes() {
    for rules in [
        json!(null),
        json!([]),
        json!({"deny":"Write(*)"}),
        json!({"allow":[true]}),
        json!({"denny":["Write(*)"]}),
        json!({"deny":["  "]}),
    ] {
        let agent = A3sCodeAgent::new(tests::test_launch(None));
        let error = agent
            .initialize(
                acp::InitializeRequest::new(acp::ProtocolVersion::V1).meta(
                    json!({"rules":"must not commit","permissionRules":rules})
                        .as_object()
                        .cloned(),
                ),
            )
            .await
            .unwrap_err();
        assert!(format!("{error:?}").contains("permissionRules"));
        assert!(agent.client_rules.borrow().is_none());
        assert!(agent.client_permissions.borrow().is_empty());
        assert!(agent.sessions.lock().await.is_empty());
        assert!(agent.core_agent.lock().await.is_none());
    }
}

#[tokio::test]
async fn cli_permission_allow_preserves_the_host_source_review_ceiling() {
    let root = tempfile::tempdir().unwrap();
    let agent = configured(root.path(), json!({"allow":["Write(*)"]})).await;
    let id = agent
        .new_session(
            acp::NewSessionRequest::new(root.path()).meta(
                json!({"a3sHost":{"sourceReview":true}})
                    .as_object()
                    .cloned(),
            ),
        )
        .await
        .unwrap()
        .session_id;
    assert_ne!(write(&agent, &id, "must-not-write.txt").await, 0);
    assert!(!root.path().join("must-not-write.txt").exists());
}

#[tokio::test]
async fn cli_permission_whitespace_keeps_the_cli_rules_meaning() {
    let root = tempfile::tempdir().unwrap();
    let agent = configured(root.path(), json!({"deny":["  Write(*)  "]})).await;
    let id = agent
        .new_session(
            acp::NewSessionRequest::new(root.path())
                .meta(json!({"yoloMode":true}).as_object().cloned()),
        )
        .await
        .unwrap()
        .session_id;
    assert_ne!(write(&agent, &id, "protected.txt").await, 0);
    assert!(!root.path().join("protected.txt").exists());
}
