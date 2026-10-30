use super::*;
use serde_json::json;

#[test]
fn postures_separate_workspace_approval_from_host_execution() {
    for posture in ["default", "plan", "auto", "yolo"] {
        let policy = policy_for_posture(posture);
        for (tool, args) in [
            ("read", json!({"file_path":"README.md"})),
            ("office_read", json!({"file_path":"source.xlsx"})),
            (
                "output_check",
                json!({"action":"text_length","text":"内容"}),
            ),
            ("ask_user_form", json!({})),
        ] {
            assert_eq!(
                policy.check(tool, &args),
                PermissionDecision::Allow,
                "{posture}: {tool}"
            );
        }
        let expected = match posture {
            "plan" => PermissionDecision::Deny,
            "default" => PermissionDecision::Ask,
            _ => PermissionDecision::Allow,
        };
        assert_eq!(
            policy.check(
                "write",
                &json!({"file_path":"result.txt","content":"result"})
            ),
            expected
        );
        assert_eq!(
            policy.check("bash", &json!({"command":"cargo test"})),
            expected
        );
        assert_eq!(policy.allows_unsandboxed_execution(), posture == "yolo");
        assert_eq!(
            policy
                .deny("bash")
                .check("bash", &json!({"command":"cargo test"})),
            PermissionDecision::Deny
        );
    }
}

#[tokio::test]
async fn only_yolo_can_waive_the_exact_host_execution_prompt() {
    use crate::hitl::{ConfirmationManager, ConfirmationPolicy};
    use crate::safety_gate::{ToolGateDecision, ToolGateInput, ToolSafetyGate};
    use std::sync::Arc;
    let args = json!({"command":"git status", "sandbox_permissions":"require_escalated", "justification":"Read host repository state"});
    for posture in ["default", "plan", "auto", "yolo"] {
        let mut config = crate::agent::AgentConfig::default();
        config.permission_checker = Some(Arc::new(policy_for_posture(posture)));
        config.confirmation_manager = Some(Arc::new(ConfirmationManager::new(
            ConfirmationPolicy::enabled(),
            tokio::sync::broadcast::channel(8).0,
        )));
        let decision = ToolSafetyGate::new(&config)
            .decide(ToolGateInput {
                tool_name: "bash",
                args: &args,
                pre_tool_denial: None,
                tool_requires_confirmation: true,
            })
            .await;
        match posture {
            "plan" => assert!(matches!(decision, ToolGateDecision::Deny { .. })),
            "yolo" => assert!(matches!(decision, ToolGateDecision::Execute { .. })),
            _ => assert!(matches!(decision, ToolGateDecision::Confirm { .. })),
        }
    }
}
