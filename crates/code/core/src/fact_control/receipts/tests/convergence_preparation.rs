use super::*;
use std::collections::VecDeque;

struct RepeatedCalls(std::sync::Mutex<VecDeque<aster_effect::ModelDecision>>);

#[derive(Debug)]
struct DenyingInputHook(PathBuf);

#[async_trait::async_trait]
impl crate::hooks::HookExecutor for DenyingInputHook {
    async fn fire(&self, event: &crate::hooks::HookEvent) -> crate::hooks::HookResult {
        if matches!(event, crate::hooks::HookEvent::PreToolUse(_)) {
            std::fs::write(&self.0, b"original hook effect").unwrap();
            return crate::hooks::HookResult::block("controlled input-hook refusal");
        }
        crate::hooks::HookResult::Continue(None)
    }
}

impl aster_effect::Completion for RepeatedCalls {
    fn complete(
        &self,
        _: aster_effect::CompletionRequest,
        _: tokio_util::sync::CancellationToken,
    ) -> aster_effect::coding::BoxFuture<Result<aster_effect::ModelDecision, ActorError>> {
        let next = self.0.lock().unwrap().pop_front().unwrap();
        Box::pin(async move { Ok(next) })
    }
}

#[tokio::test]
async fn scheduler_duplicate_refusal_does_not_run_core_input_hooks() {
    let root = tempfile::tempdir().unwrap();
    let (tools, executions, hook_calls) = argument_validation::schema_tools(root.path(), None);
    let mut script: VecDeque<_> = (0..4)
        .map(|index| aster_effect::ModelDecision::Tool {
            call: request(&format!("call-{index}"), json!({"write":false})).call,
            also: vec![],
        })
        .collect();
    script.push_back(aster_effect::ModelDecision::Text {
        text: "done".into(),
    });
    let services = Arc::new(CodingServices {
        completion: Arc::new(RepeatedCalls(std::sync::Mutex::new(script))),
        tools,
        compactor: Arc::new(NoCompaction),
    });
    let config = aster_effect::HarnessConfig::new(16, usize::MAX, 64, 1, vec![], vec![])
        .unwrap()
        .with_duplicate_threshold(3);
    let settled = aster_effect::ingest_coding(
        &aster_effect::coding_actor(config),
        &aster_effect::MemoryLog::new(),
        services,
        "convergence",
        aster_effect::message_fact("start", "Observe the unchanged source."),
        64,
    )
    .await
    .unwrap();
    assert_eq!(settled.view.assistant.as_deref(), Some("done"));
    assert_eq!(executions.load(Ordering::SeqCst), 3);
    assert_eq!(
        hook_calls.load(Ordering::SeqCst),
        3,
        "the fourth invocation was refused before its input hook"
    );
    let refused = settled
        .log
        .iter()
        .filter(|fact| fact.kind == "tool.result")
        .map(|fact| {
            serde_json::from_value::<ToolExecutionResultV1>(fact.payload["receipt"].clone())
                .unwrap()
        })
        .find(|result| result.identity.tool_call_id == "call-3")
        .unwrap();
    assert_eq!(refused.outcome, ToolExecutionOutcome::Denied);
    assert_eq!(
        refused.effects.completeness,
        ToolObservationCompleteness::Complete
    );
    assert!(refused.effects.changes.is_empty());
    assert!(refused.effects.unresolved_external_operations.is_empty());
}

#[tokio::test]
async fn scheduler_refusal_preserves_original_validation_before_input_hooks() {
    let root = tempfile::tempdir().unwrap();
    let (tools, executions, hooks) = argument_validation::schema_tools(root.path(), None);
    let prepared = tools
        .clone()
        .prepare_with_refusal(
            request("invalid", json!({"write":false,"unknown":true})),
            Some("scheduler duplicate".into()),
            Default::default(),
        )
        .await
        .unwrap();
    let result = execute_prepared(tools, prepared).await;
    assert_eq!(result.outcome, ToolExecutionOutcome::Failed);
    assert!(result
        .output
        .as_str()
        .unwrap()
        .contains("Invalid arguments"));
    assert_eq!(executions.load(Ordering::SeqCst), 0);
    assert_eq!(hooks.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn scheduler_refusal_keeps_the_original_denying_authority() {
    let root = tempfile::tempdir().unwrap();
    let (base, executions, hooks) = argument_validation::schema_tools(root.path(), None);
    let mut tools = base.as_ref().clone();
    tools.permission = crate::permissions::PermissionPolicy {
        default_decision: crate::permissions::PermissionDecision::Deny,
        ..Default::default()
    };
    let tools = Arc::new(tools);
    let prepared = tools
        .clone()
        .prepare_with_refusal(
            request("denied", json!({"write":false})),
            Some("scheduler duplicate".into()),
            Default::default(),
        )
        .await
        .unwrap();
    let result = execute_prepared(tools, prepared).await;
    assert_eq!(result.outcome, ToolExecutionOutcome::Denied);
    assert!(result.output.as_str().unwrap().contains("policy_denied"));
    assert_eq!(executions.load(Ordering::SeqCst), 0);
    assert_eq!(hooks.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn scheduled_hook_refusal_preserves_the_canary_and_blocks_completion_evidence() {
    let root = tempfile::tempdir().unwrap();
    let (base, executions) = executor_tools(root.path());
    let canary = root.path().join("hook-effect.txt");
    let mut tools = base.as_ref().clone();
    tools.agent = Some(crate::agent::AgentLoop::new(
        Arc::new(crate::agent::tests::MockLlmClient::new(Vec::new())),
        Arc::clone(&tools.executor),
        tools.context.clone(),
        crate::agent::AgentConfig {
            hook_engine: Some(Arc::new(DenyingInputHook(canary.clone()))),
            ..Default::default()
        },
    ));
    let services = Arc::new(CodingServices {
        completion: Arc::new(RepeatedCalls(std::sync::Mutex::new(VecDeque::from([
            aster_effect::ModelDecision::Tool {
                call: request("hook-refused", json!({"write":true})).call,
                also: vec![],
            },
            aster_effect::ModelDecision::Text {
                text: "model narrative".into(),
            },
        ])))),
        tools: Arc::new(tools),
        compactor: Arc::new(NoCompaction),
    });
    let settled = aster_effect::ingest_coding(
        &aster_effect::coding_actor(
            aster_effect::HarnessConfig::new(16, usize::MAX, 64, 1, vec![], vec![])
                .unwrap()
                .with_duplicate_threshold(3),
        ),
        &aster_effect::MemoryLog::new(),
        services,
        "hook-refusal",
        aster_effect::message_fact("start", "Perform the admitted action."),
        64,
    )
    .await
    .unwrap();
    assert!(settled.view.schema_error.is_none());
    assert_eq!(std::fs::read(canary).unwrap(), b"original hook effect");
    assert_eq!(executions.load(Ordering::SeqCst), 0);
    let result: ToolExecutionResultV1 = serde_json::from_value(
        settled
            .log
            .iter()
            .find(|fact| fact.kind == "tool.result")
            .unwrap()
            .payload["receipt"]
            .clone(),
    )
    .unwrap();
    assert_eq!(result.outcome, ToolExecutionOutcome::Denied);
    assert_eq!(
        result.effects.completeness,
        ToolObservationCompleteness::Incomplete
    );
    assert!(!fold_cache_evidence(&settled.log).unresolved.is_empty());
}
