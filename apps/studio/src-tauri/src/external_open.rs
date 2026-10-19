//! Opens one http(s) page in the system browser.
//! Knowledge previews and other external links use this route.
//! A published about notice opens in the notice webview.

use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
struct OpenBody {
    url: String,
}

pub fn open_external_json(body: &[u8]) -> Result<String, u16> {
    let parsed: OpenBody = serde_json::from_slice(body).map_err(|_| 400u16)?;
    let url = http_page_url(&parsed.url)?;
    open_with_system(&url)?;
    Ok(json!({ "ok": true }).to_string())
}

/// Accepts only an absolute http(s) page. Other schemes never reach the opener.
pub fn http_page_url(raw: &str) -> Result<String, u16> {
    if raw.is_empty()
        || raw.len() > 2048
        || raw.chars().any(|ch| ch.is_control() || ch.is_whitespace())
    {
        return Err(400);
    }
    let parsed = url::Url::parse(raw).map_err(|_| 400u16)?;
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err(400);
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(400);
    }
    let host = parsed.host_str().ok_or(400u16)?;
    if host.is_empty() {
        return Err(400);
    }
    let page = parsed.to_string();
    if !page.starts_with("http://") && !page.starts_with("https://") {
        return Err(400);
    }
    Ok(page)
}

fn open_with_system(url: &str) -> Result<(), u16> {
    let launch = crate::host_family::open_http_launch(crate::host_family::current_family(), url);
    let status = crate::host_family::spawn_launch(&launch).map_err(|_| 500u16)?;
    if status.success() { Ok(()) } else { Err(500) }
}

#[cfg(test)]
mod tests {
    use super::http_page_url;

    #[test]
    fn accepts_the_published_notice_address() {
        let page = http_page_url("http://127.0.0.1:3000/v1/api/legal/terms").unwrap();
        assert_eq!(page, "http://127.0.0.1:3000/v1/api/legal/terms");
        let secure = http_page_url("https://docs.example.com/v1/api/legal/privacy").unwrap();
        assert_eq!(secure, "https://docs.example.com/v1/api/legal/privacy");
    }

    #[test]
    fn refuses_other_schemes_and_relative_addresses() {
        for raw in [
            "javascript:alert(1)",
            "file:///etc/passwd",
            "mailto:support@towork.local",
            "/v1/api/legal/terms",
            "http://user:secret@127.0.0.1/v1/api/legal/terms",
            "http://127.0.0.1:3000/v1/api/legal/terms ",
            "",
        ] {
            assert!(http_page_url(raw).is_err(), "{raw}");
        }
    }
}
