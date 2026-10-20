//! Spawn the A3S Code ACP agent as a stdio subprocess and bridge it into
//! the pager's typed ACP channels (same pattern as [`super::leader_bridge`]).

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::thread;

use anyhow::{Context, Result, anyhow};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};
use tokio_util::sync::CancellationToken;

use a3s_acp_lib::{AcpGatewayReceiver, AcpGatewaySender, LineBufferedRead, acp_channels};
use a3s_code_shell::util::grok_home::grok_home;
use agent_client_protocol as acp;

use super::spawn::{SpawnedAgent, boot_auth_manager};
use a3s_code_shell::agent::config::Config as AgentConfig;

const MAX_BUF: usize = 8 * 1024 * 1024;

/// Spawn the shipped ACP agent and bridge its stdio into an [`SpawnedAgent`].
pub async fn spawn_a3s_acp(
    agent_config: &AgentConfig,
    cancel: &CancellationToken,
    cwd: Option<&std::path::Path>,
    model: Option<&str>,
) -> Result<SpawnedAgent> {
    let bin = a3s_acp_executable().ok_or_else(|| {
        anyhow!(
            "no a3s-code-acp beside the pager and A3S_ACP_AGENT_BIN is not set; cannot spawn the agent"
        )
    })?;
    let auth_manager = boot_auth_manager(&grok_home(), agent_config);

    let agent_cancel = cancel.child_token();
    let (client_channel, agent_channel) = acp_channels();

    let cwd = cwd
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_default();
    let model = model.map(str::to_owned);
    let effort = agent_config
        .reasoning_effort_override
        .or(agent_config.models.default_reasoning_effort)
        .unwrap_or(a3s_code_shell::sampling::types::ReasoningEffort::Medium);
    let bridge_cancel = agent_cancel.clone();

    let thread_handle = thread::Builder::new()
        .name("a3s-acp-bridge".into())
        .stack_size(8 * 1024 * 1024)
        .spawn(move || -> Result<()> {
            let mut builder = tokio::runtime::Builder::new_current_thread();
            let rt = a3s_tty_utils::runtime::apply_blocking_pool(builder.enable_all()).build()?;
            let local = tokio::task::LocalSet::new();
            local.block_on(&rt, async move {
                let mut command = Command::new(&bin);
                command
                    .arg("--reasoning-effort")
                    .arg(effort.to_string())
                    .arg("--cwd")
                    .arg(&cwd)
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::inherit())
                    .kill_on_drop(true);
                if let Some(model) = model.as_deref() {
                    command.arg("--model").arg(model);
                }
                if let Some(config) = std::env::var_os("A3S_CONFIG").filter(|value| !value.is_empty())
                {
                    command.arg("--config").arg(config);
                }
                let mut child = command
                    .spawn()
                    .with_context(|| format!("failed to spawn a3s ACP agent at {}", bin.display()))?;
                let child_stdin = child
                    .stdin
                    .take()
                    .ok_or_else(|| anyhow!("a3s ACP agent stdin missing"))?;
                let child_stdout = child
                    .stdout
                    .take()
                    .ok_or_else(|| anyhow!("a3s ACP agent stdout missing"))?;

                let (incoming_read, mut incoming_write) = tokio::io::simplex(MAX_BUF);
                let (outgoing_read, outgoing_write) = tokio::io::simplex(MAX_BUF);

                // Child stdout → ClientSideConnection incoming
                let cancel_r = bridge_cancel.clone();
                let reader_task = tokio::task::spawn_local(async move {
                    let mut reader = BufReader::new(child_stdout);
                    let mut line = String::new();
                    loop {
                        line.clear();
                        tokio::select! {
                            biased;
                            _ = cancel_r.cancelled() => break,
                            result = reader.read_line(&mut line) => {
                                match result {
                                    Ok(0) => break,
                                    Ok(_) => {
                                        if incoming_write.write_all(line.as_bytes()).await.is_err() {
                                            break;
                                        }
                                    }
                                    Err(_) => break,
                                }
                            }
                        }
                    }
                });

                // ClientSideConnection outgoing → child stdin
                let cancel_w = bridge_cancel.clone();
                let writer_task = tokio::task::spawn_local(async move {
                    let mut reader = BufReader::new(outgoing_read);
                    let mut stdin = child_stdin;
                    let mut line = String::new();
                    loop {
                        line.clear();
                        tokio::select! {
                            biased;
                            _ = cancel_w.cancelled() => break,
                            result = reader.read_line(&mut line) => {
                                match result {
                                    Ok(0) => break,
                                    Ok(_) => {
                                        if stdin.write_all(line.as_bytes()).await.is_err() {
                                            break;
                                        }
                                    }
                                    Err(_) => break,
                                }
                            }
                        }
                    }
                });

                let gw_tx = AcpGatewaySender::new(agent_channel.tx).with_tracing(true);
                let incoming = LineBufferedRead::spawn_local(incoming_read.compat());
                let (conn, handle_io) = acp::ClientSideConnection::new(
                    gw_tx,
                    outgoing_write.compat_write(),
                    incoming,
                    |fut| {
                        tokio::task::spawn_local(fut);
                    },
                );
                let gw_rx = AcpGatewayReceiver::new(agent_channel.rx, conn).with_tracing(true);
                let mut io_task = tokio::task::spawn_local(handle_io);
                let gateway_task = tokio::task::spawn_local(gw_rx.run());
                tokio::task::yield_now().await;

                let outcome = tokio::select! {
                    biased;
                    _ = bridge_cancel.cancelled() => Ok(()),
                    status = child.wait() => {
                        Err(anyhow!("A3S Code ACP exited: {}", status.context("wait for ACP agent")?))
                    }
                    result = &mut io_task => {
                        result.context("ACP transport task")??;
                        Err(anyhow!("A3S Code ACP transport closed"))
                    }
                };
                reader_task.abort();
                writer_task.abort();
                io_task.abort();
                gateway_task.abort();
                let _ = child.kill().await;
                outcome
            })
        })
        .context("failed to start a3s ACP bridge thread")?;

    Ok(SpawnedAgent {
        thread_handle,
        channel: client_channel,
        cancel: agent_cancel,
        auth_manager,
    })
}

/// The sentence the interactive pager returns when no sidecar executable is configured.
pub const MISSING_AGENT_BACKEND: &str = "no agent backend available: build aster and a3s-code-acp together with `cargo build --locked`, or set A3S_ACP_AGENT_BIN to the agent executable";

fn missing_agent_backend() -> anyhow::Error {
    anyhow!(MISSING_AGENT_BACKEND)
}

/// Spawn the shipped ACP agent, or fail closed with [`MISSING_AGENT_BACKEND`].
pub async fn spawn_required_sidecar(
    agent_config: &AgentConfig,
    cancel: &CancellationToken,
    cwd: Option<&std::path::Path>,
    model: Option<&str>,
) -> Result<SpawnedAgent> {
    if !a3s_acp_enabled() {
        return Err(missing_agent_backend());
    }
    spawn_a3s_acp(agent_config, cancel, cwd, model).await
}

/// Run the shipped ACP agent on this process's stdin and stdout.
pub async fn run_sidecar_on_stdio() -> Result<()> {
    if !a3s_acp_enabled() {
        return Err(missing_agent_backend());
    }
    let bin = a3s_acp_executable().ok_or_else(missing_agent_backend)?;
    let mut child = tokio::process::Command::new(&bin)
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .with_context(|| format!("failed to spawn a3s ACP agent at {}", bin.display()))?;
    let status = child.wait().await.context("wait for ACP agent")?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("A3S Code ACP exited: {status}"))
    }
}

/// True when the pager can spawn the shipped ACP agent.
///
/// A set `A3S_ACP_AGENT_BIN` is the only candidate. An empty value, a missing
/// path, or a Unix file without an execute bit stops the search. When the
/// variable is unset, only an executable sibling of this process is accepted.
pub fn a3s_acp_enabled() -> bool {
    a3s_acp_executable().is_some()
}

/// Path of the ACP agent this process should spawn.
pub fn a3s_acp_executable() -> Option<PathBuf> {
    resolve_sidecar_executable(
        std::env::var_os("A3S_ACP_AGENT_BIN"),
        sibling_agent_path().as_deref(),
    )
}

fn sibling_agent_path() -> Option<PathBuf> {
    let name = if cfg!(windows) {
        "a3s-code-acp.exe"
    } else {
        "a3s-code-acp"
    };
    let current = std::env::current_exe().ok()?;
    Some(current.parent()?.join(name))
}

/// `configured` is `Some` whenever `A3S_ACP_AGENT_BIN` is set, including an empty value.
pub(crate) fn resolve_sidecar_executable(
    configured: Option<std::ffi::OsString>,
    sibling: Option<&Path>,
) -> Option<PathBuf> {
    if let Some(configured) = configured {
        let path = PathBuf::from(configured);
        return is_executable(&path).then_some(path);
    }
    sibling
        .filter(|path| is_executable(path))
        .map(Path::to_path_buf)
}

fn is_executable(path: &Path) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

#[cfg(test)]
pub(crate) static ACP_AGENT_BIN_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use super::{ACP_AGENT_BIN_TEST_LOCK, a3s_acp_executable, resolve_sidecar_executable};

    fn executable_file(path: &std::path::Path) {
        std::fs::write(path, b"").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
    }

    #[test]
    fn a_set_override_stops_when_the_path_cannot_be_executed() {
        let dir = std::env::temp_dir().join(format!(
            "pager-acp-override-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let sibling = dir.join("sibling");
        executable_file(&sibling);
        let plain = dir.join("plain");
        std::fs::write(&plain, b"").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&plain, std::fs::Permissions::from_mode(0o644)).unwrap();
        }
        assert_eq!(
            resolve_sidecar_executable(Some(dir.join("missing").into_os_string()), Some(&sibling)),
            None
        );
        assert_eq!(
            resolve_sidecar_executable(Some(OsString::new()), Some(&sibling)),
            None
        );
        assert_eq!(
            resolve_sidecar_executable(Some(plain.into_os_string()), Some(&sibling)),
            None
        );
        let chosen = dir.join("chosen");
        executable_file(&chosen);
        assert_eq!(
            resolve_sidecar_executable(Some(chosen.clone().into_os_string()), Some(&sibling)),
            Some(chosen)
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_unset_override_accepts_only_an_executable_sibling() {
        let dir = std::env::temp_dir().join(format!(
            "pager-acp-sibling-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let sibling = dir.join("a3s-code-acp");
        assert_eq!(resolve_sidecar_executable(None, Some(&sibling)), None);
        executable_file(&sibling);
        assert_eq!(
            resolve_sidecar_executable(None, Some(&sibling)),
            Some(sibling)
        );
        let plain = dir.join("plain-sibling");
        std::fs::write(&plain, b"").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&plain, std::fs::Permissions::from_mode(0o644)).unwrap();
            assert_eq!(resolve_sidecar_executable(None, Some(&plain)), None);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a3s_acp_executable_reads_the_environment_override() {
        let _guard = ACP_AGENT_BIN_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let dir = std::env::temp_dir().join(format!(
            "pager-acp-env-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let previous = std::env::var_os("A3S_ACP_AGENT_BIN");
        let missing = dir.join("missing");
        let plain = dir.join("plain");
        std::fs::write(&plain, b"").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&plain, std::fs::Permissions::from_mode(0o644)).unwrap();
        }
        for value in [
            missing.into_os_string(),
            OsString::new(),
            plain.into_os_string(),
        ] {
            unsafe { std::env::set_var("A3S_ACP_AGENT_BIN", &value) };
            assert_eq!(
                a3s_acp_executable(),
                None,
                "override {value:?} must not select a sibling"
            );
        }
        let chosen = dir.join("chosen");
        executable_file(&chosen);
        unsafe { std::env::set_var("A3S_ACP_AGENT_BIN", &chosen) };
        assert_eq!(a3s_acp_executable(), Some(chosen));
        unsafe { std::env::remove_var("A3S_ACP_AGENT_BIN") };
        let sibling = super::sibling_agent_path();
        let expected = sibling
            .as_deref()
            .filter(|path| super::is_executable(path))
            .map(std::path::Path::to_path_buf);
        assert_eq!(a3s_acp_executable(), expected);
        unsafe {
            match previous {
                Some(value) => std::env::set_var("A3S_ACP_AGENT_BIN", value),
                None => std::env::remove_var("A3S_ACP_AGENT_BIN"),
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
