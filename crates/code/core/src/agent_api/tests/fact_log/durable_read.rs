use super::*;
use a3s_effect::LogStore;

#[tokio::test]
async fn durable_transcript_uses_the_owned_log_while_its_writer_is_fenced() {
    let workspace = tempfile::tempdir().unwrap();
    let agent = crate::Agent::from_config(test_config()).await.unwrap();
    let session = agent
        .build_session(
            workspace.path().to_string_lossy().to_string(),
            Arc::new(CountingText {
                calls: Arc::new(AtomicUsize::new(0)),
            }),
            &crate::SessionOptions::new()
                .with_session_id("durable-read-owner")
                .with_planning_mode(crate::prompts::PlanningMode::Disabled),
        )
        .unwrap();
    session
        .send("owned durable transcript", None)
        .await
        .unwrap();
    let expected = serde_json::to_value(session.durable_transcript()).unwrap();
    assert_eq!(expected.as_array().unwrap().len(), 2);
    let log = session
        .run_admission
        .fact_log_slot()
        .lock()
        .unwrap()
        .clone()
        .unwrap();
    let writer = log.try_writer_fence().unwrap();
    let foreign = a3s_effect::FileLog::open(crate::session_folder::effect_log_dir(
        workspace.path(),
        "durable-read-owner",
    ))
    .unwrap();
    let thread = crate::fact_control::thread_for_session("durable-read-owner");
    assert!(matches!(
        foreign.read(&thread),
        Err(a3s_effect::ActorError::LogBusy)
    ));
    assert_eq!(
        serde_json::to_value(session.durable_transcript()).unwrap(),
        expected
    );
    drop(writer);
    assert!(foreign.read(&thread).is_ok());
    session.close().await;
}
