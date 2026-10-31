//! One step in one fresh `aster-box` MicroVM.
//!
//! The job's secrets and the step command travel in a 0600 `step.sh`
//! mounted at `/run/aster`, never in argv: the box command is constant.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, watch};
use tokio::time::Instant;

use crate::state::private_file;

pub const BOX_CLI_ENV: &str = "ASTER_BOX_CLI";
pub const GUEST_WORKSPACE: &str = "/workspace";
pub const GUEST_ENV_DIR: &str = "/run/aster";
pub const STEP_SCRIPT: &str = "step.sh";
pub const BOX_COMMAND: &str = ". /run/aster/step.sh && exec sh -c \"$ASTER_STEP\"";
/// The box's own timeout is a backstop behind the runner's deadline.
const BOX_TIMEOUT_GRACE_SECS: u64 = 10;
const STOP_TIMEOUT: Duration = Duration::from_secs(30);
const STOP_GRACE: Duration = Duration::from_secs(15);
/// A helper that inherited the box's pipes must not hold the step open.
const READER_DRAIN: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoxSettings {
    pub image: String,
    pub cpus: u32,
    pub memory: String,
}

pub struct StepRun<'a> {
    pub box_name: String,
    pub checkout: &'a Path,
    pub env_dir: &'a Path,
    pub deadline: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepExit {
    Exited(i32),
    TimedOut,
    Cancelled,
}

/// Set when the server ended the job or the runner is shutting down.
#[derive(Clone)]
pub struct Cancel {
    pub ended: watch::Receiver<bool>,
    pub shutdown: watch::Receiver<bool>,
}

impl Cancel {
    pub fn ended(&self) -> bool {
        *self.ended.borrow()
    }

    pub fn shutting_down(&self) -> bool {
        *self.shutdown.borrow()
    }

    pub fn is_set(&self) -> bool {
        self.ended() || self.shutting_down()
    }

    pub async fn wait(mut self) {
        tokio::select! {
            _ = until_set(&mut self.ended) => {}
            _ = until_set(&mut self.shutdown) => {}
        }
    }
}

async fn until_set(receiver: &mut watch::Receiver<bool>) {
    loop {
        if *receiver.borrow_and_update() {
            return;
        }
        if receiver.changed().await.is_err() {
            std::future::pending::<()>().await;
        }
    }
}

#[async_trait]
pub trait Executor: Send + Sync {
    /// Runs one step; its output goes to `output` as it arrives.
    async fn run(
        &self,
        step: &StepRun<'_>,
        output: mpsc::UnboundedSender<String>,
        cancel: Cancel,
    ) -> Result<StepExit, String>;
}

pub fn box_run_arguments(
    settings: &BoxSettings,
    step: &StepRun<'_>,
    timeout_secs: u64,
) -> Result<Vec<String>, String> {
    let checkout = mount_source(step.checkout)?;
    let env_dir = mount_source(step.env_dir)?;
    Ok(vec![
        "run".into(),
        "--rm".into(),
        "--no-stdin".into(),
        "--log-driver".into(),
        "none".into(),
        "--name".into(),
        step.box_name.clone(),
        "--cpus".into(),
        settings.cpus.to_string(),
        "--memory".into(),
        settings.memory.clone(),
        "--timeout".into(),
        timeout_secs.to_string(),
        "-v".into(),
        format!("{checkout}:{GUEST_WORKSPACE}"),
        "-v".into(),
        format!("{env_dir}:{GUEST_ENV_DIR}"),
        "-w".into(),
        GUEST_WORKSPACE.into(),
        settings.image.clone(),
        "--".into(),
        "sh".into(),
        "-c".into(),
        BOX_COMMAND.into(),
    ])
}

fn mount_source(path: &Path) -> Result<&str, String> {
    let text = path
        .to_str()
        .filter(|text| path.is_absolute() && !text.contains(':') && !text.contains('\n'))
        .ok_or_else(|| format!("{} cannot be mounted into a box", path.display()))?;
    Ok(text)
}

fn shell_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

/// Single-quoted `export` lines, in order; a later name wins.
pub fn render_step_script(variables: &[(String, String)]) -> Result<String, String> {
    let mut script = String::new();
    for (name, value) in variables {
        if !shell_name(name) {
            return Err(format!("`{name}` is not a shell variable name"));
        }
        if value.contains('\0') {
            return Err(format!("`{name}` holds a NUL byte"));
        }
        script.push_str(&format!(
            "export {name}='{}'\n",
            value.replace('\'', "'\\''")
        ));
    }
    Ok(script)
}

/// Replaces `<env_dir>/step.sh` with a fresh 0600 file.
pub fn write_step_script(env_dir: &Path, script: &str) -> Result<(), String> {
    let path = env_dir.join(STEP_SCRIPT);
    let failed = |error: std::io::Error| format!("step script: {error}");
    match std::fs::remove_file(&path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(failed(error)),
    }
    let mut file = private_file(&path).map_err(failed)?;
    file.write_all(script.as_bytes()).map_err(failed)?;
    file.sync_all().map_err(failed)
}

/// Explicit path, then `ASTER_BOX_CLI`, then `aster-box` on `PATH`. A set
/// `ASTER_BOX_CLI` that is not a file does not fall through. The path is
/// canonicalized: aster-box finds its guest init next to the real binary.
pub fn resolve_box_cli(explicit: Option<&Path>) -> Result<PathBuf, String> {
    let chosen = if let Some(path) = explicit {
        path.to_path_buf()
    } else if let Some(raw) = std::env::var_os(BOX_CLI_ENV) {
        PathBuf::from(raw)
    } else {
        std::env::var_os("PATH")
            .and_then(|paths| {
                std::env::split_paths(&paths)
                    .map(|dir| dir.join("aster-box"))
                    .find(|candidate| candidate.is_file())
            })
            .ok_or_else(|| "aster-box is not on PATH (set box_cli or ASTER_BOX_CLI)".to_string())?
    };
    if !chosen.is_file() {
        return Err(format!("{} is not an aster-box executable", chosen.display()));
    }
    std::fs::canonicalize(&chosen).map_err(|error| format!("{}: {error}", chosen.display()))
}

pub struct BoxExecutor {
    cli: PathBuf,
    settings: BoxSettings,
    stop_grace: Duration,
}

impl BoxExecutor {
    pub fn new(cli: PathBuf, settings: BoxSettings) -> Self {
        Self {
            cli,
            settings,
            stop_grace: STOP_GRACE,
        }
    }

    #[cfg(test)]
    fn with_stop_grace(mut self, stop_grace: Duration) -> Self {
        self.stop_grace = stop_grace;
        self
    }

    /// Ctrl-C first: `aster-box run` then stops and removes its own box.
    /// A box still running after the grace period is killed and removed.
    async fn stop(&self, child: &mut Child, name: &str) {
        if let Some(pid) = child.id() {
            let interrupt = Command::new("kill")
                .args(["-INT", &pid.to_string()])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            let _ = tokio::time::timeout(STOP_TIMEOUT, interrupt).await;
            if tokio::time::timeout(self.stop_grace, child.wait())
                .await
                .is_ok()
            {
                return;
            }
        }
        self.force_stop(name).await;
        let _ = child.kill().await;
    }

    /// Kills and removes the box; it may already be gone.
    async fn force_stop(&self, name: &str) {
        for args in [vec!["kill", name], vec!["rm", "-f", name]] {
            let mut command = Command::new(&self.cli);
            command
                .args(args)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            let _ = tokio::time::timeout(STOP_TIMEOUT, command.status()).await;
        }
    }
}

#[async_trait]
impl Executor for BoxExecutor {
    async fn run(
        &self,
        step: &StepRun<'_>,
        output: mpsc::UnboundedSender<String>,
        cancel: Cancel,
    ) -> Result<StepExit, String> {
        let remaining = step.deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Ok(StepExit::TimedOut);
        }
        let args = box_run_arguments(
            &self.settings,
            step,
            remaining.as_secs() + BOX_TIMEOUT_GRACE_SECS,
        )?;
        let mut command = Command::new(&self.cli);
        command
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        #[cfg(unix)]
        command.process_group(0);
        let mut child = command
            .spawn()
            .map_err(|error| format!("aster-box did not start: {error}"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "aster-box has no stdout".to_string())?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| "aster-box has no stderr".to_string())?;
        let readers = [
            tokio::spawn(forward(stdout, output.clone())),
            tokio::spawn(forward(stderr, output)),
        ];
        let exit = tokio::select! {
            status = child.wait() => match status {
                Ok(status) => StepExit::Exited(status.code().unwrap_or(-1)),
                Err(error) => return Err(format!("aster-box: {error}")),
            },
            _ = tokio::time::sleep_until(step.deadline) => StepExit::TimedOut,
            _ = cancel.wait() => StepExit::Cancelled,
        };
        if !matches!(exit, StepExit::Exited(_)) {
            self.stop(&mut child, &step.box_name).await;
        }
        for reader in readers {
            let abort = reader.abort_handle();
            if tokio::time::timeout(READER_DRAIN, reader).await.is_err() {
                abort.abort();
            }
        }
        Ok(exit)
    }
}

/// Forwards text as it arrives; a character split across reads waits for
/// its remaining bytes.
async fn forward<R: AsyncRead + Unpin>(mut reader: R, output: mpsc::UnboundedSender<String>) {
    let mut buffer = [0u8; 8192];
    let mut carry = Vec::new();
    loop {
        let read = match reader.read(&mut buffer).await {
            Ok(0) | Err(_) => break,
            Ok(read) => read,
        };
        carry.extend_from_slice(&buffer[..read]);
        let text = take_utf8(&mut carry);
        if !text.is_empty() {
            let _ = output.send(text);
        }
    }
    if !carry.is_empty() {
        let _ = output.send(String::from_utf8_lossy(&carry).into_owned());
    }
}

/// The decodable prefix of `bytes`, leaving an incomplete trailing
/// character in place; invalid bytes decode lossily.
pub fn take_utf8(bytes: &mut Vec<u8>) -> String {
    match std::str::from_utf8(bytes) {
        Ok(text) => {
            let text = text.to_string();
            bytes.clear();
            text
        }
        Err(error) if error.error_len().is_none() => {
            let valid = error.valid_up_to();
            let text = String::from_utf8_lossy(&bytes[..valid]).into_owned();
            bytes.drain(..valid);
            text
        }
        Err(_) => {
            let text = String::from_utf8_lossy(bytes).into_owned();
            bytes.clear();
            text
        }
    }
}

#[cfg(all(test, unix))]
#[path = "executor_tests.rs"]
mod tests;
