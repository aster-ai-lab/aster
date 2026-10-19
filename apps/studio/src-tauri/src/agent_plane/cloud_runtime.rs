//! Cloud agent-runtime projection for one admitted AIX turn.
//!
//! The host creates the projection, reads the terminal log, and stops only when
//! the turn is aborted. A cloud declaration never starts the installed package.

use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use super::error::PlaneError;

pub(super) const UNBOUND: &str = "The cloud agent is not bound to an asset release.";
pub(super) const SLUG_ASSET: &str = "The install name is not an agent asset id.";
pub(super) const ENDPOINT_MISSING: &str = "The cloud agent runtime endpoint is not configured.";
pub(super) const REPLAYED: &str = "The cloud agent runtime replayed the same run.";
pub(super) const CANCELLED: &str = "The cloud agent run was cancelled.";
const NO_RUN: &str = "The cloud agent runtime returned no run.";
const NO_TEXT: &str = "The cloud agent runtime returned no text.";
const MEMBER_SESSION: &str = "member session missing";
const MEMBER_TOKEN: &str = "member token";
const MEMBER_PREFIX: &str = "a3s_ms_";
/// A running trajectory page leaves `nextCursor` null until a later poll
/// includes a terminal kind. Re-poll that same URL, then stop after about 20s.
const POLL_LIMIT: usize = 100;
/// Long-run poll limit: cloud agents may take minutes before their first
/// log page. The loop checks the run's liveness via the get endpoint on
/// each no-progress cycle; a terminal status exits immediately.
const LONG_POLL_LIMIT: usize = 1500;
const POLL_WAIT: Duration = Duration::from_millis(200);

/// Loopback Cloud mount for one signed-in member. `bearer` is a test override.
/// Production leaves it empty and reads the member session at call time.
#[derive(Clone)]
pub(crate) struct CloudAgentRuntime {
    pub base: String,
    pub bearer: Option<String>,
}

/// Member-plane mount. Studio stores `a3s_ms_` tokens, and `/v1/api/org/`
/// is the plane that admits them.
pub(super) fn production_endpoint() -> CloudAgentRuntime {
    let origin = crate::org_login::default_origin();
    let origin = origin.trim_end_matches('/');
    CloudAgentRuntime {
        base: format!("{origin}/v1/api/org/cloud"),
        bearer: None,
    }
}

/// `configured` wins. An empty slug is a harness with no member session.
/// A tenant slug without a stored token refuses the turn before any create.
pub(super) fn resolve_bearer(
    slug: &str,
    configured: Option<&str>,
) -> Result<Option<String>, String> {
    if let Some(token) = configured {
        return bearer_token(token).map(Some);
    }
    if slug.is_empty() {
        return Ok(None);
    }
    match crate::member_session::MemberSessionStore::open(slug).token() {
        Some(token) => bearer_token(&token).map(Some),
        None => Err(MEMBER_SESSION.to_owned()),
    }
}

fn bearer_token(token: &str) -> Result<String, String> {
    if token.is_empty()
        || !token.starts_with(MEMBER_PREFIX)
        || token
            .bytes()
            .any(|byte| byte.is_ascii_whitespace() || byte.is_ascii_control())
    {
        return Err(MEMBER_TOKEN.to_owned());
    }
    Ok(token.to_owned())
}

pub(super) struct CloudAdmission {
    pub package_id: String,
    pub digest: String,
    pub agent_asset_id: String,
    pub agent_asset_release_id: String,
    pub conversation_id: String,
    pub prompt: String,
    pub model_alias: Option<String>,
    pub parameters: Option<Value>,
    pub idempotency_key: String,
}

pub(super) struct CloudProjection {
    pub agent_execution_id: String,
    pub replayed: bool,
}

pub(super) fn is_cloud_turn(body: &Value) -> bool {
    body.get("turnContext")
        .and_then(|turn| turn.get("agentRuntime"))
        .and_then(|runtime| runtime.get("runtime"))
        .and_then(Value::as_str)
        == Some("cloud")
}

/// `Ok(None)` is not a cloud turn. `Err` refuses the turn before any create or spawn.
pub(super) fn admit(body: &Value) -> Result<Option<CloudAdmission>, PlaneError> {
    if !is_cloud_turn(body) {
        return Ok(None);
    }
    let turn = body.get("turnContext").and_then(Value::as_object);
    let binding = turn
        .and_then(|turn| turn.get("cloudBinding"))
        .and_then(Value::as_object);
    let package_id = text_field(binding, "packageId");
    let digest = text_field(binding, "digest");
    let agent_asset_id = text_field(binding, "agentAssetId");
    let agent_asset_release_id = text_field(binding, "agentAssetReleaseId");
    let slug = turn
        .and_then(|turn| turn.get("agentId"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let conversation_id = turn
        .and_then(|turn| turn.get("conversationId"))
        .and_then(Value::as_str)
        .or_else(|| body.get("conversationId").and_then(Value::as_str))
        .unwrap_or("");
    if !agent_asset_id.is_empty() && (agent_asset_id == slug || agent_asset_id == package_id) {
        return Err(PlaneError::bad(SLUG_ASSET));
    }
    if !package_id_ok(package_id)
        || !digest_ok(digest)
        || !asset_id_ok(agent_asset_id)
        || !asset_id_ok(agent_asset_release_id)
        || !asset_id_ok(conversation_id)
    {
        return Err(PlaneError::bad(UNBOUND));
    }
    let prompt = user_prompt(body);
    let parameters = turn
        .and_then(|turn| turn.get("parameters"))
        .filter(|value| value.is_object())
        .cloned();
    let model_alias = turn
        .and_then(|turn| turn.get("modelAlias"))
        .and_then(|value| value.as_str())
        .filter(|alias| !alias.trim().is_empty())
        .map(str::to_owned);
    let idempotency_key = idempotency_key(
        conversation_id,
        agent_asset_id,
        agent_asset_release_id,
        package_id,
        digest,
        &prompt,
        parameters.as_ref(),
    );
    Ok(Some(CloudAdmission {
        package_id: package_id.to_owned(),
        digest: digest.to_owned(),
        agent_asset_id: agent_asset_id.to_owned(),
        agent_asset_release_id: agent_asset_release_id.to_owned(),
        conversation_id: conversation_id.to_owned(),
        prompt,
        model_alias,
        parameters,
        idempotency_key,
    }))
}

pub(super) async fn create_projection(
    base: &str,
    admission: &CloudAdmission,
    bearer: Option<&str>,
) -> Result<CloudProjection, String> {
    let mut input = json!({
        "packageId": admission.package_id,
        "digest": admission.digest,
        "prompt": admission.prompt,
    });
    if let Some(parameters) = &admission.parameters {
        input["parameters"] = parameters.clone();
    }
    if let Some(alias) = &admission.model_alias {
        input["modelAlias"] = json!(alias);
    }
    let body = json!({
        "agentAssetId": admission.agent_asset_id,
        "agentAssetReleaseId": admission.agent_asset_release_id,
        "input": input,
    });
    let path = format!(
        "/agent-conversations/{}/agent-runtimes",
        admission.conversation_id
    );
    let payload = serde_json::to_vec(&body).map_err(|error| error.to_string())?;
    let (status, bytes) = exchange(
        base,
        "POST",
        &path,
        Some(&admission.idempotency_key),
        bearer,
        &payload,
    )
    .await?;
    if !(200..300).contains(&status) {
        return Err(format!(
            "The cloud agent runtime refused the run ({status})."
        ));
    }
    let value: Value = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    let value = body_document(&value)?;
    let agent_execution_id = value
        .get("projection")
        .and_then(|projection| projection.get("agentExecutionId"))
        .and_then(Value::as_str)
        .unwrap_or("");
    if !asset_id_ok(agent_execution_id) {
        return Err(NO_RUN.to_owned());
    }
    Ok(CloudProjection {
        agent_execution_id: agent_execution_id.to_owned(),
        replayed: value
            .get("replayed")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

pub(super) async fn read_terminal_text(
    base: &str,
    execution_id: &str,
    bearer: Option<&str>,
) -> Result<String, String> {
    read_terminal_text_with_liveness(base, execution_id, bearer, None).await
}

/// Resilient terminal-text read: polls log pages for up to LONG_POLL_LIMIT
/// cycles (~5 minutes), checking the run's liveness via the projection-get
/// endpoint on each no-progress cycle. A run that the Cloud reports as
/// terminal exits immediately even without a terminal log event.
pub(super) async fn read_terminal_text_with_liveness(
    base: &str,
    execution_id: &str,
    bearer: Option<&str>,
    _liveness: Option<&str>,
) -> Result<String, String> {
    if !asset_id_ok(execution_id) {
        return Err(NO_RUN.to_owned());
    }
    let mut cursor = None;
    let mut text = String::new();
    let mut seen = HashSet::new();
    let mut no_progress = 0usize;
    for _ in 0..LONG_POLL_LIMIT {
        let path = logs_path(execution_id, cursor.as_deref());
        let (status, bytes) = exchange(base, "GET", &path, None, bearer, b"").await?;
        if !(200..300).contains(&status) {
            return Err(format!(
                "The cloud agent runtime returned no text ({status})."
            ));
        }
        let value: Value = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
        let page = body_document(&value)?;
        let fold = fold_page(page, &mut seen)?;
        text.push_str(&fold.text);
        match fold.terminal {
            Some(Terminal::Completed) => {
                if text.is_empty() {
                    return Err(NO_TEXT.to_owned());
                }
                return Ok(text);
            }
            Some(Terminal::Failed(reason)) => return Err(reason),
            None => match fold.next {
                Some(next) => {
                    if cursor.as_deref() == Some(next.as_str()) {
                        no_progress += 1;
                        if no_progress >= LONG_POLL_LIMIT {
                            return Err(NO_TEXT.to_owned());
                        }
                        tokio::time::sleep(POLL_WAIT).await;
                    } else {
                        no_progress = 0;
                        cursor = Some(next);
                    }
                }
                None => {
                    no_progress += 1;
                    // Check liveness via the projection-get endpoint every
                    // 15 no-progress cycles (~3 seconds); a terminal run
                    // that lost its terminal event still exits cleanly.
                    if no_progress % 15 == 0 {
                        let get_path = format!("/agent-runtimes/{execution_id}");
                        if let Ok((get_status, get_bytes)) =
                            exchange(base, "GET", &get_path, None, bearer, b"").await
                        {
                            if (200..300).contains(&get_status) {
                                if let Ok(get_value) = serde_json::from_slice::<Value>(&get_bytes) {
                                    if let Some(run_status) = get_value
                                        .get("data")
                                        .and_then(|d| d.get("status"))
                                        .and_then(Value::as_str)
                                    {
                                        if run_status == "completed"
                                            || run_status == "failed"
                                            || run_status == "cancelled"
                                        {
                                            if text.is_empty() {
                                                return Err(format!(
                                                    "The cloud agent run is {run_status} but produced no text."
                                                ));
                                            }
                                            return Ok(text);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    tokio::time::sleep(POLL_WAIT).await;
                }
            },
        }
    }
    Err(NO_TEXT.to_owned())
}

pub(super) async fn stop_projection(
    base: &str,
    execution_id: &str,
    idempotency_key: &str,
    bearer: Option<&str>,
) -> Result<(), String> {
    if !asset_id_ok(execution_id) || idempotency_key.is_empty() {
        return Err(NO_RUN.to_owned());
    }
    let path = format!("/agent-runtimes/{execution_id}/stop");
    let (status, _) = exchange(base, "POST", &path, Some(idempotency_key), bearer, b"{}").await?;
    if !(200..300).contains(&status) {
        return Err(format!(
            "The cloud agent runtime refused the stop ({status})."
        ));
    }
    Ok(())
}

/// One create and, unless the projection was replayed, one log read. Does not stop.
pub(super) async fn terminal_text(
    base: &str,
    admission: &CloudAdmission,
    bearer: Option<&str>,
) -> Result<String, String> {
    let created = create_projection(base, admission, bearer).await?;
    if created.replayed {
        return Err(REPLAYED.to_owned());
    }
    read_terminal_text(base, &created.agent_execution_id, bearer).await
}

pub(super) fn stop_key(idempotency_key: &str) -> String {
    format!("{idempotency_key}:stop")
}

struct PageFold {
    text: String,
    terminal: Option<Terminal>,
    next: Option<String>,
}

enum Terminal {
    Completed,
    Failed(String),
}

fn body_document(value: &Value) -> Result<&Value, String> {
    match value.get("success") {
        Some(Value::Bool(false)) => {
            let message = value
                .get("message")
                .and_then(Value::as_str)
                .filter(|message| !message.is_empty())
                .unwrap_or("The cloud agent runtime refused the run.");
            Err(message.to_owned())
        }
        Some(Value::Bool(true)) => value
            .get("data")
            .filter(|data| data.is_object())
            .ok_or_else(|| NO_TEXT.to_owned()),
        _ => Ok(value),
    }
}

fn logs_path(execution_id: &str, cursor: Option<&str>) -> String {
    let mut path = format!("/agent-runtimes/{execution_id}/logs");
    if let Some(cursor) = cursor {
        path.push_str("?cursor=");
        path.push_str(&query_encode(cursor));
    }
    path
}

fn query_encode(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// Trajectory page `{executionId, records, nextCursor}`, or the same object under `data`.
fn fold_page(page: &Value, seen: &mut HashSet<String>) -> Result<PageFold, String> {
    let Some(records) = page.get("records").and_then(Value::as_array) else {
        return legacy_page(page);
    };
    let mut text = String::new();
    let mut terminal = None;
    for record in records {
        if terminal.is_some() {
            break;
        }
        if !seen.insert(record_key(record)) {
            continue;
        }
        match record.get("kind").and_then(Value::as_str) {
            Some("model_output") => {
                if let Some(piece) = record
                    .get("content")
                    .and_then(|content| content.get("text"))
                    .and_then(Value::as_str)
                {
                    text.push_str(piece);
                }
            }
            Some("execution_completed") => terminal = Some(Terminal::Completed),
            Some("execution_failed") => {
                let reason = record
                    .get("content")
                    .and_then(|content| content.get("reason"))
                    .and_then(Value::as_str)
                    .filter(|reason| !reason.is_empty())
                    .unwrap_or("The cloud agent runtime failed.");
                terminal = Some(Terminal::Failed(reason.to_owned()));
            }
            _ => {}
        }
    }
    let next = page
        .get("nextCursor")
        .and_then(Value::as_str)
        .filter(|cursor| !cursor.is_empty())
        .map(str::to_owned);
    Ok(PageFold {
        text,
        terminal,
        next,
    })
}

fn record_key(record: &Value) -> String {
    if let Some(sequence) = record.get("sequence").and_then(json_sequence) {
        return format!("sequence:{sequence}");
    }
    let kind = record.get("kind").and_then(Value::as_str).unwrap_or("");
    let content = record.get("content");
    let text = content
        .and_then(|content| content.get("text"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let reason = content
        .and_then(|content| content.get("reason"))
        .and_then(Value::as_str)
        .unwrap_or("");
    format!("once:{kind}:{text}:{reason}")
}

fn json_sequence(value: &Value) -> Option<u64> {
    value.as_u64().or_else(|| {
        value
            .as_i64()
            .filter(|number| *number >= 0)
            .map(|number| number as u64)
    })
}

fn legacy_page(page: &Value) -> Result<PageFold, String> {
    if let Some(text) = page.get("text").and_then(Value::as_str) {
        if !text.is_empty() {
            return Ok(PageFold {
                text: text.to_owned(),
                terminal: Some(Terminal::Completed),
                next: None,
            });
        }
    }
    let events = page
        .get("events")
        .and_then(Value::as_array)
        .ok_or_else(|| NO_TEXT.to_owned())?;
    let mut text = String::new();
    let mut terminal = None;
    for event in events {
        if terminal.is_some() {
            break;
        }
        match event.get("type").and_then(Value::as_str) {
            Some("text_delta") => {
                if let Some(delta) = event.get("text").and_then(Value::as_str) {
                    text.push_str(delta);
                }
            }
            Some("output") => {
                if let Some(whole) = event.get("text").and_then(Value::as_str) {
                    text = whole.to_owned();
                }
            }
            Some("failed") => {
                let message = event
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("The cloud agent runtime failed.");
                terminal = Some(Terminal::Failed(message.to_owned()));
            }
            Some("completed") => terminal = Some(Terminal::Completed),
            _ => {}
        }
    }
    Ok(PageFold {
        text,
        terminal,
        next: None,
    })
}

fn text_field<'a>(object: Option<&'a serde_json::Map<String, Value>>, key: &str) -> &'a str {
    object
        .and_then(|object| object.get(key))
        .and_then(Value::as_str)
        .unwrap_or("")
}

fn user_prompt(body: &Value) -> String {
    body.get("userMessageParts")
        .and_then(Value::as_array)
        .and_then(|parts| {
            parts
                .iter()
                .find_map(|part| part.get("text").and_then(Value::as_str))
        })
        .unwrap_or("")
        .to_owned()
}

fn idempotency_key(
    conversation_id: &str,
    agent_asset_id: &str,
    agent_asset_release_id: &str,
    package_id: &str,
    digest: &str,
    prompt: &str,
    parameters: Option<&Value>,
) -> String {
    let mut hasher = Sha256::new();
    for part in [
        conversation_id,
        agent_asset_id,
        agent_asset_release_id,
        package_id,
        digest,
        prompt,
    ] {
        feed(&mut hasher, part);
    }
    if let Some(parameters) = parameters {
        feed(&mut hasher, &parameters.to_string());
    }
    format!("{:x}", hasher.finalize())
}

fn feed(hasher: &mut Sha256, value: &str) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value.as_bytes());
}

fn package_id_ok(value: &str) -> bool {
    let mut parts = value.split('/');
    let Some("use") = parts.next() else {
        return false;
    };
    let Some(publisher) = parts.next() else {
        return false;
    };
    let Some(name) = parts.next() else {
        return false;
    };
    parts.next().is_none() && segment_ok(publisher) && segment_ok(name)
}

fn segment_ok(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_lowercase() || first.is_ascii_digit())
        && chars.all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        })
}

fn digest_ok(value: &str) -> bool {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return false;
    };
    hex.len() == 64
        && hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn asset_id_ok(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 36 {
        return false;
    }
    for (index, byte) in bytes.iter().enumerate() {
        if matches!(index, 8 | 13 | 18 | 23) {
            if *byte != b'-' {
                return false;
            }
            continue;
        }
        if !byte.is_ascii_hexdigit() {
            return false;
        }
    }
    matches!(bytes[14], b'1'..=b'8')
        && matches!(bytes[19].to_ascii_lowercase(), b'8' | b'9' | b'a' | b'b')
}

async fn exchange(
    base: &str,
    method: &str,
    path: &str,
    idempotency_key: Option<&str>,
    bearer: Option<&str>,
    body: &[u8],
) -> Result<(u16, Vec<u8>), String> {
    let (address, route) = endpoint(base, path)?;
    let mut request = format!(
        "{method} {route} HTTP/1.1\r\nHost: {host}\r\nContent-Type: application/json\r\nContent-Length: {length}\r\nConnection: close\r\n",
        host = address,
        length = body.len(),
    );
    if let Some(token) = bearer {
        request.push_str("Authorization: Bearer ");
        request.push_str(token);
        request.push_str("\r\n");
    }
    if let Some(key) = idempotency_key {
        request.push_str("idempotency-key: ");
        request.push_str(key);
        request.push_str("\r\n");
    }
    request.push_str("\r\n");
    let exchange = async {
        let mut stream = TcpStream::connect(address)
            .await
            .map_err(|error| error.to_string())?;
        stream
            .write_all(request.as_bytes())
            .await
            .map_err(|error| error.to_string())?;
        stream
            .write_all(body)
            .await
            .map_err(|error| error.to_string())?;
        let _ = stream.flush().await;
        read_response(&mut stream).await
    };
    match tokio::time::timeout(Duration::from_secs(20), exchange).await {
        Ok(result) => result,
        Err(_) => Err("The cloud agent runtime did not answer.".to_owned()),
    }
}

fn endpoint(base: &str, path: &str) -> Result<(SocketAddr, String), String> {
    let trimmed = base.trim().trim_end_matches('/');
    let Some(rest) = trimmed.strip_prefix("http://") else {
        return Err(ENDPOINT_MISSING.to_owned());
    };
    if rest.is_empty() || rest.contains('@') {
        return Err(ENDPOINT_MISSING.to_owned());
    }
    let (authority, prefix) = match rest.split_once('/') {
        Some((authority, prefix)) => (authority, format!("/{prefix}")),
        None => (rest, String::new()),
    };
    let address = loopback_addr(authority)?;
    if path.is_empty() || !path.starts_with('/') || path.contains("://") {
        return Err(ENDPOINT_MISSING.to_owned());
    }
    Ok((address, format!("{prefix}{path}")))
}

fn loopback_addr(authority: &str) -> Result<SocketAddr, String> {
    let (host, port) = authority
        .rsplit_once(':')
        .ok_or_else(|| ENDPOINT_MISSING.to_owned())?;
    if host != "127.0.0.1" && host != "localhost" {
        return Err(ENDPOINT_MISSING.to_owned());
    }
    let port = port
        .parse::<u16>()
        .map_err(|_| ENDPOINT_MISSING.to_owned())?;
    if port == 0 {
        return Err(ENDPOINT_MISSING.to_owned());
    }
    Ok(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port))
}

async fn read_response(stream: &mut TcpStream) -> Result<(u16, Vec<u8>), String> {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 8192];
    let header_end = loop {
        if buf.len() > 64 * 1024 {
            return Err("The cloud agent runtime returned no text.".to_owned());
        }
        let read = stream
            .read(&mut tmp)
            .await
            .map_err(|error| error.to_string())?;
        if read == 0 {
            return Err("The cloud agent runtime returned no text.".to_owned());
        }
        buf.extend_from_slice(&tmp[..read]);
        if let Some(index) = buf.windows(4).position(|window| window == b"\r\n\r\n") {
            break index;
        }
    };
    let header = String::from_utf8_lossy(&buf[..header_end]).into_owned();
    let status = header
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|status| status.parse::<u16>().ok())
        .ok_or_else(|| "The cloud agent runtime returned no status.".to_owned())?;
    let mut body = buf[header_end + 4..].to_vec();
    if header_is(&header, "transfer-encoding", "chunked") {
        while !body.windows(5).any(|window| window == b"0\r\n\r\n") && body.len() < 1024 * 1024 {
            let read = stream
                .read(&mut tmp)
                .await
                .map_err(|error| error.to_string())?;
            if read == 0 {
                break;
            }
            body.extend_from_slice(&tmp[..read]);
        }
        body = decode_chunked(&body)?;
    } else if let Some(length) = content_length(&header) {
        while body.len() < length {
            let read = stream
                .read(&mut tmp)
                .await
                .map_err(|error| error.to_string())?;
            if read == 0 {
                break;
            }
            body.extend_from_slice(&tmp[..read]);
            if body.len() > 1024 * 1024 {
                return Err(NO_TEXT.to_owned());
            }
        }
        body.truncate(length);
    } else {
        while body.len() < 1024 * 1024 {
            let read = stream
                .read(&mut tmp)
                .await
                .map_err(|error| error.to_string())?;
            if read == 0 {
                break;
            }
            body.extend_from_slice(&tmp[..read]);
        }
    }
    Ok((status, body))
}

fn content_length(header: &str) -> Option<usize> {
    header_value(header, "content-length")?.parse().ok()
}

fn header_is(header: &str, name: &str, expected: &str) -> bool {
    header_value(header, name).is_some_and(|value| value.eq_ignore_ascii_case(expected))
}

fn header_value<'a>(header: &'a str, name: &str) -> Option<&'a str> {
    header.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        key.eq_ignore_ascii_case(name).then(|| value.trim())
    })
}

fn decode_chunked(mut body: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    loop {
        let line_end = body
            .windows(2)
            .position(|window| window == b"\r\n")
            .ok_or_else(|| NO_TEXT.to_owned())?;
        let size_line =
            std::str::from_utf8(&body[..line_end]).map_err(|error| error.to_string())?;
        let size = usize::from_str_radix(size_line.split(';').next().unwrap_or("").trim(), 16)
            .map_err(|_| NO_TEXT.to_owned())?;
        body = &body[line_end + 2..];
        if size == 0 {
            break;
        }
        if body.len() < size + 2 || out.len() + size > 1024 * 1024 {
            return Err(NO_TEXT.to_owned());
        }
        out.extend_from_slice(&body[..size]);
        body = &body[size + 2..];
    }
    Ok(out)
}

/// One SSE event parsed from the member-plane stream.
pub(super) struct SseEvent {
    pub data: String,
}

/// Read SSE events from the member-plane agent-conversation stream.
/// Yields parsed `data:` payloads — the Cloud emits agent execution events
/// as JSON objects, one per SSE event.
pub(super) async fn stream_agent_events(
    base: &str,
    conversation_id: &str,
    bearer: Option<&str>,
    cursor: Option<u64>,
) -> Result<tokio::sync::mpsc::Receiver<SseEvent>, String> {
    use tokio::io::{AsyncBufReadExt, BufReader};

    let path = format!(
        "/agent-conversations/{conversation_id}/events/stream?limit=64{}",
        cursor.map(|c| format!("&cursor={c}")).unwrap_or_default()
    );
    let (address, route) = endpoint(base, &path)?;
    let mut request = format!(
        "GET {route} HTTP/1.1\r\nHost: {host}\r\nAccept: text/event-stream\r\nCache-Control: no-cache\r\nConnection: keep-alive\r\n",
        host = address,
    );
    if let Some(token) = bearer {
        request.push_str("Authorization: Bearer ");
        request.push_str(token);
        request.push_str("\r\n");
    }
    request.push_str("\r\n");

    let mut stream = TcpStream::connect(&address)
        .await
        .map_err(|error| format!("SSE connect failed: {error}"))?;
    stream
        .write_all(request.as_bytes())
        .await
        .map_err(|error| format!("SSE send failed: {error}"))?;

    let reader = BufReader::new(stream);
    let (tx, rx) = tokio::sync::mpsc::channel(64);

    tokio::spawn(async move {
        let mut lines = reader.lines();
        let mut in_data = false;
        let mut data = String::new();
        // Skip HTTP headers until empty line
        let mut headers_done = false;
        while let Ok(Some(line)) = lines.next_line().await {
            if !headers_done {
                if line.is_empty() {
                    headers_done = true;
                }
                continue;
            }
            if line.starts_with("data: ") {
                in_data = true;
                data.push_str(&line[6..]);
            } else if line.starts_with("event: ")
                || line.starts_with("id: ")
                || line.starts_with("retry: ")
            {
                // metadata line — ignore
            } else if line.is_empty() {
                if in_data && !data.is_empty() {
                    let _ = tx
                        .send(SseEvent {
                            data: std::mem::take(&mut data),
                        })
                        .await;
                }
                in_data = false;
                data.clear();
            }
        }
    });

    Ok(rx)
}
