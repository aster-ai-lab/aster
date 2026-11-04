#![cfg(test)]

use super::*;

struct FixedChecker(PermissionDecision);

impl crate::permissions::PermissionChecker for FixedChecker {
    fn check(&self, _: &str, _: &serde_json::Value) -> PermissionDecision {
        self.0
    }
}

#[tokio::test]
async fn unchanged_denials_identify_the_authority_without_claiming_a_rewrite() {
    for (policy, checker, expected) in [
        (
            PermissionPolicy::new().deny("receipt_probe"),
            None,
            "policy_denied",
        ),
        (
            PermissionPolicy::new().allow("*"),
            Some(PermissionDecision::Deny),
            "host_checker_denied",
        ),
    ] {
        let root = tempfile::tempdir().unwrap();
        let (base, executions, hooks) = argument_validation::schema_tools(root.path(), None);
        let (events, mut received) = tokio::sync::mpsc::channel(8);
        let mut tools = base.as_ref().clone();
        tools.permission = policy;
        tools.checker = checker.map(|decision| {
            Arc::new(FixedChecker(decision)) as Arc<dyn crate::permissions::PermissionChecker>
        });
        tools.events = Some(events);
        let tools = Arc::new(tools);
        let args = json!({"write":true});
        let prepared = prepare_tools(
            Arc::clone(&tools),
            request("unchanged-denied", args.clone()),
            Default::default(),
        )
        .await
        .unwrap();
        let identity = prepared.prepared.clone();
        assert_eq!(
            identity.original_args_digest,
            identity.effective_args_digest
        );
        let result = execute_prepared(tools, prepared).await;
        result.ensure_prepared(&identity).unwrap();
        assert_eq!(result.outcome, ToolExecutionOutcome::Denied);
        assert_eq!(result.exit_code, None);
        let output = result.output.as_str().unwrap();
        assert!(output.contains("tool_effect_effective_arguments_unapproved"));
        assert!(output.contains(expected), "{output}");
        assert!(output.contains("arguments_changed=false"), "{output}");
        assert!(output.contains("execution_started=false"), "{output}");
        assert_eq!(
            result.effects,
            ToolEffectObservationV1::complete(Vec::new())
        );
        assert_eq!(executions.load(Ordering::SeqCst), 0);
        assert_eq!(hooks.load(Ordering::SeqCst), 0);
        assert!(!root.path().join("changed.txt").exists());
        assert!(
            matches!(received.try_recv().unwrap(), crate::agent::AgentEvent::PermissionDenied {
            tool_id, args: event_args, reason, ..
        } if tool_id == "unchanged-denied" && event_args == args && reason == output)
        );
        assert!(received.try_recv().is_err());
    }
}

#[tokio::test]
async fn child_ask_without_a_provider_is_distinct_from_a_checker_denial() {
    for (decision, expected) in [
        (PermissionDecision::Ask, "child_confirmation_unavailable"),
        (PermissionDecision::Deny, "child_checker_denied"),
    ] {
        let root = tempfile::tempdir().unwrap();
        let (base, executions) = executor_tools(root.path());
        let mut tools = base.as_ref().clone();
        tools.checker = Some(Arc::new(FixedChecker(decision)));
        tools.agent = Some(
            crate::agent::AgentLoop::new(
                Arc::new(crate::agent::tests::MockLlmClient::new(Vec::new())),
                Arc::clone(&tools.executor),
                tools.context.clone(),
                Default::default(),
            )
            .into_child_run(),
        );
        let tools = Arc::new(tools);
        let prepared = prepare_tools(
            Arc::clone(&tools),
            request("child", json!({"write":true})),
            Default::default(),
        )
        .await
        .unwrap();
        let result = execute_prepared(tools, prepared).await;
        assert_eq!(result.outcome, ToolExecutionOutcome::Denied);
        assert!(
            result.output.as_str().unwrap().contains(expected),
            "{:?}",
            result.output
        );
        assert_eq!(executions.load(Ordering::SeqCst), 0);
    }
}

#[tokio::test]
async fn rewritten_ask_preserves_the_hook_effect_obligation_and_names_reapproval() {
    let root = tempfile::tempdir().unwrap();
    let (base, executions, hooks) =
        argument_validation::schema_tools(root.path(), Some(json!({"write":true})));
    let mut tools = base.as_ref().clone();
    tools.permission = PermissionPolicy::new();
    let tools = Arc::new(tools);
    let mut call = request("rewritten-ask", json!({"write":false}));
    call.call.needs_confirmation = true;
    let prepared = prepare_tools(Arc::clone(&tools), call, Default::default())
        .await
        .unwrap();
    let identity = prepared.prepared.clone();
    assert_ne!(
        identity.original_args_digest,
        identity.effective_args_digest
    );
    let result = execute_prepared(tools, prepared).await;
    result.ensure_prepared(&identity).unwrap();
    assert_eq!(result.outcome, ToolExecutionOutcome::Denied);
    assert_eq!(
        result.effects.completeness,
        ToolObservationCompleteness::Incomplete
    );
    assert_eq!(
        result.effects.unresolved_external_operations,
        [identity.identity.key().unwrap()]
    );
    assert!(
        !fold_debt(&[prepared_fact(&identity), result_fact(&result)])
            .unresolved
            .is_empty()
    );
    let output = result.output.as_str().unwrap();
    assert!(
        output.contains("rewritten_arguments_require_confirmation"),
        "{output}"
    );
    assert!(output.contains("arguments_changed=true"), "{output}");
    assert_eq!(executions.load(Ordering::SeqCst), 0);
    assert_eq!(hooks.load(Ordering::SeqCst), 1);
    assert!(root.path().join("hook-effect.txt").exists());
    assert!(!root.path().join("changed.txt").exists());
}

#[tokio::test]
async fn unconfirmed_unchanged_ask_names_confirmation_instead_of_a_rewrite() {
    for host in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let (base, executions) = executor_tools(root.path());
        let mut tools = base.as_ref().clone();
        tools.permission = PermissionPolicy::new();
        if host {
            let (events, _) = tokio::sync::broadcast::channel(8);
            tools.agent = Some(crate::agent::AgentLoop::new(
                Arc::new(crate::agent::tests::MockLlmClient::new(Vec::new())),
                Arc::clone(&tools.executor),
                tools.context.clone(),
                crate::agent::AgentConfig {
                    confirmation_manager: Some(Arc::new(crate::hitl::ConfirmationManager::new(
                        crate::hitl::ConfirmationPolicy::enabled(),
                        events,
                    ))),
                    ..Default::default()
                },
            ));
        }
        let tools = Arc::new(tools);
        let prepared = prepare_tools(
            Arc::clone(&tools),
            request("ask", json!({"write":true})),
            Default::default(),
        )
        .await
        .unwrap();
        let result = execute_prepared(tools, prepared).await;
        assert_eq!(result.outcome, ToolExecutionOutcome::Denied);
        assert_eq!(
            result.effects,
            ToolEffectObservationV1::complete(Vec::new())
        );
        let output = result.output.as_str().unwrap();
        assert!(output.contains("confirmation_required"), "{output}");
        assert!(output.contains("arguments_changed=false"), "{output}");
        assert_eq!(executions.load(Ordering::SeqCst), 0);
    }
}

#[tokio::test]
async fn real_read_admits_the_instruction_fixture_and_denies_its_sibling() {
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("项目 with spaces");
    std::fs::create_dir(&directory).unwrap();
    #[cfg(unix)]
    {
        std::fs::write(directory.join("CLAUDE.md"), "authorized instructions\n").unwrap();
        std::os::unix::fs::symlink("CLAUDE.md", directory.join("AGENTS.md")).unwrap();
    }
    #[cfg(not(unix))]
    std::fs::write(directory.join("AGENTS.md"), "authorized instructions\n").unwrap();
    std::fs::write(directory.join("private.txt"), "synthetic-private-content\n").unwrap();
    let (base, _) = executor_tools(root.path());
    let mut tools = base.as_ref().clone();
    tools.permission = PermissionPolicy::new()
        .allow("read")
        .deny("read(项目 with spaces/private.txt)");
    tools.checker = Some(Arc::new(
        crate::permissions::InteractiveToolGuardrail::default().with_workspace(root.path()),
    ));
    let tools = Arc::new(tools);
    for (args, allowed) in [
        (
            json!({"file_path":"项目 with spaces/AGENTS.md","limit":220}),
            true,
        ),
        (
            json!({"files":[{"path":"项目 with spaces/AGENTS.md","limit":220}]}),
            true,
        ),
        (json!({"file_path":"项目 with spaces/private.txt"}), false),
        (
            json!({"files":[{"path":"项目 with spaces/AGENTS.md"},{"path":"项目 with spaces/private.txt"}]}),
            false,
        ),
    ] {
        let mut call = request("real-read", args);
        call.call.name = "read".into();
        let prepared = prepare_tools(Arc::clone(&tools), call, Default::default())
            .await
            .unwrap();
        let identity = prepared.prepared.clone();
        assert_eq!(
            identity.original_args_digest,
            identity.effective_args_digest
        );
        let result = execute_prepared(Arc::clone(&tools), prepared).await;
        result.ensure_prepared(&identity).unwrap();
        let output = result.output.as_str().unwrap();
        if allowed {
            assert_eq!(result.outcome, ToolExecutionOutcome::Succeeded, "{output}");
            assert!(output.contains("authorized instructions"), "{output}");
        } else {
            assert_eq!(result.outcome, ToolExecutionOutcome::Denied, "{output}");
            assert!(output.contains("policy_denied"), "{output}");
        }
        assert!(!output.contains("synthetic-private-content"), "{output}");
    }
}

#[tokio::test]
async fn effective_permission_denial_after_a_hook_keeps_the_observation_gap() {
    struct DenyWrites;
    impl crate::permissions::PermissionChecker for DenyWrites {
        fn check(&self, _: &str, args: &serde_json::Value) -> PermissionDecision {
            if args["write"] == true {
                PermissionDecision::Deny
            } else {
                PermissionDecision::Allow
            }
        }
    }
    let root = tempfile::tempdir().unwrap();
    let (base, executions, hooks) =
        argument_validation::schema_tools(root.path(), Some(json!({"write":true})));
    let mut tools = base.as_ref().clone();
    tools.checker = Some(Arc::new(DenyWrites));
    let tools = Arc::new(tools);
    let prepared = prepare_tools(
        Arc::clone(&tools),
        request("denied-rewrite", json!({"write":false})),
        Default::default(),
    )
    .await
    .unwrap();
    let identity = prepared.prepared.clone();
    let result = execute_prepared(tools, prepared).await;
    assert_eq!(result.outcome, ToolExecutionOutcome::Denied);
    assert_eq!(
        result.effects.completeness,
        ToolObservationCompleteness::Incomplete
    );
    assert_eq!(
        result.effects.unresolved_external_operations,
        [identity.identity.key().unwrap()]
    );
    let output = result.output.as_str().unwrap();
    assert!(output.contains("host_checker_denied"), "{output}");
    assert!(output.contains("arguments_changed=true"), "{output}");
    assert_eq!(executions.load(Ordering::SeqCst), 0);
    assert_eq!(hooks.load(Ordering::SeqCst), 1);
    assert!(!root.path().join("changed.txt").exists());
}

#[derive(Debug)]
struct StoppingHook {
    effect_path: PathBuf,
    cancellation: Option<tokio_util::sync::CancellationToken>,
}

#[async_trait::async_trait]
impl crate::hooks::HookExecutor for StoppingHook {
    async fn fire(&self, event: &crate::hooks::HookEvent) -> crate::hooks::HookResult {
        if matches!(event, crate::hooks::HookEvent::PreToolUse(_)) {
            std::fs::write(&self.effect_path, b"observed hook side effect").unwrap();
            if let Some(cancellation) = &self.cancellation {
                cancellation.cancel();
            } else {
                return crate::hooks::HookResult::block("synthetic hook refusal");
            }
        }
        crate::hooks::HookResult::Continue(None)
    }
}

#[tokio::test]
async fn hook_refusal_and_cancellation_preserve_effects_without_running_the_tool() {
    for cancel in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let (base, executions) = executor_tools(root.path());
        let cancellation = tokio_util::sync::CancellationToken::new();
        let mut tools = base.as_ref().clone();
        tools.agent = Some(crate::agent::AgentLoop::new(
            Arc::new(crate::agent::tests::MockLlmClient::new(Vec::new())),
            Arc::clone(&tools.executor),
            tools.context.clone(),
            crate::agent::AgentConfig {
                hook_engine: Some(Arc::new(StoppingHook {
                    effect_path: root.path().join("hook-effect.txt"),
                    cancellation: cancel.then(|| cancellation.clone()),
                })),
                ..Default::default()
            },
        ));
        let tools = Arc::new(tools);
        let prepared = prepare_tools(
            Arc::clone(&tools),
            request("stopped-hook", json!({"write":true})),
            cancellation,
        )
        .await
        .unwrap();
        let identity = prepared.prepared.clone();
        let result = execute_prepared(tools, prepared).await;
        result.ensure_prepared(&identity).unwrap();
        assert_eq!(
            result.outcome,
            if cancel {
                ToolExecutionOutcome::Cancelled
            } else {
                ToolExecutionOutcome::Denied
            }
        );
        assert_eq!(
            result.effects.completeness,
            ToolObservationCompleteness::Incomplete
        );
        assert_eq!(
            result.effects.unresolved_external_operations,
            [identity.identity.key().unwrap()]
        );
        assert_eq!(executions.load(Ordering::SeqCst), 0);
        assert!(root.path().join("hook-effect.txt").exists());
        assert!(!root.path().join("changed.txt").exists());
        if !cancel {
            assert!(result
                .output
                .as_str()
                .unwrap()
                .contains("synthetic hook refusal"));
        }
    }
}

#[cfg(unix)]
#[tokio::test]
async fn symlink_escape_refusal_names_the_symlink_boundary() {
    use std::os::unix::fs::symlink;

    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("ok.txt"), "authorized instructions\n").unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(
        outside.path().join("secret.txt"),
        "synthetic-private-content\n",
    )
    .unwrap();
    symlink(
        outside.path().join("secret.txt"),
        root.path().join("link.txt"),
    )
    .unwrap();
    let (base, executions) = executor_tools(root.path());
    let mut tools = base.as_ref().clone();
    tools.permission = PermissionPolicy::new().allow("read");
    tools.checker = Some(Arc::new(
        crate::permissions::InteractiveToolGuardrail::default().with_workspace(root.path()),
    ));
    let tools = Arc::new(tools);
    let mut call = request("symlink-denied", json!({"file_path": "link.txt"}));
    call.call.name = "read".into();
    let prepared = prepare_tools(Arc::clone(&tools), call, Default::default())
        .await
        .unwrap();
    let result = execute_prepared(tools, prepared).await;
    assert_eq!(result.outcome, ToolExecutionOutcome::Denied);
    let output = result.output.as_str().unwrap();
    assert!(
        output.contains("symlink_boundary_escape"),
        "refusal must name the symlink boundary: {output}"
    );
    assert!(
        output.contains("tool_effect_effective_arguments_unapproved"),
        "{output}"
    );
    assert!(!output.contains("synthetic-private-content"), "{output}");
    assert_eq!(executions.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn original_checker_denial_is_not_relabelled_by_a_later_checker_result() {
    struct ChangingChecker(AtomicUsize);
    impl crate::permissions::PermissionChecker for ChangingChecker {
        fn check(&self, _: &str, _: &serde_json::Value) -> PermissionDecision {
            if self.0.fetch_add(1, Ordering::SeqCst) == 0 {
                PermissionDecision::Deny
            } else {
                PermissionDecision::Ask
            }
        }
    }
    let root = tempfile::tempdir().unwrap();
    let (base, executions) = executor_tools(root.path());
    let mut tools = base.as_ref().clone();
    tools.permission = PermissionPolicy::new();
    tools.checker = Some(Arc::new(ChangingChecker(AtomicUsize::new(0))));
    let tools = Arc::new(tools);
    let prepared = prepare_tools(
        Arc::clone(&tools),
        request("original-refusal", json!({"write":true})),
        Default::default(),
    )
    .await
    .unwrap();
    let result = execute_prepared(tools, prepared).await;
    assert_eq!(result.outcome, ToolExecutionOutcome::Denied);
    let output = result.output.as_str().unwrap();
    assert!(output.contains("host_checker_denied"), "{output}");
    assert!(!output.contains("obtain approval"), "{output}");
    assert_eq!(executions.load(Ordering::SeqCst), 0);
}

#[test]
fn diagnostic_evaluation_preserves_the_permission_matrix_and_checker_call_count() {
    use crate::fact_control::tool_results::{evaluate_run_permission, execution_permission};
    struct CountingChecker {
        decision: PermissionDecision,
        calls: AtomicUsize,
    }
    impl crate::permissions::PermissionChecker for CountingChecker {
        fn check(&self, _: &str, _: &serde_json::Value) -> PermissionDecision {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.decision
        }
    }
    use PermissionDecision::{Allow, Ask, Deny};
    for (policy_decision, checker_decision, read, write) in [
        (Deny, Allow, Deny, Deny),
        (Deny, Ask, Deny, Deny),
        (Deny, Deny, Deny, Deny),
        (Allow, Allow, Allow, Allow),
        (Allow, Ask, Allow, Allow),
        (Allow, Deny, Deny, Deny),
        (Ask, Allow, Allow, Ask),
        (Ask, Ask, Ask, Ask),
        (Ask, Deny, Deny, Deny),
    ] {
        let mut policy = PermissionPolicy::new();
        policy.default_decision = policy_decision;
        for (tool, expected) in [("read", read), ("write", write)] {
            let checker = CountingChecker {
                decision: checker_decision,
                calls: AtomicUsize::new(0),
            };
            let evaluation =
                evaluate_run_permission(&policy, Some(&checker), None, tool, &json!({}));
            assert_eq!(evaluation.decision(), expected);
            assert_eq!(
                checker.calls.load(Ordering::SeqCst),
                usize::from(policy_decision != Deny)
            );
            assert_eq!(
                execution_permission(&policy, None, tool, &json!({})),
                policy_decision
            );
        }
    }
}
