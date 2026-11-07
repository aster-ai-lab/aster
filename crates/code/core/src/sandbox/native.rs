//! Native bash sandbox adapter for Aster Code.
//!
//! Wraps [`ksandbox::NativeSandbox`] so local sessions get a real OS process
//! boundary (Seatbelt / namespaces / AppContainer) without depending on kbox
//! MicroVM. Construction fails closed when the platform backend cannot start;
//! callers then fall back to process-host (opt-in) or unavailable.

use super::{
    BashSandbox, NetworkGrant, SandboxCommandRequest, SandboxExecutionOutput, SandboxNetworkMode,
    SandboxOutput,
};
use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use ksandbox::{
    CommandRequest, NativeSandbox, NetworkAllowRule, OutputObserver, OutputSummary, PathRule,
    SandboxPolicy,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub(crate) use super::workspace_boundary::{
    hard_link_count, hard_link_count_for_open_file, sensitive_paths,
    should_skip_workspace_scan_directory, workspace_hardlink_paths, workspace_sensitive_paths,
};

/// Backend label from the bound ksandbox platform (e.g. `macos-seatbelt`).
pub const NATIVE_SANDBOX_BACKEND: &str = ksandbox::NATIVE_SANDBOX_BACKEND;

/// Fail-closed native sandbox: ksandbox process isolation over a workspace.
#[derive(Debug)]
pub struct NativeBashSandbox {
    inner: NativeSandbox,
    mediated: bool,
}

impl NativeBashSandbox {
    pub fn new(workspace: impl Into<PathBuf>) -> Result<Self> {
        let workspace = canonicalize_workspace(workspace)?;
        let policy = aster_bash_policy(&workspace)?;
        Ok(Self {
            inner: NativeSandbox::with_policy(workspace, policy)?,
            mediated: false,
        })
    }

    /// Deny baseline, or mediated egress with a non-empty allowlist.
    pub fn with_network_mode(
        workspace: impl Into<PathBuf>,
        mode: SandboxNetworkMode,
        allow_origins: &[String],
    ) -> Result<Self> {
        match mode {
            SandboxNetworkMode::Deny => Self::new(workspace),
            SandboxNetworkMode::Mediated => {
                if allow_origins.is_empty() {
                    bail!(
                        "mediated network requires a non-empty allowlist \
                         (mediating nothing is not a posture)"
                    );
                }
                let workspace = canonicalize_workspace(workspace)?;
                let allow = parse_network_allow_rule_list(allow_origins)?;
                let mut policy = aster_bash_policy(&workspace)?;
                policy.features.mediated_network = true;
                policy.network.allow = allow;
                Ok(Self {
                    inner: NativeSandbox::with_policy(workspace, policy)?,
                    mediated: true,
                })
            }
        }
    }

    pub fn network_mediation_armed(&self) -> bool {
        self.mediated && self.inner.policy().features.mediated_network
    }

    pub fn workspace(&self) -> &Path {
        self.inner.workspace()
    }

    pub fn backend(&self) -> &'static str {
        self.inner.backend()
    }

    pub async fn probe(&self) -> Result<()> {
        self.inner.probe().await
    }
}

/// Stable reason used when a host or test injects an unqualified-native refusal.
///
/// The live [`NativeBashSandbox`] no longer emits this on every call; it remains
/// for mock sandboxes and receipt classifiers that still recognize the type.
pub(crate) const NATIVE_PROCESS_LIFETIME_UNQUALIFIED: &str = "native_process_lifetime_unqualified: the native process sandbox could not qualify this command. Use process-host (outer container opt-in), require_escalated host approval, or Full access (yolo); the command did not start.";

/// The native backend refused before any command process existed.
#[derive(Debug)]
pub(crate) struct NativeProcessLifetimeUnqualified;

impl std::fmt::Display for NativeProcessLifetimeUnqualified {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(NATIVE_PROCESS_LIFETIME_UNQUALIFIED)
    }
}

impl std::error::Error for NativeProcessLifetimeUnqualified {}

/// Fixture helper for tests that inject an unqualified-native refusal.
#[cfg(test)]
pub(crate) fn native_process_lifetime_unqualified() -> anyhow::Error {
    NativeProcessLifetimeUnqualified.into()
}

pub(crate) fn is_native_process_lifetime_unqualified(error: &anyhow::Error) -> bool {
    error.is::<NativeProcessLifetimeUnqualified>()
        || error
            .chain()
            .any(|cause| cause.is::<NativeProcessLifetimeUnqualified>())
}

struct BridgedOutputObserver {
    inner: Arc<dyn crate::workspace::CommandOutputObserver>,
}

#[async_trait]
impl OutputObserver for BridgedOutputObserver {
    async fn on_output_delta(&self, delta: &str) {
        self.inner.on_output_delta(delta).await;
    }

    async fn on_output_complete(&self, summary: &OutputSummary) {
        self.inner
            .on_output_complete(&crate::workspace::CommandOutputSummary {
                total_bytes: summary.total_bytes,
                captured_bytes: summary.captured_bytes,
                truncated: summary.truncated,
                timed_out: summary.timed_out,
            })
            .await;
    }
}

#[async_trait]
impl BashSandbox for NativeBashSandbox {
    fn allows_host_execution_override(&self) -> bool {
        true
    }

    fn execution_target(&self) -> super::ShellExecutionTarget {
        super::ShellExecutionTarget::local_host_shell()
    }

    fn validate_execution_available(&self) -> Result<()> {
        Ok(())
    }

    fn supports_background_execution(&self) -> bool {
        // Process-group kill-on-drop and bounded capture. macOS Seatbelt does
        // not reclaim setsid() descendants; escape suites record that gap
        // without withdrawing ordinary background admission.
        true
    }

    fn shell_note(&self) -> Option<String> {
        if cfg!(target_os = "macos") {
            Some(
                "macos-seatbelt: process-group kill-on-drop; setsid descendants are not reclaimed"
                    .into(),
            )
        } else {
            None
        }
    }

    fn policy_digest(&self) -> Option<String> {
        Some(self.inner.policy_digest())
    }

    fn apply_network_grant(
        &self,
        grant: NetworkGrant,
        expected_base_digest: &str,
    ) -> Result<String> {
        let grant = ksandbox::NetworkGrant::new(grant.host, grant.port)?;
        self.inner.apply_network_grant(grant, expected_base_digest)
    }

    async fn exec_command(&self, command: &str, _guest_workspace: &str) -> Result<SandboxOutput> {
        let output = self.inner.exec_command(command).await?;
        Ok(SandboxOutput {
            stdout: output.stdout,
            stderr: output.stderr,
            exit_code: output.exit_code,
        })
    }

    async fn exec(&self, request: SandboxCommandRequest) -> Result<SandboxExecutionOutput> {
        let output_observer = request.output_observer.map(|observer| {
            Arc::new(BridgedOutputObserver { inner: observer }) as Arc<dyn OutputObserver>
        });
        let output = self
            .inner
            .execute(CommandRequest {
                command: request.command,
                timeout_ms: request.timeout_ms,
                output_observer,
                env: request.env,
            })
            .await?;
        Ok(SandboxExecutionOutput {
            stdout: output.stdout,
            stderr: output.stderr,
            exit_code: output.exit_code,
            timed_out: output.timed_out,
        })
    }

    async fn shutdown(&self) {}
}

fn canonicalize_workspace(workspace: impl Into<PathBuf>) -> Result<PathBuf> {
    let workspace = workspace.into();
    let workspace = workspace.canonicalize().with_context(|| {
        format!(
            "failed to canonicalize the native sandbox workspace {}",
            workspace.display()
        )
    })?;
    if !workspace.is_dir() {
        bail!(
            "native sandbox workspace is not a directory: {}",
            workspace.display()
        );
    }
    Ok(workspace)
}

/// Aster control-plane overlay on the ksandbox baseline.
///
/// ksandbox protects `.kbox`; Aster's durable control plane is `.aster`.
/// Existing Aster protected directories are write-denied when present; `.aster`
/// is always materialised so Seatbelt can pin the deny before first use.
fn aster_bash_policy(workspace: &Path) -> Result<SandboxPolicy> {
    let mut policy = SandboxPolicy::kbash_baseline();
    let aster = workspace.join(".aster");
    let loops = aster.join("loops");
    std::fs::create_dir_all(&loops).with_context(|| {
        format!(
            "failed to create Aster control-plane carve-out {}",
            loops.display()
        )
    })?;
    policy
        .filesystem
        .deny_write
        .push(PathRule::Exact(".aster".into()));
    policy
        .filesystem
        .write_exceptions
        .push(PathRule::Exact(".aster/loops".into()));

    for name in super::PROTECTED_WORKSPACE_DIRECTORIES {
        if name.eq_ignore_ascii_case(".aster") {
            continue;
        }
        if workspace.join(name).exists() {
            policy
                .filesystem
                .deny_write
                .push(PathRule::Exact((*name).into()));
        }
    }
    for name in super::PROTECTED_WORKSPACE_FILES {
        if workspace.join(name).exists() {
            policy
                .filesystem
                .deny_write
                .push(PathRule::Exact((*name).into()));
        }
    }
    Ok(policy)
}

/// Parse `host` / `host:port` origin strings for config-time validation.
pub(crate) fn parse_network_allow_rules(origins: &[String]) -> Result<()> {
    parse_network_allow_rule_list(origins).map(|_| ())
}

fn parse_network_allow_rule_list(origins: &[String]) -> Result<Vec<NetworkAllowRule>> {
    let mut rules = Vec::with_capacity(origins.len());
    for origin in origins {
        let origin = origin.trim();
        let (host, port) = match origin.rsplit_once(':') {
            Some((host, port)) => {
                let port: u16 = port.parse().map_err(|_| {
                    anyhow::anyhow!(
                        "sandbox network allow origin {origin:?} has a non-numeric port"
                    )
                })?;
                (host.trim().to_string(), Some(port))
            }
            None => (origin.to_string(), None),
        };
        if host.is_empty()
            || host
                .chars()
                .any(|character| matches!(character, '/' | ' ' | ':' | '[' | ']'))
        {
            bail!(
                "sandbox network allow origin {origin:?} is not a bare host[:port] \
                 (bracketed or bare IPv6 literals are not accepted yet)"
            );
        }
        if port == Some(0) {
            bail!("sandbox network allow origin {origin:?} has port 0");
        }
        rules.push(NetworkAllowRule {
            host,
            port,
            path_prefix: None,
        });
    }
    Ok(rules)
}

#[cfg(test)]
mod lifetime_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_backend_constructs_and_exposes_workspace() {
        let workspace = tempfile::tempdir().unwrap();
        let sandbox = NativeBashSandbox::new(workspace.path()).unwrap();
        assert_eq!(
            sandbox.workspace(),
            workspace.path().canonicalize().unwrap()
        );
        assert_eq!(sandbox.backend(), NATIVE_SANDBOX_BACKEND);
        assert!(!sandbox.network_mediation_armed());
        assert!(sandbox.policy_digest().is_some());
        #[cfg(target_os = "macos")]
        assert!(sandbox
            .shell_note()
            .is_some_and(|note| note.contains("setsid")));
        #[cfg(not(target_os = "macos"))]
        assert!(sandbox.shell_note().is_none());
    }

    #[tokio::test]
    async fn probe_and_exec_run_inside_ksandbox() {
        let workspace = tempfile::tempdir().unwrap();
        let sandbox = NativeBashSandbox::new(workspace.path()).unwrap();
        sandbox.probe().await.expect("ksandbox probe");

        let output = sandbox
            .exec_command("printf ksandbox-ok", "/workspace")
            .await
            .expect("ksandbox exec");
        assert_eq!(output.exit_code, 0);
        assert_eq!(output.stdout, "ksandbox-ok");
    }

    #[tokio::test]
    async fn aster_control_plane_write_is_denied() {
        let workspace = tempfile::tempdir().unwrap();
        let sandbox = NativeBashSandbox::new(workspace.path()).unwrap();
        let output = sandbox
            .exec_command("printf leak > .aster/host-leak", "/workspace")
            .await
            .expect("command starts under ksandbox");
        assert_ne!(output.exit_code, 0, "stderr={}", output.stderr);
        assert!(!workspace.path().join(".aster/host-leak").exists());
    }

    #[test]
    fn mediated_mode_arms_allowlist() {
        let workspace = tempfile::tempdir().unwrap();
        let mediated = NativeBashSandbox::with_network_mode(
            workspace.path(),
            SandboxNetworkMode::Mediated,
            &[
                "registry.example".to_string(),
                "packages.internal:8443".to_string(),
            ],
        )
        .unwrap();
        assert!(mediated.network_mediation_armed());

        let refusal = NativeBashSandbox::with_network_mode(
            workspace.path(),
            SandboxNetworkMode::Mediated,
            &[],
        )
        .unwrap_err();
        assert!(
            format!("{refusal:#}").contains("allow"),
            "refusal names the allowlist: {refusal:#}"
        );
    }

    #[test]
    fn unqualified_helper_remains_recognizable() {
        let error = native_process_lifetime_unqualified();
        assert!(is_native_process_lifetime_unqualified(&error));
        assert_eq!(error.to_string(), NATIVE_PROCESS_LIFETIME_UNQUALIFIED);
    }
}
