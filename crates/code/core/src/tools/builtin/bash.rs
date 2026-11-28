//! Bash tool - Execute shell commands

use crate::tools::types::{
    Tool, ToolContext, ToolErrorKind, ToolEventSender, ToolOutput, ToolStreamEvent,
};
use crate::workspace::{CommandOutputObserver, CommandOutputSummary, CommandRequest};
use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
#[cfg(windows)]
use std::ffi::OsStr;
use std::future::Future;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::process::Command;

pub(crate) mod output_streams;
#[cfg(windows)]
pub(crate) mod windows;
#[cfg(any(windows, test))]
pub(crate) mod windows_shell;
#[cfg(windows)]
pub(crate) use windows::maybe_execute_simple_windows_http_command;
#[cfg(windows)]
pub(crate) use windows::{
    build_powershell_command, encode_powershell_command, windows_host_powershell, CREATE_NO_WINDOW,
};
#[cfg(all(test, windows))]
use windows::{
    normalize_json_like_literal, parse_simple_windows_http_command, preprocess_windows_command,
};

/// Attach shared byte-budget discard accounting on every bash capture summary
/// (E4 vocabulary on the hot path; hosts already read total/captured/truncated).
fn capture_output_metadata(
    summary: crate::workspace::CommandOutputSummary,
    timed_out: bool,
) -> serde_json::Value {
    serde_json::json!({
        "total_bytes": summary.total_bytes,
        "captured_bytes": summary.captured_bytes,
        "truncated": summary.truncated,
        "timed_out": timed_out,
        "discarded_bytes": summary.discarded_bytes(),
    })
}

/// Fallback timeout in milliseconds (10 minutes).
///
/// This is a *fallback*, not a task budget: the model is expected to estimate
/// the duration of each command and pass `timeout` explicitly. A short default
/// used to kill healthy long-running work (site builds, full test suites) that
/// keeps producing output, which is a progress signal, not a stall.
pub(crate) const DEFAULT_TIMEOUT_MS: u64 = 600_000;
const MIN_TIMEOUT_MS: u64 = 1_000;
/// No-output budget for the outer watchdog: a command that keeps streaming
/// output must not be killed just because it runs longer than expected.
pub(crate) const IDLE_TIMEOUT_MS: u64 = 300_000;
/// Absolute ceiling for progress-driven runs, so a command that streams
/// forever still cannot hold a turn indefinitely.
pub(crate) const PROGRESS_HARD_CEILING_MS: u64 = 3_600_000;

/// How much of the no-output budget is still left. Pure so the watchdog
/// decision can be unit-tested without spawning a process.
fn idle_remaining_ms(now_ms: u64, last_output_ms: u64, idle_ms: u64) -> u64 {
    idle_ms.saturating_sub(now_ms.saturating_sub(last_output_ms))
}

/// Observed output progress of one command. Output chunks are the progress
/// signal; elapsed wall-clock time alone is not.
pub(crate) struct CommandProgress {
    started: Instant,
    last_output_ms: AtomicU64,
    output_chunks: AtomicU64,
}

impl CommandProgress {
    fn new() -> Self {
        Self {
            started: Instant::now(),
            last_output_ms: AtomicU64::new(0),
            output_chunks: AtomicU64::new(0),
        }
    }

    fn note_output(&self) {
        self.last_output_ms
            .store(self.started.elapsed().as_millis() as u64, Ordering::Relaxed);
        self.output_chunks.fetch_add(1, Ordering::Relaxed);
    }

    fn idle_remaining_ms(&self, idle_ms: u64) -> u64 {
        idle_remaining_ms(
            self.started.elapsed().as_millis() as u64,
            self.last_output_ms.load(Ordering::Relaxed),
            idle_ms,
        )
    }

    fn snapshot(&self) -> serde_json::Value {
        serde_json::json!({
            "elapsed_ms": self.started.elapsed().as_millis() as u64,
            "last_output_ms": self.last_output_ms.load(Ordering::Relaxed),
            "output_chunks": self.output_chunks.load(Ordering::Relaxed),
        })
    }
}

/// Make a timeout actionable: it is a budget decision, not a command verdict.
fn timeout_retry_hint(declared_timeout_ms: Option<u64>) -> String {
    match declared_timeout_ms {
        Some(ms) => format!(
            "[This is a time budget, not a command failure. You estimated {ms}ms; \
             if the command was still producing output, retry once with a larger `timeout`.]"
        ),
        None => format!(
            "[This is a time budget, not a command failure. No `timeout` was supplied \
             (fallback {DEFAULT_TIMEOUT_MS}ms). Estimate the command duration and retry \
             with an explicit `timeout`.]"
        ),
    }
}

/// Wait for a command, but only count *time without output* against it.
/// An explicit model-supplied timeout is honoured exactly by the caller; this
/// watchdog is used when the model gave no estimate.
async fn wait_for_progress<F: Future>(
    future: F,
    progress: &CommandProgress,
    hard_ms: u64,
    idle_ms: u64,
) -> Result<F::Output, ()> {
    let mut future = std::pin::pin!(future);
    let started = Instant::now();
    loop {
        let elapsed = started.elapsed().as_millis() as u64;
        if elapsed >= hard_ms {
            return Err(());
        }
        let wait_ms = progress
            .idle_remaining_ms(idle_ms)
            .min(hard_ms - elapsed)
            .max(1);
        tokio::select! {
            output = &mut future => return Ok(output),
            _ = tokio::time::sleep(Duration::from_millis(wait_ms)) => {
                if progress.idle_remaining_ms(idle_ms) == 0 {
                    return Err(());
                }
            }
        }
    }
}

/// Adapter that forwards `CommandOutputObserver` deltas to a tool event channel.
///
/// Keeps `workspace::CommandRequest` free of `ToolEventSender`. Constructed in
/// the bash tool when a session has installed an event channel; backend
/// implementations only see `&dyn CommandOutputObserver`.
///
/// Chatty commands used to emit one ACP ToolCallUpdate per OS read chunk.
/// Coalesce by byte/time so hosts and the pager spend less on per-chunk JSON.
const OUTPUT_DELTA_COALESCE_BYTES: usize = 4 * 1024;
const OUTPUT_DELTA_COALESCE_MS: u128 = 50;

struct DeltaCoalesce {
    buf: String,
    last_flush: Instant,
}

impl Default for DeltaCoalesce {
    fn default() -> Self {
        Self {
            buf: String::new(),
            last_flush: Instant::now(),
        }
    }
}

struct ToolEventObserver {
    tx: Option<ToolEventSender>,
    summary: Mutex<Option<CommandOutputSummary>>,
    progress: CommandProgress,
    coalesce: Mutex<DeltaCoalesce>,
}

impl ToolEventObserver {
    fn take_flush_chunk(&self, force: bool) -> Option<String> {
        let mut coalesce = self.coalesce.lock().unwrap();
        if coalesce.buf.is_empty() {
            return None;
        }
        let aged = coalesce.last_flush.elapsed().as_millis() >= OUTPUT_DELTA_COALESCE_MS;
        if force || coalesce.buf.len() >= OUTPUT_DELTA_COALESCE_BYTES || aged {
            coalesce.last_flush = Instant::now();
            return Some(std::mem::take(&mut coalesce.buf));
        }
        None
    }

    async fn flush_output_delta(&self, force: bool) {
        let Some(chunk) = self.take_flush_chunk(force) else {
            return;
        };
        if let Some(tx) = &self.tx {
            tx.send(ToolStreamEvent::OutputDelta(chunk)).await.ok();
        }
    }
}

#[cfg(test)]
mod coalesce_tests {
    use super::*;

    #[tokio::test]
    async fn output_deltas_coalesce_until_byte_budget_or_complete() {
        let (tx, mut rx) = tokio::sync::mpsc::channel(8);
        let observer = ToolEventObserver {
            tx: Some(tx),
            summary: Mutex::new(None),
            progress: CommandProgress::new(),
            coalesce: Mutex::new(DeltaCoalesce::default()),
        };
        observer.on_output_delta("abc").await;
        assert!(rx.try_recv().is_err(), "tiny delta must wait for coalesce");
        let big = "x".repeat(OUTPUT_DELTA_COALESCE_BYTES);
        observer.on_output_delta(&big).await;
        let first = rx.recv().await.expect("byte-budget flush");
        match first {
            ToolStreamEvent::OutputDelta(delta) => {
                assert!(delta.starts_with("abc"));
                assert_eq!(delta.len(), 3 + OUTPUT_DELTA_COALESCE_BYTES);
            }
            other => panic!("unexpected {other:?}"),
        }
        observer.on_output_delta("tail").await;
        observer
            .on_output_complete(&CommandOutputSummary {
                total_bytes: 4,
                captured_bytes: 4,
                truncated: false,
                timed_out: false,
            })
            .await;
        match rx.recv().await.expect("complete flush") {
            ToolStreamEvent::OutputDelta(delta) => assert_eq!(delta, "tail"),
            other => panic!("unexpected {other:?}"),
        }
    }
}

#[async_trait]
impl CommandOutputObserver for ToolEventObserver {
    async fn on_output_delta(&self, delta: &str) {
        self.progress.note_output();
        if self.tx.is_none() {
            return;
        }
        {
            let mut coalesce = self.coalesce.lock().unwrap();
            coalesce.buf.push_str(delta);
        }
        self.flush_output_delta(false).await;
    }

    async fn on_output_complete(&self, summary: &CommandOutputSummary) {
        self.flush_output_delta(true).await;
        *self.summary.lock().unwrap() = Some(*summary);
    }
}

fn with_changed_paths(metadata: serde_json::Value, paths: &[String]) -> serde_json::Value {
    let mut wrapped = Some(metadata);
    crate::porcelain::attach(&mut wrapped, paths);
    wrapped.unwrap_or_else(|| serde_json::json!({}))
}

pub struct BashTool;

async fn workspace_watch(root: &std::path::Path) -> crate::porcelain::Watch {
    crate::porcelain::Watch::start(root).await
}

/// The host shell executable was not started. No command process exists.
#[derive(Debug)]
struct HostShellSpawnRefused {
    source: std::io::Error,
}

impl std::fmt::Display for HostShellSpawnRefused {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "Failed to spawn shell: {}", self.source)
    }
}

impl std::error::Error for HostShellSpawnRefused {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

pub(crate) fn host_shell_spawn_refused(source: std::io::Error) -> anyhow::Error {
    HostShellSpawnRefused { source }.into()
}

pub(crate) fn is_host_shell_spawn_refused(error: &anyhow::Error) -> bool {
    error.is::<HostShellSpawnRefused>()
        || error
            .chain()
            .any(|cause| cause.is::<HostShellSpawnRefused>())
}

/// A typed pre-spawn refusal never created a process. Close the span, and
/// do not attribute paths seen in that window to the command. A refusal that
/// carries a stable sandbox violation code reports it as
/// `metadata.sandbox_violation.code`.
async fn settled_pre_spawn_refusal(
    ctx: &ToolContext,
    span: CommandSpan,
    error: &anyhow::Error,
    sandboxed: bool,
    sandbox_available: bool,
) -> ToolOutput {
    let message = error.to_string();
    let _ = span.finish(ctx).await;
    let mut metadata = serde_json::json!({
        "exit_code": null,
        "sandboxed": sandboxed,
        "sandbox_available": sandbox_available,
        "command_started": false,
    });
    if let Some(code) = pre_spawn_violation_code(error) {
        metadata["sandbox_violation"] = serde_json::json!({ "code": code.as_str() });
    }
    ToolOutput::error(message.clone())
        .with_metadata(metadata)
        .with_error_kind(ToolErrorKind::Unsupported { message })
}

fn pre_spawn_violation_code(error: &anyhow::Error) -> Option<crate::sandbox::SandboxViolationCode> {
    if crate::sandbox::native::is_native_process_lifetime_unqualified(error) {
        return Some(crate::sandbox::SandboxViolationCode::LifetimeUnqualified);
    }
    crate::sandbox::violation_code_of(error)
}

async fn cancelled_command(
    ctx: &ToolContext,
    span: CommandSpan,
    output: String,
    exit_code: i32,
    mut metadata: serde_json::Value,
) -> ToolOutput {
    metadata["exit_code"] = exit_code.into();
    metadata["command_cancelled"] = true.into();
    metadata["output_complete"] = false.into();
    let paths = span.finish(ctx).await;
    let result = ToolOutput::error(format!("{output}\n\n[Command cancelled]"))
        .with_metadata(with_changed_paths(metadata, &paths))
        .with_error_kind(ToolErrorKind::Cancelled { op: "bash".into() });
    ctx.record_cancelled_command(exit_code);
    result
}

/// A session that dirtied a path owns it. The command is not parsed; ownership
/// follows the observed delta, including a non-zero exit that still wrote.
fn claim_dirtied_paths(ctx: &ToolContext, paths: &[String]) {
    let Some(session_id) = ctx.session_id.as_deref().filter(|id| !id.trim().is_empty()) else {
        return;
    };
    for path in paths {
        let _ = crate::external_observation::claim_bound_write(
            Some(session_id),
            ctx.workspace.as_path(),
            path,
        );
    }
}

/// One bash invocation's workspace watch plus a PATH `git` witness.
///
/// The witness records that this process ran a HEAD-moving git subcommand.
/// Read-only git (`status`, `ls-files`, `log`) does not own a parallel commit.
/// A missing witness file means the command removed it, so the revision stays
/// attributed.
pub(crate) struct CommandSpan {
    watch: crate::porcelain::Watch,
    witness: Option<GitWitness>,
}

impl CommandSpan {
    pub(crate) async fn begin(workspace: &std::path::Path) -> Self {
        Self {
            watch: workspace_watch(workspace).await,
            witness: GitWitness::install(workspace),
        }
    }

    pub(crate) fn env(
        &self,
        base: Option<Arc<HashMap<String, String>>>,
    ) -> Option<Arc<HashMap<String, String>>> {
        let Some(witness) = &self.witness else {
            return base;
        };
        let mut merged = base
            .as_ref()
            .map(|env| env.as_ref().clone())
            .unwrap_or_default();
        witness.apply(&mut merged);
        Some(Arc::new(merged))
    }

    pub(crate) async fn finish(self, ctx: &ToolContext) -> Vec<String> {
        self.finish_observed(ctx).await.paths
    }

    pub(crate) async fn finish_observed(self, ctx: &ToolContext) -> crate::porcelain::PathDelta {
        let attribute_revision = self
            .witness
            .as_ref()
            .map(GitWitness::moved_head)
            .unwrap_or(true);
        let mut observed = self
            .watch
            .finish_observed_with_revision(ctx.workspace.as_path(), attribute_revision)
            .await;
        observed
            .paths
            .retain(|path| !crate::porcelain::is_harness_path(path));
        claim_dirtied_paths(ctx, &observed.paths);
        observed
    }
}

struct GitWitness {
    root: std::path::PathBuf,
    bin: std::path::PathBuf,
    witness: std::path::PathBuf,
}

impl GitWitness {
    fn install(workspace: &std::path::Path) -> Option<Self> {
        #[cfg(not(unix))]
        {
            let _ = workspace;
            return None;
        }
        #[cfg(unix)]
        {
            let unique = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or(0);
            let root = crate::state_root::workspace_state_dir(workspace)
                .join("loops")
                .join(format!(
                    "command-git-witness-{}-{unique}",
                    std::process::id()
                ));
            let bin = root.join("bin");
            let witness = root.join("seen");
            if std::fs::create_dir_all(&bin).is_err() {
                return None;
            }
            let script = bin.join("git");
            // Parse argv here. The user's shell text is not evidence of which
            // paths moved, and a read-only `git status` must not claim HEAD.
            let body = r#"#!/bin/sh
skip_next=0
moved=0
for arg in "$@"; do
  if [ "$skip_next" = 1 ]; then
    skip_next=0
    continue
  fi
  if [ "$skip_next" = 2 ]; then
    skip_next=0
    case "$arg" in
      -*) ;;
      *) continue ;;
    esac
  fi
  case "$arg" in
    -c|-C|--git-dir|--work-tree|--namespace|--super-prefix|--config-env)
      skip_next=1
      ;;
    --exec-path)
      skip_next=2
      ;;
    --git-dir=*|--work-tree=*|--namespace=*|--exec-path=*|--super-prefix=*|--config-env=*|--list-cmds=*)
      ;;
    -C*)
      ;;
    -*)
      ;;
    commit|checkout|switch|reset|merge|rebase|pull|cherry-pick|revert|am)
      moved=1
      break
      ;;
    *)
      break
      ;;
  esac
done
if [ "$moved" = 1 ] && [ -n "$ASTER_GIT_WITNESS" ]; then
  printf '%s\n' git >> "$ASTER_GIT_WITNESS" 2>/dev/null || true
fi
if [ -n "$ASTER_GIT_REAL_PATH" ]; then
  PATH=$ASTER_GIT_REAL_PATH
  export PATH
fi
exec git "$@"
"#;
            if std::fs::write(&script, body).is_err() {
                let _ = std::fs::remove_dir_all(&root);
                return None;
            }
            use std::os::unix::fs::PermissionsExt;
            if std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).is_err() {
                let _ = std::fs::remove_dir_all(&root);
                return None;
            }
            if std::fs::write(&witness, b"").is_err() {
                let _ = std::fs::remove_dir_all(&root);
                return None;
            }
            Some(Self { root, bin, witness })
        }
    }

    fn apply(&self, env: &mut HashMap<String, String>) {
        let original = env
            .get("PATH")
            .cloned()
            .or_else(|| std::env::var("PATH").ok())
            .unwrap_or_default();
        env.insert(
            "PATH".to_string(),
            format!("{}:{original}", self.bin.display()),
        );
        env.insert("ASTER_GIT_REAL_PATH".to_string(), original);
        env.insert(
            "ASTER_GIT_WITNESS".to_string(),
            self.witness.display().to_string(),
        );
    }

    fn moved_head(&self) -> bool {
        match std::fs::read_to_string(&self.witness) {
            Ok(text) => text.contains("git"),
            // An unreadable marker is not proof this command moved HEAD. Prefer
            // dropping a parallel foreign commit over inventing ownership when
            // the seatbelt guest could not update the host-side witness file.
            Err(_) => false,
        }
    }
}

impl Drop for GitWitness {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[cfg(test)]
fn changed_paths_from_porcelain(before: &[String], after: &[String]) -> Vec<String> {
    crate::porcelain::changed_paths(before, after)
}

#[cfg(windows)]
fn prepare_windows_command(
    command: &mut Command,
    workspace: &std::path::Path,
    command_env: Option<&HashMap<String, String>>,
) {
    command
        .current_dir(workspace)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .creation_flags(CREATE_NO_WINDOW);
    let policy = command_env.cloned().unwrap_or_default();
    command.envs(windows_shell::complete_host_powershell_env(
        &policy,
        &inherited_host_powershell_env(),
    ));
}

/// Non-empty process keys for the host PowerShell defaults. An empty
/// variable counts as unset. The returned values are not copied onto the
/// child; key presence stops the defaults from overriding inheritance.
#[cfg(windows)]
fn inherited_host_powershell_env() -> HashMap<String, String> {
    ["PATHEXT", "PYTHONUTF8", "PYTHONIOENCODING"]
        .into_iter()
        .filter_map(|key| {
            let value = std::env::var_os(key).filter(|value| !value.is_empty())?;
            Some((key.to_string(), value.to_string_lossy().into_owned()))
        })
        .collect()
}

/// Windows `CreateProcess` rejects command lines longer than 32767 characters.
#[cfg(windows)]
const MAX_POWERSHELL_COMMAND_CHARS: usize = 30_000;

#[cfg(windows)]
fn spawn_windows_shell(
    powershell_program: &OsStr,
    command: &str,
    workspace: &std::path::Path,
    command_env: Option<&HashMap<String, String>>,
) -> std::io::Result<tokio::process::Child> {
    let wrapped_command = build_powershell_command(command);
    let encoded_command = encode_powershell_command(&wrapped_command);
    let mut powershell = Command::new(powershell_program);
    powershell.args([
        "-NoLogo",
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy",
        "Bypass",
    ]);
    let encoded_line = format!("{powershell_program:?} -EncodedCommand {encoded_command}");
    let script_file = if encoded_line.encode_utf16().count() <= MAX_POWERSHELL_COMMAND_CHARS {
        powershell.arg("-EncodedCommand").arg(&encoded_command);
        None
    } else {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or(0);
        let path =
            std::env::temp_dir().join(format!("aster-host-{}-{unique}.ps1", std::process::id()));
        let literal = path.to_string_lossy().replace('\'', "''");
        // -File does not fail the process when a cmdlet fails. Match
        // -EncodedCommand, and delete the script when the engine exits.
        let body = format!(
            "trap {{ exit 1 }}\nRegister-EngineEvent -SourceIdentifier PowerShell.Exiting -Action {{ Remove-Item -LiteralPath '{literal}' -Force -ErrorAction SilentlyContinue }} | Out-Null\n{wrapped_command}\nif (-not $?) {{ exit 1 }}\n"
        );
        std::fs::write(&path, body.as_bytes())?;
        powershell.arg("-File").arg(&path);
        Some(path)
    };
    prepare_windows_command(&mut powershell, workspace, command_env);

    match crate::tools::process::spawn_tokio_child(&mut powershell) {
        Ok(child) => {
            if let Err(error) = bind_host_shell_job(&child) {
                drop(child);
                if let Some(path) = script_file {
                    let _ = std::fs::remove_file(path);
                }
                return Err(error);
            }
            Ok(child)
        }
        Err(source) => {
            if let Some(path) = script_file {
                let _ = std::fs::remove_file(path);
            }
            Err(std::io::Error::new(
                source.kind(),
                format!(
                    "failed to spawn PowerShell executable {powershell_program:?}: {source}; refusing to reinterpret the command with another shell"
                ),
            ))
        }
    }
}

/// Put the host shell in a job that dies with it. `kill_on_drop` only ends
/// the direct process; a descendant started by that shell would otherwise
/// keep running and write after cancellation.
#[cfg(windows)]
pub(crate) fn bind_windows_process_tree(
    raw_process: std::os::windows::io::RawHandle,
    pid: u32,
) -> std::io::Result<()> {
    use std::ffi::c_void;
    use std::mem::{size_of, zeroed};
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, TerminateJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };
    use windows_sys::Win32::System::Threading::{OpenProcess, WaitForSingleObject};

    const SYNCHRONIZE: u32 = 0x0010_0000;

    let raw_job = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
    if raw_job.is_null() {
        return Err(std::io::Error::last_os_error());
    }
    let job = unsafe { OwnedHandle::from_raw_handle(raw_job) };
    let mut limits = unsafe { zeroed::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() };
    limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    let size = u32::try_from(size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>())
        .map_err(|_| std::io::Error::other("Job Object limit structure size overflowed"))?;
    if unsafe {
        SetInformationJobObject(
            job.as_raw_handle(),
            JobObjectExtendedLimitInformation,
            (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast::<c_void>(),
            size,
        )
    } == 0
    {
        return Err(std::io::Error::last_os_error());
    }
    if unsafe { AssignProcessToJobObject(job.as_raw_handle(), raw_process) } == 0 {
        return Err(std::io::Error::last_os_error());
    }
    let raw_wait = unsafe { OpenProcess(SYNCHRONIZE, 0, pid) };
    if raw_wait.is_null() {
        return Err(std::io::Error::last_os_error());
    }
    let wait = unsafe { OwnedHandle::from_raw_handle(raw_wait) };
    std::thread::Builder::new()
        .name("aster-host-shell-job".to_string())
        .spawn(move || {
            if unsafe { WaitForSingleObject(wait.as_raw_handle(), u32::MAX) } == 0 {
                let _ = unsafe { TerminateJobObject(job.as_raw_handle(), 1) };
            }
        })
        .map(|_| ())
}

#[cfg(windows)]
fn bind_host_shell_job(child: &tokio::process::Child) -> std::io::Result<()> {
    let Some(raw_process) = child.raw_handle() else {
        return Ok(());
    };
    let Some(pid) = child.id() else {
        return Ok(());
    };
    bind_windows_process_tree(raw_process, pid)
}

/// Spawn a shell command cross-platform.
///
/// - Unix: `bash -c <command>`
/// - Windows: PowerShell 7 when `pwsh.exe` is a regular file, otherwise the
///   built-in Windows PowerShell 5.1. The process is placed in a Job Object
///   that kills descendants when the shell exits, so dropping the child cannot
///   leave a later side effect. Short commands use `-EncodedCommand`. Commands
///   that would exceed the 32767-character `CreateProcess` limit use `-File`.
///   Reparse points, including WindowsApps aliases, are skipped. If the chosen
///   executable fails to start, the command is not reinterpreted with `cmd`
///   or the other PowerShell.
pub(crate) fn spawn_shell(
    command: &str,
    workspace: &std::path::Path,
    command_env: Option<&HashMap<String, String>>,
) -> std::io::Result<tokio::process::Child> {
    #[cfg(windows)]
    {
        let powershell = windows_host_powershell(workspace).map_err(|error| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!(
                    "failed to resolve host PowerShell for workspace {}: {error}",
                    workspace.display()
                ),
            )
        })?;
        spawn_windows_shell(powershell.as_os_str(), command, workspace, command_env)
    }
    #[cfg(not(windows))]
    {
        let mut cmd = Command::new("bash");
        cmd.arg("-c")
            .arg(command)
            .current_dir(workspace)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        crate::tools::process::configure_process_group(&mut cmd);
        if let Some(env) = command_env {
            cmd.envs(env);
        }
        crate::tools::process::spawn_tokio_child(&mut cmd)
    }
}

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str {
        "bash"
    }

    fn description(&self) -> &str {
        "Execute a workspace command for <env> target (OS/shell may differ from host)."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "command": {
                    "type": "string"
                },
                "timeout": {
                    "type": "integer",
                    "description": format!(
                        "ms (fallback {DEFAULT_TIMEOUT_MS}, min 1000)."
                    )
                },
                "sandbox_permissions": {
                    "type": "string",
                    "enum": ["use_default", "require_escalated", "request_network_grant"],
                    "default": "use_default",
                    "description": "use_default (sandbox), require_escalated (host retry after sandbox denial), or request_network_grant."
                },
                "justification": {
                    "type": "string",
                    "description": "For require_escalated."
                },
                "network_grant": {
                    "type": "object",
                    "properties": {
                        "host": {
                            "type": "string",
                            "description": "Exact host (no wildcards; localhost != 127.0.0.1)."
                        },
                        "port": {
                            "type": "integer"
                        }
                    },
                    "required": ["host"],
                    "description": "For request_network_grant."
                }
            },
            "required": ["command"],
            "examples": [
                {
                    "command": "cargo test -p aster-core skill::"
                },
                {
                    "command": "npm test",
                    "timeout": 900000
                }
            ]
        })
    }

    fn requires_confirmation(&self, args: &serde_json::Value) -> bool {
        matches!(
            args.get("sandbox_permissions").and_then(|v| v.as_str()),
            Some("require_escalated") | Some("request_network_grant")
        )
    }

    fn validate_execution_available(
        &self,
        args: &serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<()> {
        // Observation and session-shell control do not start a command.
        if args
            .get("command")
            .and_then(serde_json::Value::as_str)
            .is_none()
            || args
                .get("sandbox_permissions")
                .and_then(serde_json::Value::as_str)
                == Some("require_escalated")
        {
            return Ok(());
        }
        if let Some(sandbox) = &ctx.sandbox {
            sandbox.validate_execution_available()?;
        }
        Ok(())
    }

    async fn execute(&self, args: &serde_json::Value, ctx: &ToolContext) -> Result<ToolOutput> {
        if let Some(output) = observe_detached_job(args, ctx).await {
            return Ok(output);
        }
        if let Some(output) = session_shell_control(args, ctx) {
            return Ok(output);
        }
        let command = match args.get("command").and_then(|v| v.as_str()) {
            Some(c) => c,
            None => return Ok(ToolOutput::error("command parameter is required")),
        };
        let command = prefix_session_cwd(command, ctx);
        let command = command.as_str();
        let raw_mode = args
            .get("sandbox_permissions")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("use_default");
        let require_escalated = match raw_mode {
            "use_default" => false,
            "require_escalated" => {
                if args
                    .get("justification")
                    .and_then(serde_json::Value::as_str)
                    .is_none_or(|value| value.trim().is_empty())
                {
                    return Ok(ToolOutput::error(
                        "justification is required when sandbox_permissions is require_escalated",
                    ));
                }
                true
            }
            // The grant itself is applied right before the sandboxed run
            // below, so a denied grant never falls through to the host.
            "request_network_grant" => false,
            value => {
                return Ok(ToolOutput::error(format!(
                    "unsupported sandbox_permissions value: {value}"
                )));
            }
        };
        let request_network_grant = raw_mode == "request_network_grant"
            && ctx
                .sandbox
                .as_ref()
                .is_none_or(|sandbox| sandbox.is_isolated());
        if let Err(error) = self.validate_execution_available(args, ctx) {
            let message = error.to_string();
            return Ok(
                ToolOutput::error(&message).with_error_kind(ToolErrorKind::Unsupported { message })
            );
        }
        if let Some(denied) = refuse_hidden_foreign_write(ctx) {
            return Ok(denied);
        }

        let declared_timeout_ms = args.get("timeout").and_then(|v| v.as_u64());
        let requested_timeout_ms = declared_timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS);
        let timeout_ms = requested_timeout_ms.max(MIN_TIMEOUT_MS);
        let event_observer = Arc::new(ToolEventObserver {
            tx: ctx.event_tx.clone(),
            summary: Mutex::new(None),
            progress: CommandProgress::new(),
            coalesce: Mutex::new(DeltaCoalesce::default()),
        });
        let output_observer = Some(Arc::clone(&event_observer) as Arc<dyn CommandOutputObserver>);

        // The normal path uses the configured workspace sandbox. An explicit
        // escalation is intentionally routed to the host runner only after the
        // session permission layer authorizes the exact invocation.
        if !require_escalated
            && ctx.sandbox.is_none()
            && ctx.workspace_services.local_root().is_some()
            && ctx.has_run_governance()
        {
            let message = "default bash execution requires a configured sandbox; refusing to execute the command on the host";
            let mut denied = ToolOutput::error(message);
            denied.metadata = Some(serde_json::json!({
                "exit_code": null,
                "sandboxed": false,
                "sandbox_available": false,
            }));
            denied.error_kind = Some(ToolErrorKind::Unsupported {
                message: message.to_string(),
            });
            return Ok(denied);
        }
        if request_network_grant && ctx.sandbox.is_none() {
            let message = "request_network_grant requires a configured sandbox";
            let mut denied = ToolOutput::error(message);
            denied.metadata = Some(serde_json::json!({
                "sandboxed": false,
                "sandbox_available": false,
            }));
            denied.error_kind = Some(ToolErrorKind::Unsupported {
                message: message.to_string(),
            });
            return Ok(denied);
        }
        if !require_escalated {
            if let Some(ref sandbox) = ctx.sandbox {
                let isolated = sandbox.is_isolated();
                // Gate 10: the host confirmation layer has already surfaced
                // this exact origin to the user (requires_confirmation covers
                // request_network_grant); apply the digest-pinned grant and
                // run the command sandboxed under the widened policy.
                let grant_metadata = if request_network_grant {
                    match resolve_network_grant(args, Some(sandbox.as_ref())) {
                        Ok(value) => Some(value),
                        Err(output) => return Ok(output),
                    }
                } else {
                    None
                };
                let span = CommandSpan::begin(ctx.workspace.as_path()).await;
                let effective_timeout_ms = match sandbox.effective_timeout_ms(timeout_ms) {
                    Ok(value) => value,
                    Err(error) => {
                        return Ok(
                            settled_pre_spawn_refusal(ctx, span, &error, isolated, true).await
                        );
                    }
                };
                let command_env = span.env(ctx.command_env.clone());
                let timeout_declared = declared_timeout_ms.is_some();
                // A child token keeps the outer turn's cancel authority, while
                // the progress watchdog can end a *silent* command through the
                // same token. ksandbox still performs its owned cleanup because
                // we never drop the execution future on the owned-deadline path.
                let command_cancellation = ctx.cancellation_token().child_token();
                let sandbox_timeout_ms = if timeout_declared {
                    timeout_ms
                } else {
                    // Without a model estimate, ksandbox only holds the absolute
                    // ceiling; output progress, not wall clock, decides stalls.
                    PROGRESS_HARD_CEILING_MS
                };
                let idle_timed_out = Arc::new(std::sync::atomic::AtomicBool::new(false));
                let watchdog = if timeout_declared {
                    None
                } else {
                    let observer = Arc::clone(&event_observer);
                    let token = command_cancellation.clone();
                    let idle_flag = Arc::clone(&idle_timed_out);
                    Some(tokio::spawn(async move {
                        loop {
                            tokio::time::sleep(Duration::from_millis(1_000)).await;
                            if token.is_cancelled() {
                                return;
                            }
                            if observer.progress.idle_remaining_ms(IDLE_TIMEOUT_MS) == 0 {
                                idle_flag.store(true, std::sync::atomic::Ordering::Release);
                                token.cancel();
                                return;
                            }
                        }
                    }))
                };
                let execution = sandbox.exec_cancellable(
                    crate::sandbox::SandboxCommandRequest {
                        command: command_for_sandbox(command),
                        guest_workspace: "/workspace".to_string(),
                        timeout_ms: sandbox_timeout_ms,
                        output_observer: output_observer.clone(),
                        env: command_env,
                    },
                    &command_cancellation,
                );
                let wall_ms = timeout_ms.saturating_add(sandbox.execution_overhead_ms());
                let execution = if sandbox.owns_command_deadline() {
                    // The sandbox owns its deadline and cleanup; the watchdog
                    // cancels the token when the command goes silent.
                    Ok(execution.await)
                } else if timeout_declared {
                    // The model estimated a duration: honour that estimate exactly.
                    tokio::time::timeout(Duration::from_millis(wall_ms), execution)
                        .await
                        .map_err(|_| ())
                } else {
                    // No estimate: a streaming command is making progress, so
                    // wall-clock time is not a stall signal. Only a silent
                    // command (or the absolute ceiling) ends the wait.
                    wait_for_progress(
                        execution,
                        &event_observer.progress,
                        PROGRESS_HARD_CEILING_MS,
                        IDLE_TIMEOUT_MS,
                    )
                    .await
                };
                if let Some(handle) = watchdog {
                    handle.abort();
                }
                let (result, termination) = match execution {
                    Ok(Err(error))
                        if crate::sandbox::native::is_native_process_lifetime_unqualified(
                            &error,
                        ) || crate::sandbox::process_host::is_process_host_spawn_refused(
                            &error,
                        ) || is_host_shell_spawn_refused(&error)
                            || crate::sandbox::violation_code_of(&error).is_some() =>
                    {
                        // The process does not exist. A later string wrap would
                        // drop this type and the receipt would book Unknown.
                        return Ok(
                            settled_pre_spawn_refusal(ctx, span, &error, isolated, true).await
                        );
                    }
                    Ok(result) => result
                        .map_err(|e| anyhow::anyhow!("Sandbox bash execution failed: {}", e))?,
                    Err(_) => {
                        let capture_summary = *event_observer
                            .summary
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        let capture_metadata =
                            capture_summary.map(|summary| capture_output_metadata(summary, true));
                        let mut timed_out = ToolOutput::error(format!(
                            "[Command timed out after {}ms]\n[Command termination was not observed]\n{}\n[progress {}]",
                            timeout_ms,
                            timeout_retry_hint(declared_timeout_ms),
                            event_observer.progress.snapshot(),
                        ));
                        timed_out.metadata = Some(with_changed_paths(
                            serde_json::json!({
                                "exit_code": null,
                                "timeout_ms": timeout_ms,
                                "observation_incomplete": true,
                                "output_complete": false,
                                "sandboxed": isolated,
                                "output": capture_metadata,
                            }),
                            &span.finish(ctx).await,
                        ));
                        timed_out.error_kind = Some(ToolErrorKind::Timeout {
                            op: "bash".to_string(),
                            duration_ms: timeout_ms,
                        });
                        return Ok(timed_out);
                    }
                };

                // Strip terminal ANSI escapes and collapse bare `\r` progress
                // frames while keeping `\r\n` verbatim, BEFORE hashing streams.
                let clean_stdout = clean_terminal_output(&result.stdout);
                let clean_stderr = clean_terminal_output(&result.stderr);
                let streams = output_streams::capture(clean_stdout.as_ref(), clean_stderr.as_ref());
                let mut output = clean_stdout.into_owned();
                if !clean_stderr.is_empty() {
                    output.push_str(clean_stderr.as_ref());
                }

                let capture_summary = *event_observer
                    .summary
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let capture_metadata = capture_summary
                    .map(|summary| capture_output_metadata(summary, summary.timed_out));
                let watchdog_stalled = idle_timed_out.load(std::sync::atomic::Ordering::Acquire)
                    && !ctx.cancellation_token().is_cancelled();
                if termination == crate::tools::OwnedProcessTermination::Cancelled
                    && !watchdog_stalled
                {
                    return Ok(cancelled_command(
                        ctx,
                        span,
                        output,
                        result.exit_code,
                        serde_json::json!({
                            "sandboxed": isolated,
                            "output": capture_metadata,
                            "shell_output_streams": streams,
                        }),
                    )
                    .await);
                }
                if result.timed_out || watchdog_stalled {
                    let reported_timeout_ms = if watchdog_stalled {
                        IDLE_TIMEOUT_MS
                    } else {
                        effective_timeout_ms
                    };
                    let mut timed_out = ToolOutput::error(format!(
                        "{}\n\n[Command timed out after {}ms]\n{}\n[progress {}]",
                        output,
                        reported_timeout_ms,
                        timeout_retry_hint(declared_timeout_ms),
                        event_observer.progress.snapshot(),
                    ));
                    timed_out.metadata = Some(with_changed_paths(
                        serde_json::json!({
                            "exit_code": result.exit_code,
                            "timeout_ms": reported_timeout_ms,
                            "requested_timeout_ms": requested_timeout_ms,
                            "sandboxed": isolated,
                            "output": capture_metadata,
                            "shell_output_streams": streams,
                            "progress": event_observer.progress.snapshot(),
                        }),
                        &span.finish(ctx).await,
                    ));
                    timed_out.error_kind = Some(ToolErrorKind::Timeout {
                        op: "bash".to_string(),
                        duration_ms: reported_timeout_ms,
                    });
                    return Ok(timed_out);
                }

                let mut base_metadata = serde_json::json!({
                    "exit_code": result.exit_code,
                    "sandboxed": isolated,
                    "output": capture_metadata,
                    "shell_output_streams": streams,
                });
                if let Some(grant) = grant_metadata {
                    base_metadata["network_grant"] = grant;
                }
                let changed_paths = span.finish(ctx).await;
                return Ok(ToolOutput {
                    content: output,
                    success: result.exit_code == 0,
                    metadata: crate::verification::merge_shell_verification_metadata(
                        Some(with_changed_paths(base_metadata, &changed_paths)),
                        Some(ctx.workspace.as_path()),
                        command,
                        result.exit_code,
                        None,
                    ),
                    images: vec![],
                    error_kind: None,
                    trust: crate::tools::ToolResultTrustV1::WorkspaceData,
                });
            }
        }

        let Some(runner) = ctx.workspace_services.command_runner() else {
            return Ok(ToolOutput::error(
                "Workspace backend does not provide a command runner for bash execution",
            ));
        };
        let span = CommandSpan::begin(ctx.workspace.as_path()).await;
        let command_env = span.env(ctx.command_env.clone());
        let cancellation = ctx.cancellation_token();
        let (result, termination) = match runner
            .exec_cancellable(
                CommandRequest {
                    command: command.to_string(),
                    timeout_ms,
                    output_observer,
                    env: command_env,
                },
                &cancellation,
            )
            .await
        {
            Ok(outcome) => outcome,
            Err(error) if is_host_shell_spawn_refused(&error) => {
                return Ok(settled_pre_spawn_refusal(
                    ctx,
                    span,
                    &error,
                    false,
                    ctx.sandbox.is_some(),
                )
                .await);
            }
            Err(error) => {
                return Err(anyhow::anyhow!(
                    "Workspace bash execution failed: {}",
                    error
                ));
            }
        };
        let capture_summary = *event_observer.summary.lock().unwrap();
        let capture_metadata =
            capture_summary.map(|summary| capture_output_metadata(summary, summary.timed_out));
        let clean_output = clean_terminal_output(&result.output).into_owned();
        if termination == crate::tools::OwnedProcessTermination::Cancelled {
            return Ok(cancelled_command(
                ctx,
                span,
                clean_output,
                result.exit_code,
                serde_json::json!({
                    "sandboxed": false,
                    "output": capture_metadata,
                }),
            )
            .await);
        }
        let changed_paths = span.finish(ctx).await;

        if result.timed_out {
            let mut output = ToolOutput::error(format!(
                "{}\n\n[Command timed out after {}ms]",
                clean_output, timeout_ms
            ));
            output.metadata = crate::verification::merge_shell_verification_metadata(
                Some(with_changed_paths(
                    serde_json::json!({
                        "exit_code": result.exit_code,
                        "timeout_ms": timeout_ms,
                        "sandboxed": false,
                        "output": capture_metadata,
                    }),
                    &changed_paths,
                )),
                Some(ctx.workspace.as_path()),
                command,
                result.exit_code,
                Some("command timed out"),
            );
            output.error_kind = Some(ToolErrorKind::Timeout {
                op: "bash".to_string(),
                duration_ms: timeout_ms,
            });
            return Ok(output);
        }

        Ok(ToolOutput {
            content: clean_output,
            success: result.exit_code == 0,
            metadata: crate::verification::merge_shell_verification_metadata(
                Some(with_changed_paths(
                    serde_json::json!({
                        "exit_code": result.exit_code,
                        "sandboxed": false,
                        "output": capture_metadata,
                    }),
                    &changed_paths,
                )),
                Some(ctx.workspace.as_path()),
                command,
                result.exit_code,
                None,
            ),
            images: vec![],
            error_kind: None,
            trust: crate::tools::ToolResultTrustV1::WorkspaceData,
        })
    }
}

/// A command that does not name its files must not hide a path another
/// session already claimed. A missing session with an empty claim map still
/// runs; that is observation-after, not a pre-apply session gate.
fn refuse_hidden_foreign_write(ctx: &ToolContext) -> Option<ToolOutput> {
    match crate::external_observation::refuse_foreign_workspace_owner(
        ctx.session_id.as_deref(),
        &ctx.workspace,
    ) {
        Ok(()) => None,
        Err(error) => Some(ToolOutput::error(error)),
    }
}

/// Resolve and apply a host-approved network grant for the bash tool.
///
/// The caller has already routed the request through host confirmation
/// (`requires_confirmation`), so reaching this point means the user approved
/// exactly this origin. The grant is digest-pinned: if the session policy
/// moved since the sandbox snapshot the model observed, application refuses
/// and the model must re-request.
#[allow(clippy::result_large_err)]
fn resolve_network_grant(
    args: &serde_json::Value,
    sandbox: Option<&dyn crate::sandbox::BashSandbox>,
) -> Result<serde_json::Value, ToolOutput> {
    let Some(sandbox) = sandbox else {
        return Err(ToolOutput::error(
            "request_network_grant requires a configured sandbox",
        ));
    };
    let grant_args = args.get("network_grant");
    let host = grant_args
        .and_then(|g| g.get("host"))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|host| !host.is_empty());
    let Some(host) = host else {
        return Err(ToolOutput::error(
            "network_grant.host is required when sandbox_permissions is request_network_grant",
        ));
    };
    let port = match grant_args
        .and_then(|g| g.get("port"))
        .and_then(|v| v.as_u64())
    {
        Some(raw) => match u16::try_from(raw) {
            Ok(port) => Some(port),
            Err(_) => {
                return Err(ToolOutput::error(format!(
                    "network_grant.port {raw} is out of range"
                )));
            }
        },
        None => None,
    };
    let grant = match crate::sandbox::NetworkGrant::new(host, port) {
        Ok(grant) => grant,
        Err(error) => return Err(ToolOutput::error(error.to_string())),
    };
    let Some(base) = sandbox.policy_digest() else {
        return Err(ToolOutput::error(
            "this sandbox backend does not expose a policy digest; grants are unavailable",
        ));
    };
    match sandbox.apply_network_grant(grant, &base) {
        Ok(digest) => Ok(serde_json::json!({
            "host": host,
            "port": port,
            "policy_digest": digest,
        })),
        Err(error) => Err(ToolOutput::error(format!(
            "network grant refused: {error}. Re-check the current policy and re-request."
        ))),
    }
}

/// A detached job shares the workspace and outlives this tool result. The
/// completion gate waits for the same child slot foreground tasks use.
async fn observe_detached_job(args: &serde_json::Value, ctx: &ToolContext) -> Option<ToolOutput> {
    let session_id = ctx.session_id.as_deref()?;
    crate::shell_session::cwd(session_id)?;
    if args.get("job_action").and_then(|value| value.as_str()) != Some("detach") {
        return None;
    }
    let command = args.get("command").and_then(|value| value.as_str())?;
    if let Some(denied) = refuse_hidden_foreign_write(ctx) {
        return Some(denied);
    }
    let watch = crate::porcelain::Watch::start(ctx.workspace.as_path()).await;
    match crate::shell_session::detach(session_id, command, shell_admission(ctx, command)) {
        Ok(job_id) => {
            crate::porcelain::install_workspace_child(&job_id, watch);
            let session = session_id.to_string();
            let job = job_id.clone();
            let workspace = ctx.workspace.clone();
            tokio::spawn(async move {
                let mut guard = crate::porcelain::WorkspaceChildGuard::new(&job);
                loop {
                    match crate::shell_session::poll(&session, &job) {
                        Ok(status) if status == "running" => {
                            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                        }
                        _ => break,
                    }
                }
                crate::porcelain::settle_workspace_child_guard(&mut guard, &workspace).await;
                if let Some(paths) = crate::porcelain::peek_settled_workspace_child(&job) {
                    claim_dirtied_paths(
                        &ToolContext::new(workspace.as_path().to_path_buf())
                            .with_session_id(&session),
                        &paths,
                    );
                }
            });
            Some(
                ToolOutput::success(job_id.clone()).with_metadata(serde_json::json!({
                    "job_id": job_id,
                    "workspace_child": job_id,
                })),
            )
        }
        Err(error) => {
            drop(watch);
            Some(ToolOutput::error(error.to_string()))
        }
    }
}

fn session_shell_control(args: &serde_json::Value, ctx: &ToolContext) -> Option<ToolOutput> {
    let session_id = ctx.session_id.as_deref()?;
    crate::shell_session::cwd(session_id)?;
    let action = args.get("job_action").and_then(|value| value.as_str())?;
    let job_id = args
        .get("job_id")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    let result = match action {
        "poll" => crate::shell_session::poll(session_id, job_id),
        "kill" => crate::shell_session::kill(session_id, job_id).map(|()| "killed".to_string()),
        _ => {
            return Some(ToolOutput::error(format!(
                "unsupported job_action: {action}"
            )));
        }
    };
    Some(match result {
        Ok(text) => ToolOutput::success(text),
        Err(error) => ToolOutput::error(error.to_string()),
    })
}

/// Command text handed to the configured sandbox.
///
/// Unix sandboxes run a POSIX shell, so the command is unchanged. The Windows
/// native sandbox runs PowerShell and does not define `test`. A single
/// `test -f` / `test -e` check becomes a short `Test-Path` script. The full
/// host compatibility shim is not prepended: AppContainer rejects command
/// lines that exceed the Windows limit (error 206). Every other command,
/// including `echo`, is left unchanged. The completion gate still parses the
/// original `test -f` text from the tool arguments.
pub(crate) fn command_for_sandbox(command: &str) -> String {
    #[cfg(windows)]
    {
        windows_existence_check(command).unwrap_or_else(|| command.to_string())
    }
    #[cfg(not(windows))]
    {
        command.to_string()
    }
}

#[cfg(windows)]
fn windows_existence_check(command: &str) -> Option<String> {
    let tail = command.rsplit("&&").next()?.trim();
    if tail.contains(['\n', ';', '|', '>']) {
        return None;
    }
    let path = crate::verification::path_from_existence_check_command(tail)?;
    if path.contains(['\'', '\n', '\r']) {
        return None;
    }
    let path_type =
        if tail.contains(" -e ") || tail.starts_with("test -e") || tail.contains("[ -e ") {
            ""
        } else {
            " -PathType Leaf"
        };
    Some(format!(
        "if (Test-Path -LiteralPath '{path}'{path_type}) {{ exit 0 }} else {{ exit 1 }}"
    ))
}

/// Quote a path for safe single-word interpolation into a POSIX shell
/// command. The session cwd can contain anything the filesystem allows
/// (spaces, `$(`, backticks): unquoted interpolation would execute
/// metacharacters at shell parse time, after every permission surface has
/// approved a different string.
fn shell_quote_path(path: &std::path::Path) -> String {
    let text = path.to_string_lossy();
    if text.is_empty() {
        return "''".to_string();
    }
    if text
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '_' | '-'))
    {
        return text.into_owned();
    }
    format!("'{}'", text.replace('\'', "'\\''"))
}

fn prefix_session_cwd(command: &str, ctx: &ToolContext) -> String {
    let Some(session_id) = ctx.session_id.as_deref() else {
        return command.to_string();
    };
    if let Ok(admitted) =
        crate::shell_session::admit(session_id, command, shell_admission(ctx, command))
    {
        let cwd = shell_quote_path(&admitted.cwd);
        if command.trim().starts_with("cd ") {
            return format!("cd {cwd}");
        }
        return format!("cd {cwd} && {command}");
    }
    command.to_string()
}

/// The shell applies cwd only after admission. A run checker that denies this
/// command must not move the session, even if an earlier command was allowed.
fn shell_admission(ctx: &ToolContext, command: &str) -> crate::shell_session::CommandAdmission {
    let Some(checker) = ctx.run_permission_checker() else {
        return crate::shell_session::CommandAdmission::Allow;
    };
    if checker.check("bash", &serde_json::json!({ "command": command }))
        == crate::permissions::PermissionDecision::Deny
    {
        crate::shell_session::CommandAdmission::Deny
    } else {
        crate::shell_session::CommandAdmission::Allow
    }
}

/// Strip ANSI CSI/OSC escape sequences and collapse bare `\r` progress-bar
/// overwrites while preserving `\r\n` CRLF line endings verbatim.
pub(crate) fn clean_terminal_output(input: &str) -> std::borrow::Cow<'_, str> {
    if !input.as_bytes().contains(&0x1b) && !has_bare_cr(input) {
        return std::borrow::Cow::Borrowed(input);
    }
    let stripped = if input.as_bytes().contains(&0x1b) {
        std::borrow::Cow::Owned(strip_ansi_escapes(input))
    } else {
        std::borrow::Cow::Borrowed(input)
    };
    if !has_bare_cr(stripped.as_ref()) {
        return stripped;
    }
    std::borrow::Cow::Owned(collapse_bare_cr(stripped.as_ref()))
}

fn has_bare_cr(s: &str) -> bool {
    let bytes = s.as_bytes();
    bytes
        .iter()
        .enumerate()
        .any(|(i, &b)| b == b'\r' && bytes.get(i + 1) != Some(&b'\n'))
}

fn strip_ansi_escapes(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == 0x1b {
            match bytes.get(i + 1).copied() {
                Some(b'[') => {
                    i += 2;
                    while i < bytes.len() && (0x20..=0x3F).contains(&bytes[i]) {
                        i += 1;
                    }
                    if i < bytes.len() && (0x40..=0x7E).contains(&bytes[i]) {
                        i += 1;
                    }
                    continue;
                }
                Some(b']') => {
                    i += 2;
                    while i < bytes.len() {
                        if bytes[i] == 0x07 {
                            i += 1;
                            break;
                        }
                        if bytes[i] == 0x1b && bytes.get(i + 1) == Some(&b'\\') {
                            i += 2;
                            break;
                        }
                        i += 1;
                    }
                    continue;
                }
                Some(b) if (0x40..=0x7E).contains(&b) => {
                    i += 2;
                    continue;
                }
                _ => {
                    i += 1;
                    continue;
                }
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    // SAFETY: We copied `input.as_bytes()` verbatim except for ASCII escape
    // sequences (bytes in 0x00..=0x7E), so every UTF-8 multi-byte sequence is intact.
    String::from_utf8(out).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned())
}

fn collapse_bare_cr(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for line in input.split_inclusive('\n') {
        let (body, ending) = if let Some(stripped) = line.strip_suffix("\r\n") {
            (stripped, "\r\n")
        } else if let Some(stripped) = line.strip_suffix('\n') {
            (stripped, "\n")
        } else {
            (line, "")
        };
        if let Some(last) = body.split('\r').rfind(|segment| !segment.is_empty()) {
            out.push_str(last);
        }
        out.push_str(ending);
    }
    out
}

#[cfg(test)]
#[path = "bash/tests.rs"]
mod tests;
