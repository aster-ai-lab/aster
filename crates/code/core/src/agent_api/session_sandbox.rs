//! Default process-isolation binding for local sessions.

use super::SessionOptions;
use crate::sandbox::process_host::ProcessHostBashSandbox;
use crate::sandbox::{BashSandbox, SandboxOutput};
use std::path::Path;
use std::sync::Arc;

/// Environment opt-in for process-host fallback when the native sandbox cannot
/// initialize (Harbor / Terminal-Bench containers without bubblewrap).
pub const ALLOW_PROCESS_HOST_SANDBOX_ENV: &str = "A3S_CODE_ALLOW_PROCESS_HOST_SANDBOX";

/// Bind the built-in native sandbox before capabilities are assembled so the
/// top-level Bash tool, workflows, and delegated child runs all inherit the
/// same process-isolation boundary.
///
/// A host-provided sandbox remains authoritative. Non-local workspace services
/// retain their own command runner because they do not expose a local root. If
/// the native backend cannot initialize, the default remains fail-closed
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
    // A mediated request needs the native posture. This deny-only VM has no
    // mediator, so it must not replace that request. The direct HVF guest
    // shim is not the product default.
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    let native = if matches!(requested, crate::sandbox::SandboxNetworkMode::Deny) {
        match crate::sandbox::oci::OciBashSandbox::discover(local_root) {
            Ok(sandbox) => {
                tracing::debug!(
                    workspace = %local_root.display(),
                    "macOS OCI Host sandbox selected"
                );
                return Arc::new(sandbox);
            }
            Err(error) => {
                let reason = format!("{error:#}");
                tracing::warn!(
                    workspace = %local_root.display(),
                    error = %reason,
                    "macOS OCI Host sandbox is unavailable; native execution remains refused"
                );
                native.map(|sandbox| sandbox.with_guest_unavailable_reason(reason))
            }
        }
    } else {
        native
    };

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
                    "the default A3S native sandbox is unavailable for '{}': {error:#}; the \
                     process-host fallback was NOT installed because it cannot honor the \
                     requested '{}' network posture — install the native backend (e.g. \
                     bubblewrap) or drop the mediated posture",
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
            let message = format!(
                "the default A3S native sandbox is unavailable for '{}': {error:#}",
                local_root.display()
            );
            tracing::warn!(workspace = %local_root.display(), error = %error, "default native Bash sandbox is unavailable; Bash remains denied");
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
mod guest_lifetime_e2e;

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use crate::sandbox::SandboxCommandRequest;
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
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
            .contains("the default A3S native sandbox is unavailable"));
    }

    // Harbor / Terminal-Bench process-host bash is a Unix outer-isolation path.
    // Windows product bash uses PowerShell; Git Bash spawned from MSVC is not
    // the supported process-host contract.
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[tokio::test]
    async fn native_execution_refusal_never_selects_initialization_fallback() {
        use crate::tools::Tool;
        let workspace = tempfile::tempdir().unwrap();
        let missing = workspace.path().join("missing-oci-companions");
        let previous_assets = std::env::var_os(crate::sandbox::oci::SANDBOX_ASSETS_ENV);
        std::env::set_var(crate::sandbox::oci::SANDBOX_ASSETS_ENV, &missing);
        let companion_error = crate::sandbox::oci::OciBashSandbox::discover(workspace.path())
            .err()
            .expect("this hermetic refusal test has no OCI companions");
        let native = crate::sandbox::native::NativeBashSandbox::new(workspace.path()).unwrap();
        let digest = native.policy_digest();
        let sandbox = select_default_local_sandbox(
            workspace.path(),
            crate::sandbox::SandboxNetworkMode::Deny,
            true,
            Ok(native),
        );
        match previous_assets {
            Some(value) => std::env::set_var(crate::sandbox::oci::SANDBOX_ASSETS_ENV, value),
            None => std::env::remove_var(crate::sandbox::oci::SANDBOX_ASSETS_ENV),
        }
        assert_eq!(sandbox.policy_digest(), digest);
        for _ in 0..2 {
            let output = crate::tools::builtin::bash::BashTool
                .execute(
                    &serde_json::json!({"command":"printf unexpected-host-fallback > no-fallback-sentinel"}),
                    &crate::tools::ToolContext::new(workspace.path().to_path_buf())
                        .with_sandbox(Arc::clone(&sandbox)),
                )
                .await
                .expect("a pre-spawn native refusal is a settled tool result");
            assert!(!output.success);
            assert!(output
                .content
                .contains("native_process_lifetime_unqualified"));
            assert!(output.content.contains(&companion_error.to_string()));
            assert!(matches!(
                output.error_kind,
                Some(crate::tools::ToolErrorKind::Unsupported { .. })
            ));
            assert!(!workspace.path().join("no-fallback-sentinel").exists());
            assert_eq!(sandbox.policy_digest(), digest);
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn process_host_fallback_runs_when_native_fails_and_host_opts_in() {
        let workspace = tempfile::tempdir().unwrap();
        let sandbox = select_default_local_sandbox(
            workspace.path(),
            crate::sandbox::SandboxNetworkMode::Deny,
            true,
            Err(anyhow::anyhow!("Linux native sandbox requires bubblewrap")),
        );
        let output = sandbox
            .exec(SandboxCommandRequest {
                command: "printf '%s\\n' 'process-host-ok'".into(),
                guest_workspace: workspace.path().display().to_string(),
                timeout_ms: 5_000,
                output_observer: None,
                env: None,
            })
            .await
            .expect("process-host sandbox must execute");
        assert_eq!(
            output.exit_code, 0,
            "process-host bash exit={}; stdout={:?} stderr={}",
            output.exit_code, output.stdout, output.stderr
        );
        assert!(
            output.stdout.contains("process-host-ok"),
            "stdout={:?} stderr={:?}",
            output.stdout,
            output.stderr
        );
    }

    #[tokio::test]
    async fn process_host_fallback_stays_fail_closed_without_opt_in() {
        let workspace = tempfile::tempdir().unwrap();
        let sandbox = select_default_local_sandbox(
            workspace.path(),
            crate::sandbox::SandboxNetworkMode::Deny,
            false,
            Err(anyhow::anyhow!("Linux native sandbox requires bubblewrap")),
        );
        let error = match sandbox
            .exec_command("printf should-not-run", "/workspace")
            .await
        {
            Ok(_) => panic!("without opt-in the default remains unavailable"),
            Err(error) => error,
        };
        assert!(error
            .to_string()
            .contains("the default A3S native sandbox is unavailable"));
    }

    #[test]
    fn session_option_enables_process_host_opt_in() {
        let opts = SessionOptions::new().with_allow_process_host_sandbox(true);
        assert!(allow_process_host_sandbox(&opts));
    }

    #[test]
    fn env_opt_in_accepts_common_truthy_values() {
        let previous = std::env::var_os(ALLOW_PROCESS_HOST_SANDBOX_ENV);
        let opts = SessionOptions::new();
        for value in ["1", "true", "YES", " On "] {
            std::env::set_var(ALLOW_PROCESS_HOST_SANDBOX_ENV, value);
            assert!(
                allow_process_host_sandbox(&opts),
                "expected env value {value:?} to opt in"
            );
        }
        std::env::set_var(ALLOW_PROCESS_HOST_SANDBOX_ENV, "0");
        assert!(!allow_process_host_sandbox(&opts));
        match previous {
            Some(value) => std::env::set_var(ALLOW_PROCESS_HOST_SANDBOX_ENV, value),
            None => std::env::remove_var(ALLOW_PROCESS_HOST_SANDBOX_ENV),
        }
    }

    #[tokio::test]
    async fn unavailable_sandbox_shutdown_is_a_noop() {
        let sandbox = UnavailableDefaultSandbox {
            message: "denied".into(),
        };
        sandbox.shutdown().await;
    }

    #[tokio::test]
    async fn custom_sandbox_handle_remains_callable() {
        let workspace = tempfile::tempdir().unwrap();
        let expected: Arc<dyn BashSandbox> = Arc::new(CustomSandbox);
        let mut opts = SessionOptions::new().with_sandbox_handle(Arc::clone(&expected));
        install_default_local_sandbox(workspace.path(), &mut opts);
        let sandbox = opts.sandbox_handle.as_ref().unwrap();
        let output = sandbox
            .exec_command("unused", "/workspace")
            .await
            .expect("custom sandbox must run");
        assert_eq!(output.exit_code, 0);
        sandbox.shutdown().await;
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn process_host_opt_in_selects_process_host_when_native_fails() {
        let workspace = tempfile::tempdir().unwrap();
        let sandbox = select_default_local_sandbox(
            workspace.path(),
            crate::sandbox::SandboxNetworkMode::Deny,
            true,
            Err(anyhow::anyhow!("native unavailable for coverage")),
        );
        let output = sandbox
            .exec_command("printf 'selected-process-host\\n'", "/workspace")
            .await
            .expect("process-host selection must execute");
        assert_eq!(output.exit_code, 0, "stderr={}", output.stderr);
        assert!(output.stdout.contains("selected-process-host"));
        sandbox.shutdown().await;
    }

    #[tokio::test]
    async fn mediated_request_refuses_the_process_host_fallback() {
        let workspace = tempfile::tempdir().unwrap();
        // The host opted into the fallback AND asked for mediated egress;
        // the fallback cannot mediate, so the default must stay denied
        // instead of silently widening to unmediated host execution.
        let sandbox = select_default_local_sandbox(
            workspace.path(),
            crate::sandbox::SandboxNetworkMode::Mediated,
            true,
            Err(anyhow::anyhow!("Linux native sandbox requires bubblewrap")),
        );
        let error = match sandbox
            .exec_command("printf must-not-run", "/workspace")
            .await
        {
            Ok(_) => panic!("mediated request must not degrade to the process-host sandbox"),
            Err(error) => error,
        };
        let message = error.to_string();
        assert!(
            message.contains("the default A3S native sandbox is unavailable"),
            "stable prefix kept: {message}"
        );
        assert!(
            message.contains("process-host fallback was NOT installed"),
            "the refusal names the withheld fallback: {message}"
        );
        assert!(
            message.contains("mediated"),
            "the refusal names the posture that cannot be honored: {message}"
        );
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[tokio::test]
    async fn missing_oci_companions_keep_both_native_apis_refused() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        use crate::sandbox::guest::{GUEST_KRUN_SHIM_ENV, GUEST_SYSTEM_IMAGE_ENV};
        use crate::sandbox::SandboxCommandRequest;
        use crate::workspace::{CommandOutputObserver, CommandOutputSummary};

        struct Observer(AtomicUsize);

        #[async_trait::async_trait]
        impl CommandOutputObserver for Observer {
            async fn on_output_delta(&self, _delta: &str) {
                self.0.fetch_add(1, Ordering::SeqCst);
            }

            async fn on_output_complete(&self, _summary: &CommandOutputSummary) {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
        }

        let previous_image = std::env::var_os(GUEST_SYSTEM_IMAGE_ENV);
        let previous_shim = std::env::var_os(GUEST_KRUN_SHIM_ENV);
        let previous_assets = std::env::var_os(crate::sandbox::oci::SANDBOX_ASSETS_ENV);
        std::env::remove_var(GUEST_SYSTEM_IMAGE_ENV);
        std::env::remove_var(GUEST_KRUN_SHIM_ENV);

        let workspace = tempfile::tempdir().unwrap();
        std::env::set_var(
            crate::sandbox::oci::SANDBOX_ASSETS_ENV,
            workspace.path().join("missing-oci-companions"),
        );
        let mut opts = SessionOptions::new().with_allow_process_host_sandbox(true);
        install_default_local_sandbox(workspace.path(), &mut opts);
        let sandbox = opts.sandbox_handle.as_ref().unwrap();
        let observer = Arc::new(Observer(AtomicUsize::new(0)));
        let sentinel = workspace.path().join("probe-sentinel");
        let command = "printf probe-sentinel > probe-sentinel";
        let exec_command = sandbox.exec_command(command, "/workspace").await;
        let exec = sandbox
            .exec(SandboxCommandRequest {
                command: command.into(),
                guest_workspace: "/workspace".into(),
                timeout_ms: 0,
                output_observer: Some(observer.clone()),
                env: None,
            })
            .await;
        match previous_image {
            Some(value) => std::env::set_var(GUEST_SYSTEM_IMAGE_ENV, value),
            None => std::env::remove_var(GUEST_SYSTEM_IMAGE_ENV),
        }
        match previous_shim {
            Some(value) => std::env::set_var(GUEST_KRUN_SHIM_ENV, value),
            None => std::env::remove_var(GUEST_KRUN_SHIM_ENV),
        }
        match previous_assets {
            Some(value) => std::env::set_var(crate::sandbox::oci::SANDBOX_ASSETS_ENV, value),
            None => std::env::remove_var(crate::sandbox::oci::SANDBOX_ASSETS_ENV),
        }
        eprintln!(
            "missing exec_command={} exec={} observer={} sentinel={}",
            exec_command
                .as_ref()
                .err()
                .map(|error| error.to_string())
                .unwrap_or_else(|| "ran".into()),
            exec.as_ref()
                .err()
                .map(|error| error.to_string())
                .unwrap_or_else(|| "ran".into()),
            observer.0.load(Ordering::SeqCst),
            sentinel.exists()
        );
        for error in [exec_command.err(), exec.err()] {
            let error = error.expect("a missing guest probe must not run the command");
            assert!(
                error
                    .to_string()
                    .contains("native_process_lifetime_unqualified"),
                "{error}"
            );
            assert!(crate::sandbox::native::is_native_process_lifetime_unqualified(&error));
        }
        assert_eq!(observer.0.load(Ordering::SeqCst), 0);
        assert!(!sentinel.exists());
        assert!(!sandbox.supports_background_execution());
    }

    /// Companion files that pass discovery keep Deny on the OCI Host even
    /// when process-host is opted in. Mediated never takes that Host.
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[tokio::test]
    async fn validated_companion_files_keep_deny_on_oci_and_mediated_off_the_vm() {
        let root = tempfile::tempdir().unwrap();
        let assets = root.path().join("assets");
        for relative in [
            "a3s-oci",
            "a3s-oci-krun-shim",
            "system-image.json",
            "bundle/config.json",
            "bundle/rootfs/bin/bash",
        ] {
            let path = assets.join(relative);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, b"stub").unwrap();
        }
        let workspace = root.path().join("work");
        std::fs::create_dir(&workspace).unwrap();
        let _assets_env = AssetsEnv::set(&assets);
        crate::sandbox::oci::OciBashSandbox::discover(&workspace)
            .expect("stub companion files must satisfy discovery");

        let mut deny = SessionOptions::new().with_allow_process_host_sandbox(true);
        install_default_local_sandbox(&workspace, &mut deny);
        let deny = deny.sandbox_handle.as_ref().unwrap();
        assert_eq!(
            deny.execution_overhead_ms(),
            90_000,
            "opt-in must not replace a discovered OCI Host"
        );
        assert!(
            deny.policy_digest().is_none(),
            "the OCI Host is not the native backend"
        );

        let sentinel = workspace.join("mediated-sentinel");
        let mut mediated = SessionOptions::new()
            .with_allow_process_host_sandbox(true)
            .with_sandbox_network_mode(crate::sandbox::SandboxNetworkMode::Mediated)
            .with_sandbox_network_allow(vec!["example.com".into()]);
        install_default_local_sandbox(&workspace, &mut mediated);
        let mediated = mediated.sandbox_handle.as_ref().unwrap();
        assert_ne!(
            mediated.execution_overhead_ms(),
            90_000,
            "mediated posture must not select the deny-only OCI Host"
        );
        let command = format!("printf leaked > '{}'", sentinel.display());
        let exec_command = mediated.exec_command(&command, "/workspace").await;
        let exec = mediated
            .exec(SandboxCommandRequest {
                command,
                guest_workspace: "/workspace".into(),
                timeout_ms: 1_000,
                output_observer: None,
                env: None,
            })
            .await;
        for error in [exec_command.err(), exec.err()] {
            let error = error.expect("mediated posture must not run the command");
            let message = error.to_string();
            assert!(
                message.contains("native_process_lifetime_unqualified"),
                "{error:#}"
            );
            assert!(
                !message.contains("OCI companion discovery:"),
                "mediated posture does not ask OCI for companions: {error:#}"
            );
            assert!(crate::sandbox::native::is_native_process_lifetime_unqualified(&error));
        }
        assert!(!sentinel.exists());
        eprintln!(
            "hermetic deny_overhead={} mediated_overhead={} mediated_digest={}",
            deny.execution_overhead_ms(),
            mediated.execution_overhead_ms(),
            mediated.policy_digest().is_some()
        );
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn write_stub_companions(assets: &std::path::Path, skip: Option<&str>) {
        for relative in [
            "a3s-oci",
            "a3s-oci-krun-shim",
            "system-image.json",
            "bundle/config.json",
            "bundle/rootfs/bin/bash",
        ] {
            if skip == Some(relative) {
                continue;
            }
            let path = assets.join(relative);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, b"stub").unwrap();
        }
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[tokio::test]
    async fn each_missing_companion_file_stays_a_typed_refusal() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        use crate::sandbox::SandboxCommandRequest;
        use crate::workspace::{CommandOutputObserver, CommandOutputSummary};

        struct Observer(AtomicUsize);

        #[async_trait::async_trait]
        impl CommandOutputObserver for Observer {
            async fn on_output_delta(&self, _delta: &str) {
                self.0.fetch_add(1, Ordering::SeqCst);
            }

            async fn on_output_complete(&self, _summary: &CommandOutputSummary) {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
        }

        for skip in [
            "a3s-oci",
            "a3s-oci-krun-shim",
            "system-image.json",
            "bundle/config.json",
            "bundle/rootfs/bin/bash",
        ] {
            let root = tempfile::tempdir().unwrap();
            let assets = root.path().join("assets");
            write_stub_companions(&assets, Some(skip));
            let workspace = root.path().join("work");
            std::fs::create_dir(&workspace).unwrap();
            let _env = AssetsEnv::set(&assets);
            let discovered = crate::sandbox::oci::OciBashSandbox::discover(&workspace)
                .expect_err("one missing companion file must fail discovery");
            let discovered = discovered.to_string();
            assert!(
                discovered.contains("sandbox companion is missing:"),
                "{skip}: {discovered}"
            );
            assert!(discovered.contains(skip), "{skip}: {discovered}");

            let mut opts = SessionOptions::new();
            install_default_local_sandbox(&workspace, &mut opts);
            let sandbox = opts.sandbox_handle.as_ref().unwrap();
            assert_ne!(sandbox.execution_overhead_ms(), 90_000, "{skip}");
            let observer = Arc::new(Observer(AtomicUsize::new(0)));
            let sentinel = workspace.join("missing-file-sentinel");
            let command = format!("printf leaked > '{}'", sentinel.display());
            let exec_command = sandbox.exec_command(&command, "/workspace").await;
            let exec = sandbox
                .exec(SandboxCommandRequest {
                    command,
                    guest_workspace: "/workspace".into(),
                    timeout_ms: 1_000,
                    output_observer: Some(observer.clone()),
                    env: None,
                })
                .await;
            for error in [exec_command.err(), exec.err()] {
                let error = error.expect("a missing companion file must not run the command");
                let message = error.to_string();
                assert!(
                    message.contains("native_process_lifetime_unqualified"),
                    "{skip}: {error:#}"
                );
                assert!(
                    message.contains("OCI companion discovery:"),
                    "{skip}: {error:#}"
                );
                assert!(message.contains(skip), "{skip}: {error:#}");
                assert!(crate::sandbox::native::is_native_process_lifetime_unqualified(&error));
            }
            assert_eq!(observer.0.load(Ordering::SeqCst), 0, "{skip}");
            assert!(!sentinel.exists(), "{skip}");
        }
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[tokio::test]
    async fn companions_inside_the_workspace_are_rejected() {
        use crate::sandbox::SandboxCommandRequest;

        async fn assert_rejected(assets: &std::path::Path, workspace: &std::path::Path) {
            let _env = AssetsEnv::set(assets);
            let discovered = crate::sandbox::oci::OciBashSandbox::discover(workspace)
                .expect_err("companions inside the workspace must not be discovered");
            assert!(
                discovered
                    .to_string()
                    .contains("outside the writable workspace"),
                "{discovered:#}"
            );
            let mut opts = SessionOptions::new();
            install_default_local_sandbox(workspace, &mut opts);
            let sandbox = opts.sandbox_handle.as_ref().unwrap();
            assert_ne!(sandbox.execution_overhead_ms(), 90_000);
            let sentinel = workspace.join("containment-sentinel");
            let command = format!("printf leaked > '{}'", sentinel.display());
            let exec_command = sandbox.exec_command(&command, "/workspace").await;
            let exec = sandbox
                .exec(SandboxCommandRequest {
                    command,
                    guest_workspace: "/workspace".into(),
                    timeout_ms: 1_000,
                    output_observer: None,
                    env: None,
                })
                .await;
            for error in [exec_command.err(), exec.err()] {
                let error = error.expect("contained companions must not run the command");
                assert!(crate::sandbox::native::is_native_process_lifetime_unqualified(&error));
            }
            assert!(!sentinel.exists());
        }

        let nested = tempfile::tempdir().unwrap();
        let workspace = nested.path().join("work");
        std::fs::create_dir(&workspace).unwrap();
        let inside = workspace.join("sandbox");
        write_stub_companions(&inside, None);
        assert_rejected(&inside, &workspace).await;

        let containing = tempfile::tempdir().unwrap();
        let assets = containing.path().join("assets");
        write_stub_companions(&assets, None);
        let child = assets.join("work");
        std::fs::create_dir(&child).unwrap();
        assert_rejected(&assets, &child).await;
    }

    /// Real companions: Deny plus the process-host opt-in executes on the OCI
    /// Host. The same companions with a mediated posture do not run.
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[ignore = "requires signed HVF assets; run explicitly on Apple Silicon"]
    async fn validated_companions_select_oci_for_deny_and_refuse_mediated() {
        let assets = std::env::var_os("ASTER_TEST_OCI_ASSETS")
            .expect("explicit real OCI assets in ASTER_TEST_OCI_ASSETS");
        let _assets_env = AssetsEnv::set(&assets);
        let workspace = tempfile::tempdir_in("/private/var/tmp").unwrap();
        let root = workspace.path().canonicalize().unwrap();
        crate::sandbox::oci::OciBashSandbox::discover(&root)
            .expect("validated companions must discover before either posture runs");

        let mut deny_opts = SessionOptions::new().with_allow_process_host_sandbox(true);
        install_default_local_sandbox(&root, &mut deny_opts);
        let deny = Arc::clone(deny_opts.sandbox_handle.as_ref().unwrap());
        assert_eq!(deny.execution_overhead_ms(), 90_000);
        assert!(deny.policy_digest().is_none());
        let baseline_hosts = hvf_host_count();
        let watch = HostWatch::start();
        let exec_output = deny
            .exec(SandboxCommandRequest {
                command: "printf 'deny-exec\\n'; uname -s; exit 7".into(),
                guest_workspace: root.display().to_string(),
                timeout_ms: 120_000,
                output_observer: None,
                env: None,
            })
            .await
            .expect("deny exec through the installed OCI Host");
        let exec_host = watch.saw();
        watch.reset();
        let command_output = deny
            .exec_command(
                "printf 'deny-exec-command\\n'; uname -s; exit 8",
                &root.display().to_string(),
            )
            .await
            .expect("deny exec_command through the installed OCI Host");
        let command_host = watch.saw();
        let deny_transcript = format!(
            "deny workspace={} overhead={} exec_exit={} exec_host={exec_host} exec_stdout={:?} exec_stderr={:?} command_exit={} command_host={command_host} command_stdout={:?} command_stderr={:?}",
            root.display(),
            deny.execution_overhead_ms(),
            exec_output.exit_code,
            exec_output.stdout,
            exec_output.stderr,
            command_output.exit_code,
            command_output.stdout,
            command_output.stderr,
        );
        eprintln!("{deny_transcript}");
        assert!(
            exec_output.stdout.contains("deny-exec\n"),
            "blank or foreign stdout is a product failure: {deny_transcript}"
        );
        assert!(
            exec_output.stdout.contains("Linux\n"),
            "a host-process result is a product failure: {deny_transcript}"
        );
        assert_eq!(exec_output.exit_code, 7, "{deny_transcript}");
        assert!(
            exec_host,
            "macos-hvf-host-service was not alive during exec"
        );
        assert!(
            command_output.stdout.contains("deny-exec-command\n"),
            "{deny_transcript}"
        );
        assert!(
            command_output.stdout.contains("Linux\n"),
            "a host-process result is a product failure: {deny_transcript}"
        );
        assert_eq!(command_output.exit_code, 8, "{deny_transcript}");
        assert!(
            command_host,
            "macos-hvf-host-service was not alive during exec_command"
        );
        deny.shutdown().await;
        drop(watch);
        wait_for_hvf_host_count(baseline_hosts).await;

        let sentinel = root.join("mediated-sentinel");
        let mut mediated_opts = SessionOptions::new()
            .with_allow_process_host_sandbox(true)
            .with_sandbox_network_mode(crate::sandbox::SandboxNetworkMode::Mediated)
            .with_sandbox_network_allow(vec!["example.com".into()]);
        install_default_local_sandbox(&root, &mut mediated_opts);
        let mediated = Arc::clone(mediated_opts.sandbox_handle.as_ref().unwrap());
        assert_ne!(mediated.execution_overhead_ms(), 90_000);
        let command = format!("printf leaked > '{}'", sentinel.display());
        let watch = HostWatch::start();
        let exec_error = mediated
            .exec(SandboxCommandRequest {
                command: command.clone(),
                guest_workspace: root.display().to_string(),
                timeout_ms: 30_000,
                output_observer: None,
                env: None,
            })
            .await
            .err()
            .expect("mediated posture must not run on the OCI Host or process-host");
        let command_error = mediated
            .exec_command(&command, &root.display().to_string())
            .await
            .err()
            .expect("mediated exec_command must not run");
        let mediated_host = watch.saw();
        let hosts_after = hvf_host_count();
        let mediated_transcript = format!(
            "mediated overhead={} digest_some={} host_seen={mediated_host} hosts_before={baseline_hosts} hosts_after={hosts_after} sentinel={} exec_error={} command_error={}",
            mediated.execution_overhead_ms(),
            mediated.policy_digest().is_some(),
            sentinel.exists(),
            exec_error,
            command_error,
        );
        eprintln!("{mediated_transcript}");
        for error in [&exec_error, &command_error] {
            let message = error.to_string();
            assert!(
                message.contains("native_process_lifetime_unqualified")
                    || message.contains("process-host fallback was NOT installed"),
                "{mediated_transcript}"
            );
        }
        assert!(!sentinel.exists(), "{mediated_transcript}");
        assert_eq!(hosts_after, baseline_hosts, "{mediated_transcript}");
        if baseline_hosts == 0 {
            assert!(!mediated_host, "{mediated_transcript}");
        }
        mediated.shutdown().await;
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    struct AssetsEnv {
        previous: Option<std::ffi::OsString>,
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    impl AssetsEnv {
        fn set(directory: impl AsRef<std::ffi::OsStr>) -> Self {
            let previous = std::env::var_os(crate::sandbox::oci::SANDBOX_ASSETS_ENV);
            std::env::set_var(crate::sandbox::oci::SANDBOX_ASSETS_ENV, directory);
            Self { previous }
        }
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    impl Drop for AssetsEnv {
        fn drop(&mut self) {
            match self.previous.take() {
                Some(value) => std::env::set_var(crate::sandbox::oci::SANDBOX_ASSETS_ENV, value),
                None => std::env::remove_var(crate::sandbox::oci::SANDBOX_ASSETS_ENV),
            }
        }
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn hvf_host_count() -> usize {
        let output = std::process::Command::new("/bin/ps")
            .args(["-ax", "-o", "command="])
            .output()
            .expect("ps");
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter(|line| line.contains("macos-hvf-host-service"))
            .count()
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    async fn wait_for_hvf_host_count(expected: usize) {
        let start = std::time::Instant::now();
        while start.elapsed() < std::time::Duration::from_secs(20) {
            if hvf_host_count() == expected {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        panic!(
            "macos-hvf-host-service count stayed {}, expected {expected}",
            hvf_host_count()
        );
    }

    /// The guest agent inside the image discovery's host will boot must hello
    /// with that host binary's runtime version. A mismatch is not success.
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[ignore = "requires signed HVF assets; run explicitly on Apple Silicon"]
    async fn guest_hello_matches_the_host_runtime_discovery_launches() {
        let assets = std::env::var_os("ASTER_TEST_OCI_ASSETS")
            .expect("explicit real OCI assets in ASTER_TEST_OCI_ASSETS");
        let _assets_env = AssetsEnv::set(&assets);
        let directory = std::path::PathBuf::from(&assets);
        let workspace = tempfile::tempdir_in("/private/var/tmp").unwrap();
        let root = workspace.path().canonicalize().unwrap();
        crate::sandbox::oci::OciBashSandbox::discover(&root)
            .expect("discovery must open the companions this test reads");

        let manifest: serde_json::Value = serde_json::from_slice(
            &std::fs::read(directory.join("system-image.json")).expect("system image manifest"),
        )
        .expect("system image manifest is json");
        let image_name = manifest["image"]["name"]
            .as_str()
            .expect("manifest image.name is the file the host boots");
        assert!(
            !image_name.is_empty()
                && !image_name.contains('/')
                && !image_name.contains('\\')
                && !image_name.contains('\0'),
            "image.name must be one plain file name, got {image_name:?}"
        );
        let image = directory.join(image_name);
        let host_path = directory.join("a3s-oci");
        let host_bytes = std::fs::read(&host_path).expect("host binary discovery launches");
        let agent_path = root.join("a3s-oci-agent");
        let agent_bytes = guest_agent_bytes(&image, &agent_path);
        let host_version = crate::sandbox::oci::host_runtime_version(&host_bytes)
            .expect("host runtime version sits before the mismatch error");
        let hello = crate::sandbox::oci::guest_agent_hello(&agent_bytes, "aarch64")
            .expect("guest hello sits immediately before aarch64");
        eprintln!(
            "host_binary={} image={} host_runtime={host_version} guest_hello={hello}",
            host_path.display(),
            image.display()
        );
        assert_eq!(
            hello, host_version,
            "the launched pair disagrees; a mismatch is not a successful command"
        );

        let mut opts = SessionOptions::new();
        install_default_local_sandbox(&root, &mut opts);
        let sandbox = Arc::clone(opts.sandbox_handle.as_ref().unwrap());
        assert_eq!(sandbox.execution_overhead_ms(), 90_000);
        let output = sandbox
            .exec(SandboxCommandRequest {
                command: "printf 'version-pair-ok\\n'; exit 4".into(),
                guest_workspace: root.display().to_string(),
                timeout_ms: 120_000,
                output_observer: None,
                env: None,
            })
            .await
            .expect("a matched pair returns the program result, not a version error");
        let transcript = format!(
            "version_exit={} version_stdout={:?} version_stderr={:?}",
            output.exit_code, output.stdout, output.stderr
        );
        eprintln!("{transcript}");
        assert!(
            !output.stdout.contains("guest agent version does not match")
                && !output.stderr.contains("guest agent version does not match"),
            "{transcript}"
        );
        assert!(output.stdout.contains("version-pair-ok\n"), "{transcript}");
        assert!(!output.stdout.trim().is_empty(), "{transcript}");
        assert_eq!(output.exit_code, 4, "{transcript}");
        sandbox.shutdown().await;
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn guest_agent_bytes(image: &std::path::Path, destination: &std::path::Path) -> Vec<u8> {
        let debugfs = "/opt/homebrew/opt/e2fsprogs/sbin/debugfs";
        let dump = format!("dump /usr/bin/a3s-oci-agent {}", destination.display());
        let output = std::process::Command::new(debugfs)
            .args(["-R", &dump])
            .arg(image)
            .output()
            .unwrap_or_else(|error| panic!("debugfs is required to read the guest agent: {error}"));
        assert!(
            output.status.success(),
            "debugfs dump of /usr/bin/a3s-oci-agent failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        std::fs::read(destination).expect("dumped guest agent")
    }

    /// Installer path: caller-only uid map, denied credential and hardlink,
    /// no outside file, and a `setsid` descendant that stops after timeout
    /// or cancellation. The timeout case runs twice.
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[ignore = "requires signed HVF assets; run explicitly on Apple Silicon"]
    async fn default_install_checks_uid_map_credentials_and_setsid() {
        use std::os::unix::fs::PermissionsExt;

        let assets = std::env::var_os("ASTER_TEST_OCI_ASSETS")
            .expect("explicit real OCI assets in ASTER_TEST_OCI_ASSETS");
        let _assets_env = AssetsEnv::set(&assets);
        let parent = tempfile::Builder::new()
            .prefix("aster-contract-")
            .tempdir_in("/private/var/tmp")
            .unwrap();
        let workspace = parent.path().join("work");
        let outside = parent.path().join("outside");
        std::fs::create_dir(&workspace).unwrap();
        std::fs::create_dir(&outside).unwrap();
        std::fs::write(workspace.join("marker.txt"), b"CONTRACT-MARKER-9f3c").unwrap();
        std::fs::write(workspace.join(".env"), b"credential").unwrap();
        std::fs::create_dir_all(workspace.join("node_modules/pkg")).unwrap();
        std::fs::hard_link(
            workspace.join(".env"),
            workspace.join("node_modules/pkg/config"),
        )
        .unwrap();
        std::fs::set_permissions(&workspace, std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::set_permissions(
            workspace.join("marker.txt"),
            std::fs::Permissions::from_mode(0o600),
        )
        .unwrap();
        std::fs::set_permissions(
            workspace.join(".env"),
            std::fs::Permissions::from_mode(0o600),
        )
        .unwrap();
        let root = workspace.canonicalize().unwrap();
        let escape = outside.canonicalize().unwrap().join("escaped");
        let euid = unsafe { libc::geteuid() };
        let egid = unsafe { libc::getegid() };

        let mut opts = SessionOptions::new();
        install_default_local_sandbox(&root, &mut opts);
        let sandbox = Arc::clone(opts.sandbox_handle.as_ref().unwrap());
        assert_eq!(sandbox.execution_overhead_ms(), 90_000);

        let command = format!(
            "marker=$(cat marker.txt) || marker=UNREADABLE\n\
             printf 'marker=%s\\n' \"$marker\"\n\
             read -r u0 u1 u2 _ < /proc/self/uid_map\n\
             printf 'uid=%s %s %s\\n' \"$u0\" \"$u1\" \"$u2\"\n\
             read -r g0 g1 g2 _ < /proc/self/gid_map\n\
             printf 'gid=%s %s %s\\n' \"$g0\" \"$g1\" \"$g2\"\n\
             if cat .env >/dev/null 2>&1; then echo env=readable; else echo env=unreadable; fi\n\
             if cat node_modules/pkg/config >/dev/null 2>&1; then echo link=readable; else echo link=unreadable; fi\n\
             if printf leaked > '{}' 2>/dev/null; then echo outside=wrote; else echo outside=blocked; fi\n\
             exit 7",
            escape.display()
        );
        let output = sandbox
            .exec(SandboxCommandRequest {
                command,
                guest_workspace: root.display().to_string(),
                timeout_ms: 120_000,
                output_observer: None,
                env: None,
            })
            .await
            .expect("installer command returns the program result");
        let transcript = format!(
            "contract_exit={} contract_stdout={:?} contract_stderr={:?} escape_exists={}",
            output.exit_code,
            output.stdout,
            output.stderr,
            escape.exists()
        );
        eprintln!("{transcript}");
        assert_eq!(output.exit_code, 7, "{transcript}");
        assert!(
            output.stdout.contains("marker=CONTRACT-MARKER-9f3c\n"),
            "a readable marker distinguishes policy denial from a uid-map failure: {transcript}"
        );
        assert!(
            output.stdout.contains(&format!("uid=0 {euid} 1\n")),
            "{transcript}"
        );
        assert!(
            output.stdout.contains(&format!("gid=0 {egid} 1\n")),
            "{transcript}"
        );
        assert!(output.stdout.contains("env=unreadable\n"), "{transcript}");
        assert!(output.stdout.contains("link=unreadable\n"), "{transcript}");
        assert!(output.stdout.contains("outside=blocked\n"), "{transcript}");
        assert!(!escape.exists(), "{transcript}");
        assert_eq!(std::fs::read(root.join(".env")).unwrap(), b"credential");

        for counter in ["counter-a", "counter-b"] {
            let output = sandbox
                .exec(SandboxCommandRequest {
                    command: setsid_loop(counter),
                    guest_workspace: root.display().to_string(),
                    timeout_ms: 1_000,
                    output_observer: None,
                    env: None,
                })
                .await
                .expect("timeout settles the setsid descendant");
            let path = root.join(counter);
            let length = std::fs::metadata(&path)
                .unwrap_or_else(|error| panic!("{counter} missing after timeout: {error}"))
                .len();
            eprintln!(
                "timeout counter={counter} exit={} timed_out={} length={length}",
                output.exit_code, output.timed_out
            );
            assert!(output.timed_out, "{counter}");
            assert_eq!(output.exit_code, 124, "{counter}");
            assert!(length > 0, "{counter}");
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            assert_eq!(
                std::fs::metadata(&path).unwrap().len(),
                length,
                "{counter} kept writing after timeout"
            );
        }

        let running = Arc::clone(&sandbox);
        let cancel_root = root.clone();
        let task = tokio::spawn(async move {
            running
                .exec(SandboxCommandRequest {
                    command: setsid_loop("counter-c"),
                    guest_workspace: cancel_root.display().to_string(),
                    timeout_ms: 60_000,
                    output_observer: None,
                    env: None,
                })
                .await
        });
        tokio::time::timeout(std::time::Duration::from_secs(45), async {
            while !root.join("counter-c").exists() {
                tokio::time::sleep(std::time::Duration::from_millis(25)).await;
            }
        })
        .await
        .expect("cancelled descendant did not start writing");
        task.abort();
        assert!(
            matches!(task.await, Err(error) if error.is_cancelled()),
            "cancellation must drop the exec future"
        );
        tokio::time::timeout(std::time::Duration::from_secs(35), sandbox.shutdown())
            .await
            .expect("shutdown settles the owned host");
        let length = std::fs::metadata(root.join("counter-c")).unwrap().len();
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        assert_eq!(
            std::fs::metadata(root.join("counter-c")).unwrap().len(),
            length,
            "cancelled setsid descendant kept writing"
        );
        assert!(length > 0);
        eprintln!("cancel counter-c length={length} stable=true");
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn setsid_loop(counter: &str) -> String {
        format!(
            "/bin/busybox setsid /bin/bash -c 'while :; do printf x >> {counter}; sleep 0.05; done' >/dev/null 2>&1 & wait"
        )
    }

    /// The session AsterDream installs: Bash, a Python program through that
    /// Bash backend, and the in-process read tool, on a caller-owned mode
    /// `0700` workspace whose path contains a literal backslash.
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[ignore = "requires signed HVF assets; run explicitly on Apple Silicon"]
    async fn default_local_sandbox_reads_a_private_backslash_workspace() {
        use std::os::unix::fs::PermissionsExt;

        use crate::tools::Tool;

        const MARKER_NAME: &str = "private-bytes.txt";
        let assets = std::env::var_os("ASTER_TEST_OCI_ASSETS")
            .expect("explicit real OCI assets in ASTER_TEST_OCI_ASSETS");
        let previous_assets = std::env::var_os(crate::sandbox::oci::SANDBOX_ASSETS_ENV);
        std::env::set_var(crate::sandbox::oci::SANDBOX_ASSETS_ENV, &assets);

        let parent = tempfile::Builder::new()
            .prefix("aster-core-")
            .tempdir_in("/private/var/tmp")
            .unwrap();
        let workspace = parent.path().join("leaf\\name");
        let outside = parent.path().join("outside-sink");
        std::fs::create_dir(&workspace).unwrap();
        std::fs::create_dir(&outside).unwrap();
        let marker = workspace.join(MARKER_NAME);
        let file_bytes = b"CORE-0700-BYTES-9f3c".to_vec();
        std::fs::write(&marker, &file_bytes).unwrap();
        std::fs::set_permissions(&marker, std::fs::Permissions::from_mode(0o600)).unwrap();
        std::fs::set_permissions(&workspace, std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::set_permissions(&outside, std::fs::Permissions::from_mode(0o700)).unwrap();
        let workspace = workspace.canonicalize().unwrap();
        assert!(
            workspace
                .components()
                .any(|component| component.as_os_str().to_string_lossy().contains('\\')),
            "the workspace path must keep a literal backslash"
        );
        let file_bytes = std::fs::read(workspace.join(MARKER_NAME)).unwrap();
        let file_text = String::from_utf8(file_bytes.clone()).unwrap();

        let mut opts = SessionOptions::new();
        install_default_local_sandbox(&workspace, &mut opts);
        let sandbox = Arc::clone(opts.sandbox_handle.as_ref().unwrap());
        let context =
            crate::tools::ToolContext::new(workspace.clone()).with_sandbox(Arc::clone(&sandbox));
        let watch = HostWatch::start();
        let escape = outside.join("escaped");
        let mut transcripts = Vec::new();
        for run in 1..=2 {
            assert!(
                !escape.exists(),
                "a previous run left a file outside the workspace"
            );
            let bash = format!(
                "cat {MARKER_NAME}; printf '\\nkern=%s\\n' \"$(uname -s)\"; printf 'pwd=%s\\n' \"$PWD\"; printf allowed > allowed-write.txt; printf leaked > '{}' || true; exit 7",
                escape.display()
            );
            watch.reset();
            let bash_output = crate::tools::builtin::bash::BashTool
                .execute(
                    &serde_json::json!({"command": bash, "timeout": 120000}),
                    &context,
                )
                .await
                .expect("bash tool returns a settled result");
            let bash_saw_host = watch.saw();
            let python = "PYTHONDONTWRITEBYTECODE=1 python3 -c 'import os, pathlib, sys; data = pathlib.Path(\"private-bytes.txt\").read_bytes(); sys.stdout.buffer.write(data); sys.stdout.buffer.write(b\"\\nkern=\"); sys.stdout.buffer.write(os.uname().sysname.encode()); sys.stdout.buffer.write(b\"\\n\"); raise SystemExit(7)'";
            watch.reset();
            let python_output = crate::tools::builtin::bash::BashTool
                .execute(
                    &serde_json::json!({"command": python, "timeout": 120000}),
                    &context,
                )
                .await
                .expect("python through bash returns a settled result");
            let python_saw_host = watch.saw();
            let read_output = crate::tools::builtin::read::ReadTool
                .execute(&serde_json::json!({"file_path": MARKER_NAME}), &context)
                .await
                .expect("read tool returns a settled result");
            let transcript = format!(
                "run={run}\nworkspace={}\nfile_bytes={file_text:?}\nbash_exit={:?}\nbash_host={bash_saw_host}\nbash={:?}\npython_exit={:?}\npython_host={python_saw_host}\npython={:?}\nread_success={}\nread={:?}\nallowed={:?}\nescape_exists={}\n",
                workspace.display(),
                exit_code_of(&bash_output),
                bash_output.content,
                exit_code_of(&python_output),
                python_output.content,
                read_output.success,
                read_output.content,
                std::fs::read(workspace.join("allowed-write.txt")).ok(),
                escape.exists(),
            );
            eprintln!("{transcript}");
            assert!(
                bash_output.content.contains(&file_text),
                "bash did not return the file bytes: {transcript}"
            );
            assert!(
                bash_output.content.contains("kern=Linux"),
                "bash did not run in the VM: {transcript}"
            );
            assert!(
                bash_output
                    .content
                    .contains(&workspace.display().to_string()),
                "bash did not use the backslash workspace path: {transcript}"
            );
            assert_eq!(
                exit_code_of(&bash_output),
                Some(7),
                "bash exit is the program's: {transcript}"
            );
            assert!(
                bash_saw_host,
                "OCI Host was not alive during bash: {transcript}"
            );
            assert!(
                python_output.content.contains(&file_text),
                "python did not return the file bytes: {transcript}"
            );
            assert!(
                python_output.content.contains("kern=Linux"),
                "python did not run in the VM: {transcript}"
            );
            assert_eq!(
                exit_code_of(&python_output),
                Some(7),
                "python exit is the program's: {transcript}"
            );
            assert!(
                python_saw_host,
                "OCI Host was not alive during python: {transcript}"
            );
            assert!(read_output.success, "read failed: {transcript}");
            assert!(
                read_output.content.contains(&file_text),
                "read did not return the file bytes: {transcript}"
            );
            assert_eq!(
                std::fs::read(workspace.join("allowed-write.txt")).unwrap(),
                b"allowed",
                "an allowed write must appear on the host: {transcript}"
            );
            assert!(
                !escape.exists(),
                "a write outside the workspace created a host file: {transcript}"
            );
            transcripts.push(transcript);
        }
        assert_eq!(
            transcripts[0]
                .lines()
                .filter(|line| !line.starts_with("run="))
                .collect::<Vec<_>>(),
            transcripts[1]
                .lines()
                .filter(|line| !line.starts_with("run="))
                .collect::<Vec<_>>()
        );
        sandbox.shutdown().await;
        drop(watch);
        match previous_assets {
            Some(value) => std::env::set_var(crate::sandbox::oci::SANDBOX_ASSETS_ENV, value),
            None => std::env::remove_var(crate::sandbox::oci::SANDBOX_ASSETS_ENV),
        }
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn exit_code_of(output: &crate::tools::ToolOutput) -> Option<i64> {
        output
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.get("exit_code"))
            .and_then(serde_json::Value::as_i64)
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    struct HostWatch {
        seen: Arc<std::sync::atomic::AtomicBool>,
        stop: Arc<std::sync::atomic::AtomicBool>,
        thread: Option<std::thread::JoinHandle<()>>,
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    impl HostWatch {
        fn start() -> Self {
            use std::sync::atomic::{AtomicBool, Ordering};
            let seen = Arc::new(AtomicBool::new(false));
            let stop = Arc::new(AtomicBool::new(false));
            let seen_flag = Arc::clone(&seen);
            let stop_flag = Arc::clone(&stop);
            let thread = std::thread::spawn(move || {
                while !stop_flag.load(Ordering::Relaxed) {
                    if let Ok(output) = std::process::Command::new("/bin/ps")
                        .args(["-ax", "-o", "command="])
                        .output()
                    {
                        if String::from_utf8_lossy(&output.stdout)
                            .contains("macos-hvf-host-service")
                        {
                            seen_flag.store(true, Ordering::Relaxed);
                        }
                    }
                    std::thread::sleep(std::time::Duration::from_millis(30));
                }
            });
            Self {
                seen,
                stop,
                thread: Some(thread),
            }
        }

        fn reset(&self) {
            self.seen.store(false, std::sync::atomic::Ordering::Relaxed);
        }

        fn saw(&self) -> bool {
            self.seen.load(std::sync::atomic::Ordering::Relaxed)
        }
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    impl Drop for HostWatch {
        fn drop(&mut self) {
            self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
            if let Some(thread) = self.thread.take() {
                let _ = thread.join();
            }
        }
    }
}
