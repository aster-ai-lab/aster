//! Spawn the Aster Code ACP agent as a stdio subprocess and bridge it into
//! the pager's typed ACP channels (same pattern as [`super::leader_bridge`]).

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow};
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};
use tokio_util::sync::CancellationToken;

use agent_client_protocol as acp;
use aster_acp_lib::{
    AcpAgentChannel, AcpAgentMessage, AcpClientTx, AcpGatewayReceiver, AcpGatewaySender,
    LineBufferedRead, acp_channels, acp_internal_error,
};
use aster_shell::util::grok_home::grok_home;
use serde::Deserialize;
use serde_json::value::RawValue;

use super::leader_bridge::ConnectionStatus;
use super::spawn::{SpawnedAgent, boot_auth_manager};
use aster_shell::agent::config::Config as AgentConfig;

static EXPECTED_PAIR_BUILD_ID: std::sync::OnceLock<&'static str> = std::sync::OnceLock::new();

pub fn set_expected_pair_build_id(id: &'static str) {
    let _ = EXPECTED_PAIR_BUILD_ID.set(id);
}

async fn validate_pair(bin: &Path) -> Result<()> {
    // Explicit external agents have their own lifecycle and are not the shipped pair.
    if std::env::var_os("ASTER_ACP_AGENT_BIN").is_some() {
        return Ok(());
    }
    let Some(expected) = EXPECTED_PAIR_BUILD_ID.get() else {
        return Ok(());
    };
    let mut command = Command::new(bin);
    command.arg("--build-id").kill_on_drop(true);
    let output = tokio::time::timeout(std::time::Duration::from_secs(10), command.output())
        .await
        .context("ACP pair identity check timed out")?
        .with_context(|| {
            format!(
                "Failed to execute ACP companion {}: rebuild or reinstall aster and aster-acp together",
                bin.display()
            )
        })?;
    let actual = std::str::from_utf8(&output.stdout)
        .context("invalid ACP build identity")?
        .trim();
    if !output.status.success() || actual != *expected {
        anyhow::bail!(
            "Aster and its ACP companion have different build identities; rebuild or reinstall both together"
        );
    }
    Ok(())
}

/// Bounded restart policy for [`spawn_supervised_bridge`].
#[derive(Clone, Copy, Debug)]
pub(crate) struct SidecarRestartPolicy {
    pub(crate) max_restarts: usize,
    pub(crate) crash_window: Duration,
    pub(crate) initial_backoff: Duration,
    pub(crate) max_backoff: Duration,
    pub(crate) heartbeat_interval: Option<Duration>,
    pub(crate) heartbeat_timeout: Duration,
}

impl Default for SidecarRestartPolicy {
    fn default() -> Self {
        Self {
            max_restarts: 5,
            crash_window: Duration::from_secs(60),
            initial_backoff: Duration::from_millis(250),
            max_backoff: Duration::from_secs(5),
            heartbeat_interval: Some(Duration::from_secs(15)),
            heartbeat_timeout: Duration::from_secs(10),
        }
    }
}

/// Spawn the shipped ACP agent and bridge its stdio into an [`SpawnedAgent`].
pub async fn spawn_aster_acp(
    agent_config: &AgentConfig,
    cancel: &CancellationToken,
    cwd: Option<&std::path::Path>,
    model: Option<&str>,
) -> Result<SpawnedAgent> {
    let (bin, auth_manager, agent_cancel, client_channel, agent_channel, arguments) =
        prepare_sidecar_spawn(agent_config, cancel, cwd, model).await?;
    let thread_handle = spawn_bridge(bin, arguments, agent_cancel.clone(), agent_channel)?;

    Ok(SpawnedAgent {
        thread_handle,
        channel: client_channel,
        cancel: agent_cancel,
        auth_manager,
        status_rx: None,
    })
}

/// Spawn the shipped ACP agent behind a self-healing supervisor that automatically
/// respawns crashed sidecar generations and broadcasts [`ConnectionStatus`] updates.
pub async fn spawn_supervised_sidecar(
    agent_config: &AgentConfig,
    cancel: &CancellationToken,
    cwd: Option<&std::path::Path>,
    model: Option<&str>,
) -> Result<SpawnedAgent> {
    if !aster_acp_enabled() {
        return Err(missing_agent_backend());
    }
    let (bin, auth_manager, agent_cancel, client_channel, agent_channel, arguments) =
        prepare_sidecar_spawn(agent_config, cancel, cwd, model).await?;
    let (thread_handle, status_rx) = spawn_supervised_bridge(
        bin,
        arguments,
        agent_cancel.clone(),
        agent_channel,
        SidecarRestartPolicy::default(),
    )?;

    Ok(SpawnedAgent {
        thread_handle,
        channel: client_channel,
        cancel: agent_cancel,
        auth_manager,
        status_rx: Some(status_rx),
    })
}

async fn prepare_sidecar_spawn(
    agent_config: &AgentConfig,
    cancel: &CancellationToken,
    cwd: Option<&std::path::Path>,
    model: Option<&str>,
) -> Result<(
    PathBuf,
    Arc<aster_login::AuthManager>,
    CancellationToken,
    aster_acp_lib::AcpClientChannel,
    AcpAgentChannel,
    Vec<std::ffi::OsString>,
)> {
    let bin = aster_acp_executable().ok_or_else(|| {
        anyhow!(
            "no aster-acp beside the pager and ASTER_ACP_AGENT_BIN is not set; cannot spawn the agent"
        )
    })?;
    validate_pair(&bin).await?;
    let auth_manager = boot_auth_manager(&grok_home(), agent_config);
    let agent_cancel = cancel.child_token();
    let (client_channel, agent_channel) = acp_channels();
    let cwd = match cwd {
        Some(cwd) => cwd.to_owned(),
        None => std::env::current_dir().context("resolve ACP workspace")?,
    };
    let arguments = sidecar_arguments(
        agent_config,
        &cwd,
        model,
        std::env::var_os("ASTER_CONFIG").as_deref(),
    );
    Ok((
        bin,
        auth_manager,
        agent_cancel,
        client_channel,
        agent_channel,
        arguments,
    ))
}

struct ChildGenerationOutcome {
    stop: BridgeStop,
    observed_status: Option<std::process::ExitStatus>,
    stderr_summary: String,
}

async fn run_child_generation(
    bin: &Path,
    arguments: &[std::ffi::OsString],
    bridge_cancel: &CancellationToken,
    outbound_tx: AcpClientTx,
    child_rx: tokio::sync::mpsc::UnboundedReceiver<AcpAgentMessage>,
    heartbeat: Option<(
        Duration,
        Duration,
        tokio::sync::mpsc::UnboundedSender<AcpAgentMessage>,
    )>,
    on_ready: impl FnOnce(),
) -> Result<ChildGenerationOutcome> {
    let mut command = Command::new(bin);
    command.args(arguments);
    // The welcome screen owns the console. ACP diagnostics stay in
    // the daily log instead of painting over that screen.
    apply_interactive_acp_stdio(&mut command);
    let mut child = command
        .spawn()
        .with_context(|| format!("failed to spawn aster ACP agent at {}", bin.display()))?;
    let child_stdin = child
        .stdin
        .take()
        .ok_or_else(|| anyhow!("aster ACP agent stdin missing"))?;
    let child_stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow!("aster ACP agent stdout missing"))?;
    let mut child_stderr = child
        .stderr
        .take()
        .ok_or_else(|| anyhow!("aster ACP agent stderr missing"))?;

    // Bounded stderr summary for failure diagnosis only; never paint
    // raw ACP stderr onto the pager console.
    let stderr_tail = Arc::new(Mutex::new(Vec::<u8>::new()));
    let stderr_buf = Arc::clone(&stderr_tail);
    let stderr_task = tokio::task::spawn_local(async move {
        let mut chunk = [0_u8; 1024];
        loop {
            match child_stderr.read(&mut chunk).await {
                Ok(0) => break,
                Ok(n) => {
                    if let Some(bytes) = chunk.get(..n) {
                        append_stderr_tail(&stderr_buf, bytes);
                    }
                }
                Err(_) => break,
            }
        }
    });

    let gw_tx = AcpGatewaySender::new(outbound_tx).with_tracing(true);
    // Apply the frame limit before any unbounded line allocation.
    // Mirror agent-side RetryTransient*: full pipes must backpressure,
    // not kill the bridge with -32603 Internal error (EAGAIN).
    let epoch = Instant::now();
    let last_activity_ms = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let activity_cb = Arc::clone(&last_activity_ms);
    let notification_tx = gw_tx.clone();
    let incoming = LineBufferedRead::spawn_local_filter(
        aster_acp_lib::RetryTransientRead::new(child_stdout.compat()),
        move |line| {
            let elapsed_ms = u64::try_from(epoch.elapsed().as_millis()).unwrap_or(u64::MAX);
            activity_cb.store(elapsed_ms, std::sync::atomic::Ordering::Relaxed);
            forward_wire_notification(line, &notification_tx)
        },
    );
    let gen_tasks: Rc<RefCell<Vec<tokio::task::AbortHandle>>> = Rc::new(RefCell::new(Vec::new()));
    let conn_tasks = Rc::clone(&gen_tasks);
    let (conn, handle_io) = acp::ClientSideConnection::new(
        gw_tx,
        aster_acp_lib::RetryTransientWrite::new(child_stdin.compat_write()),
        incoming,
        move |fut| {
            conn_tasks
                .borrow_mut()
                .push(tokio::task::spawn_local(fut).abort_handle());
        },
    );
    let gw_tasks = Rc::clone(&gen_tasks);
    let gw_rx = AcpGatewayReceiver::new(child_rx, conn)
        .with_tracing(true)
        .with_spawn_fn(move |fut| {
            gw_tasks
                .borrow_mut()
                .push(tokio::task::spawn_local(fut).abort_handle());
        });
    let mut io_task = tokio::task::spawn_local(handle_io);
    let gateway_task = tokio::task::spawn_local(gw_rx.run());
    on_ready();
    tokio::task::yield_now().await;

    // Prefer cancel over transport/exit so intentional teardown is not a failure.
    // The pager cancels before dropping client channels; both can race.
    let stop = tokio::select! {
        biased;
        () = bridge_cancel.cancelled() => BridgeStop::Cancelled,
        result = &mut io_task => BridgeStop::Failed(match result {
            Ok(Ok(())) => anyhow!("Aster Code ACP transport closed"),
            Ok(Err(error)) => anyhow::Error::new(error),
            Err(error) => anyhow::Error::new(error).context("ACP transport task"),
        }),
        status = child.wait() => BridgeStop::Failed(match status {
            Ok(status) => anyhow!("Aster Code ACP exited: {status}"),
            Err(error) => anyhow::Error::new(error).context("wait for ACP agent"),
        }),
        error = async {
            match heartbeat {
                Some((interval, timeout, tx)) => {
                    sidecar_heartbeat_watchdog(interval, timeout, tx, epoch, last_activity_ms).await
                }
                None => std::future::pending().await,
            }
        } => BridgeStop::Failed(error),
    };
    // Snapshot exit status before cleanup kill replaces it.
    let observed_status = child.try_wait().ok().flatten();
    stderr_task.abort();
    io_task.abort();
    gateway_task.abort();
    for handle in gen_tasks.borrow_mut().drain(..) {
        handle.abort();
    }
    let stderr_summary = stderr_summary(&stderr_tail);
    // Cancel and unexpected-exit paths both need a reap; a gone child is fine.
    if let Err(error) = child.kill().await {
        tracing::debug!(%error, "ACP child already gone during bridge teardown");
    }
    // Let aborted local RPC tasks drop immediately so callers receive fast disconnect errors.
    tokio::task::yield_now().await;
    Ok(ChildGenerationOutcome {
        stop,
        observed_status,
        stderr_summary,
    })
}

async fn sidecar_heartbeat_watchdog(
    interval: Duration,
    timeout: Duration,
    child_tx: tokio::sync::mpsc::UnboundedSender<AcpAgentMessage>,
    epoch: Instant,
    last_activity_ms: Arc<std::sync::atomic::AtomicU64>,
) -> anyhow::Error {
    use std::sync::atomic::Ordering;

    let idle_duration = || {
        let now_ms = u64::try_from(epoch.elapsed().as_millis()).unwrap_or(u64::MAX);
        let last_ms = last_activity_ms.load(Ordering::Relaxed);
        Duration::from_millis(now_ms.saturating_sub(last_ms))
    };
    let mark_activity = || {
        let now_ms = u64::try_from(epoch.elapsed().as_millis()).unwrap_or(u64::MAX);
        last_activity_ms.store(now_ms, Ordering::Relaxed);
    };

    loop {
        tokio::time::sleep(interval).await;
        if idle_duration() < interval {
            continue;
        }
        let Ok(raw) = RawValue::from_string("{}".to_string()) else {
            return anyhow!("failed to build ACP health probe payload");
        };
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        let message = AcpAgentMessage::ExtMethod(aster_acp_lib::AcpArgs {
            request: acp::ExtRequest::new("aster/health", Arc::from(raw)),
            response_tx,
        });
        if child_tx.send(message).is_err() {
            return anyhow!("ACP sidecar command channel closed before heartbeat");
        }
        match tokio::time::timeout(timeout, response_rx).await {
            Ok(Ok(Ok(_))) => {
                mark_activity();
            }
            Ok(Ok(Err(error))) if error.code == acp::ErrorCode::MethodNotFound => {
                // Older sidecar binaries respond with -32601 MethodNotFound,
                // which still proves the JSON-RPC event loop is responsive.
                mark_activity();
            }
            Ok(Ok(Err(error))) => {
                return anyhow!("ACP sidecar heartbeat failed: {error}");
            }
            Ok(Err(_)) => {
                return anyhow!("ACP sidecar dropped heartbeat response channel");
            }
            Err(_) => {
                if idle_duration() < timeout {
                    continue;
                }
                return anyhow!(
                    "ACP sidecar heartbeat timed out after {}ms",
                    timeout.as_millis()
                );
            }
        }
    }
}

fn spawn_bridge(
    bin: PathBuf,
    arguments: Vec<std::ffi::OsString>,
    bridge_cancel: CancellationToken,
    agent_channel: aster_acp_lib::AcpAgentChannel,
) -> Result<thread::JoinHandle<Result<()>>> {
    thread::Builder::new()
        .name("aster-acp-bridge".into())
        .stack_size(8 * 1024 * 1024)
        .spawn(move || -> Result<()> {
            let mut builder = tokio::runtime::Builder::new_current_thread();
            let rt = aster_tty_utils::runtime::apply_blocking_pool(builder.enable_all()).build()?;
            let local = tokio::task::LocalSet::new();
            local.block_on(&rt, async move {
                let outcome = run_child_generation(
                    &bin,
                    &arguments,
                    &bridge_cancel,
                    agent_channel.tx,
                    agent_channel.rx,
                    None,
                    || {},
                )
                .await?;
                match outcome.stop {
                    BridgeStop::Cancelled => Ok(()),
                    BridgeStop::Failed(error) if bridge_cancel.is_cancelled() => {
                        // Channel drop closed stdio in the same wakeup as cancel.
                        tracing::debug!(%error, "ACP bridge stop overlapped cancel; treating as clean");
                        Ok(())
                    }
                    BridgeStop::Failed(error) => Err(annotate_bridge_failure(
                        error,
                        outcome.observed_status,
                        &outcome.stderr_summary,
                    )),
                }
            })
        })
        .context("failed to start aster ACP bridge thread")
}

fn fail_acp_request(msg: AcpAgentMessage, reason: &str) {
    let err = acp_internal_error(reason);
    match msg {
        AcpAgentMessage::Initialize(args) => {
            let _ = args.response_tx.send(Err(err));
        }
        AcpAgentMessage::Authenticate(args) => {
            let _ = args.response_tx.send(Err(err));
        }
        AcpAgentMessage::NewSession(args) => {
            let _ = args.response_tx.send(Err(err));
        }
        AcpAgentMessage::LoadSession(args) => {
            let _ = args.response_tx.send(Err(err));
        }
        AcpAgentMessage::SetSessionMode(args) => {
            let _ = args.response_tx.send(Err(err));
        }
        AcpAgentMessage::Prompt(args) => {
            let _ = args.response_tx.send(Err(err));
        }
        AcpAgentMessage::Cancel(args) => {
            let _ = args.response_tx.send(Err(err));
        }
        AcpAgentMessage::ExtMethod(args) => {
            let _ = args.response_tx.send(Err(err));
        }
        AcpAgentMessage::ExtNotification(args) => {
            let _ = args.response_tx.send(Err(err));
        }
        AcpAgentMessage::SetSessionModel(args) => {
            let _ = args.response_tx.send(Err(err));
        }
    }
}

fn is_non_restartable_bridge_error(error: &anyhow::Error) -> bool {
    let text = format!("{error:#}");
    text.contains("exceeds") && text.contains("byte limit")
}

fn backoff_for_attempt(policy: &SidecarRestartPolicy, attempt: usize) -> Duration {
    let shift = attempt.saturating_sub(1).min(16) as u32;
    let multiplier = 1_u32.checked_shl(shift).unwrap_or(u32::MAX);
    policy
        .initial_backoff
        .saturating_mul(multiplier)
        .min(policy.max_backoff)
}

pub(crate) fn spawn_supervised_bridge(
    bin: PathBuf,
    arguments: Vec<std::ffi::OsString>,
    bridge_cancel: CancellationToken,
    agent_channel: AcpAgentChannel,
    policy: SidecarRestartPolicy,
) -> Result<(
    thread::JoinHandle<Result<()>>,
    tokio::sync::watch::Receiver<ConnectionStatus>,
)> {
    let (status_tx, status_rx) =
        tokio::sync::watch::channel(ConnectionStatus::Connected { generation: 0 });
    let handle = thread::Builder::new()
        .name("aster-acp-bridge".into())
        .stack_size(8 * 1024 * 1024)
        .spawn(move || -> Result<()> {
            let mut builder = tokio::runtime::Builder::new_current_thread();
            let rt = aster_tty_utils::runtime::apply_blocking_pool(builder.enable_all()).build()?;
            let local = tokio::task::LocalSet::new();
            local.block_on(&rt, async move {
                let outbound_tx = agent_channel.tx;
                let mut client_rx = agent_channel.rx;
                let active_child_tx: Rc<
                    RefCell<Option<tokio::sync::mpsc::UnboundedSender<AcpAgentMessage>>>,
                > = Rc::new(RefCell::new(None));
                let initial_ready = Rc::new(std::cell::Cell::new(false));
                let ready_notify = Rc::new(tokio::sync::Notify::new());
                let dispatch_tx = Rc::clone(&active_child_tx);
                let dispatch_initial_ready = Rc::clone(&initial_ready);
                let dispatch_notify = Rc::clone(&ready_notify);
                let dispatcher = tokio::task::spawn_local(async move {
                    while let Some(msg) = client_rx.recv().await {
                        // Wait for generation 0 to finish spawning before failing RPCs as "restarting".
                        while !dispatch_initial_ready.get() && dispatch_tx.borrow().is_none() {
                            dispatch_notify.notified().await;
                        }
                        let maybe_tx = dispatch_tx.borrow().clone();
                        match maybe_tx {
                            Some(tx) => {
                                if let Err(tokio::sync::mpsc::error::SendError(unsent)) =
                                    tx.send(msg)
                                {
                                    fail_acp_request(
                                        unsent,
                                        "ACP sidecar connection interrupted; reconnecting",
                                    );
                                }
                            }
                            None => fail_acp_request(
                                msg,
                                "ACP sidecar restarting; try again after reconnect",
                            ),
                        }
                    }
                });

                let mut generation: u64 = 0;
                let mut recent_crashes: VecDeque<Instant> = VecDeque::new();

                let supervisor_result = loop {
                    if bridge_cancel.is_cancelled() {
                        break Ok(());
                    }

                    let (child_tx, child_rx) = tokio::sync::mpsc::unbounded_channel();
                    let heartbeat = policy
                        .heartbeat_interval
                        .map(|interval| (interval, policy.heartbeat_timeout, child_tx.clone()));
                    let active_slot = Rc::clone(&active_child_tx);
                    let slot_initial_ready = Rc::clone(&initial_ready);
                    let slot_notify = Rc::clone(&ready_notify);
                    let status_sender = &status_tx;
                    let current_generation = generation;
                    let outcome = run_child_generation(
                        &bin,
                        &arguments,
                        &bridge_cancel,
                        outbound_tx.clone(),
                        child_rx,
                        heartbeat,
                        move || {
                            *active_slot.borrow_mut() = Some(child_tx);
                            slot_initial_ready.set(true);
                            slot_notify.notify_waiters();
                            if current_generation > 0 {
                                let _ = status_sender.send(ConnectionStatus::Connected {
                                    generation: current_generation,
                                });
                            }
                        },
                    )
                    .await;

                    *active_child_tx.borrow_mut() = None;
                    initial_ready.set(true);
                    ready_notify.notify_waiters();

                    let failure = match outcome {
                        Ok(ChildGenerationOutcome {
                            stop: BridgeStop::Cancelled,
                            ..
                        }) => break Ok(()),
                        Ok(ChildGenerationOutcome {
                            stop: BridgeStop::Failed(error),
                            ..
                        })
                        | Err(error)
                            if bridge_cancel.is_cancelled() =>
                        {
                            tracing::debug!(
                                %error,
                                "ACP bridge stop overlapped cancel; treating as clean"
                            );
                            break Ok(());
                        }
                        Ok(ChildGenerationOutcome {
                            stop: BridgeStop::Failed(error),
                            observed_status,
                            stderr_summary,
                        }) => annotate_bridge_failure(error, observed_status, &stderr_summary),
                        Err(error) if generation == 0 => {
                            let _ = status_tx.send(ConnectionStatus::Failed {
                                error: error.to_string(),
                            });
                            break Err(error);
                        }
                        Err(error) => error,
                    };

                    if is_non_restartable_bridge_error(&failure) {
                        let _ = status_tx.send(ConnectionStatus::Failed {
                            error: failure.to_string(),
                        });
                        break Err(failure);
                    }

                    let now = Instant::now();
                    while let Some(&front) = recent_crashes.front() {
                        if now.saturating_duration_since(front) > policy.crash_window {
                            recent_crashes.pop_front();
                        } else {
                            break;
                        }
                    }
                    recent_crashes.push_back(now);
                    if recent_crashes.len() > policy.max_restarts {
                        let msg = format!(
                            "ACP sidecar crash loop ({} restarts in {}s): {failure}",
                            policy.max_restarts,
                            policy.crash_window.as_secs(),
                        );
                        let _ = status_tx.send(ConnectionStatus::Failed { error: msg.clone() });
                        break Err(anyhow!(msg));
                    }

                    let attempt = recent_crashes.len();
                    let _ = status_tx.send(ConnectionStatus::Reconnecting {
                        attempt: attempt as u32,
                    });
                    let backoff = backoff_for_attempt(&policy, attempt);
                    tracing::warn!(
                        attempt,
                        generation,
                        backoff_ms = backoff.as_millis() as u64,
                        error = %failure,
                        "ACP sidecar exited unexpectedly; scheduling respawn"
                    );
                    tokio::select! {
                        biased;
                        () = bridge_cancel.cancelled() => break Ok(()),
                        () = tokio::time::sleep(backoff) => {}
                    }
                    generation = generation.saturating_add(1);
                };

                dispatcher.abort();
                supervisor_result
            })
        })
        .context("failed to start supervised aster ACP bridge thread")?;

    Ok((handle, status_rx))
}

#[derive(Deserialize)]
struct WireNotification<'a> {
    id: Option<&'a RawValue>,
    #[serde(borrow)]
    method: Cow<'a, str>,
    params: Option<&'a RawValue>,
}

fn forward_wire_notification(line: &[u8], gateway: &AcpGatewaySender<acp::AgentSide>) -> bool {
    // The RPC connection dispatches notifications on spawned callbacks but
    // resolves responses inline. Enqueue notifications while reading their
    // frames so a prompt response cannot overtake its own updates.
    let Ok(WireNotification {
        id: None,
        method,
        params,
    }) = serde_json::from_slice(line)
    else {
        return false;
    };
    match <acp::ClientSide as acp::Side>::decode_notification(&method, params) {
        Ok(acp::AgentNotification::SessionNotification(update)) => {
            gateway.forward_fire_and_forget(update);
        }
        Ok(acp::AgentNotification::ExtNotification(update)) => {
            gateway.forward_fire_and_forget(update);
        }
        _ => return false,
    }
    true
}

const STDERR_TAIL_LIMIT: usize = 2048;

fn append_stderr_tail(buf: &Arc<Mutex<Vec<u8>>>, chunk: &[u8]) {
    let Ok(mut guard) = buf.lock() else {
        return;
    };
    guard.extend_from_slice(chunk);
    if guard.len() > STDERR_TAIL_LIMIT {
        let overflow = guard.len() - STDERR_TAIL_LIMIT;
        guard.drain(..overflow);
    }
}

fn stderr_summary(buf: &Arc<Mutex<Vec<u8>>>) -> String {
    let Ok(guard) = buf.lock() else {
        return String::new();
    };
    let text = String::from_utf8_lossy(&guard);
    let cleaned = text
        .chars()
        .map(|ch| if ch.is_control() { ' ' } else { ch })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if cleaned.chars().count() <= 512 {
        return cleaned;
    }
    cleaned
        .chars()
        .rev()
        .take(512)
        .collect::<String>()
        .chars()
        .rev()
        .collect()
}

fn annotate_bridge_failure(
    error: anyhow::Error,
    observed_status: Option<std::process::ExitStatus>,
    stderr_summary: &str,
) -> anyhow::Error {
    let mut message = error.to_string();
    if let Some(status) = observed_status {
        let status_text = format!("Aster Code ACP exited: {status}");
        if !message.contains(&status_text) {
            message = format!("{message}; {status_text}");
        }
    }
    if !stderr_summary.is_empty() && !message.contains(stderr_summary) {
        message = format!("{message}; acp_stderr={stderr_summary}");
    }
    tracing::error!(
        error = %message,
        exit_code = observed_status.and_then(|status| status.code()),
        "ACP bridge failed"
    );
    anyhow!(message)
}

/// Why the ACP bridge select finished.
enum BridgeStop {
    Cancelled,
    Failed(anyhow::Error),
}

/// The sentence the interactive pager returns when no sidecar executable is configured.
pub const MISSING_AGENT_BACKEND: &str = "no agent backend available: build aster and aster-acp together with `cargo build --locked`, or set ASTER_ACP_AGENT_BIN to the agent executable";

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
    if !aster_acp_enabled() {
        return Err(missing_agent_backend());
    }
    spawn_aster_acp(agent_config, cancel, cwd, model).await
}

/// Interactive ACP stdio. Stdout carries the protocol. Stderr is captured so
/// a warn line cannot land on the pager's console.
pub(crate) fn apply_interactive_acp_stdio(command: &mut Command) {
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
}

fn sidecar_arguments(
    config: &AgentConfig,
    cwd: &Path,
    model: Option<&str>,
    acl: Option<&std::ffi::OsStr>,
) -> Vec<std::ffi::OsString> {
    let mut args = Vec::new();
    if let Some(effort) = config.reasoning_effort_override {
        args.extend(["--reasoning-effort".into(), effort.to_string().into()]);
    }
    args.extend(["--cwd".into(), cwd.as_os_str().to_owned()]);
    if let Some(model) = model {
        args.extend(["--model".into(), model.into()]);
    }
    if let Some(acl) = acl.filter(|value| !value.is_empty()) {
        args.extend(["--config".into(), acl.to_owned()]);
    }
    args
}

/// Run the shipped ACP agent on this process's stdin and stdout.
pub async fn run_sidecar_on_stdio() -> Result<()> {
    run_sidecar_on_stdio_with_config(&AgentConfig::default()).await
}

pub async fn run_sidecar_on_stdio_with_config(config: &AgentConfig) -> Result<()> {
    if !aster_acp_enabled() {
        return Err(missing_agent_backend());
    }
    let bin = aster_acp_executable().ok_or_else(missing_agent_backend)?;
    validate_pair(&bin).await?;
    let cwd = std::env::current_dir().context("resolve ACP workspace")?;
    let arguments = sidecar_arguments(
        config,
        &cwd,
        config.default_model_override.as_deref(),
        std::env::var_os("ASTER_CONFIG").as_deref(),
    );
    let mut child = tokio::process::Command::new(&bin)
        .args(arguments)
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .with_context(|| format!("failed to spawn aster ACP agent at {}", bin.display()))?;
    let status = child.wait().await.context("wait for ACP agent")?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("Aster Code ACP exited: {status}"))
    }
}

/// True when the pager can spawn the shipped ACP agent.
///
/// A set `ASTER_ACP_AGENT_BIN` is the only candidate. An empty value, a missing
/// path, or a Unix file without an execute bit stops the search. When the
/// variable is unset, only an executable sibling of this process is accepted.
pub fn aster_acp_enabled() -> bool {
    aster_acp_executable().is_some()
}

/// Path of the ACP agent this process should spawn.
pub fn aster_acp_executable() -> Option<PathBuf> {
    resolve_sidecar_executable(
        std::env::var_os("ASTER_ACP_AGENT_BIN"),
        sibling_agent_path().as_deref(),
    )
}

fn sibling_agent_path() -> Option<PathBuf> {
    let name = if cfg!(windows) {
        "aster-acp.exe"
    } else {
        "aster-acp"
    };
    let current = std::env::current_exe().ok()?;
    Some(current.parent()?.join(name))
}

/// `configured` is `Some` whenever `ASTER_ACP_AGENT_BIN` is set, including an empty value.
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

    use super::{ACP_AGENT_BIN_TEST_LOCK, aster_acp_executable, resolve_sidecar_executable};

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
        let sibling = dir.join("aster-acp");
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
    fn aster_acp_executable_reads_the_environment_override() {
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
        let previous = std::env::var_os("ASTER_ACP_AGENT_BIN");
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
            unsafe { std::env::set_var("ASTER_ACP_AGENT_BIN", &value) };
            assert_eq!(
                aster_acp_executable(),
                None,
                "override {value:?} must not select a sibling"
            );
        }
        let chosen = dir.join("chosen");
        executable_file(&chosen);
        unsafe { std::env::set_var("ASTER_ACP_AGENT_BIN", &chosen) };
        assert_eq!(aster_acp_executable(), Some(chosen));
        unsafe { std::env::remove_var("ASTER_ACP_AGENT_BIN") };
        let sibling = super::sibling_agent_path();
        let expected = sibling
            .as_deref()
            .filter(|path| super::is_executable(path))
            .map(std::path::Path::to_path_buf);
        assert_eq!(aster_acp_executable(), expected);
        unsafe {
            match previous {
                Some(value) => std::env::set_var("ASTER_ACP_AGENT_BIN", value),
                None => std::env::remove_var("ASTER_ACP_AGENT_BIN"),
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn interactive_acp_stdio_captures_stderr() {
        let mut command = if cfg!(windows) {
            let mut command = tokio::process::Command::new("cmd");
            command.args(["/C", "echo ACP_STDERR_PROBE 1>&2"]);
            command
        } else {
            let mut command = tokio::process::Command::new("sh");
            command.args(["-c", "echo ACP_STDERR_PROBE >&2"]);
            command
        };
        super::apply_interactive_acp_stdio(&mut command);
        let mut child = command.spawn().expect("spawn stderr probe");
        let mut stderr = child.stderr.take().expect("stderr is captured");
        let mut stdout = child.stdout.take().expect("stdout is captured");
        let mut err = Vec::new();
        let mut out = Vec::new();
        tokio::io::AsyncReadExt::read_to_end(&mut stderr, &mut err)
            .await
            .unwrap();
        tokio::io::AsyncReadExt::read_to_end(&mut stdout, &mut out)
            .await
            .unwrap();
        let status = child.wait().await.unwrap();
        assert!(status.success(), "{status}");
        let text = String::from_utf8_lossy(&err);
        assert!(text.contains("ACP_STDERR_PROBE"), "{text}");
    }
}

#[cfg(test)]
mod argument_tests;

#[cfg(test)]
mod transport_tests;
