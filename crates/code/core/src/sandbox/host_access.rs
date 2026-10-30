//! Explicit full-access execution through the workspace's existing host runner.

use super::{BashSandbox, SandboxCommandRequest, SandboxExecutionOutput, SandboxOutput};
use crate::workspace::{CommandRequest, WorkspaceCommandRunner};
use std::sync::Arc;

pub(crate) struct HostAccessSandbox {
    pub(crate) runner: Arc<dyn WorkspaceCommandRunner>,
}

#[async_trait::async_trait]
impl BashSandbox for HostAccessSandbox {
    fn is_isolated(&self) -> bool {
        false
    }

    fn execution_target(&self) -> super::ShellExecutionTarget {
        self.runner.execution_target()
    }

    fn shell_note(&self) -> Option<String> {
        self.runner.shell_note()
    }

    async fn exec_command(&self, command: &str, _workspace: &str) -> anyhow::Result<SandboxOutput> {
        let output = self
            .exec(SandboxCommandRequest {
                command: command.into(),
                guest_workspace: String::new(),
                timeout_ms: 120_000,
                output_observer: None,
                env: None,
            })
            .await?;
        Ok(SandboxOutput {
            stdout: output.stdout,
            stderr: output.stderr,
            exit_code: output.exit_code,
        })
    }

    async fn exec(&self, request: SandboxCommandRequest) -> anyhow::Result<SandboxExecutionOutput> {
        let output = self
            .runner
            .exec(CommandRequest {
                command: request.command,
                timeout_ms: request.timeout_ms,
                output_observer: request.output_observer,
                env: request.env,
            })
            .await?;
        Ok(SandboxExecutionOutput {
            stdout: output.output,
            stderr: String::new(),
            exit_code: output.exit_code,
            timed_out: output.timed_out,
        })
    }

    async fn shutdown(&self) {}
}

#[cfg(test)]
#[path = "host_access_tests.rs"]
mod tests;
