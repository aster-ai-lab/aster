#![cfg(test)]

use super::{test_launch, AsterCodeAgent};
use agent_client_protocol::{self as acp, Agent as _};
use serde_json::json;

fn agent_without_model(workspace: &std::path::Path) -> AsterCodeAgent {
    let mut launch = test_launch(Some(workspace.join("must-not-create-store")));
    launch.workspace = workspace.to_owned();
    // Deliberately no model source: the old path must fail during model
    // resolution, before session runtime/sandbox construction. This baseline
    // can observe the premature directory write without activating execution.
    launch.config.providers.clear();
    launch.config.default_model = None;
    AsterCodeAgent::new(launch)
}

#[tokio::test]
async fn registered_secondary_workspace_resume_is_bound_to_its_original_core_root() {
    let root = tempfile::tempdir().unwrap();
    let secondary = tempfile::tempdir().unwrap();
    let mut launch = test_launch(Some(root.path().join("sessions")));
    launch.workspace = root.path().canonicalize().unwrap();
    let agent = AsterCodeAgent::new(launch);
    let opened = agent
        .new_session(acp::NewSessionRequest::new(secondary.path()))
        .await
        .unwrap();
    let sid = opened.session_id.0.as_ref();
    assert_eq!(
        agent.admit_resume(sid, secondary.path()).await.unwrap(),
        secondary.path().canonicalize().unwrap()
    );
    assert!(agent.admit_resume(sid, root.path()).await.is_err());
    assert!(agent
        .admit_resume("unregistered", secondary.path())
        .await
        .is_err());
    #[cfg(unix)]
    {
        let original = secondary.path().with_extension("original");
        std::fs::rename(secondary.path(), &original).unwrap();
        std::os::unix::fs::symlink(root.path(), secondary.path()).unwrap();
        let refused = agent.admit_resume(sid, secondary.path()).await.is_err();
        std::fs::remove_file(secondary.path()).unwrap();
        std::fs::rename(&original, secondary.path()).unwrap();
        assert!(
            refused,
            "a retargeted workspace must not rebind a live session"
        );
    }
    agent
        .sessions
        .lock()
        .await
        .get(sid)
        .unwrap()
        .session
        .close()
        .await;
}

#[tokio::test]
async fn unknown_resume_refuses_before_any_session_or_model_initialization() {
    let root = tempfile::tempdir().expect("workspace");
    let agent = agent_without_model(root.path());
    let error = agent
        .load_session(acp::LoadSessionRequest::new(
            "does-not-exist",
            root.path().to_owned(),
        ))
        .await
        .expect_err("unknown session");
    println!(
        "unknown resume observations: {}",
        json!({
            "createdStateDirectory": root.path().join(".aster").exists(),
            "createdSnapshotStore": root.path().join("must-not-create-store").exists(),
            "initializedCoreAgent": agent.core_agent.lock().await.is_some(),
        })
    );
    assert_eq!(
        serde_json::to_value(error).expect("error"),
        serde_json::to_value(acp::Error::resource_not_found(None)).expect("not found")
    );
    assert!(!root.path().join(".aster").exists());
    assert!(!root.path().join("must-not-create-store").exists());
    assert!(agent.core_agent.lock().await.is_none());
    assert!(agent.config_mcp.lock().await.is_none());
    assert!(agent.sessions.lock().await.is_empty());
}

#[tokio::test]
async fn foreign_resume_refuses_before_touching_the_requested_workspace() {
    let root = tempfile::tempdir().expect("workspace");
    let foreign = tempfile::tempdir().expect("foreign workspace");
    let agent = agent_without_model(root.path());
    let error = agent
        .load_session(acp::LoadSessionRequest::new(
            "foreign-id",
            foreign.path().to_owned(),
        ))
        .await
        .expect_err("foreign workspace");
    println!(
        "foreign resume observations: {}",
        json!({
            "createdForeignStateDirectory": foreign.path().join(".aster").exists(),
            "initializedCoreAgent": agent.core_agent.lock().await.is_some(),
        })
    );
    assert_eq!(
        serde_json::to_value(error).expect("error"),
        serde_json::to_value(acp::Error::resource_not_found(None)).expect("not found")
    );
    assert!(!foreign.path().join(".aster").exists());
    assert!(agent.core_agent.lock().await.is_none());
}

fn record(workspace: &std::path::Path, id: &str) -> std::path::PathBuf {
    let home = workspace.join(".aster/effect-log");
    std::fs::create_dir_all(&home).expect("legacy home");
    let path = home.join(format!("{id}.jsonl"));
    let fact =
        json!({"seq":1,"type":"user.message","key":"user:1","payload":{"text":"retained prompt"}});
    std::fs::write(&path, format!("{fact}\n")).expect("fact");
    path
}

#[tokio::test]
async fn existing_fact_record_is_admitted_without_activating_any_surface() {
    let root = tempfile::tempdir().expect("workspace");
    let path = record(root.path(), "existing");
    let before = std::fs::read(&path).expect("before");
    let agent = agent_without_model(root.path());
    let workspace = agent
        .admit_resume("existing", root.path())
        .await
        .expect("existing record");
    assert_eq!(workspace, root.path().canonicalize().expect("canonical"));
    assert_eq!(std::fs::read(path).expect("after"), before);
    assert!(!root.path().join("must-not-create-store").exists());
    assert!(!root.path().join(".aster/sessions").exists());
    assert!(agent.core_agent.lock().await.is_none());
    assert!(agent.config_mcp.lock().await.is_none());
    assert!(agent.documents.borrow().is_none());
}

#[tokio::test]
async fn a_legacy_record_without_a_snapshot_refuses_instead_of_creating_an_empty_session() {
    let root = tempfile::tempdir().expect("workspace");
    let path = record(root.path(), "existing");
    let before = std::fs::read(&path).expect("before");
    let agent = agent_without_model(root.path());
    let error = agent
        .load_session(acp::LoadSessionRequest::new(
            "existing",
            root.path().to_owned(),
        ))
        .await
        .expect_err("restore requires an existing snapshot");
    let error = serde_json::to_value(error).expect("error");
    assert_eq!(error["code"], -32603);
    assert!(error["data"]
        .as_str()
        .expect("snapshot recovery reason")
        .contains("snapshot store is missing"));
    assert_eq!(std::fs::read(path).expect("after"), before);
    assert!(!root.path().join(".aster/sessions").exists());
    assert!(!root.path().join("must-not-create-store").exists());
    assert!(agent.sessions.lock().await.is_empty());
}

#[tokio::test]
async fn a_foreign_existing_record_and_corrupt_local_record_share_not_found() {
    let root = tempfile::tempdir().expect("workspace");
    let foreign = tempfile::tempdir().expect("foreign");
    record(foreign.path(), "existing");
    let bad = record(root.path(), "bad");
    std::fs::write(bad, b"corrupt").expect("corrupt");
    let agent = agent_without_model(root.path());
    for (id, cwd) in [
        ("existing", foreign.path()),
        ("bad", root.path()),
        ("missing", root.path()),
    ] {
        let error = agent
            .load_session(acp::LoadSessionRequest::new(id, cwd.to_owned()))
            .await
            .expect_err("not found");
        assert_eq!(
            serde_json::to_value(error).expect("error"),
            serde_json::to_value(acp::Error::resource_not_found(None)).expect("not found")
        );
    }
    assert!(agent.core_agent.lock().await.is_none());
    assert!(agent.config_mcp.lock().await.is_none());
}

#[tokio::test]
async fn refused_resume_does_not_admit_an_opted_in_document_surface() {
    let root = tempfile::tempdir().expect("workspace");
    let agent = agent_without_model(root.path());
    let mut meta = acp::Meta::new();
    meta.insert("asterStudioDocuments".into(), json!(true));
    let request = acp::LoadSessionRequest::new("missing", root.path().to_owned()).meta(Some(meta));
    assert!(agent.load_session(request).await.is_err());
    assert!(agent.documents.borrow().is_none());
    assert!(agent.core_agent.lock().await.is_none());
    assert!(!root.path().join(".aster").exists());
}

#[tokio::test]
async fn an_empty_new_session_can_resume_without_a_turn() {
    use aster_core::store::{FileSessionStore, SessionStore};

    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("sessions");
    let mut launch = test_launch(Some(directory.clone()));
    launch.workspace = root.path().to_owned();
    let expected_model = launch.model_id.clone();
    let host = AsterCodeAgent::new(launch.clone());
    let id = host
        .new_session(acp::NewSessionRequest::new(root.path()))
        .await
        .unwrap()
        .session_id;
    let store = FileSessionStore::new(&directory).await.unwrap();
    let snapshot = store.load_snapshot(id.0.as_ref()).await.unwrap();
    assert!(
        snapshot.is_some(),
        "new session must be durable before its response"
    );
    assert!(snapshot.unwrap().session.messages.is_empty());
    host.sessions
        .lock()
        .await
        .get(id.0.as_ref())
        .unwrap()
        .session
        .close()
        .await;
    drop(host);

    let host = AsterCodeAgent::new(launch);
    let restored = host
        .load_session(acp::LoadSessionRequest::new(id.clone(), root.path()))
        .await
        .unwrap();
    assert_eq!(
        restored.models.unwrap().current_model_id.0.as_ref(),
        expected_model
    );
    host.sessions
        .lock()
        .await
        .get(id.0.as_ref())
        .unwrap()
        .session
        .close()
        .await;
}

#[tokio::test]
async fn snapshot_resume_keeps_usage_and_refuses_foreign_or_corrupt_snapshot() {
    use aster_core::store::{FileSessionStore, SessionStore};

    let root = tempfile::tempdir().unwrap();
    let foreign = tempfile::tempdir().unwrap();
    let directory = root.path().join("sessions");
    let mut launch = test_launch(Some(directory.clone()));
    launch.workspace = root.path().to_owned();
    let host = AsterCodeAgent::new(launch.clone());
    let id = host
        .new_session(
            acp::NewSessionRequest::new(root.path()).meta(
                json!({"asterEngineering":{"policy":"none"}})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await
        .unwrap()
        .session_id;
    let session = host
        .sessions
        .lock()
        .await
        .get(id.0.as_ref())
        .unwrap()
        .session
        .clone();
    session.save().await.unwrap();
    session.close().await;
    drop(host);
    let thread = aster_core::fact_control::thread_for_session(id.0.as_ref());
    let source = aster_core::session_folder::effect_log_dir(root.path(), id.0.as_ref())
        .join(format!("{thread}.jsonl"));
    let fact =
        json!({"seq":1,"type":"user.message","key":"user:1","payload":{"text":"retained prompt"}});
    std::fs::write(&source, format!("{fact}\n")).unwrap();
    let source_bytes = std::fs::read(&source).unwrap();
    let store = FileSessionStore::new(&directory).await.unwrap();
    let mut snapshot = store.load_snapshot(id.0.as_ref()).await.unwrap().unwrap();
    snapshot.session.total_usage.prompt_tokens = 17;
    snapshot.session.total_usage.completion_tokens = 3;
    snapshot.session.total_usage.total_tokens = 20;
    store.save_snapshot(&snapshot).await.unwrap();
    let host = AsterCodeAgent::new(launch.clone());
    host.load_session(acp::LoadSessionRequest::new(id.clone(), root.path()))
        .await
        .unwrap();
    let session = host
        .sessions
        .lock()
        .await
        .get(id.0.as_ref())
        .unwrap()
        .session
        .clone();
    assert_eq!(session.recorded_usage_tokens(), (17, 3, 20));
    session.close().await;
    drop(host);

    snapshot.session.config.workspace = foreign.path().to_string_lossy().into_owned();
    store.save_snapshot(&snapshot).await.unwrap();
    let host = AsterCodeAgent::new(launch);
    let error = host
        .load_session(acp::LoadSessionRequest::new(id.clone(), root.path()))
        .await
        .unwrap_err();
    let error = serde_json::to_value(error).unwrap();
    assert_eq!(error["code"], -32602);
    assert!(error["data"]
        .as_str()
        .is_some_and(|reason| reason.contains("foreign workspace")));
    assert!(host.sessions.lock().await.is_empty());
    assert!(!foreign.path().join(".aster").exists());

    let path = std::fs::read_dir(directory.join("v1/sessions"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    std::fs::write(&path, "invalid snapshot").unwrap();
    assert!(host
        .load_session(acp::LoadSessionRequest::new(id, root.path()))
        .await
        .is_err());
    assert!(host.sessions.lock().await.is_empty());
    assert_eq!(std::fs::read_to_string(path).unwrap(), "invalid snapshot");
    assert_eq!(std::fs::read(source).unwrap(), source_bytes);
}
