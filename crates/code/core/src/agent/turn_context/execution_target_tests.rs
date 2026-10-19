use super::super::{render_env_block, render_env_block_for_host};
use crate::sandbox::{BashSandbox, SandboxOutput, ShellExecutionTarget};
use crate::tools::ToolContext;
use crate::workspace::{CommandOutput, CommandRequest, WorkspaceCommandRunner, WorkspaceServices};
use std::sync::Arc;

struct DeclaredSandbox(ShellExecutionTarget);

#[async_trait::async_trait]
impl BashSandbox for DeclaredSandbox {
    fn execution_target(&self) -> ShellExecutionTarget {
        self.0
    }

    async fn exec_command(&self, _: &str, _: &str) -> anyhow::Result<SandboxOutput> {
        anyhow::bail!("context metadata must not execute commands")
    }

    async fn shutdown(&self) {}
}

struct UnreportedRunner;

#[async_trait::async_trait]
impl WorkspaceCommandRunner for UnreportedRunner {
    async fn exec(&self, _: CommandRequest) -> anyhow::Result<CommandOutput> {
        anyhow::bail!("context metadata must not execute commands")
    }
}

fn custom_context(runner: bool) -> ToolContext {
    let root = std::env::temp_dir();
    let fs = Arc::new(crate::workspace::LocalWorkspaceBackend::new(root.clone()));
    let mut builder = WorkspaceServices::builder(
        crate::workspace::WorkspaceRef::new("custom", "custom://workspace"),
        fs,
    );
    if runner {
        builder = builder.command_runner(Arc::new(UnreportedRunner));
    }
    ToolContext::new(root)
        .with_workspace_services(builder.build())
        .with_run_governance(None, None)
}

#[test]
fn execution_target_closed_set_round_trips_without_guessing() {
    for target in [
        ShellExecutionTarget::Unknown,
        ShellExecutionTarget::HostBash,
        ShellExecutionTarget::HostPowerShell,
        ShellExecutionTarget::LinuxGuestBash,
        ShellExecutionTarget::LinuxGuestSh,
    ] {
        assert_eq!(
            ShellExecutionTarget::parse(target.as_str()).unwrap(),
            target
        );
    }
    for unknown in ["Linux", "gnu", "HOST_BASH", " host_bash"] {
        assert!(ShellExecutionTarget::parse(unknown).is_err());
    }
}

#[test]
fn bound_linux_target_is_independent_of_macos_or_windows_host() {
    let ctx = ToolContext::new(std::env::temp_dir()).with_sandbox(Arc::new(DeclaredSandbox(
        ShellExecutionTarget::LinuxGuestBash,
    )));
    for (os, arch) in [("macos", "aarch64"), ("windows", "x86_64")] {
        let block = render_env_block_for_host(&ctx, os, arch);
        assert!(block.contains(&format!("Host platform: {os} ({arch})")));
        assert!(block.contains("Default Bash execution target: linux guest; shell: /bin/bash"));
        assert!(!block.contains("Default Bash execution target: host"));
        assert!(!block.contains("GNU"));
        assert!(!block.contains("BSD"));
    }
}

#[test]
fn plain_local_context_reports_its_actual_runner_shell() {
    let ctx = ToolContext::new(std::env::temp_dir());
    assert_eq!(
        ctx.workspace_services
            .command_runner()
            .unwrap()
            .execution_target(),
        ShellExecutionTarget::local_host_shell()
    );
    let block = render_env_block(&ctx);
    assert!(block.contains(&format!(
        "Default Bash execution target: host {} ({})",
        std::env::consts::OS,
        std::env::consts::ARCH
    )));
    let expected = if cfg!(windows) {
        "shell: PowerShell"
    } else {
        "shell: bash"
    };
    assert!(block.contains(expected));
}

#[test]
fn governed_local_missing_sandbox_does_not_advertise_default_host_execution() {
    let ctx = ToolContext::new(std::env::temp_dir()).with_run_governance(None, None);
    let block = render_env_block(&ctx);
    assert!(block
        .contains("Default Bash execution target: unavailable (configured sandbox is missing)"));
    assert!(block.contains("Escalated Bash execution target: host"));
    assert!(block.contains(
        "Only after the exact require_escalated invocation passes permission and confirmation"
    ));
    assert!(block.contains("never an automatic fallback"));
}

#[test]
fn unknown_bound_sandbox_is_not_overridden_by_known_local_runner() {
    let ctx = ToolContext::new(std::env::temp_dir())
        .with_sandbox(Arc::new(DeclaredSandbox(ShellExecutionTarget::Unknown)));
    let block = render_env_block(&ctx);
    assert!(
        block.contains("Default Bash execution target: unknown (backend did not report OS/shell)")
    );
    assert!(block.contains("Escalated Bash execution target: host"));
}

#[test]
fn custom_runner_without_metadata_remains_unknown() {
    let block = render_env_block(&custom_context(true));
    assert!(
        block.contains("Default Bash execution target: unknown (backend did not report OS/shell)")
    );
    assert!(block
        .contains("Escalated Bash execution target: unknown (backend did not report OS/shell)"));
}

#[test]
fn workspace_without_command_runner_reports_unavailable() {
    let block = render_env_block(&custom_context(false));
    assert!(block
        .contains("Default Bash execution target: unavailable (workspace has no command runner)"));
    assert!(block.contains(
        "Escalated Bash execution target: unavailable (workspace has no command runner)"
    ));
}
