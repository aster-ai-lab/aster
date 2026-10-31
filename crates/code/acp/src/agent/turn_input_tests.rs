#![cfg(test)]

use super::*;
use acp::Agent as _;
use std::future::Future;

#[tokio::test]
async fn send_now_cancels_before_waiting_for_the_previous_turn_permit() {
    for (meta, cancelled) in [
        (serde_json::json!({"sendNow": true}), true),
        (serde_json::json!({"sendNow": false}), false),
        (serde_json::json!({}), false),
    ] {
        let root = tempfile::tempdir().unwrap();
        let mut launch = tests::test_launch(Some(root.path().join("sessions")));
        launch.workspace = root.path().to_owned();
        let agent = A3sCodeAgent::new(launch);
        let id = agent
            .new_session(acp::NewSessionRequest::new(root.path()))
            .await
            .unwrap()
            .session_id;
        agent
            .turn_generation
            .lock()
            .await
            .insert(id.0.to_string(), 11);
        let (queue, session) = {
            let sessions = agent.sessions.lock().await;
            let live = sessions.get(id.0.as_ref()).unwrap();
            (Arc::clone(&live.turn_queue), Arc::clone(&live.session))
        };
        let _previous_turn = queue.acquire_owned().await.unwrap();
        let request = acp::PromptRequest::new(
            id.clone(),
            vec![acp::ContentBlock::Text(acp::TextContent::new(
                "exact next message",
            ))],
        )
        .meta(meta.as_object().cloned());
        let mut next = Box::pin(agent.prompt(request));
        assert!(
            std::future::poll_fn(|cx| std::task::Poll::Ready(next.as_mut().poll(cx).is_pending()))
                .await
        );
        assert_eq!(
            agent
                .cancelled_generation
                .lock()
                .await
                .get(id.0.as_ref())
                .copied(),
            cancelled.then_some(11)
        );
        drop(next);
        session.close().await;
    }
}

#[tokio::test]
async fn malformed_send_now_is_refused_without_cancelling_a_turn() {
    let root = tempfile::tempdir().unwrap();
    let mut launch = tests::test_launch(Some(root.path().join("sessions")));
    launch.workspace = root.path().to_owned();
    let agent = A3sCodeAgent::new(launch);
    let id = agent
        .new_session(acp::NewSessionRequest::new(root.path()))
        .await
        .unwrap()
        .session_id;
    for value in [
        serde_json::json!("true"),
        serde_json::json!(1),
        serde_json::Value::Null,
    ] {
        let request = acp::PromptRequest::new(
            id.clone(),
            vec![acp::ContentBlock::Text(acp::TextContent::new(
                "must not run",
            ))],
        )
        .meta(serde_json::json!({"sendNow": value}).as_object().cloned());
        let result =
            tokio::time::timeout(std::time::Duration::from_secs(2), agent.prompt(request)).await;
        let error = result
            .expect("invalid send-now metadata must be refused before work")
            .unwrap_err();
        assert!(error
            .data
            .unwrap()
            .as_str()
            .unwrap()
            .contains("sendNow must be a boolean"));
        assert!(agent.cancelled_generation.lock().await.is_empty());
    }
}

#[test]
fn direct_bash_metadata_is_never_reinterpreted_as_model_text() {
    for command in [
        serde_json::json!("printf direct-command"),
        serde_json::Value::Null,
        serde_json::json!(17),
        serde_json::json!({"command": "printf direct-command"}),
    ] {
        let block = acp::TextContent::new("text must not replace command intent").meta(
            serde_json::json!({"bash_command": command})
                .as_object()
                .cloned(),
        );
        let error = A3sCodeAgent::prompt_content(&[
            acp::ContentBlock::Text(acp::TextContent::new("ordinary prefix")),
            acp::ContentBlock::Text(block),
        ])
        .unwrap_err();
        assert_eq!(
            serde_json::to_value(error).unwrap()["data"],
            "Direct shell commands are unavailable with the Code backend."
        );
    }
}

#[test]
fn ordinary_text_and_unrelated_metadata_preserve_exact_user_bytes() {
    let text = "  中文 e\u{301}\nordinary text  ";
    let block = acp::TextContent::new(text).meta(
        serde_json::json!({"client_label": "text"})
            .as_object()
            .cloned(),
    );
    let (actual, attachments) =
        A3sCodeAgent::prompt_content(&[acp::ContentBlock::Text(block)]).unwrap();
    assert_eq!(actual, text);
    assert!(attachments.is_empty());
}
