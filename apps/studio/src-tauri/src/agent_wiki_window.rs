//! Native window for one installed agent's encyclopedia.
//!
//! The frame matches a workbench applet window. Pages live in the default
//! workspace at `wiki/agents/{id}`.

use sha2::{Digest, Sha256};
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

use crate::applet_window::AppletWindowHost;

const WINDOW_WIDTH: f64 = 1280.0;
const WINDOW_HEIGHT: f64 = 860.0;

pub fn agent_wiki_folder_ok(id: &str) -> bool {
    let mut chars = id.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_alphanumeric() {
        return false;
    }
    let mut count = 1usize;
    for character in chars {
        count += 1;
        if count > 64
            || !(character.is_ascii_alphanumeric() || character == '-' || character == '_')
        {
            return false;
        }
    }
    true
}

pub fn is_agent_wiki_label(label: &str) -> bool {
    let Some(rest) = label.strip_prefix("agent-wiki-") else {
        return false;
    };
    rest.len() == 16 && rest.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn window_label(agent_id: &str) -> String {
    let digest = Sha256::digest(agent_id.as_bytes());
    let hex: String = digest
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect();
    format!("agent-wiki-{hex}")
}

fn page_url(agent_id: &str, title: &str) -> Result<String, String> {
    if !agent_wiki_folder_ok(agent_id) {
        return Err("Agent encyclopedia id is invalid.".to_owned());
    }
    let mut url = crate::main_renderer_url(crate::packaged_shell())
        .parse::<url::Url>()
        .map_err(|_| "Agent encyclopedia window URL is invalid.".to_owned())?;
    url.query_pairs_mut()
        .append_pair("agent-wiki", "1")
        .append_pair("agent", agent_id)
        .append_pair("title", title);
    Ok(url.to_string())
}

fn normalize_title(title: &str, agent_id: &str) -> String {
    let cleaned: String = title
        .chars()
        .filter(|character| !character.is_control())
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() || trimmed.chars().count() > 120 {
        return agent_id.to_owned();
    }
    trimmed.to_owned()
}

fn data_store_id(agent_id: &str) -> [u8; 16] {
    let digest = Sha256::digest(format!("agent-wiki-{agent_id}").as_bytes());
    let mut id = [0u8; 16];
    id.copy_from_slice(&digest[..16]);
    id
}

#[tauri::command]
pub fn open_agent_wiki_window(
    app: tauri::AppHandle,
    agent_id: String,
    title: String,
) -> Result<(), String> {
    let agent_id = agent_id.trim();
    if !agent_wiki_folder_ok(agent_id) {
        return Err("Agent encyclopedia id is invalid.".to_owned());
    }
    let title = normalize_title(&title, agent_id);
    let label = window_label(agent_id);
    let page = page_url(agent_id, &title)?;
    let url = page
        .parse::<url::Url>()
        .map_err(|_| "Agent encyclopedia window URL is invalid.".to_owned())?;

    if let Some(existing) = app.get_webview_window(&label) {
        let _ = existing.set_title(&title);
        let _ = existing.navigate(url);
        let _ = existing.unminimize();
        crate::applet_window::reveal(&existing);
        return Ok(());
    }

    let host = app
        .try_state::<AppletWindowHost>()
        .ok_or_else(|| "Agent encyclopedia window host is missing.".to_owned())?;
    let (slug, product, runtime) = host.boot_context();
    drop(host);
    let mut script =
        crate::boot_initialization_script(true, false, &slug, &product, &runtime, false);
    script.push_str("document.documentElement.dataset.a3sAgentWikiPopout='true';");

    let builder = crate::tenant::isolate_webview(
        WebviewWindowBuilder::new(&app, &label, WebviewUrl::External(url))
            .initialization_script(script)
            .data_store_identifier(data_store_id(agent_id)),
        data_store_id(agent_id),
    )
    .map_err(|error| error.to_string())?
    .title(title)
    .inner_size(WINDOW_WIDTH, WINDOW_HEIGHT)
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
        if !is_agent_wiki_label(window.label()) {
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
        .map_err(|error| format!("Could not open the agent encyclopedia window: {error}"))?;
    crate::applet_window::reveal(&window);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{agent_wiki_folder_ok, is_agent_wiki_label, page_url};

    #[test]
    fn agent_wiki_ids_stay_inside_one_folder() {
        assert!(agent_wiki_folder_ok("legal-expert"));
        assert!(!agent_wiki_folder_ok("../wiki"));
        assert!(!agent_wiki_folder_ok("a/b"));
        assert!(!agent_wiki_folder_ok(""));
        let url = page_url("legal-expert", "法务专家的百科").expect("url");
        assert!(url.contains("agent-wiki=1"));
        assert!(url.contains("agent=legal-expert"));
        assert!(is_agent_wiki_label("agent-wiki-0123456789abcdef"));
        assert!(!is_agent_wiki_label("agent-wiki-short"));
    }
}
