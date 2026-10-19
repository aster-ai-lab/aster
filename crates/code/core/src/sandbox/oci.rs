//! One authenticated OCI utility VM per command, with an owned Host lifetime pipe.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use a3s_oci_sdk::{
    ContainerTarget, DeleteMode, DeleteRequest, LocalIpcEndpoint, OperationContext, OperationId,
    OutputStream, ProcessId, ProcessTarget, ReadOutputRequest, RuntimeClient, StartRequest,
    WaitRequest, RUNTIME_BUNDLE_HANDOFF_EXTENSION, RUNTIME_BUNDLE_HANDOFF_EXTERNAL_STORAGE_VERSION,
    STORAGE_PATH_RESTRICTIONS_EXTENSION, STORAGE_PATH_RESTRICTIONS_VERSION,
};
use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use tokio::process::{Child, Command};
use tokio_util::{sync::CancellationToken, task::TaskTracker};

use super::{BashSandbox, SandboxCommandRequest, SandboxExecutionOutput, SandboxOutput};
use crate::tools::process::{BoundedCapture, OutputStream as CaptureStream};

mod bundle;
mod output;
mod version;

pub(crate) use version::{guest_agent_hello, host_runtime_version};

/// Explicit asset-directory override for installed or development companions.
pub const SANDBOX_ASSETS_ENV: &str = "ASTER_SANDBOX_DIR";

/// `ASTER_SANDBOX_DIR` wins. Otherwise companions sit beside the executable.
pub(crate) fn companion_directory(explicit: Option<&Path>, executable: &Path) -> Result<PathBuf> {
    if let Some(directory) = explicit {
        return Ok(directory.to_path_buf());
    }
    Ok(executable
        .parent()
        .context("executable has no companion directory")?
        .join("sandbox"))
}
const STARTUP_TIMEOUT: Duration = Duration::from_secs(45);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone, Debug)]
pub struct OciSandboxAssets {
    directory: PathBuf,
    host: PathBuf,
    shim: PathBuf,
    image: PathBuf,
    bundle: PathBuf,
}

impl OciSandboxAssets {
    /// Open companions from a trusted installed directory, outside the workspace.
    pub fn from_directory(directory: &Path) -> Result<Self> {
        let directory = directory
            .canonicalize()
            .context("sandbox companion directory is unavailable")?;
        let assets = Self {
            host: directory.join("a3s-oci"),
            shim: directory.join("a3s-oci-krun-shim"),
            image: directory.join("system-image.json"),
            bundle: directory.join("bundle"),
            directory,
        };
        assets.validate()?;
        Ok(assets)
    }

    fn validate(&self) -> Result<()> {
        for path in [
            &self.host,
            &self.shim,
            &self.image,
            &self.bundle.join("config.json"),
            &self.bundle.join("rootfs/bin/bash"),
        ] {
            if !path.is_file() {
                bail!("sandbox companion is missing: {}", path.display());
            }
        }
        Ok(())
    }
}

/// Default macOS Bash boundary backed only by the public OCI SDK.
#[derive(Debug)]
pub struct OciBashSandbox {
    workspace: PathBuf,
    assets: OciSandboxAssets,
    cancellation: CancellationToken,
    workers: TaskTracker,
}

impl OciBashSandbox {
    pub fn discover(workspace: &Path) -> Result<Self> {
        let explicit = std::env::var_os(SANDBOX_ASSETS_ENV).map(PathBuf::from);
        let executable = std::env::current_exe()?;
        let directory = companion_directory(explicit.as_deref(), &executable)?;
        Self::from_artifacts(workspace, OciSandboxAssets::from_directory(&directory)?)
    }

    pub fn from_artifacts(workspace: &Path, assets: OciSandboxAssets) -> Result<Self> {
        let workspace = workspace.canonicalize()?;
        if !workspace.is_dir() {
            bail!("sandbox workspace is not a directory");
        }
        if assets.directory.starts_with(&workspace) || workspace.starts_with(&assets.directory) {
            bail!("sandbox companions must be outside the writable workspace");
        }
        Ok(Self {
            workspace,
            assets,
            cancellation: CancellationToken::new(),
            workers: TaskTracker::new(),
        })
    }
}

#[cfg(test)]
mod companion_directory_tests {
    use super::companion_directory;
    use std::path::Path;

    #[test]
    fn companion_directory_uses_the_env_and_otherwise_the_executable_sibling() {
        let root = tempfile::tempdir().unwrap();
        let explicit = root.path().join("explicit");
        let executable = root.path().join("bin").join("a3s-code-acp");
        std::fs::create_dir_all(executable.parent().unwrap().join("sandbox")).unwrap();

        let chosen = companion_directory(Some(&explicit), &executable).unwrap();
        assert_eq!(chosen, explicit);

        let sibling = companion_directory(None, &executable).unwrap();
        assert_eq!(sibling, executable.parent().unwrap().join("sandbox"));

        let error = companion_directory(None, Path::new("/")).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("executable has no companion directory"),
            "{error}"
        );
    }
}

struct CancelOnDrop(CancellationToken);
impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.cancel();
    }
}
impl Drop for OciBashSandbox {
    fn drop(&mut self) {
        self.cancellation.cancel();
    }
}

#[async_trait]
impl BashSandbox for OciBashSandbox {
    fn execution_target(&self) -> super::ShellExecutionTarget {
        super::ShellExecutionTarget::LinuxGuestBash
    }

    fn validate_execution_available(&self) -> Result<()> {
        if self.cancellation.is_cancelled() {
            bail!("OCI sandbox has shut down");
        }
        self.assets.validate()
    }

    async fn exec_command(&self, command: &str, guest_workspace: &str) -> Result<SandboxOutput> {
        let output = self
            .exec(SandboxCommandRequest {
                command: command.into(),
                guest_workspace: guest_workspace.into(),
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

    async fn exec(&self, request: SandboxCommandRequest) -> Result<SandboxExecutionOutput> {
        self.validate_execution_available()
            .map_err(|error| super::native::native_process_lifetime_unqualified().context(error))?;
        if request.timeout_ms == 0 || request.command.contains('\0') {
            bail!("invalid sandbox command or timeout");
        }
        let cancellation = self.cancellation.child_token();
        let _cancel = CancelOnDrop(cancellation.clone());
        let assets = self.assets.clone();
        let workspace = self.workspace.clone();
        self.workers
            .spawn(async move { execute(workspace, assets, request, cancellation).await })
            .await
            .context("OCI command worker failed")?
    }

    fn execution_overhead_ms(&self) -> u64 {
        90_000
    }

    async fn shutdown(&self) {
        self.cancellation.cancel();
        self.workers.close();
        self.workers.wait().await;
    }
}

struct Host {
    child: Child,
}

impl Host {
    fn spawn(assets: &OciSandboxAssets, root: &Path) -> Result<Self> {
        let log = std::fs::File::create(root.join("host.log"))?;
        let child = Command::new(&assets.host)
            .args(["macos-hvf-host-service", "--exit-on-stdin-eof", "--root"])
            .arg(root.join("s"))
            .arg("--shim")
            .arg(&assets.shim)
            .arg("--system-image-manifest")
            .arg(&assets.image)
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(log)
            .kill_on_drop(true)
            .spawn()
            .context("start OCI Host companion")?;
        Ok(Self { child })
    }

    async fn connect(&mut self, root: &Path) -> Result<RuntimeClient> {
        let endpoint = LocalIpcEndpoint::unix_socket(root.join("s/runtime.sock"))?;
        loop {
            if let Some(status) = self.child.try_wait()? {
                bail!("OCI Host exited during initialization: {status}");
            }
            if let Ok(Ok(client)) =
                tokio::time::timeout(Duration::from_secs(1), RuntimeClient::connect(&endpoint))
                    .await
            {
                let features = client.features().await?;
                for (name, version) in [
                    (
                        RUNTIME_BUNDLE_HANDOFF_EXTENSION,
                        RUNTIME_BUNDLE_HANDOFF_EXTERNAL_STORAGE_VERSION,
                    ),
                    (
                        STORAGE_PATH_RESTRICTIONS_EXTENSION,
                        STORAGE_PATH_RESTRICTIONS_VERSION,
                    ),
                ] {
                    if !features.attachments.supports_extension(name, version) {
                        bail!("OCI Host cannot enforce required extension {name} v{version}");
                    }
                }
                return Ok(client);
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }

    async fn shutdown(&mut self) -> Result<()> {
        drop(self.child.stdin.take());
        let status = match tokio::time::timeout(SHUTDOWN_TIMEOUT, self.child.wait()).await {
            Ok(status) => status?,
            Err(_) => {
                self.child.kill().await?;
                bail!("OCI Host shutdown did not prove VM settlement");
            }
        };
        if !status.success() {
            bail!("OCI Host cleanup failed: {status}");
        }
        Ok(())
    }
}

#[tracing::instrument(name = "oci_command", skip_all)]
async fn execute(
    workspace: PathBuf,
    assets: OciSandboxAssets,
    request: SandboxCommandRequest,
    cancellation: CancellationToken,
) -> Result<SandboxExecutionOutput> {
    if cancellation.is_cancelled() {
        bail!("OCI command cancelled before startup");
    }
    // macOS TMPDIR can exceed the Unix-socket path limit.
    let root = tempfile::Builder::new()
        .prefix("aster-vm-")
        .tempdir_in("/private/var/tmp")?;
    let root_path = root.path().canonicalize()?;
    let mut host = Host::spawn(&assets, &root_path)?;
    let mut capture = BoundedCapture::new();
    let mut payload_may_have_started = false;
    let startup = async {
        let began = Instant::now();
        let client = host.connect(&root_path).await?;
        tracing::debug!(
            phase = "host_connect",
            elapsed_ms = began.elapsed().as_secs_f64() * 1000.0,
            "OCI command phase completed"
        );
        let began = Instant::now();
        let create =
            bundle::prepare(&workspace, &assets, &root_path, &request, &cancellation).await?;
        tracing::debug!(
            phase = "bundle_prepare",
            elapsed_ms = began.elapsed().as_secs_f64() * 1000.0,
            "OCI command phase completed"
        );
        let id = create.id.clone();
        let began = Instant::now();
        let record = client.create(create).await?;
        tracing::debug!(
            phase = "vm_create",
            elapsed_ms = began.elapsed().as_secs_f64() * 1000.0,
            "OCI command phase completed"
        );
        let target = ContainerTarget::exact(id, record.generation);
        payload_may_have_started = true;
        let began = Instant::now();
        client
            .start(StartRequest {
                context: operation("start")?,
                target: target.clone(),
            })
            .await?;
        tracing::debug!(
            phase = "process_start",
            elapsed_ms = began.elapsed().as_secs_f64() * 1000.0,
            "OCI command phase completed"
        );
        Ok::<_, anyhow::Error>((client, target))
    };
    let prepared = tokio::select! {
        _ = cancellation.cancelled() => Err(anyhow::anyhow!("OCI command cancelled during startup")),
        result = tokio::time::timeout(STARTUP_TIMEOUT, startup) => result.context("OCI sandbox startup exceeded its deadline").and_then(|value| value),
    };
    let result = match prepared {
        Ok((client, target)) => {
            let began = Instant::now();
            let execution = collect(
                &client,
                &target,
                request.output_observer.as_deref(),
                &mut capture,
            );
            let result = tokio::select! {
                _ = cancellation.cancelled() => Err(anyhow::anyhow!("OCI command cancelled")),
                result = tokio::time::timeout(Duration::from_millis(request.timeout_ms), execution) => match result {
                    Ok(result) => result.map(|code| (code, false)),
                    Err(_) => Ok((124, true)),
                },
            };
            tracing::debug!(
                phase = "collect_wait",
                elapsed_ms = began.elapsed().as_secs_f64() * 1000.0,
                "OCI command phase completed"
            );
            if result.is_ok() {
                let began = Instant::now();
                match operation("delete") {
                    Ok(context) => match client
                        .delete(DeleteRequest {
                            context,
                            target,
                            mode: DeleteMode::Force,
                        })
                        .await
                    {
                        Ok(()) => {
                            tracing::debug!(
                                phase = "vm_delete",
                                elapsed_ms = began.elapsed().as_secs_f64() * 1000.0,
                                "OCI command phase completed"
                            );
                            result
                        }
                        Err(error) => Err(error.into()),
                    },
                    Err(error) => Err(error.into()),
                }
            } else {
                result
            }
        }
        Err(error) => Err(error),
    };
    // EOF makes the independent Host await every owned VM before reporting success.
    let began = Instant::now();
    host.shutdown()
        .await
        .context("OCI command settlement is unknown")?;
    tracing::debug!(
        phase = "host_shutdown",
        elapsed_ms = began.elapsed().as_secs_f64() * 1000.0,
        "OCI command phase completed"
    );
    let (exit_code, timed_out) = result.map_err(|error| {
        if payload_may_have_started {
            error
        } else {
            super::native::native_process_lifetime_unqualified().context(error)
        }
    })?;
    if let Some(observer) = request.output_observer {
        tokio::time::timeout(
            Duration::from_secs(2),
            observer.on_output_complete(&capture.summary(timed_out)),
        )
        .await
        .context("command observer did not finish")?;
    }
    let output = SandboxExecutionOutput {
        stdout: capture.render_stream(CaptureStream::Stdout),
        stderr: capture.render_stream(CaptureStream::Stderr),
        exit_code,
        timed_out,
    };
    let began = Instant::now();
    drop(root);
    tracing::debug!(
        phase = "scratch_remove",
        elapsed_ms = began.elapsed().as_secs_f64() * 1000.0,
        "OCI command phase completed"
    );
    Ok(output)
}

async fn collect(
    client: &RuntimeClient,
    target: &ContainerTarget,
    observer: Option<&dyn crate::workspace::CommandOutputObserver>,
    capture: &mut BoundedCapture,
) -> Result<i32> {
    let mut sequence = 0;
    let mut eof = [false; 2];
    let mut decoders = [output::Decoder::default(), output::Decoder::default()];
    while !eof.iter().all(|value| *value) {
        for chunk in client
            .read_output(ReadOutputRequest {
                process: ProcessTarget {
                    container: target.clone(),
                    process_id: ProcessId::init(),
                },
                after_sequence: sequence,
                max_bytes: 8192,
                wait_timeout_ms: Some(100),
            })
            .await?
        {
            sequence = chunk.sequence;
            let (index, stream) = match chunk.stream {
                OutputStream::Stdout => (0, CaptureStream::Stdout),
                OutputStream::Stderr => (1, CaptureStream::Stderr),
            };
            capture.push(stream, &chunk.data);
            let text = decoders[index].push(&chunk.data, chunk.eof);
            if let Some(observer) = observer {
                if !text.is_empty() {
                    observer.on_output_delta(&text).await;
                }
            }
            eof[index] |= chunk.eof;
        }
    }
    let status = client
        .wait(WaitRequest {
            target: target.clone(),
            timeout_ms: None,
        })
        .await?;
    match (status.exit_code, status.signal) {
        (Some(code), None) => Ok(code),
        (None, Some(signal)) => Ok(128 + signal),
        _ => bail!("OCI Guest returned no unambiguous terminal receipt"),
    }
}

fn operation(name: &str) -> a3s_oci_sdk::Result<OperationContext> {
    Ok(OperationContext::new(OperationId::new(name)?))
}

#[cfg(test)]
mod tests;
