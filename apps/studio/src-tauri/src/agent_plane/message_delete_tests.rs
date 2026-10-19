#![cfg(test)]

use super::super::{
    LiveTurn,
    tests::{plane_fixture, stored_line},
};
use super::*;
use serde_json::json;
use tokio::sync::{mpsc, watch};

#[tokio::test]
async fn delete_message_preserves_other_rows_and_invalidates_old_resume_tokens_on_disk() {
    let dir = tempfile::tempdir().unwrap();
    let (turns, store, sessions) = plane_fixture(dir.path());
    let session = sessions
        .create(&json!({"workspace":{"type":"system"}}))
        .unwrap();
    let target = stored_line(&session.id, "assistant", "success", "remove me");
    let mut kept = stored_line(&session.id, "user", "success", "keep me");
    kept.runtime_resume_token = Some("old transcript".into());
    store.save_message(target.clone()).unwrap();
    store.save_message(kept.clone()).unwrap();
    turns.delete_message(&session.id, &target.id).await.unwrap();
    let reopened = crate::agent_plane::store::PlaneStore::open(Some(dir.path().join("state")));
    let rows = reopened.messages(&session.id);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, kept.id);
    assert_eq!(rows[0].searchable_text, "keep me");
    assert!(rows[0].runtime_resume_token.is_none());
    assert!(matches!(
        turns.delete_message(&session.id, &target.id).await,
        Err(PlaneError::NotFound(_))
    ));
}

#[tokio::test]
async fn delete_message_refuses_foreign_and_missing_message_with_the_same_contract() {
    let dir = tempfile::tempdir().unwrap();
    let (turns, store, sessions) = plane_fixture(dir.path());
    let first = sessions
        .create(&json!({"workspace":{"type":"system"}}))
        .unwrap();
    let second = sessions
        .create(&json!({"workspace":{"type":"system"}}))
        .unwrap();
    let target = stored_line(&first.id, "assistant", "success", "private");
    store.save_message(target.clone()).unwrap();
    for id in [target.id.as_str(), "missing"] {
        assert!(matches!(
            turns.delete_message(&second.id, id).await,
            Err(PlaneError::NotFound(_))
        ));
    }
    assert_eq!(store.messages(&first.id).len(), 1);
}

#[tokio::test]
async fn delete_message_refuses_live_review_pending_and_warming_tasks() {
    let dir = tempfile::tempdir().unwrap();
    let (turns, store, sessions) = plane_fixture(dir.path());
    let session = sessions
        .create(&json!({"workspace":{"type":"system"}}))
        .unwrap();
    let mut target = stored_line(&session.id, "assistant", "pending", "pending");
    store.save_message(target.clone()).unwrap();
    assert!(matches!(
        turns.delete_message(&session.id, &target.id).await,
        Err(PlaneError::Conflict(_))
    ));
    target.status = "success".into();
    store.save_message(target.clone()).unwrap();
    turns.warming.lock().unwrap().insert(session.id.clone());
    assert!(matches!(
        turns.delete_message(&session.id, &target.id).await,
        Err(PlaneError::Conflict(_))
    ));
    turns.warming.lock().unwrap().clear();
    let (abort, _) = watch::channel(false);
    let (steer_tx, _) = mpsc::unbounded_channel();
    turns.lock_live().insert(
        session.id.clone(),
        LiveTurn {
            owner: target.id.clone(),
            execution_id: "review".into(),
            status: "running".into(),
            observed: None,
            buffered: vec![],
            abort,
            approvals: Default::default(),
            steer_tx,
        },
    );
    assert!(matches!(
        turns.delete_message(&session.id, &target.id).await,
        Err(PlaneError::Conflict(_))
    ));
    assert_eq!(store.messages(&session.id).len(), 1);
}

#[tokio::test]
async fn delete_message_keeps_memory_and_disk_unchanged_when_persistence_fails() {
    let dir = tempfile::tempdir().unwrap();
    let (turns, store, sessions) = plane_fixture(dir.path());
    let session = sessions
        .create(&json!({"workspace":{"type":"system"}}))
        .unwrap();
    let target = stored_line(&session.id, "assistant", "success", "keep on failure");
    store.save_message(target.clone()).unwrap();
    let path = dir
        .path()
        .join(format!("state/messages/{}.json", session.id));
    let before = std::fs::read(&path).unwrap();
    std::fs::create_dir(path.with_extension("json.tmp")).unwrap();
    assert!(matches!(
        turns.delete_message(&session.id, &target.id).await,
        Err(PlaneError::Internal(_))
    ));
    assert_eq!(store.messages(&session.id).len(), 1);
    assert_eq!(std::fs::read(path).unwrap(), before);
}

#[tokio::test]
async fn delete_message_http_binds_scope_and_requires_the_native_bearer() {
    use crate::agent_plane::{PlaneConfig, application, serve};
    use std::sync::Arc;
    let dir = tempfile::tempdir().unwrap();
    let (_, store, sessions) = plane_fixture(dir.path());
    let first = sessions
        .create(&json!({"workspace":{"type":"system"}}))
        .unwrap();
    let second = sessions
        .create(&json!({"workspace":{"type":"system"}}))
        .unwrap();
    let target = stored_line(&first.id, "assistant", "success", "remove through HTTP");
    store.save_message(target.clone()).unwrap();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let token = "ab".repeat(32);
    let app = application(
        PlaneConfig {
            root: Some(dir.path().join("state")),
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
    let server = tokio::spawn(serve(app, listener));
    let client = reqwest::Client::new();
    let url = format!(
        "{base}/api/agent-sessions/{}/messages/{}",
        first.id, target.id
    );
    assert_eq!(client.delete(&url).send().await.unwrap().status(), 403);
    let foreign = format!(
        "{base}/api/agent-sessions/{}/messages/{}",
        second.id, target.id
    );
    assert_eq!(
        client
            .delete(foreign)
            .bearer_auth(&token)
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    let response = client
        .delete(&url)
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    let status = response.status();
    let body = response.text().await.unwrap();
    assert_eq!(status, 200, "{body}");
    assert_eq!(body, "null");
    assert_eq!(
        client
            .get(&url)
            .bearer_auth(&token)
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    server.abort();
}
