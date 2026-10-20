//! Independent window for one published About notice.
//!
//! The frame matches a workbench applet window. The page paints the controls.
//! The document is the public legal page. This window does not receive the shell token.

use sha2::{Digest, Sha256};
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

const NOTICE_SLUGS: &[&str] = &["terms", "privacy", "copyright", "support"];
const NOTICE_WINDOW_WIDTH: f64 = 880.0;
const NOTICE_WINDOW_HEIGHT: f64 = 720.0;
const LEGAL_PREFIX: &str = "/v1/api/legal/";

pub fn is_notice_slug(slug: &str) -> bool {
    NOTICE_SLUGS.contains(&slug)
}

pub fn is_notice_label(label: &str) -> bool {
    let Some(slug) = label.strip_prefix("notice-") else {
        return false;
    };
    is_notice_slug(slug)
}

pub fn notice_window_label(slug: &str) -> Result<String, String> {
    if !is_notice_slug(slug) {
        return Err("The notice address is invalid.".to_owned());
    }
    Ok(format!("notice-{slug}"))
}

fn org_origin_url() -> Result<url::Url, String> {
    crate::org_login::default_origin()
        .parse()
        .map_err(|_| "The notice address is invalid.".to_owned())
}

fn origin_text(value: &url::Url) -> Result<String, String> {
    let host = value
        .host_str()
        .ok_or_else(|| "The notice address is invalid.".to_owned())?;
    let mut text = format!("{}://{host}", value.scheme());
    if let Some(port) = value.port() {
        text.push(':');
        text.push_str(&port.to_string());
    }
    Ok(text)
}

/// The publication URL must be the org API legal page for this slug.
pub fn legal_page_url(org: &url::Url, slug: &str, raw: &str) -> Result<url::Url, String> {
    if !is_notice_slug(slug) {
        return Err("The notice address is invalid.".to_owned());
    }
    let parsed = raw
        .parse::<url::Url>()
        .map_err(|_| "The notice address is invalid.".to_owned())?;
    let https = parsed.scheme() == "https";
    let http = parsed.scheme() == "http";
    if !http && !https {
        return Err("The notice address is invalid.".to_owned());
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err("The notice address is invalid.".to_owned());
    }
    if parsed.query().is_some() || parsed.fragment().is_some() {
        return Err("The notice address is invalid.".to_owned());
    }
    if parsed.scheme() != org.scheme()
        || parsed.host_str() != org.host_str()
        || parsed.port_or_known_default() != org.port_or_known_default()
    {
        return Err("The notice address is invalid.".to_owned());
    }
    if parsed.path() != format!("{LEGAL_PREFIX}{slug}") {
        return Err("The notice address is invalid.".to_owned());
    }
    Ok(parsed)
}

pub fn notice_page_url(slug: &str) -> Result<String, String> {
    if !is_notice_slug(slug) {
        return Err("The notice address is invalid.".to_owned());
    }
    let mut url = crate::main_renderer_url(crate::packaged_shell())
        .parse::<url::Url>()
        .map_err(|_| "The notice address is invalid.".to_owned())?;
    url.query_pairs_mut().append_pair("notice", slug);
    Ok(url.to_string())
}

pub fn notice_initialization_script(origin: &str) -> String {
    let origin_js = serde_json::to_string(origin).unwrap_or_else(|_| "\"\"".to_owned());
    let host_platform = if cfg!(target_os = "macos") {
        "darwin"
    } else if cfg!(target_os = "windows") {
        "win32"
    } else {
        "linux"
    };
    let overlay = if cfg!(target_os = "macos") {
        "document.documentElement.dataset.overlayTitlebar='true';"
    } else {
        ""
    };
    let caps = crate::host_family::desktop_capabilities_assignment();
    format!(
        "document.documentElement.dataset.a3sPlatform='{host_platform}';\
         {overlay}{caps}\
         document.documentElement.dataset.a3sNoticeWindow='true';\
         document.documentElement.dataset.a3sNoticeOrigin={origin_js};"
    )
}

fn normalize_title(title: &str, slug: &str) -> String {
    let cleaned: String = title
        .chars()
        .filter(|character| !character.is_control())
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() || trimmed.chars().count() > 120 {
        return slug.to_owned();
    }
    trimmed.to_owned()
}

fn data_store_id(slug: &str) -> [u8; 16] {
    let digest = Sha256::digest(format!("notice-{slug}").as_bytes());
    let mut id = [0u8; 16];
    id.copy_from_slice(&digest[..16]);
    id
}

#[tauri::command]
pub fn open_notice_window(
    app: tauri::AppHandle,
    slug: String,
    title: String,
    url: String,
) -> Result<(), String> {
    let slug = slug.trim();
    let org = org_origin_url()?;
    let _legal = legal_page_url(&org, slug, url.trim())?;
    let origin = origin_text(&org)?;
    let title = normalize_title(&title, slug);
    let label = notice_window_label(slug)?;
    let page = notice_page_url(slug)?;
    let target = page
        .parse::<url::Url>()
        .map_err(|_| "The notice address is invalid.".to_owned())?;

    if let Some(existing) = app.get_webview_window(&label) {
        let _ = existing.set_title(&title);
        let _ = existing.navigate(target);
        let _ = existing.unminimize();
        crate::applet_window::reveal(&existing);
        return Ok(());
    }

    let builder = crate::tenant::isolate_webview(
        WebviewWindowBuilder::new(&app, &label, WebviewUrl::External(target))
            .initialization_script(notice_initialization_script(&origin))
            .data_store_identifier(data_store_id(slug)),
        data_store_id(slug),
    )
    .map_err(|error| error.to_string())?
    .title(title)
    .inner_size(NOTICE_WINDOW_WIDTH, NOTICE_WINDOW_HEIGHT)
    .min_inner_size(640.0, 480.0)
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
        if !is_notice_label(window.label()) {
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
        .map_err(|error| format!("Could not open the notice window: {error}"))?;
    crate::applet_window::reveal(&window);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        is_notice_label, legal_page_url, notice_initialization_script, notice_page_url,
        notice_window_label, org_origin_url,
    };

    fn org() -> url::Url {
        "http://127.0.0.1:3000".parse().unwrap()
    }

    #[test]
    fn page_url_stays_on_the_renderer_and_the_script_has_no_shell_token() {
        let url = notice_page_url("terms").unwrap();
        assert_eq!(
            url,
            "http://127.0.0.1:4318/windows/main/index.html?notice=terms"
        );
        assert!(notice_page_url("blog").is_err());
        assert!(notice_page_url("notice").is_err());
        let script = notice_initialization_script("http://127.0.0.1:3000");
        assert!(script.contains("dataset.overlayTitlebar='true'"));
        assert!(script.contains("dataset.a3sNoticeWindow='true'"));
        assert!(script.contains("dataset.a3sNoticeOrigin=\"http://127.0.0.1:3000\""));
        assert!(!script.contains("token"));
        assert!(!script.contains("__A3S_STUDIO_RUNTIME__"));
        assert!(!script.contains("__A3S_APPLET_RUNTIME__"));
    }

    #[test]
    fn legal_address_must_match_the_org_origin_and_slug() {
        let org = org();
        let url =
            legal_page_url(&org, "terms", "http://127.0.0.1:3000/v1/api/legal/terms").unwrap();
        assert_eq!(url.as_str(), "http://127.0.0.1:3000/v1/api/legal/terms");
        assert!(
            legal_page_url(
                &org,
                "privacy",
                "http://127.0.0.1:3000/v1/api/legal/privacy"
            )
            .is_ok()
        );
        assert!(
            legal_page_url(&org, "terms", "https://127.0.0.1:3000/v1/api/legal/terms").is_err()
        );
        assert!(
            legal_page_url(
                &org,
                "terms",
                "http://user:pass@127.0.0.1:3000/v1/api/legal/terms"
            )
            .is_err()
        );
        assert!(
            legal_page_url(
                &org,
                "terms",
                "http://127.0.0.1:3000/v1/api/legal/terms?x=1"
            )
            .is_err()
        );
        assert!(
            legal_page_url(
                &org,
                "terms",
                "http://127.0.0.1:3000/v1/api/legal/terms#part"
            )
            .is_err()
        );
        assert!(legal_page_url(&org, "terms", "http://127.0.0.1:9/v1/api/legal/terms").is_err());
        assert!(legal_page_url(&org, "terms", "http://evil.test/v1/api/legal/terms").is_err());
        assert!(
            legal_page_url(&org, "terms", "http://127.0.0.1:3000/v1/api/legal/privacy").is_err()
        );
        assert!(
            legal_page_url(&org, "terms", "http://127.0.0.1:3000/v1/api/legal/terms/").is_err()
        );
        assert!(legal_page_url(&org, "blog", "http://127.0.0.1:3000/v1/api/legal/blog").is_err());
        assert!(org_origin_url().is_ok());
    }

    #[test]
    fn labels_match_the_notice_capability() {
        assert_eq!(notice_window_label("support").unwrap(), "notice-support");
        assert!(is_notice_label("notice-terms"));
        assert!(is_notice_label("notice-copyright"));
        assert!(!is_notice_label("notice-"));
        assert!(!is_notice_label("notice-blog"));
        assert!(!is_notice_label("applet-notice"));
        assert!(!is_notice_label("file-editor-note"));
    }
}
