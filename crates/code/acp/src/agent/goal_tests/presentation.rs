use super::*;
use crate::goal::{apply_bound_goal, GoalLanguage, GoalOp};

#[test]
fn goal_wait_message_speaks_to_the_user_instead_of_printing_a_decision_code() {
    assert_eq!(
        GoalLanguage::English.decision(aster_core::goal_control::GoalTickDecisionV1::NeedsInput),
        "Your input is needed to continue."
    );
}

#[tokio::test]
async fn chinese_goal_request_controls_host_summary_and_planning_language() {
    let root = tempfile::tempdir().unwrap();
    let (launch, provider) =
        super::planning::launch(root.path().join("store"), super::planning::SIMPLE_PLAN).await;
    let host = AsterCodeAgent::new(launch);
    let id = host
        .new_session(acp::NewSessionRequest::new(root.path()))
        .await
        .unwrap()
        .session_id;
    let session = host.sessions.lock().await[&id.0.to_string()]
        .session
        .clone();
    let reply = apply_bound_goal(
        &session,
        &GoalOp::Set("准备好 agent-desktop 环境".into()),
        GoalLanguage::English,
    )
    .await
    .unwrap()
    .unwrap();
    let requests = provider.await.unwrap();
    assert_eq!(session.reply_language().as_deref(), Some("zh-CN"));
    assert!(requests[0]["system"].to_string().contains("Use zh-CN only"));
    assert!(reply.reply.starts_with("目标"), "{}", reply.reply);
    assert!(!reply.reply.contains("Durable goal"));
    assert!(!reply.reply.contains("r1"));
    assert!(!reply.reply.contains("acp-tick-"));
    assert!(!reply.reply.contains("目标已完成"));
    session.close().await;
}
