use a3s_code_core::store::{MemorySessionStore, TranscriptPageRequest};
use a3s_code_core::{Agent, AgentSession, CodeConfig, PlanningMode, SessionOptions};
use a3s_effect::{FileLog, LogStore, NewFact};
use std::sync::{Arc, Barrier};

async fn fixture() -> (tempfile::TempDir, Agent, AgentSession) {
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("host.acl");
    std::fs::write(
        &config,
        r#"
default_model = "mock/test"
providers "mock" {
  api_key = "synthetic-unused-key"
  base_url = "http://127.0.0.1:9/v1"
  models "test" {}
}
"#,
    )
    .unwrap();
    let agent = Agent::from_config(CodeConfig::from_file(&config).unwrap())
        .await
        .unwrap();
    let session = agent
        .session_async(
            root.path().display().to_string(),
            Some(
                SessionOptions::new()
                    .with_session_id("shared-reader")
                    .with_session_store(Arc::new(MemorySessionStore::new()))
                    .with_auto_save(false)
                    .with_planning_mode(PlanningMode::Disabled)
                    .with_continuation(false),
            ),
        )
        .await
        .unwrap();
    let log = FileLog::open(a3s_code_core::session_folder::effect_log_dir(
        root.path(),
        session.id(),
    ))
    .unwrap();
    let records: Vec<_> = (0..32)
        .map(|index| NewFact {
            kind: "user.message".into(),
            key: format!("user-{index}"),
            payload: serde_json::json!({"text":format!("row-{index}: {}", "x".repeat(16384))}),
        })
        .collect();
    log.append(
        &a3s_code_core::fact_control::thread_for_session(session.id()),
        &records,
        None,
    )
    .unwrap();
    drop(log);
    (root, agent, session)
}

#[tokio::test]
async fn concurrent_transcript_readers_share_the_session_log() {
    let (_root, agent, session) = fixture().await;
    let barrier = Barrier::new(16);
    let results = std::thread::scope(|scope| {
        let readers: Vec<_> = (0..16)
            .map(|_| {
                scope.spawn(|| {
                    barrier.wait();
                    session.transcript_page(TranscriptPageRequest {
                        cursor: None,
                        limit: 50,
                    })
                })
            })
            .collect();
        readers
            .into_iter()
            .map(|reader| reader.join().unwrap())
            .collect::<Vec<_>>()
    });
    let errors: Vec<_> = results
        .iter()
        .filter_map(|result| result.as_ref().err())
        .collect();
    assert!(errors.is_empty(), "same-session reads failed: {errors:?}");
    assert!(results
        .iter()
        .all(|page| page.as_ref().unwrap().entries.len() == 32));
    session.close().await;
    agent.close().await;
}

#[tokio::test]
async fn a_foreign_log_owner_still_refuses_transcript_reads() {
    let (root, agent, session) = fixture().await;
    let foreign = FileLog::open(a3s_code_core::session_folder::effect_log_dir(
        root.path(),
        session.id(),
    ))
    .unwrap();
    let fence = foreign.try_writer_fence().unwrap();
    let error = session
        .transcript_page(TranscriptPageRequest {
            cursor: None,
            limit: 50,
        })
        .err()
        .expect("a foreign writer must refuse the read");
    assert!(
        error
            .to_string()
            .contains("fact log is busy in another instance"),
        "{error}"
    );
    drop((fence, foreign));
    assert_eq!(
        session
            .transcript_page(TranscriptPageRequest {
                cursor: None,
                limit: 50
            })
            .unwrap()
            .entries
            .len(),
        32
    );
    session.close().await;
    agent.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn transcript_reads_do_not_refuse_a_same_session_save() {
    let (_root, agent, session) = fixture().await;
    let barrier = Barrier::new(16);
    let runtime = tokio::runtime::Handle::current();
    let results = std::thread::scope(|scope| {
        let readers: Vec<_> = (0..16)
            .map(|index| {
                let session = &session;
                let barrier = &barrier;
                let runtime = &runtime;
                scope.spawn(move || {
                    barrier.wait();
                    if index == 0 {
                        runtime.block_on(session.save())
                    } else {
                        session
                            .transcript_page(TranscriptPageRequest {
                                cursor: None,
                                limit: 50,
                            })
                            .map(|page| assert_eq!(page.entries.len(), 32))
                    }
                })
            })
            .collect();
        readers
            .into_iter()
            .map(|reader| reader.join().unwrap())
            .collect::<Vec<_>>()
    });
    let errors: Vec<_> = results
        .iter()
        .filter_map(|result| result.as_ref().err())
        .collect();
    assert!(
        errors.is_empty(),
        "same-session reads/save failed: {errors:?}"
    );
    session.close().await;
    agent.close().await;
}
