//! Independent editor window for one workspace file.
//!
//! The frame matches a workbench applet window: native overlay chrome, hidden
//! system traffic lights, and a page that paints close / minimize / zoom.

use std::path::{Component, Path, PathBuf};

use sha2::{Digest, Sha256};
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

use crate::applet_window::AppletWindowHost;

const FILE_EDITOR_WINDOW_WIDTH: f64 = 1280.0;
const FILE_EDITOR_WINDOW_HEIGHT: f64 = 860.0;
const CLOUD_DRIVE_SCHEME: &str = "a3s-drive://";

pub fn is_file_editor_label(label: &str) -> bool {
    let Some(rest) = label.strip_prefix("file-editor-") else {
        return false;
    };
    !rest.is_empty()
        && rest
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

fn lexical_absolute(raw: &str) -> Result<PathBuf, String> {
    if raw.is_empty() || raw.chars().any(char::is_control) {
        return Err("File editor path is invalid.".to_owned());
    }
    let path = Path::new(raw);
    if !path.is_absolute() {
        return Err("File editor path is invalid.".to_owned());
    }
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => out.push(prefix.as_os_str()),
            Component::RootDir => out.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => return Err("File editor path is invalid.".to_owned()),
            Component::Normal(part) => out.push(part),
        }
    }
    Ok(out)
}

fn workspace_file(root: &str, path: &str) -> Result<(PathBuf, PathBuf), String> {
    let root = lexical_absolute(root)?;
    let path = lexical_absolute(path)?;
    if !path.starts_with(&root) || path == root {
        return Err("File editor path is outside the workspace.".to_owned());
    }
    Ok((root, path))
}

/// A team drive file stays under `a3s-drive://{orgId}`. The org id is the same
/// shape the drive proxy accepts. `..` and a sibling org id stay outside.
fn cloud_editor_target(root: &str, path: &str) -> Result<(String, String), String> {
    if !root.starts_with(CLOUD_DRIVE_SCHEME) || !path.starts_with(CLOUD_DRIVE_SCHEME) {
        return Err("File editor path is outside the workspace.".to_owned());
    }
    let org_id = &root[CLOUD_DRIVE_SCHEME.len()..];
    if !crate::org_login::org_id_ok(org_id) {
        return Err("File editor path is invalid.".to_owned());
    }
    let prefix = format!("{root}/");
    if !path.starts_with(&prefix) {
        return Err("File editor path is outside the workspace.".to_owned());
    }
    let rest = &path[prefix.len()..];
    if rest.is_empty()
        || rest.split('/').any(|segment| {
            segment.is_empty()
                || segment == "."
                || segment == ".."
                || segment.contains('\\')
                || segment.chars().any(char::is_control)
        })
    {
        return Err("File editor path is invalid.".to_owned());
    }
    Ok((root.to_owned(), path.to_owned()))
}

fn editor_target(root: &str, path: &str) -> Result<(String, String), String> {
    let root = root.trim();
    let path = path.trim();
    if root.starts_with(CLOUD_DRIVE_SCHEME) || path.starts_with(CLOUD_DRIVE_SCHEME) {
        return cloud_editor_target(root, path);
    }
    let (root, path) = workspace_file(root, path)?;
    Ok((
        root.to_string_lossy().into_owned(),
        path.to_string_lossy().into_owned(),
    ))
}

pub fn file_editor_window_label(path: &str) -> String {
    let digest = Sha256::digest(path.as_bytes());
    let hex: String = digest
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect();
    format!("file-editor-{hex}")
}

fn file_id_ok(raw: &str) -> bool {
    let bytes = raw.as_bytes();
    bytes.len() == 36
        && bytes.iter().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                *byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
}

pub fn file_editor_page_url(
    root: &str,
    path: &str,
    read_only: bool,
    file_id: Option<&str>,
) -> Result<String, String> {
    let (root, path) = editor_target(root, path)?;
    if let Some(file_id) = file_id {
        if !file_id_ok(file_id) {
            return Err("File editor path is invalid.".to_owned());
        }
    }
    let mut url = crate::main_renderer_url(crate::packaged_shell())
        .parse::<url::Url>()
        .map_err(|_| "File editor window URL is invalid.".to_owned())?;
    {
        let mut query = url.query_pairs_mut();
        query
            .append_pair("file-editor", "1")
            .append_pair("root", &root)
            .append_pair("path", &path);
        if read_only {
            query.append_pair("readonly", "1");
        }
        if let Some(file_id) = file_id {
            query.append_pair("file", file_id);
        }
    }
    Ok(url.to_string())
}

fn preview_key(raw: &str) -> Result<String, String> {
    let key = raw.trim();
    if key.is_empty()
        || key.len() > 120
        || key == "."
        || key == ".."
        || key
            .chars()
            .any(|character| character.is_control() || matches!(character, '/' | '\\'))
    {
        return Err("Message file preview key is invalid.".to_owned());
    }
    Ok(key.to_owned())
}

/// A private directory for one chat attachment. The file editor reads it back
/// through the workspace file API, and the page opens that copy read-only.
pub fn message_file_preview_slot(key: &str) -> Result<(String, String), String> {
    let key = preview_key(key)?;
    let root =
        std::env::temp_dir().join(format!("a3s-message-file-preview-{}", std::process::id()));
    let directory = root.join(&key);
    std::fs::create_dir_all(&directory)
        .map_err(|_| "Could not prepare the file preview.".to_owned())?;
    Ok((
        root.to_string_lossy().into_owned(),
        directory.to_string_lossy().into_owned(),
    ))
}

fn file_name(path: &str) -> String {
    path.rsplit(['/', '\\'])
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or("A3S")
        .to_owned()
}

fn normalize_title(title: &str, fallback: &str) -> String {
    let cleaned: String = title
        .chars()
        .filter(|character| !character.is_control())
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() || trimmed.chars().count() > 120 {
        return fallback.to_owned();
    }
    trimmed.to_owned()
}

fn data_store_id(path: &str) -> [u8; 16] {
    let digest = Sha256::digest(path.as_bytes());
    let mut id = [0u8; 16];
    id.copy_from_slice(&digest[..16]);
    id
}

#[tauri::command]
pub fn prepare_message_file_preview(key: String) -> Result<serde_json::Value, String> {
    let (root, directory) = message_file_preview_slot(&key)?;
    Ok(serde_json::json!({ "root": root, "directory": directory }))
}

#[tauri::command]
pub fn open_file_editor_window(
    app: tauri::AppHandle,
    root: String,
    path: String,
    title: String,
    read_only: Option<bool>,
    file_id: Option<String>,
) -> Result<(), String> {
    let (root_path, file_path) = editor_target(&root, &path)?;
    let title = normalize_title(&title, &file_name(&file_path));
    let label = file_editor_window_label(&file_path);
    let page = file_editor_page_url(
        &root_path,
        &file_path,
        read_only.unwrap_or(false),
        file_id.as_deref(),
    )?;
    let url = page
        .parse::<url::Url>()
        .map_err(|_| "File editor window URL is invalid.".to_owned())?;

    if let Some(existing) = app.get_webview_window(&label) {
        // Same path already has a window. Navigating reloads the page and drops the editor.
        let _ = existing.set_title(&title);
        let _ = existing.unminimize();
        crate::applet_window::reveal(&existing);
        return Ok(());
    }

    let host = app
        .try_state::<AppletWindowHost>()
        .ok_or_else(|| "File editor window host is missing.".to_owned())?;
    let (slug, product, runtime) = host.boot_context();
    drop(host);
    let mut script =
        crate::boot_initialization_script(true, false, &slug, &product, &runtime, false);
    script.push_str("document.documentElement.dataset.a3sFileEditorPopout='true';");

    let builder = crate::tenant::isolate_webview(
        WebviewWindowBuilder::new(&app, &label, WebviewUrl::External(url))
            .initialization_script(script)
            .data_store_identifier(data_store_id(&file_path)),
        data_store_id(&file_path),
    )
    .map_err(|error| error.to_string())?
    .title(title)
    .inner_size(FILE_EDITOR_WINDOW_WIDTH, FILE_EDITOR_WINDOW_HEIGHT)
    .min_inner_size(720.0, 520.0)
    .resizable(true)
    .maximizable(true)
    .minimizable(true)
    .closable(true)
    .visible(false)
    .focused(false)
    .center()
    .background_color(tauri::webview::Color(255, 255, 255, 255))
    .on_page_load(|window, payload| {
        if payload.event() != tauri::webview::PageLoadEvent::Finished {
            return;
        }
        if !is_file_editor_label(window.label()) {
            return;
        }
        crate::applet_window::reveal(&window);
    });
    #[cfg(target_os = "macos")]
    let builder = builder
        .hidden_title(true)
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .accept_first_mouse(true);
    let window = builder
        .build()
        .map_err(|error| format!("Could not open the file editor window: {error}"))?;
    crate::applet_window::reveal(&window);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        file_editor_page_url, file_editor_window_label, is_file_editor_label,
        message_file_preview_slot, workspace_file,
    };

    #[test]
    fn page_url_stays_inside_the_workspace() {
        let url = file_editor_page_url(
            "/tmp/a3s-workspace",
            "/tmp/a3s-workspace/项目资料/纪要.txt",
            false,
            None,
        )
        .unwrap();
        assert!(url.starts_with("http://127.0.0.1:4318/windows/main/index.html?"));
        assert!(url.contains("file-editor=1"));
        assert!(!url.contains("readonly=1"));
        assert!(url.contains("root=%2Ftmp%2Fa3s-workspace"));
        assert!(url.contains("%E7%BA%AA%E8%A6%81.txt"));
        let locked = file_editor_page_url(
            "/tmp/a3s-workspace",
            "/tmp/a3s-workspace/项目资料/纪要.txt",
            true,
            None,
        )
        .unwrap();
        assert!(locked.contains("readonly=1"));
        assert!(workspace_file("/tmp/a3s-workspace", "/tmp/a3s-workspace/../etc/passwd").is_err());
        assert!(workspace_file("/tmp/a3s-workspace", "/tmp/a3s-workspace-other/note.txt").is_err());
        assert!(workspace_file("/tmp/a3s-workspace", "note.txt").is_err());
        assert!(workspace_file("/tmp/a3s-workspace", "/tmp/a3s-workspace").is_err());
        let cloud = file_editor_page_url(
            "a3s-drive://org-1",
            "a3s-drive://org-1/团队文档/满江红.docx",
            false,
            Some("bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb"),
        )
        .unwrap();
        assert!(cloud.contains("file-editor=1"));
        assert!(cloud.contains("root=a3s-drive%3A%2F%2Forg-1"));
        assert!(cloud.contains("%E6%BB%A1%E6%B1%9F%E7%BA%A2.docx"));
        assert!(cloud.contains("file=bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb"));
        assert!(
            file_editor_page_url(
                "a3s-drive://org-1",
                "a3s-drive://org-1/../org-2/note.docx",
                false,
                None,
            )
            .is_err()
        );
        assert!(
            file_editor_page_url(
                "a3s-drive://org-1",
                "a3s-drive://org-12/note.docx",
                false,
                None
            )
            .is_err()
        );
        assert!(
            file_editor_page_url("a3s-drive://org-1", "a3s-drive://org-1", false, None).is_err()
        );
        assert!(
            file_editor_page_url(
                "/tmp/a3s-workspace",
                "a3s-drive://org-1/note.docx",
                false,
                None
            )
            .is_err()
        );
        assert!(
            file_editor_page_url(
                "a3s-drive://org-1",
                "a3s-drive://org-1/note.docx",
                false,
                Some("not-a-file-id"),
            )
            .is_err()
        );
    }

    #[test]
    fn message_preview_slot_stays_in_one_directory() {
        let (root, directory) = message_file_preview_slot("m-direct-3").unwrap();
        assert!(directory.starts_with(&root));
        assert!(directory.ends_with("m-direct-3"));
        assert!(std::path::Path::new(&directory).is_dir());
        assert!(message_file_preview_slot("../etc").is_err());
        assert!(message_file_preview_slot("a/b").is_err());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn labels_match_the_file_editor_capability() {
        let label = file_editor_window_label("/tmp/a3s-workspace/纪要.txt");
        assert!(is_file_editor_label(&label));
        assert!(label.starts_with("file-editor-"));
        assert!(!is_file_editor_label("file-editor-"));
        assert!(!is_file_editor_label("applet-leave"));
        assert!(!is_file_editor_label("file-editor-note/extra"));
    }
}
