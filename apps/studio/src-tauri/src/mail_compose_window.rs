//! One reused compose window. It shares the shell session, so send uses the member token.
//!
//! The frame matches a file editor: overlay chrome, hidden traffic lights, and a page bar.

use sha2::{Digest, Sha256};
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

use crate::applet_window::AppletWindowHost;

const LABEL: &str = "mail-compose";
const WINDOW_WIDTH: f64 = 960.0;
const WINDOW_HEIGHT: f64 = 720.0;
const DRAFT_LIMIT: usize = 3_000_000;

pub fn is_mail_compose_label(label: &str) -> bool {
    label == LABEL
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ComposeSeed {
    kind: String,
    to: Vec<String>,
    cc: Vec<String>,
    subject: String,
    body: String,
    #[serde(default)]
    thread_id: String,
    #[serde(default)]
    draft_id: String,
    focus: String,
    from: String,
    title: String,
    /// Team center. Empty for a personal mailbox. The page uses it to list teammates.
    #[serde(default)]
    org_id: String,
    #[serde(default)]
    attachments: Vec<ComposeFile>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct ComposeFile {
    name: String,
    mime: String,
    data: String,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct CleanSeed {
    kind: String,
    to: Vec<String>,
    cc: Vec<String>,
    subject: String,
    body: String,
    thread_id: String,
    draft_id: String,
    focus: String,
    from: String,
    title: String,
    org_id: String,
    attachments: Vec<ComposeFile>,
}

fn clip_text(value: &str, max_chars: usize) -> Result<String, String> {
    let cleaned: String = value
        .chars()
        .filter(|character| !character.is_control())
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.chars().count() > max_chars {
        return Err("Mail draft is too large.".to_owned());
    }
    Ok(trimmed.to_owned())
}

fn addresses(values: &[String]) -> Result<Vec<String>, String> {
    if values.len() > 20 {
        return Err("Mail draft is too large.".to_owned());
    }
    values.iter().map(|value| clip_text(value, 80)).collect()
}

fn files(values: &[ComposeFile]) -> Result<Vec<ComposeFile>, String> {
    if values.len() > 8 {
        return Err("Mail draft is too large.".to_owned());
    }
    let mut out = Vec::with_capacity(values.len());
    for file in values {
        let name = clip_text(&file.name, 120)?;
        if name.is_empty() || name.contains('/') || name.contains('\\') {
            return Err("Mail draft is invalid.".to_owned());
        }
        let mime = clip_text(&file.mime, 120)?;
        let data = file.data.trim();
        if data.len() > 700_000 || data.chars().any(char::is_control) {
            return Err("Mail draft is too large.".to_owned());
        }
        out.push(ComposeFile {
            name,
            mime,
            data: data.to_owned(),
        });
    }
    Ok(out)
}

fn clean_seed(raw: &str) -> Result<CleanSeed, String> {
    if raw.len() > DRAFT_LIMIT {
        return Err("Mail draft is too large.".to_owned());
    }
    let seed: ComposeSeed =
        serde_json::from_str(raw).map_err(|_| "Mail draft is invalid.".to_owned())?;
    let kind = match seed.kind.as_str() {
        "new" | "reply" | "draft" => seed.kind,
        _ => return Err("Mail draft is invalid.".to_owned()),
    };
    let focus = match seed.focus.as_str() {
        "to" | "body" => seed.focus,
        _ => return Err("Mail draft is invalid.".to_owned()),
    };
    Ok(CleanSeed {
        kind,
        to: addresses(&seed.to)?,
        cc: addresses(&seed.cc)?,
        subject: clip_text(&seed.subject, 200)?,
        body: clip_text(&seed.body, 100_000)?,
        thread_id: clip_text(&seed.thread_id, 80)?,
        draft_id: clip_text(&seed.draft_id, 80)?,
        focus,
        from: clip_text(&seed.from, 80)?,
        org_id: clip_text(&seed.org_id, 80)?,
        title: {
            let title = clip_text(&seed.title, 120)?;
            if title.is_empty() {
                "Mail".to_owned()
            } else {
                title
            }
        },
        attachments: files(&seed.attachments)?,
    })
}

fn json_literal(seed: &CleanSeed) -> Result<String, String> {
    let json = serde_json::to_string(seed).map_err(|_| "Mail draft is invalid.".to_owned())?;
    serde_json::to_string(&json).map_err(|_| "Mail draft is invalid.".to_owned())
}

fn assign_script(seed: &CleanSeed) -> Result<String, String> {
    let literal = json_literal(seed)?;
    Ok(format!(
        "window.__A3S_MAIL_COMPOSE__=JSON.parse({literal});"
    ))
}

fn event_script(seed: &CleanSeed) -> Result<String, String> {
    let literal = json_literal(seed)?;
    Ok(format!(
        "window.dispatchEvent(new CustomEvent(\"a3s:mail-compose\",{{detail:JSON.parse({literal})}}));"
    ))
}

pub fn mail_compose_page_url() -> Result<String, String> {
    let mut url = crate::main_renderer_url(crate::packaged_shell())
        .parse::<url::Url>()
        .map_err(|_| "Mail window URL is invalid.".to_owned())?;
    url.query_pairs_mut().append_pair("mail-compose", "1");
    Ok(url.to_string())
}

fn data_store_id() -> [u8; 16] {
    let digest = Sha256::digest(b"mail-compose");
    let mut id = [0u8; 16];
    id.copy_from_slice(&digest[..16]);
    id
}

#[tauri::command]
pub fn open_mail_compose_window(app: tauri::AppHandle, draft: String) -> Result<(), String> {
    let seed = clean_seed(&draft)?;
    let title = seed.title.clone();
    if let Some(existing) = app.get_webview_window(LABEL) {
        let script = event_script(&seed)?;
        existing
            .eval(script)
            .map_err(|_| "Could not update the mail window.".to_owned())?;
        let _ = existing.set_title(&title);
        let _ = existing.unminimize();
        crate::applet_window::reveal(&existing);
        return Ok(());
    }

    let page = mail_compose_page_url()?;
    let url = page
        .parse::<url::Url>()
        .map_err(|_| "Mail window URL is invalid.".to_owned())?;
    let host = app
        .try_state::<AppletWindowHost>()
        .ok_or_else(|| "Mail window host is missing.".to_owned())?;
    let (slug, product, runtime) = host.boot_context();
    drop(host);
    let mut script =
        crate::boot_initialization_script(true, false, &slug, &product, &runtime, false);
    script.push_str(&assign_script(&seed)?);
    script.push_str("document.documentElement.dataset.a3sMailCompose='true';");

    let builder = crate::tenant::isolate_webview(
        WebviewWindowBuilder::new(&app, LABEL, WebviewUrl::External(url))
            .initialization_script(script)
            .data_store_identifier(data_store_id()),
        data_store_id(),
    )
    .map_err(|error| error.to_string())?
    .title(title)
    .inner_size(WINDOW_WIDTH, WINDOW_HEIGHT)
    .min_inner_size(720.0, 560.0)
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
        if !is_mail_compose_label(window.label()) {
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
        .map_err(|error| format!("Could not open the mail window: {error}"))?;
    crate::applet_window::reveal(&window);
    Ok(())
}

#[tauri::command]
pub fn publish_mail_compose_result(
    window: tauri::WebviewWindow,
    folder: String,
) -> Result<(), String> {
    if !is_mail_compose_label(window.label()) {
        return Err("This window cannot publish mail.".to_owned());
    }
    if folder != "sent" && folder != "drafts" {
        return Err("Mail folder is invalid.".to_owned());
    }
    let detail = serde_json::json!({ "folder": folder });
    let script =
        format!("window.dispatchEvent(new CustomEvent(\"a3s:mail-changed\",{{detail:{detail}}}))");
    let main = window
        .app_handle()
        .get_webview_window("main")
        .ok_or_else(|| "Main window is not open.".to_owned())?;
    main.eval(script)
        .map_err(|_| "Could not refresh mail.".to_owned())
}

#[cfg(test)]
mod tests {
    use super::{
        assign_script, clean_seed, event_script, is_mail_compose_label, mail_compose_page_url,
    };

    #[test]
    fn page_url_stays_on_the_renderer_without_the_letter() {
        let url = mail_compose_page_url().unwrap();
        assert_eq!(
            url,
            "http://127.0.0.1:4318/windows/main/index.html?mail-compose=1"
        );
        assert!(is_mail_compose_label("mail-compose"));
        assert!(!is_mail_compose_label("mail-compose-2"));
        assert!(!is_mail_compose_label("applet-inbox"));
    }

    #[test]
    fn the_init_script_reparses_json_and_does_not_run_the_letter() {
        let raw = r#"{"kind":"reply","to":["ada@mail.towork"],"cc":[],"subject":"Re: hi","body":"</script>\");alert(1);//","focus":"body","from":"roy@mail.towork","title":"回复","threadId":"thread-1","draftId":"","orgId":"11111111-1111-4111-8111-111111111111","attachments":[]}"#;
        let seed = clean_seed(raw).unwrap();
        let script = assign_script(&seed).unwrap();
        assert!(script.starts_with("window.__A3S_MAIL_COMPOSE__=JSON.parse("));
        assert!(script.ends_with(");"));
        let literal = script
            .trim_start_matches("window.__A3S_MAIL_COMPOSE__=JSON.parse(")
            .trim_end_matches(");");
        let encoded: String = serde_json::from_str(literal).unwrap();
        let value: serde_json::Value = serde_json::from_str(&encoded).unwrap();
        assert_eq!(value["body"], "</script>\");alert(1);//");
        assert_eq!(value["kind"], "reply");
        assert_eq!(value["threadId"], "thread-1");
        assert_eq!(value["orgId"], "11111111-1111-4111-8111-111111111111");
        let event = event_script(&seed).unwrap();
        assert!(event.contains("a3s:mail-compose"));
        assert!(event.contains("JSON.parse("));
        assert!(clean_seed(&format!("{{}}")).is_err());
        assert!(clean_seed(&"x".repeat(3_000_001)).is_err());
    }
}
