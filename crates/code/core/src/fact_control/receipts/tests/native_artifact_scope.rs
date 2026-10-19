use super::*;
use crate::tools::{Tool, ToolCapabilities, ToolContext, ToolOutput};

struct ScopeProbe {
    barrier: tokio::sync::Barrier,
    observations: Mutex<Vec<(Vec<String>, Vec<String>)>>,
}

#[async_trait::async_trait]
impl Tool for ScopeProbe {
    fn name(&self) -> &str {
        "native_scope_probe"
    }
    fn description(&self) -> &str {
        "Observe private prepared-call evidence isolation"
    }
    fn parameters(&self) -> serde_json::Value {
        json!({"type":"object","properties":{"marker":{"type":"string"}},"required":["marker"],"additionalProperties":false})
    }
    fn capabilities(&self, _: &serde_json::Value) -> ToolCapabilities {
        ToolCapabilities::parallel_safe_read(2)
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        let scope = ctx
            .native_artifact_verifications
            .as_ref()
            .expect("prepared owner must install evidence scope");
        let before = {
            let mut scope = scope.lock().unwrap();
            let before = scope.paths().map(str::to_owned).collect::<Vec<_>>();
            scope.observe_unseen_paths(&[args["marker"].as_str().unwrap().into()]);
            before
        };
        self.barrier.wait().await;
        let after = scope.lock().unwrap().paths().map(str::to_owned).collect();
        self.observations.lock().unwrap().push((before, after));
        Ok(ToolOutput::success("observed"))
    }
}

#[tokio::test]
async fn parallel_prepared_invocations_never_inherit_or_share_native_evidence() {
    let root = tempfile::tempdir().unwrap();
    let (base, _) = executor_tools(root.path());
    let mut tools = base.as_ref().clone();
    tools.context = tools.context.with_native_artifact_verifications();
    tools
        .context
        .native_artifact_verifications
        .as_ref()
        .unwrap()
        .lock()
        .unwrap()
        .observe_unseen_paths(&["earlier-owner.png".into()]);
    let probe = Arc::new(ScopeProbe {
        barrier: tokio::sync::Barrier::new(2),
        observations: Mutex::new(vec![]),
    });
    tools.executor.register_dynamic_tool(probe.clone());
    let tools = Arc::new(tools);
    let mut first = request("first-owner", json!({"marker":"first.png"}));
    first.call.name = "native_scope_probe".into();
    let mut second = request("second-owner", json!({"marker":"second.png"}));
    second.call.name = "native_scope_probe".into();
    let first = prepare_tools(tools.clone(), first, Default::default())
        .await
        .unwrap();
    let second = prepare_tools(tools.clone(), second, Default::default())
        .await
        .unwrap();
    let (first, second) = tokio::time::timeout(std::time::Duration::from_secs(2), async {
        tokio::join!(
            execute_prepared(tools.clone(), first),
            execute_prepared(tools.clone(), second)
        )
    })
    .await
    .expect("both prepared read-only calls must run concurrently");
    assert_eq!(first.outcome, ToolExecutionOutcome::Succeeded);
    assert_eq!(second.outcome, ToolExecutionOutcome::Succeeded);
    let mut observed = probe.observations.lock().unwrap().clone();
    observed.sort();
    assert_eq!(
        observed,
        vec![
            (vec![], vec!["first.png".into()]),
            (vec![], vec!["second.png".into()])
        ]
    );
    assert_eq!(
        tools
            .context
            .native_artifact_verifications
            .as_ref()
            .unwrap()
            .lock()
            .unwrap()
            .paths()
            .collect::<Vec<_>>(),
        ["earlier-owner.png"]
    );
}
