//! Native directory picker for the workspace folder card.
//! Tests never open a panel.

pub struct DirectoryRequest {
    pub default_path: String,
    pub title: String,
}

pub fn directory_request_from_body(body: &[u8]) -> Result<DirectoryRequest, u16> {
    if body.is_empty() {
        return Ok(DirectoryRequest {
            default_path: String::new(),
            title: String::new(),
        });
    }
    let value: serde_json::Value = serde_json::from_slice(body).map_err(|_| 400u16)?;
    let text = |key: &str| {
        value
            .get(key)
            .and_then(|item| item.as_str())
            .unwrap_or("")
            .to_owned()
    };
    Ok(DirectoryRequest {
        default_path: text("defaultPath"),
        title: text("title"),
    })
}

/// Directory the panel should open. A missing folder uses the nearest existing parent.
pub fn panel_directory(default_path: &str) -> String {
    let trimmed = default_path.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let path = std::path::Path::new(trimmed);
    if path.is_dir() {
        return trimmed.to_owned();
    }
    let mut cursor = path.parent();
    while let Some(dir) = cursor {
        if dir.as_os_str().is_empty() {
            break;
        }
        if dir.is_dir() {
            return dir.to_string_lossy().into_owned();
        }
        cursor = dir.parent();
    }
    String::new()
}

pub enum DirectoryChoice {
    Selected(String),
    Cancelled,
    Unavailable,
}

pub fn directory_response(choice: DirectoryChoice) -> String {
    match choice {
        DirectoryChoice::Selected(path) => serde_json::json!({ "path": path }).to_string(),
        DirectoryChoice::Cancelled => {
            serde_json::json!({ "path": serde_json::Value::Null }).to_string()
        }
        DirectoryChoice::Unavailable => serde_json::json!({
            "path": serde_json::Value::Null,
            "reason": crate::host_family::CAPABILITY_UNAVAILABLE
        })
        .to_string(),
    }
}

pub fn choose_directory(default_path: &str, title: &str) -> DirectoryChoice {
    #[cfg(test)]
    {
        let _ = (default_path, title);
        DirectoryChoice::Cancelled
    }
    #[cfg(not(test))]
    {
        choose_directory_native(default_path, title)
    }
}

#[cfg(all(not(test), target_os = "macos"))]
fn choose_directory_native(default_path: &str, title: &str) -> DirectoryChoice {
    match choose_directory_macos(default_path, title) {
        Some(path) => DirectoryChoice::Selected(path),
        None => DirectoryChoice::Cancelled,
    }
}

#[cfg(all(not(test), target_os = "windows"))]
fn choose_directory_native(default_path: &str, title: &str) -> DirectoryChoice {
    const SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms
$dialog = New-Object System.Windows.Forms.FolderBrowserDialog
if ($env:A3S_FOLDER_TITLE) { $dialog.Description = $env:A3S_FOLDER_TITLE }
if ($env:A3S_FOLDER_DIR -and (Test-Path -LiteralPath $env:A3S_FOLDER_DIR)) {
  $dialog.SelectedPath = $env:A3S_FOLDER_DIR
}
$result = $dialog.ShowDialog()
if ($result -eq [System.Windows.Forms.DialogResult]::OK -and $dialog.SelectedPath) {
  [Console]::Out.Write($dialog.SelectedPath)
  exit 0
}
exit 1
"#;
    let output = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-STA", "-Command", SCRIPT])
        .env("A3S_FOLDER_TITLE", title)
        .env("A3S_FOLDER_DIR", default_path)
        .output();
    match output {
        Err(_) => DirectoryChoice::Unavailable,
        Ok(output) if output.status.success() => {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_owned();
            if path.is_empty() {
                DirectoryChoice::Cancelled
            } else {
                DirectoryChoice::Selected(path)
            }
        }
        Ok(output) if output.status.code() == Some(1) => DirectoryChoice::Cancelled,
        Ok(_) => DirectoryChoice::Unavailable,
    }
}

#[cfg(all(not(test), not(any(target_os = "macos", target_os = "windows"))))]
fn choose_directory_native(default_path: &str, title: &str) -> DirectoryChoice {
    let mut command = std::process::Command::new("zenity");
    command.args(["--file-selection", "--directory"]);
    if !title.is_empty() {
        command.arg("--title").arg(title);
    }
    let start = panel_directory(default_path);
    if !start.is_empty() {
        let mut filename = start;
        if !filename.ends_with('/') {
            filename.push('/');
        }
        command.arg(format!("--filename={filename}"));
    }
    match command.output() {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => DirectoryChoice::Unavailable,
        Err(_) => DirectoryChoice::Unavailable,
        Ok(output) if output.status.success() => {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_owned();
            if path.is_empty() {
                DirectoryChoice::Cancelled
            } else {
                DirectoryChoice::Selected(path)
            }
        }
        Ok(output) if output.status.code() == Some(1) => DirectoryChoice::Cancelled,
        Ok(_) => DirectoryChoice::Unavailable,
    }
}

#[cfg(all(not(test), target_os = "macos"))]
fn choose_directory_macos(default_path: &str, title: &str) -> Option<String> {
    use std::sync::mpsc;

    if objc2::MainThreadMarker::new().is_some() {
        return show_directory_panel(default_path, title);
    }

    let (sender, receiver) = mpsc::channel();
    let job = Box::new(DirectoryJob {
        default_path: default_path.to_owned(),
        title: title.to_owned(),
        sender,
    });
    unsafe {
        let queue = std::ptr::addr_of!(_dispatch_main_q).cast_mut().cast();
        dispatch_sync_f(
            queue,
            Box::into_raw(job).cast(),
            dispatch_show_directory_panel,
        );
    }
    receiver.recv().ok().flatten()
}

#[cfg(all(not(test), target_os = "macos"))]
struct DirectoryJob {
    default_path: String,
    title: String,
    sender: std::sync::mpsc::Sender<Option<String>>,
}

#[cfg(all(not(test), target_os = "macos"))]
unsafe extern "C" fn dispatch_show_directory_panel(context: *mut std::ffi::c_void) {
    if context.is_null() {
        return;
    }
    let job = unsafe { Box::from_raw(context.cast::<DirectoryJob>()) };
    let chosen = show_directory_panel(&job.default_path, &job.title);
    let _ = job.sender.send(chosen);
}

#[cfg(all(not(test), target_os = "macos"))]
fn show_directory_panel(default_path: &str, title: &str) -> Option<String> {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSApplication, NSModalResponseOK, NSOpenPanel};
    use objc2_foundation::{NSString, NSURL};

    let marker = MainThreadMarker::new()?;
    let app = NSApplication::sharedApplication(marker);
    app.activate();
    let panel = NSOpenPanel::openPanel(marker);
    panel.setCanChooseFiles(false);
    panel.setCanChooseDirectories(true);
    panel.setAllowsMultipleSelection(false);
    panel.setCanCreateDirectories(true);
    if !title.is_empty() {
        panel.setMessage(Some(&NSString::from_str(title)));
    }
    let start = panel_directory(default_path);
    if !start.is_empty() {
        let path = NSString::from_str(&start);
        panel.setDirectoryURL(Some(&NSURL::fileURLWithPath(&path)));
    }
    if panel.runModal() != NSModalResponseOK {
        return None;
    }
    let url = panel.URL()?;
    let path = url.path()?;
    let chosen = path.to_string();
    if chosen.is_empty() {
        None
    } else {
        Some(chosen)
    }
}

#[cfg(all(not(test), target_os = "macos"))]
#[link(name = "System")]
unsafe extern "C" {
    static _dispatch_main_q: u8;
    fn dispatch_sync_f(
        queue: *mut std::ffi::c_void,
        context: *mut std::ffi::c_void,
        work: unsafe extern "C" fn(*mut std::ffi::c_void),
    );
}

#[cfg(test)]
mod tests {
    use super::{directory_request_from_body, panel_directory};

    #[test]
    fn reads_the_default_directory_and_rejects_a_bad_body() {
        let empty = directory_request_from_body(b"").expect("empty");
        assert_eq!(empty.default_path, "");
        assert_eq!(empty.title, "");
        let raw = r#"{"defaultPath":"/Users/roylin/.a3s","title":"Workspace"}"#;
        let request = directory_request_from_body(raw.as_bytes()).expect("path");
        assert_eq!(request.default_path, "/Users/roylin/.a3s");
        assert_eq!(request.title, "Workspace");
        assert_eq!(directory_request_from_body(b"{").err(), Some(400));
    }

    #[test]
    fn a_missing_dialog_is_not_a_cancel() {
        use super::{DirectoryChoice, directory_response};
        let cancel = directory_response(DirectoryChoice::Cancelled);
        let missing = directory_response(DirectoryChoice::Unavailable);
        assert!(cancel.contains("\"path\":null"));
        assert!(!cancel.contains("reason"));
        assert!(missing.contains("desktop-capability-unavailable"));
        let selected = directory_response(DirectoryChoice::Selected("/work/notes".to_owned()));
        assert!(selected.contains("\"path\":\"/work/notes\""));
        assert!(!selected.contains("reason"));
    }

    #[test]
    fn opens_an_existing_parent_when_the_folder_is_missing() {
        let root = std::env::temp_dir();
        let missing = root.join("a3s-workspace-missing-not-created");
        let chosen = panel_directory(&missing.to_string_lossy());
        let chosen_path = std::path::Path::new(&chosen);
        assert!(chosen_path.is_dir());
        assert!(missing.starts_with(chosen_path));
        assert_eq!(panel_directory(""), "");
        assert_eq!(
            panel_directory(&root.to_string_lossy()),
            root.to_string_lossy()
        );
    }
}
