use super::*;
use crate::agent::AgentEvent;
use crate::subagent_task_tracker::{SubagentStatus, SubagentTaskSnapshot};
use tokio_util::sync::CancellationToken;

async fn running() -> (
    Arc<InMemorySubagentTaskTracker>,
    CancellationToken,
    ToolContext,
) {
    let tracker = Arc::new(InMemorySubagentTaskTracker::new());
    tracker
        .record_event(&AgentEvent::SubagentStart {
            task_id: "task-original".into(),
            session_id: "child-original".into(),
            parent_session_id: "parent-original".into(),
            agent: "explore".into(),
            description: "Read the original file".into(),
            started_ms: 1,
        })
        .await;
    let token = CancellationToken::new();
    tracker
        .register_canceller("task-original", token.clone())
        .await;
    (
        tracker,
        token,
        ToolContext::new(std::path::PathBuf::from(".")).with_session_id("parent-original"),
    )
}

async fn finish(tracker: &InMemorySubagentTaskTracker) {
    tracker
        .record_event(&AgentEvent::SubagentEnd {
            task_id: "task-original".into(),
            session_id: "child-original".into(),
            agent: "explore".into(),
            output: "ORIGINAL_RESULT".into(),
            success: true,
            finished_ms: 2,
        })
        .await;
    tracker.clear_canceller("task-original").await;
}

#[tokio::test]
async fn output_polls_and_then_waits_for_the_same_original_task() {
    let (tracker, token, context) = running().await;
    let tool = TaskControlTool::Output(Arc::clone(&tracker));
    let poll = tool
        .execute(&json!({"task_id":"task-original"}), &context)
        .await
        .unwrap();
    let snapshot: SubagentTaskSnapshot = serde_json::from_str(&poll.content).unwrap();
    assert_eq!(snapshot.status, SubagentStatus::Running);
    let join = tokio::spawn({
        let tracker = Arc::clone(&tracker);
        async move {
            TaskControlTool::Output(tracker)
                .execute(
                    &json!({"task_id":"task-original","timeout_ms":10000}),
                    &context,
                )
                .await
        }
    });
    tokio::task::yield_now().await;
    assert!(!join.is_finished());
    finish(&tracker).await;
    let output = tokio::time::timeout(Duration::from_secs(1), join)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let snapshot: SubagentTaskSnapshot = serde_json::from_str(&output.content).unwrap();
    assert_eq!(snapshot.status, SubagentStatus::Completed);
    assert_eq!(snapshot.output.as_deref(), Some("ORIGINAL_RESULT"));
    assert!(!token.is_cancelled());
    assert_eq!(tracker.list().await.len(), 1);
}

#[tokio::test]
async fn wait_timeout_and_wait_cancellation_leave_the_child_running() {
    let (tracker, child_token, context) = running().await;
    let tool = TaskControlTool::Output(Arc::clone(&tracker));
    let output = tool
        .execute(&json!({"task_id":"task-original","timeout_ms":1}), &context)
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_str::<SubagentTaskSnapshot>(&output.content)
            .unwrap()
            .status,
        SubagentStatus::Running
    );
    let wait_token = CancellationToken::new();
    let context = context.with_cancellation(wait_token.clone());
    let join = tokio::spawn(async move {
        tool.execute(
            &json!({"task_id":"task-original","timeout_ms":10000}),
            &context,
        )
        .await
    });
    tokio::task::yield_now().await;
    wait_token.cancel();
    assert!(tokio::time::timeout(Duration::from_secs(1), join)
        .await
        .unwrap()
        .unwrap()
        .is_err());
    assert!(!child_token.is_cancelled());
    assert_eq!(
        tracker.get("task-original").await.unwrap().status,
        SubagentStatus::Running
    );
}

#[tokio::test]
async fn query_and_stop_refuse_foreign_tasks_without_exposing_or_cancelling_them() {
    let (tracker, token, context) = running().await;
    let foreign = context.with_session_id("other-parent");
    for tool in [
        TaskControlTool::Output(Arc::clone(&tracker)),
        TaskControlTool::Stop(Arc::clone(&tracker)),
    ] {
        let output = tool
            .execute(&json!({"task_id":"task-original"}), &foreign)
            .await
            .unwrap();
        assert!(!output.success);
        assert!(!output.content.contains("ORIGINAL_RESULT"));
        assert!(!output.content.contains("child-original"));
    }
    assert!(!token.is_cancelled());
}

#[tokio::test]
async fn stop_requests_original_cancellation_and_late_end_preserves_cancelled() {
    let (tracker, token, context) = running().await;
    let tool = TaskControlTool::Stop(Arc::clone(&tracker));
    let output = tool
        .execute(&json!({"task_id":"task-original"}), &context)
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&output.content).unwrap()["cancellation_requested"],
        true
    );
    assert!(token.is_cancelled());
    let task = tracker.get("task-original").await.unwrap();
    assert_eq!(task.status, SubagentStatus::Cancelled);
    assert!(task.finished_ms.is_none());
    finish(&tracker).await;
    assert_eq!(
        tracker.get("task-original").await.unwrap().status,
        SubagentStatus::Cancelled
    );
    let output = tool
        .execute(&json!({"task_id":"task-original"}), &context)
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&output.content).unwrap()["cancellation_requested"],
        false
    );
}

#[tokio::test]
async fn task_control_rejects_out_of_contract_input_before_mutating_the_tracker() {
    let (tracker, token, context) = running().await;
    let output = TaskControlTool::Output(Arc::clone(&tracker));
    for args in [
        json!({"task_id":"task-original","timeout_ms":300001}),
        json!({"task_id":"task-original","timeout_ms":-1}),
        json!({"task_id":""}),
        json!({"task_id":"x".repeat(129)}),
        json!({"task_id":"task-original","relaunch":true}),
    ] {
        assert!(output.execute(&args, &context).await.is_err());
    }
    assert!(TaskControlTool::Stop(tracker)
        .execute(&json!({"task_id":"task-original","timeout_ms":1}), &context)
        .await
        .is_err());
    assert!(!token.is_cancelled());
}
