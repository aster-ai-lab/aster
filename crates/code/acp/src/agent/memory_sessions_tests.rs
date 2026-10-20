#![cfg(test)]

use super::{test_launch, A3sCodeAgent};
use agent_client_protocol::{self as acp, Agent as _};

#[tokio::test]
async fn two_acp_host_sessions_recall_and_write_the_same_durable_collection() {
    let root = tempfile::tempdir().unwrap();
    let mut launch = test_launch(Some(root.path().join("sessions")));
    launch.workspace = root.path().into();
    launch.config.memory_dir = Some(root.path().join("memory"));
    let host = A3sCodeAgent::new(launch);
    let mut sessions = Vec::new();
    for _ in 0..2 {
        let id = host
            .new_session(acp::NewSessionRequest::new(root.path()))
            .await
            .unwrap()
            .session_id;
        sessions.push(
            host.sessions.lock().await[&id.0.to_string()]
                .session
                .clone(),
        );
    }
    let first = sessions[0].memory().unwrap().store();
    let second = sessions[1].memory().unwrap().store();
    let item = sessions[0]
        .memory()
        .unwrap()
        .remember_success_item("shared acp durable handoff marker", &[], "private workflow")
        .await
        .unwrap();
    assert_eq!(
        second.retrieve(&item.id).await.unwrap().unwrap().content,
        item.content
    );
    let second_item = sessions[1]
        .memory()
        .unwrap()
        .remember_success_item("second acp durable marker", &[], "private workflow")
        .await
        .unwrap();
    assert_eq!(
        first
            .retrieve(&second_item.id)
            .await
            .unwrap()
            .unwrap()
            .content,
        second_item.content
    );
    assert_eq!(
        sessions[1]
            .memory()
            .unwrap()
            .recall_similar("handoff", 5)
            .await
            .unwrap()
            .len(),
        1
    );
    for session in sessions {
        session.close().await;
    }
}
