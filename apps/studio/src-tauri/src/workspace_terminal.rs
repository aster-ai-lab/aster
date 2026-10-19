//! Host PTY for the task conversation terminal.
//!
//! The web view pulls output. It does not receive a push for every byte, so a
//! busy shell cannot stall the page.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;

use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use serde::Serialize;
use tauri::State;
use uuid::Uuid;

const DRAIN_CAP: usize = 32 * 1024;
const BUFFER_CAP: usize = 1024 * 1024;

#[derive(Default)]
pub struct WorkspaceTerminalState {
    sessions: Mutex<HashMap<String, LiveSession>>,
}

struct LiveSession {
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    master: Arc<Mutex<Box<dyn portable_pty::MasterPty + Send>>>,
    output: Arc<Mutex<OutputBuffer>>,
    child: Arc<Mutex<Box<dyn portable_pty::Child + Send + Sync>>>,
}

struct OutputBuffer {
    bytes: Vec<u8>,
    closed: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTerminalResponse {
    session_id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DrainTerminalResponse {
    bytes: Vec<u8>,
    closed: bool,
}

fn shell_program() -> std::ffi::OsString {
    std::env::var_os("SHELL")
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| {
            #[cfg(windows)]
            {
                std::ffi::OsString::from("cmd.exe")
            }
            #[cfg(not(windows))]
            {
                std::ffi::OsString::from("/bin/zsh")
            }
        })
}

fn push_output(output: &Mutex<OutputBuffer>, chunk: &[u8]) {
    let Ok(mut guard) = output.lock() else {
        return;
    };
    guard.bytes.extend_from_slice(chunk);
    if guard.bytes.len() > BUFFER_CAP {
        let drop_count = guard.bytes.len() - BUFFER_CAP;
        guard.bytes.drain(..drop_count);
    }
}

#[tauri::command]
pub fn workspace_terminal_create(
    state: State<'_, WorkspaceTerminalState>,
    cols: u16,
    rows: u16,
    cwd: Option<String>,
) -> Result<CreateTerminalResponse, String> {
    let cols = cols.max(20);
    let rows = rows.max(5);
    let pair = native_pty_system()
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|error| format!("Failed to open PTY: {error}"))?;

    let mut builder = CommandBuilder::new(shell_program());
    builder.env("TERM", "xterm-256color");
    #[cfg(not(windows))]
    builder.arg("-l");
    if let Some(cwd) = cwd
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let path = Path::new(cwd);
        if path.is_dir() {
            builder.cwd(path);
        }
    }

    let child = pair
        .slave
        .spawn_command(builder)
        .map_err(|error| format!("Failed to spawn shell: {error}"))?;
    drop(pair.slave);

    let mut reader = pair
        .master
        .try_clone_reader()
        .map_err(|error| format!("Failed to clone PTY reader: {error}"))?;
    let writer = pair
        .master
        .take_writer()
        .map_err(|error| format!("Failed to take PTY writer: {error}"))?;

    let session_id = Uuid::now_v7().to_string();
    let output = Arc::new(Mutex::new(OutputBuffer {
        bytes: Vec::new(),
        closed: false,
    }));
    let reader_output = Arc::clone(&output);
    thread::Builder::new()
        .name("workspace-terminal".to_owned())
        .spawn(move || {
            let mut buf = [0_u8; 8192];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(count) => push_output(&reader_output, &buf[..count]),
                    Err(_) => break,
                }
            }
            if let Ok(mut guard) = reader_output.lock() {
                guard.closed = true;
            }
        })
        .map_err(|error| format!("Failed to read PTY: {error}"))?;

    let session = LiveSession {
        writer: Arc::new(Mutex::new(writer)),
        master: Arc::new(Mutex::new(pair.master)),
        output,
        child: Arc::new(Mutex::new(child)),
    };
    state
        .sessions
        .lock()
        .map_err(|_| "Workspace terminal registry is unavailable.".to_owned())?
        .insert(session_id.clone(), session);

    Ok(CreateTerminalResponse { session_id })
}

#[tauri::command]
pub fn workspace_terminal_drain(
    state: State<'_, WorkspaceTerminalState>,
    session_id: String,
) -> Result<DrainTerminalResponse, String> {
    let output = {
        let sessions = state
            .sessions
            .lock()
            .map_err(|_| "Workspace terminal registry is unavailable.".to_owned())?;
        let session = sessions
            .get(&session_id)
            .ok_or_else(|| "Unknown workspace terminal session.".to_owned())?;
        Arc::clone(&session.output)
    };
    let mut guard = output
        .lock()
        .map_err(|_| "Workspace terminal output is locked.".to_owned())?;
    let count = guard.bytes.len().min(DRAIN_CAP);
    let bytes = guard.bytes.drain(..count).collect();
    Ok(DrainTerminalResponse {
        bytes,
        closed: guard.closed && guard.bytes.is_empty(),
    })
}

#[tauri::command]
pub fn workspace_terminal_write(
    state: State<'_, WorkspaceTerminalState>,
    session_id: String,
    data: String,
) -> Result<(), String> {
    let writer = {
        let sessions = state
            .sessions
            .lock()
            .map_err(|_| "Workspace terminal registry is unavailable.".to_owned())?;
        let session = sessions
            .get(&session_id)
            .ok_or_else(|| "Unknown workspace terminal session.".to_owned())?;
        Arc::clone(&session.writer)
    };
    let mut writer = writer
        .lock()
        .map_err(|_| "Workspace terminal writer is locked.".to_owned())?;
    writer
        .write_all(data.as_bytes())
        .map_err(|error| format!("Failed to write to PTY: {error}"))?;
    writer
        .flush()
        .map_err(|error| format!("Failed to flush PTY: {error}"))?;
    Ok(())
}

#[tauri::command]
pub fn workspace_terminal_resize(
    state: State<'_, WorkspaceTerminalState>,
    session_id: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    let master = {
        let sessions = state
            .sessions
            .lock()
            .map_err(|_| "Workspace terminal registry is unavailable.".to_owned())?;
        let session = sessions
            .get(&session_id)
            .ok_or_else(|| "Unknown workspace terminal session.".to_owned())?;
        Arc::clone(&session.master)
    };
    let master = master
        .lock()
        .map_err(|_| "Workspace terminal master is locked.".to_owned())?;
    master
        .resize(PtySize {
            rows: rows.max(5),
            cols: cols.max(20),
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|error| format!("Failed to resize PTY: {error}"))?;
    Ok(())
}

#[tauri::command]
pub fn workspace_terminal_kill(
    state: State<'_, WorkspaceTerminalState>,
    session_id: String,
) -> Result<(), String> {
    let Some(session) = state
        .sessions
        .lock()
        .map_err(|_| "Workspace terminal registry is unavailable.".to_owned())?
        .remove(&session_id)
    else {
        return Ok(());
    };
    if let Ok(mut child) = session.child.lock() {
        let _ = child.kill();
        let _ = child.wait();
    }
    Ok(())
}
