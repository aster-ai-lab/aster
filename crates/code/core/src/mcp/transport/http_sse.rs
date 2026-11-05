//! Legacy MCP HTTP+SSE transport. The configured URL receives SSE events;
//! the server advertises the POST endpoint and returns JSON-RPC replies on SSE.

use super::McpTransport;
use super::notifications::{NotificationReceiver, NotificationSender};
use crate::llm::http::build_reqwest_client;
use crate::mcp::protocol::{JsonRpcNotification, JsonRpcRequest, JsonRpcResponse, McpNotification};
use crate::sse::{Utf8StreamDecoder, data_field_value, take_sse_events};
use anyhow::{Context, Result, anyhow, bail};
use async_trait::async_trait;
use futures::StreamExt;
use reqwest::{Client, Url};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

const DEFAULT_REQUEST_TIMEOUT_SECS: u64 = 60;
const MAX_SSE_FRAME_BYTES: usize = 8 * 1024 * 1024;

#[derive(Default)]
struct Session {
    endpoint: Option<Url>,
    pending: HashMap<u64, oneshot::Sender<JsonRpcResponse>>,
}

impl Session {
    fn disconnect(&mut self) {
        self.endpoint = None;
        self.pending.clear();
    }
}

struct PendingRequest<'a> {
    session: &'a Mutex<Session>,
    id: u64,
}

impl Drop for PendingRequest<'_> {
    fn drop(&mut self) {
        self.session
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .pending
            .remove(&self.id);
    }
}

struct ReaderSession(Arc<Mutex<Session>>);

impl Drop for ReaderSession {
    fn drop(&mut self) {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .disconnect();
    }
}

/// A remote MCP session with bounded requests and one owned SSE reader.
/// Closing or dropping the transport stops the reader and pending requests.
/// Interrupted requests are never replayed when the SSE connection reconnects.
pub struct HttpSseTransport {
    client: Client,
    session: Arc<Mutex<Session>>,
    notification_rx: NotificationReceiver,
    request_timeout: Duration,
    closed: CancellationToken,
    reader: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl HttpSseTransport {
    /// Connect to the exact configured SSE URL and await its advertised endpoint.
    /// Invalid URLs/headers, HTTP failures and a missing endpoint return errors.
    pub async fn connect(url: impl Into<String>, headers: HashMap<String, String>) -> Result<Self> {
        Self::connect_with_timeout(url, headers, DEFAULT_REQUEST_TIMEOUT_SECS).await
    }

    /// Connect with a deadline in seconds for setup and each subsequent request.
    pub async fn connect_with_timeout(
        url: impl Into<String>,
        headers: HashMap<String, String>,
        request_timeout_secs: u64,
    ) -> Result<Self> {
        let url = Url::parse(&url.into()).context("Invalid MCP SSE URL")?;
        if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
            bail!("MCP SSE URL must use HTTP or HTTPS");
        }
        let mut header_map = reqwest::header::HeaderMap::new();
        header_map.insert(
            reqwest::header::CONTENT_TYPE,
            reqwest::header::HeaderValue::from_static("application/json"),
        );
        for (key, value) in headers {
            let name = reqwest::header::HeaderName::from_bytes(key.as_bytes())
                .context("Invalid MCP header name")?;
            let mut value = reqwest::header::HeaderValue::from_str(&value)
                .context("Invalid MCP header value")?;
            value.set_sensitive(true);
            header_map.insert(name, value);
        }
        let client = build_reqwest_client(None, Some(header_map))
            .context("Failed to build MCP HTTP client")?;
        let session = Arc::new(Mutex::new(Session::default()));
        let (notification_tx, notification_rx) = NotificationReceiver::channel(256);
        let (ready_tx, ready_rx) = oneshot::channel();
        let reader_client = client.clone();
        let reader_session = ReaderSession(Arc::clone(&session));
        let reader = tokio::spawn(async move {
            Self::listen(
                reader_client,
                url,
                notification_tx,
                reader_session,
                ready_tx,
            )
            .await;
        });
        let transport = Self {
            client,
            session,
            notification_rx,
            request_timeout: Duration::from_secs(request_timeout_secs),
            closed: CancellationToken::new(),
            reader: Mutex::new(Some(reader)),
        };
        tokio::time::timeout(transport.request_timeout, ready_rx)
            .await
            .context("MCP SSE connection timed out")?
            .context("MCP SSE reader stopped before advertising an endpoint")??;
        Ok(transport)
    }

    async fn listen(
        client: Client,
        url: Url,
        notifications: NotificationSender,
        session: ReaderSession,
        ready: oneshot::Sender<Result<()>>,
    ) {
        let mut ready = Some(ready);
        let mut delay_ms = 500;
        loop {
            let result =
                Self::read_stream(&client, &url, &notifications, &session.0, &mut ready).await;
            {
                let mut session = session
                    .0
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if session.endpoint.is_some() {
                    delay_ms = 500;
                }
                session.disconnect();
            }
            if let Some(ready) = ready.take() {
                let _ = ready.send(result.and_then(|()| {
                    Err(anyhow!(
                        "MCP SSE stream ended before advertising an endpoint"
                    ))
                }));
                return;
            }
            if let Err(error) = result {
                tracing::warn!(error = %error, "MCP SSE connection interrupted");
            }
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            delay_ms = (delay_ms * 2).min(30_000);
        }
    }

    async fn read_stream(
        client: &Client,
        url: &Url,
        notifications: &NotificationSender,
        session: &Mutex<Session>,
        ready: &mut Option<oneshot::Sender<Result<()>>>,
    ) -> Result<()> {
        let response = client
            .get(url.clone())
            .header("Accept", "text/event-stream")
            .send()
            .await
            .map_err(reqwest::Error::without_url)
            .context("MCP SSE connection failed")?
            .error_for_status()
            .map_err(reqwest::Error::without_url)?;
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("");
        if !content_type
            .split(';')
            .next()
            .is_some_and(|value| value.trim().eq_ignore_ascii_case("text/event-stream"))
        {
            bail!("MCP SSE endpoint did not return an event stream");
        }
        let mut stream = response.bytes_stream();
        let mut decoder = Utf8StreamDecoder::default();
        let mut buffer = String::new();
        while let Some(chunk) = stream.next().await {
            let bytes = chunk.map_err(reqwest::Error::without_url)?;
            if buffer
                .len()
                .saturating_add(decoder.pending_bytes())
                .saturating_add(bytes.len())
                > MAX_SSE_FRAME_BYTES
            {
                bail!("MCP SSE frame exceeds the size limit");
            }
            decoder
                .push_to(&bytes, &mut buffer)
                .context("Invalid MCP SSE UTF-8")?;
            for event in take_sse_events(&mut buffer, false) {
                Self::receive_event(&event, url, session, notifications, ready).await?;
            }
        }
        decoder.finish().context("Incomplete MCP SSE UTF-8")?;
        for event in take_sse_events(&mut buffer, true) {
            Self::receive_event(&event, url, session, notifications, ready).await?;
        }
        Ok(())
    }

    async fn receive_event(
        event: &str,
        url: &Url,
        session: &Mutex<Session>,
        notifications: &NotificationSender,
        ready: &mut Option<oneshot::Sender<Result<()>>>,
    ) -> Result<()> {
        let mut kind = "message";
        let mut data = Vec::new();
        for line in event.split(['\r', '\n']) {
            if let Some(value) = line.strip_prefix("event:") {
                kind = value.strip_prefix(' ').unwrap_or(value);
            } else if let Some(value) = data_field_value(line) {
                data.push(value);
            }
        }
        let data = data.join("\n");
        if kind == "endpoint" {
            let endpoint = url.join(&data).context("Invalid MCP message endpoint")?;
            if endpoint.origin() != url.origin() {
                bail!("MCP message endpoint must have the SSE endpoint's origin");
            }
            session
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .endpoint = Some(endpoint);
            if let Some(ready) = ready.take() {
                let _ = ready.send(Ok(()));
            }
        } else if kind == "message" && !data.is_empty() {
            let value: serde_json::Value =
                serde_json::from_str(&data).context("Invalid MCP SSE JSON")?;
            if value.get("method").is_some() {
                if value.get("id").is_none() {
                    let notification =
                        serde_json::from_value(value).context("Invalid MCP notification")?;
                    notifications
                        .send(McpNotification::from_json_rpc(&notification))
                        .await;
                }
            } else {
                let response: JsonRpcResponse =
                    serde_json::from_value(value).context("Invalid MCP response")?;
                let sender = response.id.and_then(|id| {
                    session
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .pending
                        .remove(&id)
                });
                if let Some(sender) = sender {
                    let _ = sender.send(response);
                }
            }
        }
        Ok(())
    }

    async fn post(&self, endpoint: Url, message: &impl serde::Serialize) -> Result<()> {
        self.client
            .post(endpoint)
            .json(message)
            .send()
            .await
            .map_err(reqwest::Error::without_url)
            .context("MCP HTTP POST failed")?
            .error_for_status()
            .map_err(reqwest::Error::without_url)?;
        Ok(())
    }
}

#[async_trait]
impl McpTransport for HttpSseTransport {
    async fn request(&self, request: JsonRpcRequest) -> Result<JsonRpcResponse> {
        let (sender, receiver) = oneshot::channel();
        let endpoint = {
            let mut session = self
                .session
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let endpoint = session
                .endpoint
                .clone()
                .ok_or_else(|| anyhow!("MCP transport not connected"))?;
            if session.pending.contains_key(&request.id) {
                bail!("MCP request ID is already pending");
            }
            session.pending.insert(request.id, sender);
            endpoint
        };
        let _pending = PendingRequest {
            session: &self.session,
            id: request.id,
        };
        tokio::select! {
            _ = self.closed.cancelled() => Err(anyhow!("MCP transport closed")),
            result = tokio::time::timeout(self.request_timeout, async {
                self.post(endpoint, &request).await?;
                receiver.await.context("MCP SSE disconnected before receiving a response")
            }) => result.context("MCP request timed out")?,
        }
    }

    async fn notify(&self, notification: JsonRpcNotification) -> Result<()> {
        let endpoint = self
            .session
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .endpoint
            .clone()
            .ok_or_else(|| anyhow!("MCP transport not connected"))?;
        tokio::select! {
            _ = self.closed.cancelled() => Err(anyhow!("MCP transport closed")),
            result = tokio::time::timeout(self.request_timeout, self.post(endpoint, &notification)) => result.context("MCP notification timed out")?,
        }
    }

    fn notifications(&self) -> mpsc::Receiver<McpNotification> {
        self.notification_rx.take()
    }

    fn tool_list_changes(&self) -> Option<tokio::sync::watch::Receiver<()>> {
        Some(self.notification_rx.tool_list_changes())
    }

    fn start_close(&self) {
        self.closed.cancel();
        self.session
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .disconnect();
        if let Some(reader) = self
            .reader
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
        {
            reader.abort();
        }
    }

    async fn close(&self) -> Result<()> {
        self.start_close();
        let reader = self
            .reader
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        if let Some(reader) = reader {
            let _ = reader.await;
        }
        Ok(())
    }

    fn is_connected(&self) -> bool {
        !self.closed.is_cancelled()
            && self
                .session
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .endpoint
                .is_some()
    }
}

impl Drop for HttpSseTransport {
    fn drop(&mut self) {
        self.start_close();
    }
}

impl std::fmt::Debug for HttpSseTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpSseTransport")
            .field("connected", &self.is_connected())
            .field("request_timeout", &self.request_timeout)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
#[path = "http_sse_tests.rs"]
mod tests;
