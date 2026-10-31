#![cfg(test)]

use super::*;

const COMMAND: &str = "python3 - <<'PY'\nprint('fixture')\nPY";

#[derive(Debug, Default)]
struct CapturingSandbox(Mutex<Vec<String>>);

#[async_trait::async_trait]
impl crate::sandbox::BashSandbox for CapturingSandbox {
    async fn exec_command(
        &self,
        command: &str,
        _: &str,
    ) -> anyhow::Result<crate::sandbox::SandboxOutput> {
        self.0.lock().unwrap().push(command.into());
        Ok(crate::sandbox::SandboxOutput {
            stdout: "fixture\n".into(),
            stderr: String::new(),
            exit_code: 0,
        })
    }
    async fn shutdown(&self) {}
}

#[derive(Debug)]
struct RewrittenHeredoc;

#[async_trait::async_trait]
impl crate::hooks::HookExecutor for RewrittenHeredoc {
    async fn fire(&self, event: &crate::hooks::HookEvent) -> crate::hooks::HookResult {
        if matches!(event, crate::hooks::HookEvent::PreToolUse(_)) {
            crate::hooks::HookResult::continue_with(
                json!({"updatedInput":{"command":format!("{COMMAND}\nprintf changed")}}),
            )
        } else {
            crate::hooks::HookResult::Continue(None)
        }
    }
}

#[tokio::test]
async fn exact_approved_heredoc_reaches_bash_but_unapproved_or_rewritten_calls_do_not() {
    for (confirmed, rewrite, deny, expected) in [
        (true, false, false, ToolExecutionOutcome::Succeeded),
        (false, false, false, ToolExecutionOutcome::Denied),
        (true, true, false, ToolExecutionOutcome::Denied),
        (true, false, true, ToolExecutionOutcome::Denied),
    ] {
        let root = tempfile::tempdir().unwrap();
        let sandbox = Arc::new(CapturingSandbox::default());
        let (base, _) = executor_tools_with(root.path(), Some(sandbox.clone()));
        let mut tools = base.as_ref().clone();
        tools.permission = PermissionPolicy {
            default_decision: if deny {
                PermissionDecision::Deny
            } else {
                PermissionDecision::Ask
            },
            ..Default::default()
        };
        if rewrite {
            tools.agent = Some(crate::agent::AgentLoop::new(
                Arc::new(crate::agent::tests::MockLlmClient::new(Vec::new())),
                Arc::clone(&tools.executor),
                tools.context.clone(),
                crate::agent::AgentConfig {
                    hook_engine: Some(Arc::new(RewrittenHeredoc)),
                    ..Default::default()
                },
            ));
        }
        let tools = Arc::new(tools);
        let mut call = request("heredoc", json!({"command":COMMAND}));
        call.call.name = "bash".into();
        call.call.needs_confirmation = confirmed;
        let prepared = prepare_tools(
            Arc::clone(&tools),
            call,
            tokio_util::sync::CancellationToken::new(),
        )
        .await
        .unwrap();
        assert_eq!(
            prepared.prepared.original_args_digest,
            aster_effect::tool_args_digest(&json!({"command":COMMAND})).unwrap()
        );
        assert_eq!(
            prepared.prepared.original_args_digest != prepared.prepared.effective_args_digest,
            rewrite
        );
        let result = execute_prepared(tools, prepared).await;
        assert_eq!(
            result.outcome, expected,
            "confirmed={confirmed} rewrite={rewrite} deny={deny}: {result:?}"
        );
        let commands = sandbox.0.lock().unwrap();
        assert_eq!(
            commands.len(),
            usize::from(expected == ToolExecutionOutcome::Succeeded)
        );
        if !commands.is_empty() {
            assert_eq!(commands[0], COMMAND);
        }
    }
}
