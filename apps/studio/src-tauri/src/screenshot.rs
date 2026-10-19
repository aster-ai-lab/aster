//! Interactive region capture for the composer screenshot control.
//!
//! macOS uses the system tool (`screencapture -i`), which is the same region
//! selector as the desktop shortcut. An empty result means the user cancelled.
//! A missing Screen Recording permission returns [`SCREEN_CAPTURE_PERMISSION_REQUIRED`]
//! so the composer can open that System Settings pane. Any other failure is
//! returned as an error so the composer can say why.

#[cfg(target_os = "macos")]
use std::fs;
#[cfg(target_os = "macos")]
use std::thread;
#[cfg(target_os = "macos")]
use std::time::Duration;
#[cfg(target_os = "macos")]
use tauri::Manager;

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
const MAXIMUM_SCREENSHOT_BYTES: u64 = 20 * 1024 * 1024;
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
const PNG_MAGIC: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];

/// Stable token. The composer opens System Settings only when the invoke error contains it.
const SCREEN_CAPTURE_PERMISSION_REQUIRED: &str = "screen-capture-permission-required";

/// Privacy & Security → Screen Recording. `system.shell.open_website` rejects this scheme.
const SCREEN_CAPTURE_SETTINGS_URL: &str =
    "x-apple.systempreferences:com.apple.settings.PrivacySecurity.extension?Privacy_ScreenCapture";

#[derive(Debug, Eq, PartialEq)]
enum CaptureStop {
    Cancelled,
    PermissionRequired,
    Failed(String),
}

/// No file was written. An empty tool message is a cancel only when capture is already allowed.
fn stop_without_file(authorized: bool, stderr: &str) -> CaptureStop {
    if !authorized {
        return CaptureStop::PermissionRequired;
    }
    if stderr.is_empty() {
        return CaptureStop::Cancelled;
    }
    CaptureStop::Failed(format!("The macOS screenshot tool failed: {stderr}"))
}

fn capture_stop_result(stop: CaptureStop) -> Result<Option<Vec<u8>>, String> {
    match stop {
        CaptureStop::Cancelled => Ok(None),
        CaptureStop::PermissionRequired => Err(SCREEN_CAPTURE_PERMISSION_REQUIRED.to_owned()),
        CaptureStop::Failed(message) => Err(message),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScreenshotMode {
    Direct,
    HideWindow,
}

fn parse_mode(mode: &str) -> Result<ScreenshotMode, String> {
    match mode {
        "direct" => Ok(ScreenshotMode::Direct),
        "hide-window" => Ok(ScreenshotMode::HideWindow),
        _ => Err("Unknown screenshot mode.".to_owned()),
    }
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn png_bytes(bytes: Vec<u8>) -> Result<Vec<u8>, String> {
    if bytes.len() < PNG_MAGIC.len() || bytes[..PNG_MAGIC.len()] != PNG_MAGIC {
        return Err("The screenshot tool did not return a valid PNG image.".to_owned());
    }
    if bytes.len() as u64 > MAXIMUM_SCREENSHOT_BYTES {
        return Err("The captured screenshot exceeds the 20 MiB attachment limit.".to_owned());
    }
    Ok(bytes)
}

#[cfg(target_os = "macos")]
#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGPreflightScreenCaptureAccess() -> bool;
    fn CGRequestScreenCaptureAccess() -> bool;
}

#[cfg(target_os = "macos")]
fn screen_capture_authorized() -> bool {
    unsafe { CGPreflightScreenCaptureAccess() }
}

#[cfg(target_os = "macos")]
fn request_screen_capture_access(window: &tauri::WebviewWindow) -> Result<bool, String> {
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    window
        .app_handle()
        .run_on_main_thread(move || {
            let granted = unsafe { CGRequestScreenCaptureAccess() };
            let _ = sender.send(granted);
        })
        .map_err(|error| format!("Could not request screen recording permission: {error}"))?;
    receiver
        .recv()
        .map_err(|_| "Screen recording permission request stopped.".to_owned())
}

/// Returns the permission token when Screen Recording is still not allowed.
/// The request registers this app in the pane. macOS shows the prompt at most once.
#[cfg(target_os = "macos")]
fn require_screen_capture_access(window: &tauri::WebviewWindow) -> Result<(), String> {
    if screen_capture_authorized() {
        return Ok(());
    }
    let _ = request_screen_capture_access(window);
    if screen_capture_authorized() {
        return Ok(());
    }
    Err(SCREEN_CAPTURE_PERMISSION_REQUIRED.to_owned())
}

#[cfg(target_os = "macos")]
fn open_screen_capture_pane() -> Result<(), String> {
    let status = std::process::Command::new("/usr/bin/open")
        .arg(SCREEN_CAPTURE_SETTINGS_URL)
        .status()
        .map_err(|error| format!("Could not open Screen Recording settings: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("Could not open Screen Recording settings.".to_owned())
    }
}

#[cfg(target_os = "macos")]
fn capture_macos_screenshot() -> Result<Option<Vec<u8>>, String> {
    let destination = std::env::temp_dir().join(format!(
        "a3s-screenshot-{}.png",
        uuid::Uuid::now_v7().simple()
    ));
    let output = std::process::Command::new("/usr/sbin/screencapture")
        .args(["-i", "-x", "-t", "png"])
        .arg(&destination)
        .output()
        .map_err(|error| format!("Could not start the macOS screenshot tool: {error}"))?;

    if !destination.is_file() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        return capture_stop_result(stop_without_file(screen_capture_authorized(), &stderr));
    }

    let result = (|| {
        let metadata = fs::metadata(&destination)
            .map_err(|error| format!("Could not inspect the captured screenshot: {error}"))?;
        if metadata.len() == 0 {
            return capture_stop_result(stop_without_file(screen_capture_authorized(), ""));
        }
        if metadata.len() > MAXIMUM_SCREENSHOT_BYTES {
            return Err("The captured screenshot exceeds the 20 MiB attachment limit.".to_owned());
        }
        let bytes = fs::read(&destination)
            .map_err(|error| format!("Could not read the captured screenshot: {error}"))?;
        png_bytes(bytes).map(Some)
    })();
    let _ = fs::remove_file(destination);
    result
}

#[tauri::command]
pub async fn capture_interactive_screenshot(
    window: tauri::WebviewWindow,
    mode: String,
) -> Result<tauri::ipc::Response, String> {
    let mode = parse_mode(&mode)?;
    #[cfg(target_os = "macos")]
    {
        let access_window = window.clone();
        tauri::async_runtime::spawn_blocking(move || require_screen_capture_access(&access_window))
            .await
            .map_err(|error| format!("The screenshot worker stopped unexpectedly: {error}"))??;
        let hide_window = matches!(mode, ScreenshotMode::HideWindow);
        if hide_window {
            window.hide().map_err(|error| {
                format!("Could not hide the window before the screenshot: {error}")
            })?;
        }
        let capture = tauri::async_runtime::spawn_blocking(move || {
            if hide_window {
                thread::sleep(Duration::from_millis(180));
            }
            capture_macos_screenshot()
        })
        .await
        .map_err(|error| format!("The screenshot worker stopped unexpectedly: {error}"));
        if hide_window {
            window
                .show()
                .and_then(|_| window.set_focus())
                .map_err(|error| {
                    format!("Could not restore the window after the screenshot: {error}")
                })?;
        }
        let bytes = capture??.unwrap_or_default();
        return Ok(tauri::ipc::Response::new(bytes));
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = (window, mode);
        Err(crate::host_family::CAPABILITY_UNAVAILABLE.to_owned())
    }
}

#[tauri::command]
pub fn open_screen_capture_settings() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        return open_screen_capture_pane();
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err(crate::host_family::CAPABILITY_UNAVAILABLE.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CaptureStop, MAXIMUM_SCREENSHOT_BYTES, SCREEN_CAPTURE_PERMISSION_REQUIRED,
        SCREEN_CAPTURE_SETTINGS_URL, ScreenshotMode, capture_stop_result, parse_mode, png_bytes,
        stop_without_file,
    };

    #[test]
    fn modes_are_the_composer_pair() {
        assert_eq!(parse_mode("direct").unwrap(), ScreenshotMode::Direct);
        assert_eq!(
            parse_mode("hide-window").unwrap(),
            ScreenshotMode::HideWindow
        );
        assert!(parse_mode("clipboard").is_err());
    }

    #[test]
    fn a_png_header_is_required() {
        let mut bytes = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 1, 2, 3];
        assert_eq!(png_bytes(bytes.clone()).unwrap(), bytes);
        bytes[0] = 0;
        assert!(png_bytes(bytes).is_err());
        assert!(png_bytes(vec![1, 2, 3]).is_err());
    }

    #[test]
    fn a_missing_permission_is_not_a_cancel() {
        assert_eq!(
            stop_without_file(false, ""),
            CaptureStop::PermissionRequired
        );
        assert_eq!(
            stop_without_file(false, "could not create image from display"),
            CaptureStop::PermissionRequired
        );
        assert_eq!(
            capture_stop_result(CaptureStop::PermissionRequired).unwrap_err(),
            SCREEN_CAPTURE_PERMISSION_REQUIRED
        );
    }

    #[test]
    fn an_empty_result_with_permission_is_a_cancel() {
        assert_eq!(stop_without_file(true, ""), CaptureStop::Cancelled);
        assert_eq!(
            stop_without_file(true, "could not create image from display"),
            CaptureStop::Failed(
                "The macOS screenshot tool failed: could not create image from display".to_owned()
            )
        );
    }

    #[test]
    fn the_settings_url_is_the_screen_recording_pane() {
        assert!(SCREEN_CAPTURE_SETTINGS_URL.starts_with("x-apple.systempreferences:"));
        assert!(SCREEN_CAPTURE_SETTINGS_URL.contains("Privacy_ScreenCapture"));
        assert!(!SCREEN_CAPTURE_SETTINGS_URL.starts_with("http"));
    }

    #[test]
    fn an_oversized_png_is_rejected() {
        let mut bytes = vec![0u8; MAXIMUM_SCREENSHOT_BYTES as usize + 1];
        bytes[..8].copy_from_slice(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
        assert!(png_bytes(bytes).is_err());
    }
}
