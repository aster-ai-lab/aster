use super::*;

fn host_confirmation_policy() -> a3s_code_core::hitl::ConfirmationPolicy {
    let workspace = tempfile::tempdir().unwrap();
    let setup = SessionSetup {
        workspace: workspace.path().to_owned(),
        model_id: "controlled/model",
        effort: None,
        host: Default::default(),
        components: &[],
        mcp_servers: Vec::new(),
    };
    let mut slots = a3s_code_core::SystemPromptSlots::default();
    A3sCodeAgent::host_session_options_with_hook_roots(
        a3s_code_core::SessionOptions::new(),
        &mut slots,
        &setup,
        &a3s_code_core::CodeConfig::default(),
        &workspace.path().join("store"),
        None,
        None,
        None,
    )
    .unwrap()
    .confirmation_policy
    .unwrap()
}

#[tokio::test]
async fn host_tool_confirmation_policy_uses_the_existing_human_ask_deadline() {
    ask_env_locked(|| async {
        let expected_ms =
            u64::try_from(permission_ask::ask_deadline().as_millis()).unwrap_or(u64::MAX);
        let policy = host_confirmation_policy();
        assert!(policy.enabled);
        assert!(matches!(
            policy.timeout_action,
            a3s_code_core::hitl::TimeoutAction::Reject
        ));
        assert_eq!(
            policy.default_timeout_ms, expected_ms,
            "the tool-owned inner wait and existing human ACP ask must share a deadline"
        );
    })
    .await;
}

#[tokio::test]
async fn human_approval_deadline_preserves_default_disable_and_large_value_semantics() {
    ask_env_locked(|| async {
        for (configured, expected_ms) in [
            (None, 1_800_000),
            (Some("1"), 1_000),
            (Some("0"), u64::MAX),
            (Some("18446744073709551615"), u64::MAX),
            (Some("invalid"), 1_800_000),
        ] {
            match configured {
                Some(value) => std::env::set_var("A3S_CODE_PERMISSION_ASK_TIMEOUT_SECS", value),
                None => std::env::remove_var("A3S_CODE_PERMISSION_ASK_TIMEOUT_SECS"),
            }
            let policy = host_confirmation_policy();
            assert!(policy.enabled);
            assert!(matches!(
                policy.timeout_action,
                a3s_code_core::hitl::TimeoutAction::Reject
            ));
            assert_eq!(
                policy.default_timeout_ms, expected_ms,
                "configured deadline {configured:?}"
            );
        }
    })
    .await;
}
