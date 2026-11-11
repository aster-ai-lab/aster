use super::*;
use acp::Agent as _;

#[tokio::test]
async fn close_settles_sessions_and_refuses_admission() {
    let root = tempfile::tempdir().unwrap();
    let mut launch = tests::test_launch(Some(root.path().join("sessions")));
    launch.workspace = root.path().to_owned();
    let host = AsterCodeAgent::new(launch);
    let opened = host
        .new_session(acp::NewSessionRequest::new(root.path()))
        .await
        .unwrap();
    let session = host.sessions.lock().await[opened.session_id.0.as_ref()]
        .session
        .clone();

    host.close().await;
    assert!(matches!(
        session.send("must stay closed", None).await,
        Err(aster_core::error::CodeError::SessionClosed { .. })
    ));
    assert!(host
        .new_session(acp::NewSessionRequest::new(root.path()))
        .await
        .is_err());
    host.close().await;
}
