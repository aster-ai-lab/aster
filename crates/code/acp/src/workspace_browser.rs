//! Open a URL in Studio's workspace browser pane via the host.

use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use aster_core::tools::{Tool, ToolCapabilities, ToolContext, ToolOutput};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

const CALL_TIMEOUT: Duration = Duration::from_secs(5);
pub(crate) const TOOL_NAME: &str = "open_workspace_browser";
pub(crate) const HOST_METHOD: &str = "aster/workspace_browser/open";

/// Same admission rules as Studio's `workspaceBrowserUrl`: http(s)/file only, no credentials.
pub(crate) fn admit_workspace_browser_url(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.contains('\0') || trimmed.chars().count() > 8_192 {
        return Err("url must be a non-empty http(s) or file address without credentials".into());
    }
    let candidate = normalize_address(trimmed)?;
    let parsed = url::Url::parse(&candidate).map_err(|_| {
        "url must be a non-empty http(s) or file address without credentials".to_string()
    })?;
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err("url must not include credentials".into());
    }
    match parsed.scheme() {
        "http" | "https" | "file" => Ok(parsed.to_string()),
        other => Err(format!(
            "url scheme `{other}` is not allowed; use http, https, or file"
        )),
    }
}

fn normalize_address(trimmed: &str) -> Result<String, String> {
    let has_scheme = trimmed
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_alphabetic())
        && trimmed.contains(':')
        && !looks_like_host_port(trimmed);
    if has_scheme {
        return Ok(trimmed.to_owned());
    }
    if is_local_address(trimmed) {
        return Ok(format!("http://{trimmed}"));
    }
    Ok(format!("https://{trimmed}"))
}

fn looks_like_host_port(value: &str) -> bool {
    let host = value.split(['/', '?', '#']).next().unwrap_or(value);
    let Some((left, port)) = host.rsplit_once(':') else {
        return false;
    };
    !left.is_empty()
        && !left.contains(']')
        && port.chars().all(|ch| ch.is_ascii_digit())
        && !port.is_empty()
}

fn is_local_address(value: &str) -> bool {
    let host = value.split(['/', '?', '#']).next().unwrap_or(value);
    let host = host
        .strip_prefix('[')
        .and_then(|v| v.strip_suffix(']'))
        .unwrap_or(host);
    let host = host.split(':').next().unwrap_or(host);
    matches!(host, "localhost" | "127.0.0.1" | "0.0.0.0" | "::1")
        || host
            .strip_prefix("127.")
            .is_some_and(|rest| rest.split('.').all(|part| part.parse::<u8>().is_ok()))
}

pub(crate) struct OpenWorkspaceBrowserTool {
    bridge: Arc<crate::document_read::DocumentBridge>,
    session: String,
}

impl OpenWorkspaceBrowserTool {
    pub(crate) fn new(bridge: Arc<crate::document_read::DocumentBridge>, session: &str) -> Self {
        Self {
            bridge,
            session: session.to_owned(),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    url: String,
}

#[async_trait]
impl Tool for OpenWorkspaceBrowserTool {
    fn name(&self) -> &str {
        TOOL_NAME
    }

    fn description(&self) -> &str {
        "Open an http(s) or file URL in Aster's built-in workspace browser pane. Returns the opened URL on success, or the exact rejection reason. This confirms navigation intent only — it does not read the page DOM, click, or screenshot. Prefer this over asking the user to open the pane after a failed web_fetch or web_search for a public URL."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["url"],
            "properties": {
                "url": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": 8192,
                    "description": "http(s) or file URL to show in the workspace browser pane. Credentialed and javascript: addresses are rejected."
                }
            }
        })
    }

    fn capabilities(&self, _: &Value) -> ToolCapabilities {
        ToolCapabilities {
            read_only: true,
            idempotent: true,
            ..ToolCapabilities::conservative()
        }
    }

    async fn execute(&self, args: &Value, _ctx: &ToolContext) -> Result<ToolOutput> {
        let Ok(input) = serde_json::from_value::<Input>(args.clone()) else {
            return Ok(ToolOutput::error(
                "invalid arguments; expected {\"url\":\"https://...\"}",
            ));
        };
        let url = match admit_workspace_browser_url(&input.url) {
            Ok(url) => url,
            Err(reason) => {
                let key = reason_key(&input.url);
                return Ok(ToolOutput::error(reason).with_metadata(json!({
                    "ok": false,
                    "reason": key,
                })));
            }
        };
        let value = open_on_host(&self.bridge, &self.session, &url).await?;
        if value.get("ok") != Some(&Value::Bool(true)) {
            let reason = value
                .get("reason")
                .and_then(Value::as_str)
                .unwrap_or("workspace browser host rejected the URL")
                .to_owned();
            return Ok(ToolOutput::error(reason).with_metadata(value));
        }
        let opened = value
            .get("url")
            .and_then(Value::as_str)
            .unwrap_or(url.as_str())
            .to_owned();
        Ok(ToolOutput::success(format!("opened {opened}")).with_metadata(value))
    }
}

fn reason_key(raw: &str) -> &'static str {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return "empty";
    }
    if let Ok(parsed) = url::Url::parse(trimmed) {
        if !parsed.username().is_empty() || parsed.password().is_some() {
            return "credentials";
        }
        if !matches!(parsed.scheme(), "http" | "https" | "file") {
            return "scheme";
        }
    }
    "invalid"
}

async fn open_on_host(
    bridge: &crate::document_read::DocumentBridge,
    session: &str,
    url: &str,
) -> Result<Value> {
    let (reply, result) = tokio::sync::oneshot::channel();
    let work = async {
        bridge
            .sender
            .send(crate::document_read::Call {
                access: None,
                kind: crate::document_read::CallKind::Ext {
                    method: HOST_METHOD,
                    params: json!({"sessionId": session, "url": url}),
                    timeout: CALL_TIMEOUT,
                },
                reply,
            })
            .await
            .context("workspace browser bridge closed")?;
        let crate::document_read::HostResponse::Ext(value) = result
            .await
            .context("workspace browser host unavailable")??
        else {
            bail!("workspace browser host returned the wrong response");
        };
        anyhow::ensure!(
            serde_json::to_vec(&value)?.len() <= 64 * 1024,
            "workspace browser host response exceeds 64 KiB"
        );
        Ok(value)
    };
    tokio::time::timeout(CALL_TIMEOUT, work)
        .await
        .context("workspace browser host timeout")?
}

#[cfg(test)]
#[path = "workspace_browser_tests.rs"]
mod tests;
