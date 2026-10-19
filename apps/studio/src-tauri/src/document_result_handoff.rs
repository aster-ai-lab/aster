//! A PDF result crosses into the main window's existing Documents save dialog.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use base64::Engine;
use serde::{Deserialize, Serialize};
use tauri::Manager;

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResultFile {
    name: String,
    media_type: String,
    bytes_base64: String,
}

#[derive(Clone, Serialize)]
pub struct ResultRequest {
    id: String,
    file: ResultFile,
}

#[derive(Clone, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ResultStatus {
    Pending,
    Saved { location: String },
    Cancelled,
}

struct Transfer {
    request: ResultRequest,
    taken: bool,
    status: ResultStatus,
    created: Instant,
}

static TRANSFERS: Mutex<Vec<Transfer>> = Mutex::new(Vec::new());

fn check_window(window: &str, webview: &str, expected: &str) -> Result<(), String> {
    if window != expected || webview != expected {
        return Err("Document result request refused.".to_owned());
    }
    Ok(())
}

fn validate(file: &ResultFile) -> Result<(), String> {
    let name = file.name.trim();
    let limit = crate::workspace_file::RAW_LIMIT as usize;
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.starts_with('.')
        || name.chars().count() > 180
        || name.contains(['/', '\\', '\0'])
        || name.chars().any(char::is_control)
        || file.media_type.len() > 120
        || file.media_type.chars().any(char::is_control)
        || file.bytes_base64.len() > limit.div_ceil(3) * 4
    {
        return Err("Document result request refused.".to_owned());
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&file.bytes_base64)
        .map_err(|_| "Document result request refused.".to_owned())?;
    if bytes.is_empty() || bytes.len() > limit {
        return Err("Document result request refused.".to_owned());
    }
    Ok(())
}

fn transfers() -> Result<std::sync::MutexGuard<'static, Vec<Transfer>>, String> {
    let mut transfers = TRANSFERS
        .lock()
        .map_err(|_| "Document result request unavailable.".to_owned())?;
    transfers.retain(|value| value.created.elapsed() <= Duration::from_secs(600));
    Ok(transfers)
}

fn stage_result(transfers: &mut Vec<Transfer>, file: ResultFile) -> Result<String, String> {
    if transfers
        .iter()
        .any(|value| matches!(value.status, ResultStatus::Pending))
    {
        return Err("A document result is already being saved.".to_owned());
    }
    // Retain small receipts so the next save cannot erase an unread acknowledgement.
    if transfers.len() >= 32 {
        transfers.remove(0);
    }
    let id = uuid::Uuid::now_v7().to_string();
    transfers.push(Transfer {
        request: ResultRequest {
            id: id.clone(),
            file,
        },
        taken: false,
        status: ResultStatus::Pending,
        created: Instant::now(),
    });
    Ok(id)
}

fn take_result(transfers: &mut [Transfer]) -> Option<ResultRequest> {
    let transfer = transfers
        .iter_mut()
        .find(|value| matches!(value.status, ResultStatus::Pending))?;
    transfer.taken = true;
    Some(transfer.request.clone())
}

fn finish_result(
    transfers: &mut [Transfer],
    id: &str,
    location: Option<String>,
) -> Result<(), String> {
    let status = match location {
        Some(location) if !location.is_empty() && location.len() <= 4096 => {
            ResultStatus::Saved { location }
        }
        None => ResultStatus::Cancelled,
        _ => return Err("Document result request refused.".to_owned()),
    };
    let transfer = transfers
        .iter_mut()
        .find(|value| value.request.id == id && value.taken)
        .ok_or_else(|| "Document result request refused.".to_owned())?;
    if !matches!(transfer.status, ResultStatus::Pending) && transfer.status != status {
        return Err("Document result request refused.".to_owned());
    }
    transfer.status = status;
    transfer.request.file.bytes_base64 = String::new();
    Ok(())
}

#[tauri::command]
pub fn stage_document_result_save<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    webview: tauri::Webview<R>,
    file: ResultFile,
) -> Result<String, String> {
    check_window(webview.window().label(), webview.label(), "applet-pdf")?;
    validate(&file)?;
    let main = app
        .get_window("main")
        .ok_or_else(|| "Document result request unavailable.".to_owned())?;
    let id = stage_result(&mut *transfers()?, file)?;
    #[cfg(debug_assertions)]
    eprintln!("studio-result-save staged {id}");
    let _ = main.unminimize();
    let _ = main.show();
    let _ = main.set_focus();
    Ok(id)
}

#[tauri::command]
pub fn take_document_result_save<R: tauri::Runtime>(
    webview: tauri::Webview<R>,
) -> Result<Option<ResultRequest>, String> {
    check_window(webview.window().label(), webview.label(), "main")?;
    #[cfg(debug_assertions)]
    {
        static READY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        if !READY.swap(true, std::sync::atomic::Ordering::Relaxed) {
            eprintln!("studio-result-save receiver-ready");
        }
    }
    let request = take_result(&mut transfers()?);
    #[cfg(debug_assertions)]
    if let Some(request) = &request {
        eprintln!("studio-result-save delivered {}", request.id);
    }
    Ok(request)
}

#[tauri::command]
pub fn finish_document_result_save<R: tauri::Runtime>(
    webview: tauri::Webview<R>,
    id: String,
    location: Option<String>,
) -> Result<(), String> {
    check_window(webview.window().label(), webview.label(), "main")?;
    finish_result(&mut transfers()?, &id, location)
}

#[tauri::command]
pub fn document_result_save_status<R: tauri::Runtime>(
    webview: tauri::Webview<R>,
    id: String,
) -> Result<ResultStatus, String> {
    check_window(webview.window().label(), webview.label(), "applet-pdf")?;
    transfers()?
        .iter()
        .find(|value| value.request.id == id)
        .map(|value| value.status.clone())
        .ok_or_else(|| "Document result request unavailable.".to_owned())
}

#[cfg(test)]
#[path = "document_result_handoff_tests.rs"]
mod tests;
