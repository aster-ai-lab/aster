//! A read-only execution error that started no process settles as a failed
//! call. An all-read batch or program is read-only, so the same error does
//! not open a workspace watch. A mutating unknown call still blocks close.

use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::json;

use super::*;

fn register_read_write_batch(tools: &ExecutorTools) {
    let registry = Arc::clone(tools.executor.registry());
    registry.register(Arc::new(crate::tools::builtin::read::ReadTool));
    registry.register(Arc::new(crate::tools::WriteTool));
    crate::tools::builtin::register_batch(&registry);
    crate::tools::register_program_with_catalog(&registry, crate::program::ProgramCatalog::new());
}

fn spend_budget(tools: &ExecutorTools) -> ExecutorTools {
    let mut spent = tools.clone();
    spent.agent = Some(crate::agent::AgentLoop::new(
        Arc::new(crate::agent::tests::MockLlmClient::new(Vec::new())),
        Arc::clone(&spent.executor),
        spent.context.clone(),
        crate::agent::AgentConfig {
            max_execution_time_ms: Some(1),
            ..Default::default()
        },
    ));
    spent.execution_started = Some(Instant::now() - Duration::from_secs(5));
    spent
}

fn batch_args(tools: &[&str]) -> serde_json::Value {
    json!({
        "invocations": tools
            .iter()
            .map(|tool| json!({"tool": tool, "args": {"file_path": "note.txt", "content": "x"}}))
            .collect::<Vec<_>>()
    })
}

#[tokio::test]
async fn read_only_execution_error_settles_failed_and_the_turn_can_close() {
    let root = tempfile::tempdir().unwrap();
    let (tools, count) = executor_tools(root.path());
    let prepared = prepare_tools(
        Arc::clone(&tools),
        request(
            "read-error",
            json!({"read_only": true, "execution_error": true}),
        ),
        Default::default(),
    )
    .await
    .unwrap();
    assert!(prepared.prepared.baseline.is_none());
    let failed = execute_prepared(Arc::clone(&tools), prepared).await;
    assert_eq!(failed.outcome, ToolExecutionOutcome::Failed);
    assert_eq!(
        failed.effects,
        ToolEffectObservationV1::complete(Vec::new())
    );
    assert!(failed.effects.unresolved_external_operations.is_empty());
    let ledger = tools.ledger.as_ref().unwrap();
    assert!(ledger
        .lock()
        .unwrap()
        .checkpoint_uncertainty_keys()
        .is_empty());

    let read = prepare_tools(
        Arc::clone(&tools),
        request("later-read", json!({"read_only": true})),
        Default::default(),
    )
    .await
    .unwrap();
    let read = execute_prepared(Arc::clone(&tools), read).await;
    assert_eq!(read.outcome, ToolExecutionOutcome::Succeeded);
    assert_eq!(count.load(Ordering::SeqCst), 2);
    let guard = ledger.lock().unwrap();
    assert!(guard.checkpoint_uncertainty_keys().is_empty());
    assert!(matches!(
        crate::harness_loop::decide_completion(&guard, &[], &[], false),
        crate::harness_loop::CompletionGate::Allow(_)
    ));
}

#[tokio::test]
async fn all_read_batch_and_program_do_not_watch_or_freeze_a_later_read() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("note.txt"), "hello").unwrap();
    let (base, _) = executor_tools(root.path());
    register_read_write_batch(&base);
    let registry = base.executor.registry();
    let read_batch = batch_args(&["read", "read"]);
    let mixed_batch = batch_args(&["read", "write"]);
    assert!(
        registry
            .capabilities("batch", &read_batch)
            .unwrap()
            .read_only
    );
    assert!(
        !registry
            .capabilities("batch", &mixed_batch)
            .unwrap()
            .read_only
    );
    assert!(
        !registry
            .capabilities(
                "batch",
                &json!({"invocations":[{"tool":"missing","args":{}}]})
            )
            .unwrap()
            .read_only
    );
    assert!(
        registry
            .capabilities(
                "program",
                &json!({"type":"script","allowed_tools":["read"]})
            )
            .unwrap()
            .read_only
    );
    assert!(
        !registry
            .capabilities(
                "program",
                &json!({"type":"script","allowed_tools":["read","write"]})
            )
            .unwrap()
            .read_only
    );
    assert!(
        !registry
            .capabilities("program", &json!({"type":"script"}))
            .unwrap()
            .read_only
    );

    let spent = Arc::new(spend_budget(&base));
    let mut call = request("batch-read", read_batch);
    call.call.name = "batch".into();
    let prepared = prepare_tools(Arc::clone(&spent), call, Default::default())
        .await
        .unwrap();
    assert!(
        prepared.prepared.baseline.is_none(),
        "an all-read batch must not start a workspace watch"
    );
    let failed = execute_prepared(Arc::clone(&spent), prepared).await;
    assert_eq!(failed.outcome, ToolExecutionOutcome::Failed);
    assert_eq!(
        failed.effects,
        ToolEffectObservationV1::complete(Vec::new())
    );
    assert!(spent
        .ledger
        .as_ref()
        .unwrap()
        .lock()
        .unwrap()
        .checkpoint_uncertainty_keys()
        .is_empty());

    let mut program = request(
        "program-read",
        json!({
            "type": "script",
            "allowed_tools": ["read"],
            "source": "async function run() { return 1 }"
        }),
    );
    program.call.name = "program".into();
    let prepared = prepare_tools(Arc::clone(&spent), program, Default::default())
        .await
        .unwrap();
    assert!(
        prepared.prepared.baseline.is_none(),
        "an all-read program must not start a workspace watch"
    );
    let failed = execute_prepared(Arc::clone(&spent), prepared).await;
    assert_eq!(failed.outcome, ToolExecutionOutcome::Failed);
    assert_eq!(
        failed.effects,
        ToolEffectObservationV1::complete(Vec::new())
    );
    assert!(spent
        .ledger
        .as_ref()
        .unwrap()
        .lock()
        .unwrap()
        .checkpoint_uncertainty_keys()
        .is_empty());

    let mut open = spent.as_ref().clone();
    open.agent = None;
    open.execution_started = None;
    let open = Arc::new(open);
    let mut read = request("later-read", json!({"file_path": "note.txt"}));
    read.call.name = "read".into();
    let read = prepare_tools(Arc::clone(&open), read, Default::default())
        .await
        .unwrap();
    assert!(read.prepared.baseline.is_none());
    let read = execute_prepared(Arc::clone(&open), read).await;
    assert_eq!(read.outcome, ToolExecutionOutcome::Succeeded);
    let guard = open.ledger.as_ref().unwrap().lock().unwrap();
    assert!(guard.checkpoint_uncertainty_keys().is_empty());
    assert!(matches!(
        crate::harness_loop::decide_completion(&guard, &[], &[], false),
        crate::harness_loop::CompletionGate::Allow(_)
    ));
}

#[tokio::test]
async fn unknown_mutating_call_keeps_one_identity_and_blocks_later_writes() {
    let root = tempfile::tempdir().unwrap();
    let (tools, count) = executor_tools(root.path());
    let prepared = prepare_tools(
        Arc::clone(&tools),
        request("mutate-error", json!({"execution_error": true})),
        Default::default(),
    )
    .await
    .unwrap();
    let key = prepared.prepared.identity.key().unwrap();
    let unknown = execute_prepared(Arc::clone(&tools), prepared).await;
    assert_eq!(unknown.outcome, ToolExecutionOutcome::Unknown);
    assert!(unknown
        .effects
        .unresolved_external_operations
        .contains(&key));
    let ledger = tools.ledger.as_ref().unwrap();
    {
        let mut guard = ledger.lock().unwrap();
        assert_eq!(guard.checkpoint_uncertainty_keys().len(), 1);
        assert!(guard.checkpoint_uncertainty_keys().contains(&key));
        guard.set_observation_incomplete(false);
        let waiver =
            crate::harness_loop::CompletionWaiverV1::new("sha256:ordinary", "ordinary waiver")
                .unwrap();
        let gate = crate::harness_loop::decide_with_observations(
            &guard,
            &[],
            std::slice::from_ref(&waiver),
            true,
            &[],
        );
        match gate {
            crate::harness_loop::CompletionGate::Incomplete { message } => {
                assert!(message.contains("tool_effect_unresolved"));
                assert!(message.contains("identity-bound reconciliation"));
                assert!(message.contains("final text"));
                assert!(message.contains("ordinary waivers"));
                assert_eq!(
                    message,
                    "tool_effect_unresolved: 1 invocation obligation(s) require identity-bound reconciliation; local scans, final text, and ordinary waivers cannot discharge them."
                );
            }
            other => panic!("expected incomplete, got {other:?}"),
        }
    }

    let read = prepare_tools(
        Arc::clone(&tools),
        request("still-reads", json!({"read_only": true})),
        Default::default(),
    )
    .await
    .unwrap();
    let read = execute_prepared(Arc::clone(&tools), read).await;
    assert_eq!(read.outcome, ToolExecutionOutcome::Succeeded);
    assert_eq!(
        ledger.lock().unwrap().checkpoint_uncertainty_keys().len(),
        1
    );

    let write = prepare_tools(
        Arc::clone(&tools),
        request("later-write", json!({"write": true})),
        Default::default(),
    )
    .await
    .unwrap();
    let refused = execute_prepared(Arc::clone(&tools), write).await;
    assert_eq!(refused.outcome, ToolExecutionOutcome::Denied);
    let output = refused.output.as_str().unwrap();
    assert!(output.contains("tool_effect_unresolved"));
    assert!(output.contains("reconciliation"));
    assert!(!root.path().join("changed.txt").exists());
    assert_eq!(count.load(Ordering::SeqCst), 2);
    assert_eq!(
        ledger.lock().unwrap().checkpoint_uncertainty_keys().len(),
        1
    );
    assert!(matches!(
        crate::harness_loop::decide_completion(&ledger.lock().unwrap(), &[], &[], false),
        crate::harness_loop::CompletionGate::Incomplete { .. }
    ));
}
