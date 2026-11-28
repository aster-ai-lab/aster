//! Quark search through an existing logged-in page's network session.
//!
//! No browser process, target, window or tab is created. CDP loads the resource
//! into a response stream and only that stream is read; the existing page is
//! neither navigated nor closed. If no compatible page exists, callers retain
//! their ordinary search fallback.

use std::time::Duration;

use base64::Engine as _;
use serde_json::{json, Value};
use url::Url;

use crate::cdp::{browser_error, validate_loopback_ws_url, CdpConnection, LocalCdpEndpoint};
#[cfg(feature = "chrome")]
use crate::BrowserPool;
#[cfg(feature = "chrome")]
use crate::{RenderRequest, RenderedPage};
use crate::{UseError, UseResult};

const MAX_HTML_BYTES: usize = 8 * 1024 * 1024;
const READ_CHUNK_BYTES: usize = 256 * 1024;

/// A scoped CDP client for an existing Quark page. It has no process or target
/// ownership and no navigation/create/close-target methods.
pub struct QuarkSearchSession {
    connection: CdpConnection,
}

impl std::fmt::Debug for QuarkSearchSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QuarkSearchSession").finish_non_exhaustive()
    }
}

impl QuarkSearchSession {
    /// Attach only to an already open Quark page. Never starts the application
    /// and never creates a target, window or tab.
    pub async fn connect(endpoint: LocalCdpEndpoint, timeout: Duration) -> UseResult<Self> {
        tokio::time::timeout(timeout, Self::connect_inner(endpoint))
            .await
            .map_err(|_| {
                UseError::new(
                    "use.browser.timeout",
                    "Quark background connection timed out",
                )
            })?
    }

    async fn connect_inner(endpoint: LocalCdpEndpoint) -> UseResult<Self> {
        let mut list_url = Url::parse(endpoint.as_str())
            .map_err(|error| browser_error(format!("Invalid CDP endpoint: {error}")))?;
        list_url.set_path("/json/list");
        list_url.set_query(None);
        let targets = reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(5))
            .build()
            .map_err(|error| browser_error(format!("Cannot build CDP client: {error}")))?
            .get(list_url)
            .send()
            .await
            .map_err(|error| browser_error(format!("Cannot inspect local CDP pages: {error}")))?
            .json::<Vec<Value>>()
            .await
            .map_err(|error| browser_error(format!("Invalid local CDP page list: {error}")))?;
        let websocket = targets.into_iter().find_map(|target| {
            let url = Url::parse(target.get("url")?.as_str()?).ok()?;
            if target.get("type")?.as_str()? != "page"
                || !matches!(url.scheme(), "http" | "https")
                || !matches!(
                    url.host_str(),
                    Some("kbot.quark.cn" | "quark.sm.cn" | "s.quark.cn")
                )
            {
                return None;
            }
            let websocket = target.get("webSocketDebuggerUrl")?.as_str()?.to_string();
            validate_loopback_ws_url(&websocket)
                .ok()
                .map(|()| websocket)
        });
        let websocket = websocket.ok_or_else(|| {
            UseError::new(
                "use.browser.quark_login_required",
                "No open Quark page is available for background search; using the ordinary search fallback",
            )
        })?;
        let connection = CdpConnection::connect(&websocket).await.map_err(|error| {
            browser_error(format!("Cannot attach to Quark page: {}", error.message))
        })?;
        Ok(Self { connection })
    }

    pub async fn fetch_html(&mut self, url: &Url) -> UseResult<String> {
        if !matches!(url.scheme(), "http" | "https") {
            return Err(browser_error(
                "Quark background search accepts only HTTP(S)",
            ));
        }
        let frame = self
            .connection
            .call("Page.getFrameTree", json!({}))
            .await?
            .pointer("/frameTree/frame/id")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| browser_error("Quark page has no main frame"))?;
        let result = self
            .connection
            .call(
                "Network.loadNetworkResource",
                json!({
                    "frameId": frame,
                    "url": url.as_str(),
                    "options": {"disableCache": true, "includeCredentials": true}
                }),
            )
            .await?;
        let resource = result.get("resource").cloned().unwrap_or(Value::Null);
        if resource.get("success").and_then(Value::as_bool) != Some(true) {
            return Err(browser_error(format!(
                "Quark background request failed: {}",
                resource
                    .get("netErrorName")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown network error")
            )));
        }
        if let Some(status) = resource.get("httpStatusCode").and_then(Value::as_u64) {
            if !(200..300).contains(&status) {
                return Err(browser_error(format!(
                    "Quark background request returned HTTP {status}"
                )));
            }
        }
        let stream = resource
            .get("stream")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| browser_error("Quark background response has no body stream"))?;
        let body = self.read_stream(&stream).await?;
        String::from_utf8(body).map_err(|error| {
            browser_error(format!("Quark background response is not UTF-8: {error}"))
        })
    }

    async fn read_stream(&mut self, stream: &str) -> UseResult<Vec<u8>> {
        let result = async {
            let mut body = Vec::new();
            loop {
                let chunk = self
                    .connection
                    .call(
                        "IO.read",
                        json!({"handle": stream, "size": READ_CHUNK_BYTES}),
                    )
                    .await?;
                let data = chunk
                    .get("data")
                    .and_then(Value::as_str)
                    .ok_or_else(|| browser_error("Quark response chunk has no data"))?;
                if chunk.get("base64Encoded").and_then(Value::as_bool) == Some(true) {
                    body.extend(
                        base64::engine::general_purpose::STANDARD
                            .decode(data)
                            .map_err(|error| {
                                browser_error(format!("Invalid Quark response encoding: {error}"))
                            })?,
                    );
                } else {
                    body.extend(data.as_bytes());
                }
                if body.len() > MAX_HTML_BYTES {
                    return Err(browser_error("Quark background response exceeds 8 MiB"));
                }
                if chunk.get("eof").and_then(Value::as_bool) == Some(true) {
                    return Ok(body);
                }
            }
        }
        .await;
        let _ = self
            .connection
            .call("IO.close", json!({"handle": stream}))
            .await;
        result
    }
}

#[cfg(feature = "chrome")]
pub(crate) async fn render_background(
    pool: &BrowserPool,
    request: RenderRequest,
) -> UseResult<RenderedPage> {
    let started = std::time::Instant::now();
    let endpoint = pool.attached_cdp_endpoint().ok_or_else(|| {
        browser_error("Background Quark rendering requires an existing CDP endpoint")
    })?;
    let mut session = QuarkSearchSession::connect(endpoint.clone(), request.timeout()).await?;
    let html = tokio::time::timeout(request.timeout(), session.fetch_html(&request.url))
        .await
        .map_err(|_| UseError::new("use.browser.timeout", "Quark background search timed out"))??;
    Ok(RenderedPage {
        requested_url: request.url.clone(),
        final_url: request.url,
        status: None,
        content_type: Some("text/html".to_string()),
        html,
        elapsed_ms: started.elapsed().as_millis().min(u64::MAX as u128) as u64,
        artifacts: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "requires an already open logged-in Quark page on local CDP"]
    async fn live_background_search_neither_creates_nor_closes_targets() {
        async fn target_ids() -> Vec<String> {
            let client = reqwest::Client::builder().no_proxy().build().unwrap();
            client
                .get("http://127.0.0.1:9222/json/list")
                .send()
                .await
                .unwrap()
                .json::<Vec<Value>>()
                .await
                .unwrap()
                .into_iter()
                .filter_map(|target| target["id"].as_str().map(str::to_string))
                .collect()
        }

        let before = target_ids().await;
        let endpoint = LocalCdpEndpoint::parse("http://127.0.0.1:9222").unwrap();
        let mut session = QuarkSearchSession::connect(endpoint, Duration::from_secs(10))
            .await
            .unwrap();
        let html = session
            .fetch_html(&Url::parse("https://quark.sm.cn/s?q=Rust%20%E7%BC%96%E7%A8%8B").unwrap())
            .await
            .unwrap();
        assert!(
            html.contains("data-used-by=\"hydrate\""),
            "missing result cards"
        );
        drop(session);
        let after = target_ids().await;
        assert_eq!(
            before, after,
            "background search changed the browser targets"
        );
    }
}
