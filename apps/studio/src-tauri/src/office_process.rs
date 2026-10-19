//! Bounded lifetime and pipe consumption for one owned Office subprocess.

use std::path::Path;
use std::process::Stdio;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command;
use tokio::time::Instant;

pub(super) async fn run(
    bin: &Path,
    args: &[&str],
    deadline: Instant,
    limit: usize,
) -> Result<String, String> {
    if Instant::now() >= deadline {
        return Err("office-timeout".to_owned());
    }
    let mut child = Command::new(bin)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|_| "office".to_owned())?;
    let stdout = child.stdout.take().ok_or_else(|| "office".to_owned())?;
    let stderr = child.stderr.take().ok_or_else(|| "office".to_owned())?;
    let result = tokio::time::timeout_at(deadline, async {
        let (bytes, status, ()) = tokio::try_join!(
            bounded(stdout, limit),
            async { child.wait().await.map_err(|_| "office".to_owned()) },
            discard(stderr)
        )?;
        if !status.success() {
            return Err("unreadable".to_owned());
        }
        String::from_utf8(bytes).map_err(|_| "unreadable".to_owned())
    })
    .await
    .unwrap_or_else(|_| Err("office-timeout".to_owned()));
    if result.is_err() {
        // These handles own this exact child. Reap before returning to TempWork.
        let _ = child.kill().await;
        let _ = child.wait().await;
    }
    result
}

async fn bounded(reader: impl AsyncRead + Unpin, limit: usize) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    reader
        .take(limit.saturating_add(1) as u64)
        .read_to_end(&mut bytes)
        .await
        .map_err(|_| "office".to_owned())?;
    if bytes.len() > limit {
        return Err("office-output-limit".to_owned());
    }
    Ok(bytes)
}

async fn discard(mut reader: impl AsyncRead + Unpin) -> Result<(), String> {
    let mut buffer = [0u8; 8192];
    while reader
        .read(&mut buffer)
        .await
        .map_err(|_| "office".to_owned())?
        != 0
    {}
    Ok(())
}

#[cfg(test)]
#[path = "office_process_tests.rs"]
mod tests;
