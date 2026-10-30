#![cfg(test)]

use super::upstream::Route;
use a3s_code_core::llm::{http::ReqwestHttpClient, HttpClient};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::future::poll_fn;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::{JoinHandle, JoinSet};
use tokio_util::sync::CancellationToken;

const HEADER_LIMIT: usize = 16 * 1024;
const BODY_LIMIT: usize = 2 * 1024 * 1024;
const RESPONSE_LIMIT: usize = 4 * 1024 * 1024;
const PATH: &str = "/v1/chat/completions";

#[path = "metrics.rs"]
mod metrics;
#[path = "rejection.rs"]
mod rejection;
#[path = "usage.rs"]
mod usage;
use rejection::Reason;

#[derive(Clone, Debug, Default, Serialize)]
pub struct Stats {
    pub listener_closed: bool,
    // One single-hop HTTP dispatch; TCP connection attempts are not counted.
    pub upstream_calls: usize,
    pub rejected: usize,
    pub rejection_reasons: BTreeMap<Reason, usize>,
    pub transport_failures: usize,
    pub protected_response_failures: usize,
    pub response_bytes: usize,
    pub statuses: BTreeMap<u16, usize>,
    pub requests: Vec<metrics::RequestSummary>,
    pub responses: BTreeMap<usize, usage::Observation>,
}

impl Stats {
    fn reject(&mut self, reason: Reason) {
        self.rejected += 1;
        *self.rejection_reasons.entry(reason).or_default() += 1;
    }

    pub fn verified_token_total(&self) -> Option<u64> {
        if self.upstream_calls == 0 || self.responses.len() != self.upstream_calls {
            return None;
        }
        self.responses
            .iter()
            .enumerate()
            .try_fold(0_u64, |total, (index, (sequence, response))| {
                if *sequence != index + 1 || response.state != usage::State::Present {
                    return None;
                }
                total.checked_add(response.usage.as_ref()?.total_tokens)
            })
    }
}

pub struct Relay {
    endpoint: String,
    key: String,
    cancel: CancellationToken,
    worker: Option<JoinHandle<()>>,
    stats: Arc<Mutex<Stats>>,
}

impl Relay {
    pub async fn start(
        route: Route,
        max_requests: usize,
        output_tokens: u64,
    ) -> Result<Self, String> {
        if max_requests == 0 || output_tokens == 0 {
            return Err("relay budgets must be positive".into());
        }
        let http: Arc<dyn HttpClient> = Arc::new(
            ReqwestHttpClient::without_redirects()
                .map_err(|_| "owned relay HTTP transport could not initialize")?,
        );
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|_| "could not bind owned loopback relay")?;
        let endpoint = format!("http://{}{PATH}", listener.local_addr().unwrap());
        let key = uuid::Uuid::new_v4().to_string();
        let authorization = format!("Bearer {key}");
        let cancel = CancellationToken::new();
        let stopping = cancel.clone();
        let stats = Arc::new(Mutex::new(Stats::default()));
        let observations = stats.clone();
        let route = Arc::new(route);
        let worker = tokio::spawn(async move {
            let mut connections = JoinSet::new();
            loop {
                tokio::select! {
                    _ = stopping.cancelled() => break,
                    Some(_) = connections.join_next(), if !connections.is_empty() => {},
                    accepted = listener.accept(), if connections.len() < 4 => {
                        let Ok((socket, _)) = accepted else { break };
                        let route = route.clone();
                        let stats = observations.clone();
                        let cancel = stopping.child_token();
                        let authorization = authorization.clone();
                        let http = http.clone();
                        connections.spawn(async move {
                            tokio::select! {
                                _ = cancel.cancelled() => {},
                                _ = tokio::time::timeout(Duration::from_secs(120), serve(
                                    socket, route, authorization, max_requests,
                                    output_tokens, stats, cancel.clone(), http,
                                )) => {},
                            }
                        });
                    }
                }
            }
            stopping.cancel();
            drop(listener);
            while connections.join_next().await.is_some() {}
            observations.lock().unwrap().listener_closed = true;
        });
        Ok(Self {
            endpoint,
            key,
            cancel,
            worker: Some(worker),
            stats,
        })
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    pub fn key(&self) -> &str {
        &self.key
    }

    pub async fn close(mut self) -> Result<Stats, String> {
        self.cancel.cancel();
        let mut worker = self.worker.take().ok_or("relay already closed")?;
        match tokio::time::timeout(Duration::from_secs(5), &mut worker).await {
            Ok(Ok(())) => Ok(self.stats.lock().unwrap().clone()),
            _ => {
                worker.abort();
                let _ = worker.await;
                Err("relay cleanup did not complete".into())
            }
        }
    }
}

impl Drop for Relay {
    fn drop(&mut self) {
        self.cancel.cancel();
        if let Some(worker) = self.worker.take() {
            worker.abort();
        }
    }
}

// The full-buffer comparison follows the existing Office loopback host.
fn credential_matches(left: &[u8], right: &[u8]) -> bool {
    let mut different = left.len() ^ right.len();
    for index in 0..left.len().max(right.len()) {
        different |= usize::from(
            left.get(index).copied().unwrap_or_default()
                ^ right.get(index).copied().unwrap_or_default(),
        );
    }
    std::hint::black_box(different) == 0
}

pub(super) async fn read_request(
    socket: &mut TcpStream,
    expected_authorization: &str,
) -> Result<Value, Reason> {
    let mut bytes = Vec::new();
    let mut chunk = [0; 4096];
    let split = loop {
        if let Some(index) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
            break index + 4;
        }
        if bytes.len() > HEADER_LIMIT {
            return Err(Reason::HeaderBudget);
        }
        let count = socket
            .read(&mut chunk)
            .await
            .map_err(|_| Reason::ReadFailed)?;
        if count == 0 {
            return Err(if bytes.is_empty() {
                Reason::EmptyConnection
            } else {
                Reason::IncompleteRequest
            });
        }
        bytes.extend_from_slice(&chunk[..count]);
    };
    if split > HEADER_LIMIT {
        return Err(Reason::HeaderBudget);
    }
    let headers = std::str::from_utf8(&bytes[..split]).map_err(|_| Reason::Framing)?;
    let mut lines = headers.split("\r\n");
    if lines.next() != Some("POST /v1/chat/completions HTTP/1.1") {
        return Err(Reason::Target);
    }
    let mut length = None;
    let mut authorization = None;
    for line in lines.filter(|line| !line.is_empty()) {
        let (name, value) = line.split_once(':').ok_or(Reason::Framing)?;
        let value = value.trim();
        match name.to_ascii_lowercase().as_str() {
            "content-length" => {
                if length.is_some() {
                    return Err(Reason::Framing);
                }
                length = Some(value.parse::<usize>().map_err(|_| Reason::Framing)?);
            }
            "authorization" => {
                if authorization.is_some() {
                    return Err(Reason::Framing);
                }
                authorization = Some(value);
            }
            "transfer-encoding" => return Err(Reason::Framing),
            _ => {}
        }
    }
    if !credential_matches(
        authorization.unwrap_or_default().as_bytes(),
        expected_authorization.as_bytes(),
    ) {
        return Err(Reason::Unauthorized);
    }
    let length = length.ok_or(Reason::LengthRequired)?;
    if length > BODY_LIMIT {
        return Err(Reason::BodyBudget);
    }
    while bytes.len() < split + length {
        let count = socket
            .read(&mut chunk)
            .await
            .map_err(|_| Reason::ReadFailed)?;
        if count == 0 {
            return Err(Reason::IncompleteRequest);
        }
        bytes.extend_from_slice(&chunk[..count]);
    }
    if bytes.len() != split + length {
        return Err(Reason::Framing);
    }
    serde_json::from_slice(&bytes[split..]).map_err(|_| Reason::BodyJson)
}

async fn reply(socket: &mut TcpStream, status: u16, body: &[u8]) {
    reply_with_retry(socket, status, body, None).await;
}

async fn reply_with_retry(
    socket: &mut TcpStream,
    status: u16,
    body: &[u8],
    retry_after: Option<&str>,
) {
    let retry = retry_after
        .filter(|value| value.len() <= 1024 && value.bytes().all(|byte| (32..127).contains(&byte)))
        .map(|value| format!("Retry-After: {value}\r\n"))
        .unwrap_or_default();
    let head = format!(
        "HTTP/1.1 {status} Response\r\nContent-Type: application/json\r\nContent-Length: {}\r\n{retry}Connection: close\r\n\r\n",
        body.len()
    );
    let _ = socket.write_all(head.as_bytes()).await;
    let _ = socket.write_all(body).await;
    let _ = socket.shutdown().await;
}

fn protected(bytes: &[u8], route: &Route) -> bool {
    route
        .protected_values
        .iter()
        .any(|secret| !secret.is_empty() && bytes.windows(secret.len()).any(|part| part == secret))
}

async fn serve(
    mut socket: TcpStream,
    route: Arc<Route>,
    authorization: String,
    max_requests: usize,
    output_tokens: u64,
    stats: Arc<Mutex<Stats>>,
    cancel: CancellationToken,
    http: Arc<dyn HttpClient>,
) {
    let body = match read_request(&mut socket, &authorization).await {
        Ok(body) => body,
        Err(reason) => {
            stats.lock().unwrap().reject(reason);
            reply(
                &mut socket,
                reason.http_status(),
                br#"{"error":"relay admission refused"}"#,
            )
            .await;
            return;
        }
    };
    let token_limit = match (body.get("max_tokens"), body.get("max_completion_tokens")) {
        (Some(value), None) | (None, Some(value)) => value.as_u64(),
        _ => None,
    };
    if body.get("model").and_then(Value::as_str) != Some(route.model.as_str())
        || !token_limit.is_some_and(|limit| limit > 0 && limit <= output_tokens)
    {
        stats.lock().unwrap().reject(Reason::ModelOutputBudget);
        reply(
            &mut socket,
            400,
            br#"{"error":"relay model/output budget refused"}"#,
        )
        .await;
        return;
    }
    let admitted = {
        let mut stats = stats.lock().unwrap();
        if stats.upstream_calls < max_requests {
            stats.upstream_calls += 1;
            let sequence = stats.upstream_calls;
            stats
                .requests
                .push(metrics::RequestSummary::capture(sequence, &body));
            stats
                .responses
                .insert(sequence, usage::Observation::default());
            Some(sequence)
        } else {
            stats.reject(Reason::RequestBudget);
            None
        }
    };
    let Some(sequence) = admitted else {
        reply(
            &mut socket,
            429,
            br#"{"error":"relay request budget exhausted"}"#,
        )
        .await;
        return;
    };
    let headers = route
        .headers
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect();
    if body.get("stream").and_then(Value::as_bool) == Some(true) {
        let response = http
            .post_streaming(&route.endpoint, headers, &body, cancel.clone())
            .await;
        let Ok(mut response) = response else {
            stats.lock().unwrap().transport_failures += 1;
            reply(
                &mut socket,
                502,
                br#"{"error":"upstream transport unavailable"}"#,
            )
            .await;
            return;
        };
        *stats
            .lock()
            .unwrap()
            .statuses
            .entry(response.status)
            .or_default() += 1;
        stats
            .lock()
            .unwrap()
            .responses
            .get_mut(&sequence)
            .unwrap()
            .http_status = Some(response.status);
        if !(200..300).contains(&response.status) {
            if protected(response.error_body.as_bytes(), &route) {
                stats.lock().unwrap().protected_response_failures += 1;
                reply(
                    &mut socket,
                    502,
                    br#"{"error":"protected upstream header echoed"}"#,
                )
                .await;
            } else {
                stats.lock().unwrap().responses.insert(
                    sequence,
                    usage::Observation::json(response.status, response.error_body.as_bytes()),
                );
                reply_with_retry(
                    &mut socket,
                    response.status,
                    response.error_body.as_bytes(),
                    response
                        .retry_after
                        .as_deref()
                        .filter(|value| !protected(value.as_bytes(), &route)),
                )
                .await;
            }
            return;
        }
        let head = format!(
            "HTTP/1.1 {} Response\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
            response.status
        );
        if socket.write_all(head.as_bytes()).await.is_err() {
            return;
        }
        let retain = route
            .protected_values
            .iter()
            .map(Vec::len)
            .max()
            .unwrap_or(0)
            .saturating_sub(1);
        let mut pending = Vec::new();
        let mut total = 0_usize;
        let mut observed_usage = usage::Sse::default();
        while let Some(chunk) = poll_fn(|cx| response.byte_stream.as_mut().poll_next(cx)).await {
            let Ok(chunk) = chunk else {
                return;
            };
            total = total.saturating_add(chunk.len());
            if total > RESPONSE_LIMIT {
                return;
            }
            pending.extend_from_slice(&chunk);
            if protected(&pending, &route) {
                stats.lock().unwrap().protected_response_failures += 1;
                return;
            }
            observed_usage.feed(&chunk);
            let send = pending.len().saturating_sub(retain);
            if send > 0 {
                if socket.write_all(&pending[..send]).await.is_err() {
                    return;
                }
                stats.lock().unwrap().response_bytes += send;
                pending.drain(..send);
            }
        }
        stats
            .lock()
            .unwrap()
            .responses
            .insert(sequence, observed_usage.finish(response.status));
        if socket.write_all(&pending).await.is_ok() {
            stats.lock().unwrap().response_bytes += pending.len();
        }
    } else {
        match http.post(&route.endpoint, headers, &body, cancel).await {
            Ok(response) => {
                *stats
                    .lock()
                    .unwrap()
                    .statuses
                    .entry(response.status)
                    .or_default() += 1;
                stats
                    .lock()
                    .unwrap()
                    .responses
                    .get_mut(&sequence)
                    .unwrap()
                    .http_status = Some(response.status);
                if response.body.len() > RESPONSE_LIMIT
                    || protected(response.body.as_bytes(), &route)
                {
                    stats.lock().unwrap().protected_response_failures += 1;
                    reply(
                        &mut socket,
                        502,
                        br#"{"error":"upstream response refused"}"#,
                    )
                    .await;
                } else {
                    stats.lock().unwrap().responses.insert(
                        sequence,
                        usage::Observation::json(response.status, response.body.as_bytes()),
                    );
                    stats.lock().unwrap().response_bytes += response.body.len();
                    reply_with_retry(
                        &mut socket,
                        response.status,
                        response.body.as_bytes(),
                        response
                            .retry_after
                            .as_deref()
                            .filter(|value| !protected(value.as_bytes(), &route)),
                    )
                    .await;
                }
            }
            Err(_) => {
                stats.lock().unwrap().transport_failures += 1;
                reply(
                    &mut socket,
                    502,
                    br#"{"error":"upstream transport unavailable"}"#,
                )
                .await;
            }
        }
    }
}

#[path = "relay_tests.rs"]
mod tests;
