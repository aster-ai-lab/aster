use super::*;
use a3s_code_tools::types::SessionMode;

#[test]
fn earlier_plan_confirmation_preserves_pending_exit() {
    let mut app = make_app_with_agent("sess-plan");
    let agent = app.agents.get_mut(&AgentId(0)).unwrap();
    agent.plan_mode_active = true;
    agent.session_mode = SessionMode::Plan;
    agent.stage_session_mode(SessionMode::Default);

    detect_plan_mode_change_replayed(&make_current_mode_update("plan"), agent, false);

    assert!(
        agent.plan_mode_active,
        "confirmed state records the earlier response"
    );
    assert_eq!(
        agent.plan_mode_pending,
        Some(false),
        "an earlier response must not replace the user's pending exit"
    );
    assert_eq!(agent.effective_session_mode(), SessionMode::Default);

    detect_plan_mode_change_replayed(&make_current_mode_update("default"), agent, false);

    assert!(!agent.plan_mode_active);
    assert_eq!(agent.plan_mode_pending, None);
    assert_eq!(agent.effective_session_mode(), SessionMode::Default);
}
