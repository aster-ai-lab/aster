#![cfg(test)]

use super::*;
use crate::agent::AgentEvent;

#[tokio::test]
#[cfg(unix)]
async fn host_cancellation_retains_an_observed_terminal() {
    for sandboxed_host in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let (base, _) = executor_tools(root.path());
        let mut tools = base.as_ref().clone();
        if sandboxed_host {
            let runner = tools.context.workspace_services.command_runner().unwrap();
            tools.context = tools.context.with_sandbox(Arc::new(
                crate::sandbox::host_access::HostAccessSandbox { runner },
            ));
        }
        let tools = Arc::new(tools);
        let mut call = request(
            "owned-process",
            json!({
                "command":"printf partial > changed.txt; (touch child-started; sleep 0.6; printf leaked > late.txt) & wait",
                "timeout":5000,
            }),
        );
        call.call.name = "bash".into();
        if !sandboxed_host {
            call.call.args["sandbox_permissions"] = "require_escalated".into();
            call.call.args["justification"] =
                "Exercise the original owned host command cancellation".into();
            call.call.needs_confirmation = true;
        }
        let prepared = prepare_tools(Arc::clone(&tools), call, Default::default())
            .await
            .unwrap();
        let identity = prepared.prepared.clone();
        let execution = tokio::spawn(execute_prepared(Arc::clone(&tools), prepared));
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while !root.path().join("child-started").exists() {
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        tools.context.cancellation_token().cancel();
        let result = tokio::time::timeout(std::time::Duration::from_secs(2), execution)
            .await
            .unwrap()
            .unwrap();
        result.ensure_prepared(&identity).unwrap();
        assert_eq!(result.outcome, ToolExecutionOutcome::Cancelled);
        assert_eq!(result.exit_code, Some(137));
        assert_eq!(
            result.effects.completeness,
            ToolObservationCompleteness::Complete
        );
        assert!(result.effects.unresolved_external_operations.is_empty());
        assert!(result
            .effects
            .changes
            .iter()
            .any(|change| change.path == "changed.txt"));
        assert_eq!(
            std::fs::read(root.path().join("changed.txt")).unwrap(),
            b"partial"
        );
        tools
            .ledger
            .as_ref()
            .unwrap()
            .lock()
            .unwrap()
            .observe_tool_receipt("bash", &result);
        assert!(!tools
            .ledger
            .as_ref()
            .unwrap()
            .lock()
            .unwrap()
            .has_uncertain_tool_invocations());
        let mut next = tools.as_ref().clone();
        next.context = next.context.with_cancellation(Default::default());
        let next = Arc::new(next);
        let mut request = request("next", json!({"write":true}));
        request.identity.turn = 2;
        let prepared = prepare_tools(Arc::clone(&next), request, Default::default())
            .await
            .unwrap();
        assert_eq!(
            execute_prepared(next, prepared).await.outcome,
            ToolExecutionOutcome::Succeeded
        );
        assert_eq!(
            std::fs::read(root.path().join("changed.txt")).unwrap(),
            b"exact bytes"
        );
        tokio::time::sleep(std::time::Duration::from_millis(700)).await;
        assert!(!root.path().join("late.txt").exists());
    }
}

#[tokio::test]
async fn metadata_cannot_claim_native_cancellation_evidence() {
    let root = tempfile::tempdir().unwrap();
    let (tools, calls) = executor_tools(root.path());
    let prepared = prepare_tools(
        Arc::clone(&tools),
        request("forged", json!({"write":true,"forged_cancellation":true})),
        Default::default(),
    )
    .await
    .unwrap();
    let result = execute_prepared(Arc::clone(&tools), prepared).await;
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        std::fs::read(root.path().join("changed.txt")).unwrap(),
        b"exact bytes"
    );
    assert_eq!(result.outcome, ToolExecutionOutcome::Cancelled);
    assert_eq!(
        result.effects.completeness,
        ToolObservationCompleteness::Incomplete
    );
    assert_eq!(
        result.effects.unresolved_external_operations,
        vec![result.identity.key().unwrap()]
    );
}

#[tokio::test]
async fn empty_hooks_do_not_create_refusal_debt() {
    for command_hooks in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let (base, calls) = executor_tools(root.path());
        let hook: Arc<dyn crate::hooks::HookExecutor> = if command_hooks {
            crate::hooks::CommandHookExecutor::discover(
                root.path(),
                None,
                root.path().join("trust.json"),
            )
            .unwrap()
        } else {
            Arc::new(crate::hooks::HookEngine::new())
        };
        let mut tools = base.as_ref().clone();
        tools.agent = Some(crate::agent::AgentLoop::new(
            Arc::new(crate::agent::tests::MockLlmClient::new(Vec::new())),
            Arc::clone(&tools.executor),
            tools.context.clone(),
            crate::agent::AgentConfig {
                hook_engine: Some(hook),
                ..Default::default()
            },
        ));
        let tools = Arc::new(tools);
        let ledger = tools.ledger.as_ref().unwrap();
        ledger.lock().unwrap().settle_prior_turns(1);
        ledger
            .lock()
            .unwrap()
            .set_unresolved_tool_effects(std::collections::BTreeSet::from([
                "current-uncertainty".into()
            ]));
        let prepared = prepare_tools(
            Arc::clone(&tools),
            request("refused", json!({"write":true})),
            Default::default(),
        )
        .await
        .unwrap();
        let result = execute_prepared(Arc::clone(&tools), prepared).await;
        assert_eq!(result.outcome, ToolExecutionOutcome::Denied);
        assert_eq!(
            result.effects.completeness,
            ToolObservationCompleteness::Complete
        );
        assert!(result.effects.unresolved_external_operations.is_empty());
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert!(!root.path().join("changed.txt").exists());
        assert_eq!(
            ledger.lock().unwrap().checkpoint_uncertainty_keys(),
            &std::collections::BTreeSet::from(["current-uncertainty".into()])
        );
    }
}

#[tokio::test]
async fn actual_hook_effects_remain_uncertain_after_refusal() {
    let root = tempfile::tempdir().unwrap();
    let (tools, calls, hooks) = super::argument_validation::schema_tools(root.path(), None);
    let ledger = tools.ledger.as_ref().unwrap();
    ledger.lock().unwrap().settle_prior_turns(1);
    ledger
        .lock()
        .unwrap()
        .set_unresolved_tool_effects(std::collections::BTreeSet::from([
            "current-uncertainty".into()
        ]));
    let prepared = prepare_tools(
        Arc::clone(&tools),
        request("refused-after-hook", json!({"write":true})),
        Default::default(),
    )
    .await
    .unwrap();
    let result = execute_prepared(Arc::clone(&tools), prepared).await;
    assert_eq!(result.outcome, ToolExecutionOutcome::Denied);
    assert_eq!(
        result.effects.completeness,
        ToolObservationCompleteness::Incomplete
    );
    assert_eq!(
        result.effects.unresolved_external_operations,
        vec![result.identity.key().unwrap()]
    );
    assert_eq!(
        std::fs::read(root.path().join("hook-effect.txt")).unwrap(),
        b"actual input-hook effect"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(hooks.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn settled_tool_receipts_survive_cancellation() {
    for (failed, incomplete) in [(false, false), (true, false), (false, true)] {
        let root = tempfile::tempdir().unwrap();
        let (tools, calls) = executor_tools(root.path());
        let prepared = prepare_tools(
            Arc::clone(&tools),
            request(
                "settled-after-cancel",
                json!({"settle_on_cancel": true, "write": true,
                    "fail": failed, "incomplete_evidence": incomplete}),
            ),
            Default::default(),
        )
        .await
        .unwrap();
        let original = prepared.prepared.clone();
        let cancellation = tools.context.cancellation_token();
        let invocation = tokio::spawn(execute_prepared(Arc::clone(&tools), prepared));
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while calls.load(Ordering::SeqCst) == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert!(!root.path().join("changed.txt").exists());
        cancellation.cancel();
        let result = tokio::time::timeout(std::time::Duration::from_secs(2), invocation)
            .await
            .unwrap()
            .unwrap();
        result.ensure_prepared(&original).unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            std::fs::read(root.path().join("changed.txt")).unwrap(),
            b"exact bytes"
        );
        assert_eq!(
            result.outcome,
            if failed {
                ToolExecutionOutcome::Failed
            } else {
                ToolExecutionOutcome::Succeeded
            }
        );
        assert!(result
            .effects
            .changes
            .iter()
            .any(|change| change.path == "changed.txt"));
        assert_eq!(
            !result.effects.unresolved_external_operations.is_empty(),
            incomplete
        );
        assert_eq!(
            tools
                .ledger
                .as_ref()
                .unwrap()
                .lock()
                .unwrap()
                .has_uncertain_tool_invocations(),
            incomplete
        );
    }
}

#[tokio::test]
async fn tool_event_backpressure_ends_on_the_exact_run_token_and_keeps_fifo_order() {
    let root = tempfile::tempdir().unwrap();
    let (mut tools, _) = executor_tools(root.path());
    let (events, mut receiver) = tokio::sync::mpsc::channel(1);
    events
        .send(AgentEvent::TextDelta {
            text: "queued prefix".into(),
        })
        .await
        .unwrap();
    Arc::get_mut(&mut tools).unwrap().events = Some(events);
    let cancellation = tools.context.cancellation_token();
    let mut pending = Box::pin(tools.emit(AgentEvent::TextDelta {
        text: "blocked tail".into(),
    }));
    assert!(futures::poll!(&mut pending).is_pending());
    cancellation.cancel();
    tokio::time::timeout(std::time::Duration::from_millis(50), pending)
        .await
        .unwrap();
    assert!(
        matches!(receiver.recv().await, Some(AgentEvent::TextDelta { text }) if text == "queued prefix")
    );
    assert!(receiver.try_recv().is_err());
}

#[tokio::test]
async fn a_ready_tool_event_still_reaches_the_host_after_run_cancellation() {
    let root = tempfile::tempdir().unwrap();
    let (mut tools, _) = executor_tools(root.path());
    let (events, mut receiver) = tokio::sync::mpsc::channel(1);
    Arc::get_mut(&mut tools).unwrap().events = Some(events);
    tools.context.cancellation_token().cancel();
    tools
        .emit(AgentEvent::TextDelta {
            text: "available slot".into(),
        })
        .await;
    assert!(
        matches!(receiver.recv().await, Some(AgentEvent::TextDelta { text }) if text == "available slot")
    );
}
