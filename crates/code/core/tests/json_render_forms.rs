#![cfg(test)]

use aster_core::{
    ask_user,
    tools::{ToolContext, ToolExecutor},
    AgentEvent,
};
use serde_json::{json, Value};

const FORM: &str = include_str!("../../../../packages/intelligent-ui/fixtures/form.json");

#[tokio::test]
async fn json_form_answers_return_through_the_original_tool_call() {
    let workspace = tempfile::tempdir().unwrap();
    let executor = ToolExecutor::new(workspace.path().display().to_string());
    let (events, mut receiver) = tokio::sync::broadcast::channel(4);
    let context = ToolContext::new(workspace.path().to_path_buf())
        .with_session_id(format!("json-render-{}", uuid::Uuid::new_v4()))
        .with_agent_event_tx(events);
    let host = tokio::spawn(async move {
        let AgentEvent::UserQuestion {
            question_id, form, ..
        } = receiver.recv().await.unwrap()
        else {
            panic!("expected a form");
        };
        assert_eq!(form.as_deref(), Some(FORM));
        ask_user::answer(
            &question_id,
            r#"{"name":"Ada","amount":4,"ready":true,"choice":"beta","notes":""}"#,
        );
    });
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        executor.execute_with_context("ask_user_form", &json!({"form":FORM}), &context),
    )
    .await
    .unwrap()
    .unwrap();
    host.await.unwrap();
    assert_eq!(result.exit_code, 0, "{}", result.output);
    let answer: Value = serde_json::from_str(&result.output).unwrap();
    assert_eq!(answer["status"], "answered");
    assert_eq!(answer["values"]["amount"], 4);
    assert_eq!(answer["values"]["ready"], true);
    assert_eq!(answer["permission_grant"], false);
}

#[tokio::test]
async fn retired_source_is_rejected_without_parking_or_emitting_a_form() {
    let workspace = tempfile::tempdir().unwrap();
    let executor = ToolExecutor::new(workspace.path().display().to_string());
    let (events, mut receiver) = tokio::sync::broadcast::channel(4);
    let context = ToolContext::new(workspace.path().to_path_buf()).with_agent_event_tx(events);
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        executor.execute_with_context(
            "ask_user_form",
            &json!({"form":"root = Input(\"name\", \"Name\")"}),
            &context,
        ),
    )
    .await
    .unwrap()
    .unwrap();
    assert_ne!(result.exit_code, 0);
    assert!(result.output.contains("JSON Render"));
    assert!(receiver.try_recv().is_err());
}
