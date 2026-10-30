use super::*;

#[test]
fn code_plan_permission_toggle_does_not_reissue_plan_mode() {
    assert!(permission_toggle_request(Some(acp::SessionId::new("s")), "plan").is_none());
    for posture in ["default", "auto", "yolo"] {
        let request = permission_toggle_request(Some(acp::SessionId::new("s")), posture).unwrap();
        assert_eq!(request.session_id.0.as_ref(), "s");
        assert_eq!(request.mode_id.0.as_ref(), posture);
    }
    assert!(permission_toggle_request(None, "yolo").is_none());
}
