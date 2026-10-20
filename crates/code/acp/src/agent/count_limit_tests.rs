#![cfg(test)]

use super::*;

fn interactive_options(effort: Option<&str>, configured_limit: Option<usize>) -> SessionOptions {
    let workspace = tempfile::tempdir().unwrap();
    let setup = SessionSetup {
        workspace: workspace.path().to_owned(),
        model_id: "controlled/model",
        effort: effort.map(|token| effort_limits(token).unwrap()),
        host: Default::default(),
        components: &[],
        mcp_servers: Vec::new(),
    };
    let options = A3sCodeAgent::core_session_options("count-limit", setup.model_id, setup.effort);
    let mut slots = options.prompt_slots.clone().unwrap_or_default();
    A3sCodeAgent::host_session_options_with_hook_roots(
        options,
        &mut slots,
        &setup,
        &a3s_code_core::CodeConfig {
            max_tool_rounds: configured_limit,
            ..Default::default()
        },
        &workspace.path().join("store"),
        None,
        None,
        None,
    )
    .unwrap()
}

fn all_efforts() -> impl Iterator<Item = Option<&'static str>> {
    [None].into_iter().chain(
        [
            "none",
            "minimal",
            "low",
            "medium",
            "high",
            "xhigh",
            "max",
            "ultracode",
        ]
        .into_iter()
        .map(Some),
    )
}

#[test]
fn interactive_count_is_unbounded_without_an_explicit_limit_for_every_effort() {
    for effort in all_efforts() {
        let options = interactive_options(effort, None);
        assert_eq!(options.max_tool_rounds, Some(0), "effort {effort:?}");
        let guidelines = options.prompt_slots.unwrap().guidelines.unwrap();
        assert!(guidelines.contains("Tool calls have no count limit."));
        assert!(!guidelines.contains("tool_rounds="));
        match effort {
            Some(token) => {
                let limits = effort_limits(token).unwrap();
                assert_eq!(options.reasoning_effort.as_deref(), Some(limits.token));
                assert_eq!(options.thinking_budget, Some(limits.thinking_budget));
            }
            None => {
                assert!(options.reasoning_effort.is_none());
                assert!(options.thinking_budget.is_none());
            }
        }
    }
}

#[test]
fn interactive_explicit_acl_count_wins_over_every_effort_including_zero() {
    for configured_limit in [0, 1, 7] {
        for effort in all_efforts() {
            let options = interactive_options(effort, Some(configured_limit));
            assert_eq!(
                options.max_tool_rounds,
                Some(configured_limit),
                "configured count {configured_limit}, effort {effort:?}"
            );
            let guidelines = options.prompt_slots.unwrap().guidelines.unwrap();
            if configured_limit == 0 {
                assert!(guidelines.contains("Tool calls have no count limit."));
                assert!(!guidelines.contains("tool_rounds="));
            } else {
                assert!(guidelines.contains(&format!("tool_rounds={configured_limit}")));
                assert!(!guidelines.contains("Tool calls have no count limit."));
            }
        }
    }
}
