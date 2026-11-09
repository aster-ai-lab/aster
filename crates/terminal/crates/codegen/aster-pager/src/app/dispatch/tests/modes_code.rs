//! Code publishes a single ACP mode for both planning and permissions.

use super::*;
use std::sync::atomic::Ordering;

struct CodeBackendGuard(bool);

impl Drop for CodeBackendGuard {
    fn drop(&mut self) {
        crate::app::ASTER_ACTIVE.store(self.0, Ordering::Release);
    }
}

#[test]
fn code_shift_tab_never_selects_published_plan() {
    let mut app = test_app_with_agent();
    let _guard = CodeBackendGuard(crate::app::ASTER_ACTIVE.swap(true, Ordering::AcqRel));
    {
        let agent = app.agents.get_mut(&AgentId(0)).unwrap();
        agent.available_modes = vec![
            acp::SessionMode::new("default", "Normal"),
            acp::SessionMode::new("ask", "Ask"),
            acp::SessionMode::new("plan", "Plan"),
            acp::SessionMode::new("yolo", "Always approve"),
        ];
    }

    for _ in 0..8 {
        let effects = dispatch(Action::CycleMode, &mut app);
        assert!(
            !effects.is_empty(),
            "Aster Shift+Tab must still cycle when Plan is filtered out"
        );
        for effect in &effects {
            if let Effect::SetSessionMode { mode_id, .. } = effect {
                assert_ne!(
                    mode_id.0.as_ref(),
                    "plan",
                    "Aster Shift+Tab must not select Plan, got {effects:?}"
                );
            }
        }
        assert_ne!(
            test_agent(&app, AgentId(0)).published_mode_label(),
            Some("plan")
        );
        assert!(
            !test_agent(&app, AgentId(0))
                .plan_mode_pending
                .unwrap_or(test_agent(&app, AgentId(0)).plan_mode_active)
        );
    }
}

#[test]
fn code_plan_yolo_exit_preserves_backend_posture() {
    let mut app = test_app_with_agent();
    let _guard = CodeBackendGuard(crate::app::ASTER_ACTIVE.swap(true, Ordering::AcqRel));
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

#[test]
fn code_session_creation_preserves_confirmed_auto_and_accepts_default() {
    let _guard = CodeBackendGuard(crate::app::ASTER_ACTIVE.swap(true, Ordering::AcqRel));
    for mode in ["auto", "default"] {
        let mut app = test_app_with_agent();
        app.current_ui.permission_mode = Some("auto".into());
        let agent = app.agents.get_mut(&AgentId(0)).unwrap();
        agent.session.session_id = None;
        agent.session.auto_mode = true;
        app.apply_aster_slash_gate();
        crate::app::dispatch::downgrade_displayed_auto_if_gated(&mut app);
        let flags = crate::app::event_loop::session_flags_for_effects(&mut app, &[]);
        assert_eq!(
            flags.permission_posture(),
            Some("auto"),
            "explicit launch Auto must survive backend initialization"
        );
        let _ = dispatch(
            Action::TaskComplete(TaskResult::SessionCreated {
                agent_id: AgentId(0),
                session_id: "confirmed-auto-session".into(),
                models: None,
                modes: Some(acp::SessionModeState::new(
                    mode,
                    vec![
                        acp::SessionMode::new("default", "Normal"),
                        acp::SessionMode::new("auto", "Auto"),
                    ],
                )),
            }),
            &mut app,
        );
        let auto = mode == "auto";
        assert_eq!(test_agent(&app, AgentId(0)).session.auto_mode, auto);
        assert_eq!(
            app.current_ui.permission_mode.as_deref(),
            Some(if auto { "auto" } else { "ask" }),
            "the display and next prompt must retain the backend's confirmed mode"
        );
        assert!(!test_agent(&app, AgentId(0)).session.is_yolo());
        let flags = crate::app::event_loop::session_flags_for_effects(&mut app, &[]);
        assert_eq!(flags.permission_posture(), Some(mode));
    }
    let mut app = test_app_with_agent();
    app.auto_mode_gate = false;
    app.current_ui.permission_mode = Some("auto".into());
    app.apply_aster_slash_gate();
    crate::app::dispatch::downgrade_displayed_auto_if_gated(&mut app);
    assert!(!app.auto_mode_gate);
    assert_eq!(app.current_ui.permission_mode.as_deref(), Some("ask"));
}
