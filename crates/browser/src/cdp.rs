//! Lightweight Chrome DevTools Protocol client over `tokio-tungstenite`.
//!
//! Used by `quark` and `quark_search` so attached loopback CDP endpoints do
//! not require `chromiumoxide` or its generated CDP structs.

use std::collections::HashMap;
use std::time::Duration;

use futures::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio_tungstenite::tungstenite::Message;
use url::Url;

use aster_use_core::{UseError, UseResult};

/// A validated loopback Chrome DevTools Protocol endpoint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalCdpEndpoint(Url);

impl LocalCdpEndpoint {
    pub fn parse(value: &str) -> UseResult<Self> {
        let url = Url::parse(value.trim()).map_err(|error| {
            UseError::new(
                "use.browser.invalid_cdp_endpoint",
                format!("CDP endpoint is not a valid URL: {error}"),
            )
        })?;
        let loopback = url
            .host_str()
            .is_some_and(|host| host.eq_ignore_ascii_case("localhost"))
            || match url.host() {
                Some(url::Host::Ipv4(address)) => address.is_loopback(),
                Some(url::Host::Ipv6(address)) => address.is_loopback(),
                _ => false,
            };
        if !matches!(url.scheme(), "http" | "https")
            || !loopback
            || !url.username().is_empty()
            || url.password().is_some()
            || !url.path().trim_matches('/').is_empty()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(UseError::new(
                "use.browser.invalid_cdp_endpoint",
                "CDP endpoint must be an HTTP(S) loopback origin without credentials or a path",
            ));
        }
        Ok(Self(url))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub(crate) async fn websocket_url(&self) -> UseResult<String> {
        #[derive(serde::Deserialize)]
        struct Version {
            #[serde(rename = "webSocketDebuggerUrl")]
            websocket: String,
        }

        let mut version_url = self.0.clone();
        version_url.set_path("/json/version");
        let client = reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(5))
            .build()
            .map_err(|error| browser_error(format!("Failed to build CDP client: {error}")))?;
        let version = client
            .get(version_url)
            .send()
            .await
            .map_err(|error| {
                UseError::new(
                    "use.browser.cdp_unavailable",
                    format!("Cannot reach the local browser CDP endpoint: {error}"),
                )
                .with_suggestion(
                    "Restart the browser with --remote-debugging-port=9222 --remote-debugging-address=127.0.0.1",
                )
            })?
            .error_for_status()
            .map_err(|error| browser_error(format!("CDP version request failed: {error}")))?
            .json::<Version>()
            .await
            .map_err(|error| browser_error(format!("CDP version response was invalid: {error}")))?;
        validate_loopback_ws_url(&version.websocket)?;
        Ok(version.websocket)
    }
}

pub(crate) fn validate_loopback_ws_url(websocket: &str) -> UseResult<()> {
    let parsed = Url::parse(websocket)
        .map_err(|error| browser_error(format!("CDP WebSocket URL was invalid: {error}")))?;
    let loopback = parsed
        .host_str()
        .is_some_and(|host| host.eq_ignore_ascii_case("localhost"))
        || match parsed.host() {
            Some(url::Host::Ipv4(address)) => address.is_loopback(),
            Some(url::Host::Ipv6(address)) => address.is_loopback(),
            _ => false,
        };
    if !matches!(parsed.scheme(), "ws" | "wss") || !loopback {
        return Err(UseError::new(
            "use.browser.invalid_cdp_endpoint",
            "The local browser returned a non-loopback CDP WebSocket URL",
        ));
    }
    Ok(())
}

pub(crate) struct CdpConnection {
    socket: tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    next_id: u64,
    buffered: HashMap<u64, Value>,
}

impl CdpConnection {
    pub(crate) async fn connect(websocket: &str) -> UseResult<Self> {
        let (socket, _) = tokio_tungstenite::connect_async(websocket)
            .await
            .map_err(|error| browser_error(format!("Cannot connect to CDP WebSocket: {error}")))?;
        Ok(Self {
            socket,
            next_id: 1,
            buffered: HashMap::new(),
        })
    }

    pub(crate) async fn call(&mut self, method: &str, params: Value) -> UseResult<Value> {
        self.call_raw(method, params, None).await
    }

    pub(crate) async fn call_session(
        &mut self,
        session_id: &str,
        method: &str,
        params: Value,
    ) -> UseResult<Value> {
        self.call_raw(method, params, Some(session_id)).await
    }

    async fn call_raw(
        &mut self,
        method: &str,
        params: Value,
        session_id: Option<&str>,
    ) -> UseResult<Value> {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        let mut request = json!({"id": id, "method": method, "params": params});
        if let Some(session_id) = session_id {
            if let Some(obj) = request.as_object_mut() {
                obj.insert(
                    "sessionId".to_string(),
                    Value::String(session_id.to_string()),
                );
            }
        }
        self.socket
            .send(Message::Text(
                serde_json::to_string(&request)
                    .map_err(|error| browser_error(format!("Cannot encode CDP request: {error}")))?
                    .into(),
            ))
            .await
            .map_err(|error| browser_error(format!("Cannot send CDP request: {error}")))?;

        if let Some(value) = self.buffered.remove(&id) {
            return parse_cdp_reply(method, &value);
        }

        loop {
            let message = self.socket.next().await.ok_or_else(|| {
                browser_error("Browser closed its CDP connection before responding")
            })?;
            let message = message
                .map_err(|error| browser_error(format!("Cannot read CDP response: {error}")))?;
            let payload = match message {
                Message::Text(text) => text.to_string(),
                Message::Binary(bytes) => String::from_utf8(bytes.to_vec()).map_err(|error| {
                    browser_error(format!("CDP response is not UTF-8: {error}"))
                })?,
                Message::Ping(payload) => {
                    self.socket
                        .send(Message::Pong(payload))
                        .await
                        .map_err(|error| {
                            browser_error(format!("Cannot answer CDP ping: {error}"))
                        })?;
                    continue;
                }
                Message::Close(_) => {
                    return Err(browser_error("Browser closed its CDP connection"));
                }
                _ => continue,
            };
            let value: Value = serde_json::from_str(&payload)
                .map_err(|error| browser_error(format!("Invalid CDP response: {error}")))?;
            let Some(reply_id) = value.get("id").and_then(Value::as_u64) else {
                continue;
            };
            if reply_id == id {
                return parse_cdp_reply(method, &value);
            }
            self.buffered.insert(reply_id, value);
        }
    }

    pub(crate) async fn close_socket(&mut self) {
        let _ = self.socket.close(None).await;
    }
}

fn parse_cdp_reply(method: &str, value: &Value) -> UseResult<Value> {
    if let Some(error) = value.get("error") {
        return Err(browser_error(format!(
            "CDP {} failed: {}",
            method,
            error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("unknown error")
        )));
    }
    Ok(value.get("result").cloned().unwrap_or(Value::Null))
}

pub(crate) fn browser_error(message: impl Into<String>) -> UseError {
    UseError::new("use.browser.provider_failed", message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cdp_endpoint_admits_only_a_loopback_origin() {
        for value in [
            "http://127.0.0.1:9222",
            "http://localhost:9222/",
            "http://[::1]:9222",
            "https://127.0.0.1:9222",
        ] {
            LocalCdpEndpoint::parse(value).unwrap();
        }
        for value in [
            "http://example.com:9222",
            "http://192.168.1.2:9222",
            "ws://127.0.0.1:9222",
            "http://user@127.0.0.1:9222",
            "http://127.0.0.1:9222/json/version",
            "http://127.0.0.1:9222/?token=secret",
            "not a url",
        ] {
            assert_eq!(
                LocalCdpEndpoint::parse(value).unwrap_err().code,
                "use.browser.invalid_cdp_endpoint",
                "{value}"
            );
        }
    }
}
