//! Code publishes a single ACP mode for both planning and permissions.

use super::*;
use std::sync::atomic::Ordering;

struct CodeBackendGuard(bool);

impl Drop for CodeBackendGuard {
    fn drop(&mut self) {
        crate::app::A3S_CODE_ACTIVE.store(self.0, Ordering::Release);
    }
}

#[test]
fn code_plan_yolo_exit_preserves_backend_posture() {
    let mut app = test_app_with_agent();
    let _guard = CodeBackendGuard(crate::app::A3S_CODE_ACTIVE.swap(true, Ordering::AcqRel));
    app.default_yolo = true;
    app.current_ui.permission_mode = Some("yolo".into());
    {
        let agent = app.agents.get_mut(&AgentId(0)).unwrap();
        agent.available_modes = vec![
            acp::SessionMode::new("default", "Normal"),
            acp::SessionMode::new("plan", "Plan"),
            acp::SessionMode::new("yolo", "Always approve"),
        ];
        agent.session.yolo_mode = true;
        agent.plan_mode_pending = Some(true);
    }

    let effects = dispatch(Action::CycleMode, &mut app);

    let agent = test_agent(&app, AgentId(0));
    assert_eq!(agent.plan_mode_pending, Some(false));
    assert!(agent.session.is_yolo());
    assert_eq!(app.current_ui.permission_mode.as_deref(), Some("yolo"));
    assert!(
        matches!(effects.as_slice(), [Effect::SetSessionMode { mode_id, .. }] if mode_id.0.as_ref() == "yolo"),
        "Code must confirm the retained YOLO posture, got {effects:?}"
    );
}
