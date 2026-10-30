#![cfg(test)]

use super::*;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

#[tokio::test]
async fn cancelled_question_cancels_only_its_own_turn() {
    tokio::task::LocalSet::new().run_until(async {
        let root = tempfile::tempdir().unwrap();
        let mut launch = tests::test_launch(Some(root.path().join("sessions")));
        launch.workspace = root.path().to_owned();
        let agent = A3sCodeAgent::new(launch);
        let opened = acp::Agent::new_session(&agent, acp::NewSessionRequest::new(root.path())).await.unwrap();
        let other = acp::Agent::new_session(&agent, acp::NewSessionRequest::new(root.path())).await.unwrap();
        let session = agent.sessions.lock().await[opened.session_id.0.as_ref()].session.clone();
        for id in [&opened.session_id, &other.session_id] {
            agent.turn_generation.lock().await.insert(id.0.to_string(), 1);
        }
        let (client_end, agent_end) = tokio::io::duplex(64 * 1024);
        let (read, write) = tokio::io::split(agent_end);
        let (connection, io) = acp::AgentSideConnection::new(
            A3sCodeAgent::new(tests::test_launch(None)),
            write.compat_write(), read.compat(),
            |future| { tokio::task::spawn_local(future); },
        );
        let transport = tokio::task::spawn_local(io);
        agent.set_client(Rc::new(connection));
        let responder = tokio::task::spawn_local(async move {
            let (read, mut write) = tokio::io::split(client_end);
            let mut reader = tokio::io::BufReader::new(read);
            loop {
                let mut line = String::new();
                assert_ne!(reader.read_line(&mut line).await.unwrap(), 0);
                let frame: serde_json::Value = serde_json::from_str(&line).unwrap();
                if frame["method"] == "session/request_permission" {
                    let reply = serde_json::json!({"jsonrpc":"2.0","id":frame["id"],"result":{"outcome":{"outcome":"cancelled"}}});
                    write.write_all(format!("{reply}\n").as_bytes()).await.unwrap();
                    return;
                }
            }
        });
        let (sender, events) = tokio::sync::mpsc::channel(1);
        sender.send(AgentEvent::UserQuestion {
            question_id: "cancelled-question".into(), question: "Choose".into(),
            options: vec!["left".into(), "right".into()], allow_free_text: true, form: None,
        }).await.unwrap();
        drop(sender);
        let permit = Arc::new(tokio::sync::Mutex::new(())).lock_owned().await;
        let response = tokio::time::timeout(std::time::Duration::from_secs(5), agent.drain_turn(
            &opened.session_id, &session,
            a3s_code_core::AgentRunStream { run: None, events, worker: tokio::spawn(async {}) },
            None, 1, permit, None,
        )).await.expect("a dismissed question must release its ACP turn").unwrap();
        assert_eq!(response.stop_reason, acp::StopReason::Cancelled);
        assert!(agent.turn_was_cancelled(opened.session_id.0.as_ref(), 1).await);
        assert!(!agent.turn_was_cancelled(other.session_id.0.as_ref(), 1).await);
        responder.await.unwrap();
        transport.abort();
        session.close().await;
    }).await;
}
