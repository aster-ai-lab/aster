#![cfg(test)]

use super::*;

struct CountProbe(Arc<AtomicUsize>);

#[async_trait]
impl crate::tools::Tool for CountProbe {
    fn name(&self) -> &str {
        "count_probe"
    }
    fn description(&self) -> &str {
        "Return one controlled observation"
    }
    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({"type":"object","properties":{"index":{"type":"integer"}},"required":["index"],"additionalProperties":false})
    }
    fn capabilities(&self, _: &serde_json::Value) -> crate::tools::ToolCapabilities {
        crate::tools::ToolCapabilities::read_only_paginated(1)
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        _: &crate::tools::ToolContext,
    ) -> anyhow::Result<crate::tools::ToolOutput> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(crate::tools::ToolOutput::success(args["index"].to_string()))
    }
}

async fn counted_session(
    round_limit: usize,
    harness: Option<crate::HarnessComposeOptions>,
) -> (crate::agent::AgentResult, usize, Vec<a3s_effect::Fact>) {
    let workspace = tempfile::tempdir().unwrap();
    let mut config = test_config();
    config.memory = Some(crate::memory::MemoryConfig {
        llm_extraction: false,
        ..Default::default()
    });
    let agent = crate::Agent::from_config(config).await.unwrap();
    let mut responses: Vec<_> = (0..64)
        .map(|index| {
            tool_response(
                &format!("probe-{index}"),
                "count_probe",
                serde_json::json!({"index":index}),
            )
        })
        .collect();
    responses.push(text_response("All 64 observations are complete."));
    let mut options = crate::SessionOptions::new()
        .with_session_id("no-count-limit")
        .with_max_tool_rounds(round_limit)
        .with_permission_policy(crate::permissions::PermissionPolicy::new().allow("*"))
        .with_auto_delegation_enabled(false)
        .with_manual_delegation_enabled(false)
        .with_continuation(false)
        .with_planning_mode(crate::PlanningMode::Disabled);
    if let Some(harness) = harness {
        options = options
            .with_harness(harness)
            .with_host_harness_registry(Arc::new(crate::meta_harness::BuiltinHostHarnessRegistry));
    }
    let built = agent.build_session(
        workspace.path().display().to_string(),
        Arc::new(ScriptedClient {
            seen: Default::default(),
            responses: std::sync::Mutex::new(responses),
        }),
        &options,
    );
    let session = match built {
        Ok(session) => session,
        Err(error) => {
            agent.close().await;
            panic!("a zero tool-count setting must admit an unbounded session: {error}");
        }
    };
    let calls = Arc::new(AtomicUsize::new(0));
    session
        .register_dynamic_tool(Arc::new(CountProbe(calls.clone())))
        .unwrap();
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(20),
        session.send(
            "Collect each of the 64 controlled observations, then report completion.",
            None,
        ),
    )
    .await;
    let facts = read_workspace_facts(workspace.path(), "no-count-limit");
    session.close().await;
    agent.close().await;
    (
        result.unwrap().unwrap(),
        calls.load(Ordering::SeqCst),
        facts.unwrap(),
    )
}

#[tokio::test]
async fn explicit_no_count_limit_runs_beyond_fifty_and_completes_normally() {
    let (result, calls, facts) = counted_session(0, None).await;
    assert_eq!(calls, 64);
    assert_eq!(result.text, "All 64 observations are complete.");
    assert_eq!(result.forced_stop, None);
    assert_eq!(
        facts
            .iter()
            .filter(|fact| fact.kind == "tool.result")
            .count(),
        64
    );
    assert!(!facts.iter().any(|fact| fact.kind == "budget.denied"));
}

#[tokio::test]
async fn explicit_positive_tool_count_still_refuses_the_next_call() {
    let (result, calls, facts) = counted_session(3, None).await;
    assert_eq!(calls, 3);
    assert_eq!(
        result.forced_stop,
        Some(crate::harness_loop::ForcedStop::ToolBudget)
    );
    assert_eq!(
        facts
            .iter()
            .filter(|fact| fact.kind == "tool.result")
            .count(),
        3
    );
    assert_eq!(
        facts
            .iter()
            .filter(|fact| fact.kind == "budget.denied")
            .count(),
        1
    );
}

#[tokio::test]
async fn stock_and_host_composition_preserve_an_absent_count_ceiling() {
    for components in [
        vec!["system", "tools", "budget", "compact", "infer"],
        vec![
            "system",
            "host:intent_stamp",
            "tools",
            "budget",
            "compact",
            "infer",
        ],
    ] {
        let harness = crate::HarnessComposeOptions::compose(
            components.into_iter().map(str::to_string).collect(),
            None,
            None,
            Vec::new(),
        )
        .unwrap();
        let (result, calls, facts) = counted_session(0, Some(harness)).await;
        assert_eq!(calls, 64);
        assert_eq!(result.forced_stop, None);
        assert!(!facts.iter().any(|fact| fact.kind == "budget.denied"));
    }
}

#[tokio::test]
async fn an_explicit_composed_zero_allowance_is_not_an_absent_ceiling() {
    let harness = crate::HarnessComposeOptions::compose(
        vec!["tools".into(), "budget".into(), "infer".into()],
        Some(0),
        None,
        Vec::new(),
    )
    .unwrap();
    let (result, calls, facts) = counted_session(0, Some(harness)).await;
    assert_eq!(calls, 0);
    assert_eq!(
        result.forced_stop,
        Some(crate::harness_loop::ForcedStop::ToolBudget)
    );
    assert!(!facts.iter().any(|fact| fact.kind == "tool.result"));
}

#[tokio::test]
async fn explicit_composed_positive_allowance_still_bounds_an_unlimited_session() {
    let harness = crate::HarnessComposeOptions::compose(
        vec!["tools".into(), "budget".into(), "infer".into()],
        Some(3),
        None,
        Vec::new(),
    )
    .unwrap();
    let (result, calls, _) = counted_session(0, Some(harness)).await;
    assert_eq!(calls, 3);
    assert_eq!(
        result.forced_stop,
        Some(crate::harness_loop::ForcedStop::ToolBudget)
    );
}
