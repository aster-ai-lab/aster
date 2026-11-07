//! Default process-isolation binding for local sessions.

use super::SessionOptions;
use crate::sandbox::process_host::ProcessHostBashSandbox;
use crate::sandbox::{BashSandbox, SandboxOutput};
use std::path::Path;
use std::sync::Arc;

/// Environment opt-in for process-host fallback when the native sandbox cannot
/// initialize (Harbor / Terminal-Bench containers without bubblewrap).
pub const ALLOW_PROCESS_HOST_SANDBOX_ENV: &str = "ASTER_ALLOW_PROCESS_HOST_SANDBOX";

/// Bind the built-in ksandbox native adapter before capabilities are assembled
/// so the top-level Bash tool, workflows, and delegated child runs all inherit
/// the same process-isolation boundary.
///
/// A host-provided sandbox remains authoritative. Non-local workspace services
/// retain their own command runner because they do not expose a local root. If
/// ksandbox cannot initialize on this host, the default remains fail-closed
/// (`UnavailableDefaultSandbox`) unless the host opts into
/// [`SessionOptions::with_allow_process_host_sandbox`] /
/// [`ALLOW_PROCESS_HOST_SANDBOX_ENV`], which installs a process-host runner
/// suitable when an outer container already isolates the job. The fallback
/// only ever serves a deny posture: a mediated request degrades to
/// unavailable (with an error-level log) rather than silently dropping the
/// mediation the host asked for.
pub(super) fn install_default_local_sandbox(workspace: &Path, opts: &mut SessionOptions) {
    if opts.sandbox_handle.is_some() {
        return;
    }

    let local_root = match opts.workspace_services.as_ref() {
        Some(services) => match services.local_root() {
            Some(root) => root,
            None => return,
        },
        None => workspace,
    };

    let allow_process_host = allow_process_host_sandbox(opts);
    let native = match opts.sandbox_network {
        crate::sandbox::SandboxNetworkMode::Deny => {
            crate::sandbox::native::NativeBashSandbox::new(local_root)
        }
        crate::sandbox::SandboxNetworkMode::Mediated => {
            crate::sandbox::native::NativeBashSandbox::with_network_mode(
                local_root,
                crate::sandbox::SandboxNetworkMode::Mediated,
                &opts.sandbox_network_allow,
            )
        }
    };
    let sandbox =
        select_default_local_sandbox(local_root, opts.sandbox_network, allow_process_host, native);
    opts.sandbox_handle = Some(sandbox);
}

fn allow_process_host_sandbox(opts: &SessionOptions) -> bool {
    if opts.allow_process_host_sandbox {
        return true;
    }
    std::env::var_os(ALLOW_PROCESS_HOST_SANDBOX_ENV).is_some_and(|value| {
        matches!(
            value.to_string_lossy().trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
}

pub(crate) fn select_default_local_sandbox(
    local_root: &Path,
    requested: crate::sandbox::SandboxNetworkMode,
    allow_process_host: bool,
    native: anyhow::Result<crate::sandbox::native::NativeBashSandbox>,
) -> Arc<dyn BashSandbox> {
    // Default path: ksandbox NativeSandbox via NativeBashSandbox. process-host
    // remains an explicit outer-container opt-in when construction fails.
    // MicroVM isolation (kbox) is orthogonal and not wired here.
    let _ = local_root;
    match native {
        Ok(sandbox) => Arc::new(sandbox),
        Err(error) if allow_process_host && local_root.is_dir() => match requested {
            // The process-host runner has no network mediation, so it may
            // only serve a deny posture: installing it under a mediated
            // request would deliver unmediated host egress — wider than
            // the host asked for. A future posture must decide its own
            // fallback stance here, never inherit this one silently.
            crate::sandbox::SandboxNetworkMode::Deny => {
                tracing::warn!(
                    workspace = %local_root.display(),
                    error = %error,
                    "default native Bash sandbox is unavailable; using process-host sandbox because the host opted in"
                );
                Arc::new(ProcessHostBashSandbox::new(local_root.to_path_buf(), None))
            }
            crate::sandbox::SandboxNetworkMode::Mediated => {
                let message = format!(
                    "the default Aster native sandbox is unavailable for '{}': {error:#}; the \
                     process-host fallback was NOT installed because it cannot honor the \
                     requested '{}' network posture — fix the ksandbox backend or drop \
                     the mediated posture",
                    local_root.display(),
                    requested.as_str(),
                );
                tracing::error!(
                    workspace = %local_root.display(),
                    error = %error,
                    posture = requested.as_str(),
                    "default native Bash sandbox is unavailable and the process-host fallback cannot honor the mediated posture; Bash remains denied"
                );
                Arc::new(UnavailableDefaultSandbox { message })
            }
        },
        Err(error) => {
            let hint = if matches!(requested, crate::sandbox::SandboxNetworkMode::Deny) {
                format!(
                    "; if an outer container already isolates this job, set {ALLOW_PROCESS_HOST_SANDBOX_ENV}=1 to allow the process-host Bash fallback (network posture: deny)"
                )
            } else {
                String::new()
            };
            let message = format!(
                "the default Aster native sandbox is unavailable for '{}': {error:#}{hint}",
                local_root.display()
            );
            tracing::warn!(workspace = %local_root.display(), error = %error, hint = %hint, "default native Bash sandbox is unavailable; Bash remains denied");
            Arc::new(UnavailableDefaultSandbox { message })
        }
    }
}

#[derive(Debug)]
struct UnavailableDefaultSandbox {
    message: String,
}

#[async_trait::async_trait]
impl BashSandbox for UnavailableDefaultSandbox {
    fn allows_host_execution_override(&self) -> bool {
        true
    }

    fn validate_execution_available(&self) -> anyhow::Result<()> {
        anyhow::bail!(self.message.clone())
    }

    async fn exec_command(
        &self,
        _command: &str,
        _guest_workspace: &str,
    ) -> anyhow::Result<SandboxOutput> {
        anyhow::bail!(self.message.clone())
    }

    async fn shutdown(&self) {}
}

#[cfg(all(test, target_os = "macos"))]
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    struct CustomSandbox;

    #[async_trait::async_trait]
    impl BashSandbox for CustomSandbox {
        async fn exec_command(
            &self,
            _command: &str,
            _guest_workspace: &str,
        ) -> anyhow::Result<SandboxOutput> {
            Ok(SandboxOutput {
                stdout: String::new(),
                stderr: String::new(),
                exit_code: 0,
            })
        }

        async fn shutdown(&self) {}
    }

    #[test]
    fn explicit_sandbox_remains_authoritative() {
        let workspace = tempfile::tempdir().unwrap();
        let expected: Arc<dyn BashSandbox> = Arc::new(CustomSandbox);
        let mut opts = SessionOptions::new().with_sandbox_handle(Arc::clone(&expected));

        install_default_local_sandbox(workspace.path(), &mut opts);

        let actual = opts.sandbox_handle.as_ref().unwrap();
        assert!(Arc::ptr_eq(actual, &expected));
    }

    #[tokio::test]
    async fn unavailable_local_workspace_gets_error_only_handle() {
        let parent = tempfile::tempdir().unwrap();
        let workspace = parent.path().join("missing");
        let mut opts = SessionOptions::new();

        install_default_local_sandbox(&workspace, &mut opts);

        let sandbox = opts.sandbox_handle.as_ref().unwrap();
        let error = match sandbox
            .exec_command("this-must-never-run", "/workspace")
            .await
        {
            Ok(_) => panic!("unavailable sandbox unexpectedly executed a command"),
            Err(error) => error,
        };
        assert!(error
            .to_string()
            .contains("the default Aster native sandbox is unavailable"));
    }

    #[test]
    fn process_host_opt_in_installs_runner_when_native_construction_fails() {
        let parent = tempfile::tempdir().unwrap();
        let workspace = parent.path().join("missing");
        let mut opts = SessionOptions::new().with_allow_process_host_sandbox(true);
        install_default_local_sandbox(&workspace, &mut opts);
        // Missing workspace still fails native canonicalize; process-host
        // requires an existing directory, so the handle stays unavailable.
        assert!(opts.sandbox_handle.is_some());
    }

    #[tokio::test]
    async fn default_native_adapter_executes_via_ksandbox() {
        let workspace = tempfile::tempdir().unwrap();
        let native = crate::sandbox::native::NativeBashSandbox::new(workspace.path()).unwrap();
        let sandbox = select_default_local_sandbox(
            workspace.path(),
            crate::sandbox::SandboxNetworkMode::Deny,
            false,
            Ok(native),
        );
        let output = sandbox
            .exec_command("printf session-ok", "/workspace")
            .await
            .expect("default native adapter must execute via ksandbox");
        assert_eq!(output.exit_code, 0);
        assert_eq!(output.stdout, "session-ok");
    }

    #[tokio::test]
    async fn installed_default_sandbox_enforces_workspace_and_aster_boundary() {
        let workspace = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let external = outside.path().join("host-leak");
        let mut opts = SessionOptions::new();
        install_default_local_sandbox(workspace.path(), &mut opts);
        let sandbox = opts.sandbox_handle.as_ref().expect("default sandbox");
        assert!(sandbox.is_isolated());
        assert!(sandbox.policy_digest().is_some());

        let ok = sandbox
            .exec_command("printf boundary-ok > in-workspace.txt", "/workspace")
            .await
            .expect("in-workspace write");
        assert_eq!(ok.exit_code, 0, "stderr={}", ok.stderr);
        assert_eq!(
            std::fs::read_to_string(workspace.path().join("in-workspace.txt")).unwrap(),
            "boundary-ok"
        );

        let denied_aster = sandbox
            .exec_command("printf leak > .aster/host-leak", "/workspace")
            .await
            .expect("aster deny command starts");
        assert_ne!(denied_aster.exit_code, 0, "stderr={}", denied_aster.stderr);
        assert!(!workspace.path().join(".aster/host-leak").exists());

        let denied_external = sandbox
            .exec_command(
                &format!(
                    "printf leak > {}",
                    shell_words::quote(external.to_str().unwrap())
                ),
                "/workspace",
            )
            .await
            .expect("external deny command starts");
        assert_ne!(
            denied_external.exit_code, 0,
            "stderr={}",
            denied_external.stderr
        );
        assert!(!external.exists());
    }
}
