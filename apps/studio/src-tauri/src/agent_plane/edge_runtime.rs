//! One edge agent run. Builtin listens on loopback and speaks the A3S Code
//! command and event-page paths. A package harness is the declared entry
//! process. Stopping the run closes the listener or kills the process.

use std::io::ErrorKind;
use std::net::SocketAddr;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::process::{Child, Command};
use tokio::sync::{Mutex, oneshot};
use tokio::task::JoinHandle;

const START_SCHEMA: &str = "a3s.code.agent-run-start.v1";
const IDENTITY_SCHEMA: &str = "a3s.code.agent-run-identity.v1";
const PROTOCOL: &str = "a3s.code.agent.v1";
const LISTEN_LIMIT: Duration = Duration::from_secs(2);
const REQUEST_LIMIT: Duration = Duration::from_secs(2);
const PACKAGE_TURN_LIMIT: Duration = Duration::from_secs(5);

pub const BUILTIN_UNAVAILABLE: &str = "a builtin harness is not available";
pub const LIMITS_MISSING: &str = "edge package resources are missing";
pub const PACKAGE_TIMED_OUT: &str = "the package agent timed out";

/// Declared cpu and memory for one removed a3s-box.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EdgeResources {
    pub cpu_millis: u64,
    pub memory_bytes: u64,
}

#[derive(Clone)]
struct PeerState {
    events: Arc<Mutex<Vec<Value>>>,
}

pub struct BuiltinPeer {
    addr: SocketAddr,
    shutdown: Option<oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<()>,
}

impl BuiltinPeer {
    pub async fn start() -> Result<Self, String> {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|error| error.to_string())?;
        let addr = listener.local_addr().map_err(|error| error.to_string())?;
        let (shutdown, signal) = oneshot::channel();
        let state = PeerState {
            events: Arc::new(Mutex::new(Vec::new())),
        };
        let router = axum::Router::new()
            .route("/v1/agent/commands", axum::routing::post(admit_command))
            .route("/v1/agent/events:page", axum::routing::post(page_events))
            .with_state(state);
        let task = tokio::spawn(async move {
            let _ = axum::serve(listener, router)
                .with_graceful_shutdown(async move {
                    let _ = signal.await;
                })
                .await;
        });
        Ok(Self {
            addr,
            shutdown: Some(shutdown),
            task,
        })
    }

    pub fn endpoint(&self) -> String {
        format!("http://{}", self.addr)
    }

    pub async fn stop(mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        let _ = self.task.await;
    }
}

async fn admit_command(
    axum::extract::State(state): axum::extract::State<PeerState>,
    axum::Json(body): axum::Json<Value>,
) -> Result<axum::Json<Value>, axum::http::StatusCode> {
    let prompt = admitted_prompt(&body).map_err(|_| axum::http::StatusCode::BAD_REQUEST)?;
    let mut events = state.events.lock().await;
    if events.is_empty() {
        events.push(json!({ "sequence": 1, "type": "text_delta", "text": prompt }));
        events.push(json!({ "sequence": 2, "type": "completed" }));
    }
    Ok(axum::Json(json!({ "accepted": true })))
}

async fn page_events(
    axum::extract::State(state): axum::extract::State<PeerState>,
    axum::Json(body): axum::Json<Value>,
) -> axum::Json<Value> {
    let after = body.get("after").and_then(Value::as_u64).unwrap_or(0);
    let events = state.events.lock().await;
    let page: Vec<&Value> = events
        .iter()
        .filter(|event| event.get("sequence").and_then(Value::as_u64).unwrap_or(0) > after)
        .collect();
    axum::Json(json!({ "events": page }))
}

fn admitted_prompt(body: &Value) -> Result<String, ()> {
    let object = body.as_object().ok_or(())?;
    if object.len() != 4 {
        return Err(());
    }
    if body.get("schema").and_then(Value::as_str) != Some(START_SCHEMA) {
        return Err(());
    }
    let request_id = body.get("request_id").and_then(Value::as_str).ok_or(())?;
    if request_id.is_empty() {
        return Err(());
    }
    let identity = body.get("identity").ok_or(())?;
    let identity_object = identity.as_object().ok_or(())?;
    if identity_object.len() != 5 {
        return Err(());
    }
    if identity.get("schema").and_then(Value::as_str) != Some(IDENTITY_SCHEMA) {
        return Err(());
    }
    if identity.get("protocol").and_then(Value::as_str) != Some(PROTOCOL) {
        return Err(());
    }
    let release = identity
        .get("agent_release_identity")
        .and_then(Value::as_str)
        .ok_or(())?;
    let Some(hex) = release.strip_prefix("sha256:") else {
        return Err(());
    };
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(());
    }
    for key in ["session_id", "run_id"] {
        let value = identity.get(key).and_then(Value::as_str).ok_or(())?;
        if value.is_empty() {
            return Err(());
        }
    }
    let prompt = body.get("prompt").and_then(Value::as_str).ok_or(())?;
    if prompt.trim().is_empty() {
        return Err(());
    }
    Ok(prompt.to_owned())
}

/// The declared relative entry must be the trailing path of the published file.
pub fn declared_entry_matches(installed: &Path, declared: &str) -> bool {
    let declared_path = Path::new(declared);
    if declared.is_empty() || declared_path.is_absolute() {
        return false;
    }
    let declared_components: Vec<_> = declared_path.components().collect();
    if declared_components.is_empty()
        || declared_components
            .iter()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return false;
    }
    let installed_components: Vec<_> = installed.components().collect();
    if declared_components.len() > installed_components.len() {
        return false;
    }
    installed_components[installed_components.len() - declared_components.len()..]
        == declared_components[..]
}

pub struct PackageRun {
    child: Child,
    endpoint: String,
    stderr: Option<JoinHandle<Vec<u8>>>,
}

impl PackageRun {
    pub async fn spawn(program: &Path, args: &[String]) -> Result<Self, String> {
        let mut child = Command::new(program)
            .args(args)
            .env("A3S_AGENT_PROTOCOL", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|error| error.to_string())?;
        let stderr_pipe = child
            .stderr
            .take()
            .ok_or_else(|| "entry process has no stderr".to_owned())?;
        let stderr = tokio::spawn(async move {
            let mut buf = Vec::new();
            let mut reader = BufReader::new(stderr_pipe);
            let _ = reader.read_to_end(&mut buf).await;
            buf
        });
        let stdout = match child.stdout.take() {
            Some(stdout) => stdout,
            None => {
                stderr.abort();
                return Err("entry process has no stdout".into());
            }
        };
        let mut lines = BufReader::new(stdout).lines();
        let line = match tokio::time::timeout(LISTEN_LIMIT, lines.next_line()).await {
            Ok(Ok(Some(line))) => line,
            Ok(Ok(None)) => {
                return Err(
                    listen_error(child, stderr, "entry process ended before listening").await,
                );
            }
            Ok(Err(error)) => {
                return Err(listen_error(child, stderr, &error.to_string()).await);
            }
            Err(_) => {
                return Err(listen_error(child, stderr, "entry process did not listen").await);
            }
        };
        let port = match line
            .strip_prefix("port ")
            .and_then(|port| port.parse::<u16>().ok())
        {
            Some(port) => port,
            None => {
                return Err(
                    listen_error(child, stderr, "entry process did not announce a port").await,
                );
            }
        };
        Ok(Self {
            child,
            endpoint: format!("http://127.0.0.1:{port}"),
            stderr: Some(stderr),
        })
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    pub async fn stop(self) -> Result<(), String> {
        self.finish().await.map(|_| ())
    }

    /// Kill the child. Stderr is diagnostic text for a failed run.
    async fn finish(mut self) -> Result<String, String> {
        let kill = self.child.kill().await.map_err(|error| error.to_string());
        let _ = self.child.wait().await;
        let stderr = match self.stderr.take() {
            Some(task) => stderr_text(task).await,
            None => String::new(),
        };
        kill?;
        Ok(stderr)
    }
}

async fn listen_error(child: Child, stderr: JoinHandle<Vec<u8>>, error: &str) -> String {
    drop(child);
    with_diagnostic(error.to_owned(), &stderr_text(stderr).await)
}

async fn stderr_text(task: JoinHandle<Vec<u8>>) -> String {
    let bytes = match tokio::time::timeout(Duration::from_secs(1), task).await {
        Ok(Ok(bytes)) => bytes,
        _ => Vec::new(),
    };
    bounded_text(&bytes)
}

fn bounded_text(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let trimmed = text.trim();
    trimmed.chars().take(400).collect()
}

fn with_diagnostic(error: String, stderr: &str) -> String {
    let stderr = stderr.trim();
    if stderr.is_empty() {
        error
    } else {
        format!("{error}: {stderr}")
    }
}

pub async fn post_json(endpoint: &str, path: &str, body: &Value) -> Result<Value, String> {
    post_json_within(endpoint, path, body, REQUEST_LIMIT).await
}

/// One HTTP post. `read_limit` bounds the whole exchange. A socket failure is
/// named by `ErrorKind`, so Windows and Unix retries see the same phrase.
pub(super) async fn post_json_within(
    endpoint: &str,
    path: &str,
    body: &Value,
    read_limit: Duration,
) -> Result<Value, String> {
    if read_limit.is_zero() {
        return Err(PACKAGE_TIMED_OUT.into());
    }
    match tokio::time::timeout(read_limit, post_once(endpoint, path, body)).await {
        Ok(result) => result,
        Err(_) => Err(PACKAGE_TIMED_OUT.into()),
    }
}

async fn post_once(endpoint: &str, path: &str, body: &Value) -> Result<Value, String> {
    let addr: SocketAddr = endpoint
        .trim_start_matches("http://")
        .parse()
        .map_err(|error: std::net::AddrParseError| error.to_string())?;
    let mut stream = TcpStream::connect(addr).await.map_err(io_failure)?;
    let payload = body.to_string();
    let request = format!(
        "POST {path} HTTP/1.1\r\nHost: {addr}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
        payload.len()
    );
    stream
        .write_all(request.as_bytes())
        .await
        .map_err(io_failure)?;
    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .await
        .map_err(io_failure)?;
    let text = String::from_utf8_lossy(&response);
    let status = text
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .ok_or_else(|| "agent runtime returned no status".to_owned())?;
    if status != "200" {
        return Err(format!("agent runtime refused the command ({status})"));
    }
    let json_at = text
        .find("\r\n\r\n")
        .map(|index| index + 4)
        .ok_or_else(|| "agent runtime returned no body".to_owned())?;
    serde_json::from_str(&text[json_at..]).map_err(|error| error.to_string())
}

/// `ConnectionAborted` is the Windows form of a reset before a status line.
pub(super) fn io_failure(error: std::io::Error) -> String {
    let phrase = match error.kind() {
        ErrorKind::ConnectionRefused => "connection refused",
        ErrorKind::ConnectionReset | ErrorKind::ConnectionAborted => "connection reset",
        ErrorKind::BrokenPipe => "broken pipe",
        ErrorKind::UnexpectedEof => "agent runtime returned no status",
        _ => return error.to_string(),
    };
    format!("{phrase}: {error}")
}

/// Builtin has no harness yet. The call does not echo the prompt and does not start a listener.
pub async fn builtin_turn_text(_prompt: &str) -> Result<Vec<String>, String> {
    Err(BUILTIN_UNAVAILABLE.into())
}

/// Positive cpu and a whole number of mebibytes. a3s-box applies both caps.
pub fn require_enforceable_limits(runtime: &Value) -> Result<EdgeResources, String> {
    let Some(resources) = runtime.get("resources").and_then(Value::as_object) else {
        return Err(LIMITS_MISSING.into());
    };
    let cpu = resources.get("cpuMillis").and_then(Value::as_u64);
    let memory = resources.get("memoryBytes").and_then(Value::as_u64);
    let (Some(cpu_millis), Some(memory_bytes)) = (cpu, memory) else {
        return Err(LIMITS_MISSING.into());
    };
    if cpu_millis == 0 || memory_bytes == 0 {
        return Err(LIMITS_MISSING.into());
    }
    super::edge_box::validate_resources(cpu_millis, memory_bytes)?;
    Ok(EdgeResources {
        cpu_millis,
        memory_bytes,
    })
}

pub(super) async fn exchange_turn(endpoint: &str, body: &Value) -> Result<Vec<String>, String> {
    post_json(endpoint, "/v1/agent/commands", body).await?;
    let page = post_json(endpoint, "/v1/agent/events:page", &json!({ "after": 0 })).await?;
    text_of_page(&page)
}

pub async fn package_turn_text(entry: &Path, body: &Value) -> Result<Vec<String>, String> {
    match entry.extension().and_then(|extension| extension.to_str()) {
        Some("mjs" | "js" | "cjs") => {}
        _ => return Err("package entry is not a node program".to_owned()),
    }
    let node = Path::new("node");
    let run = PackageRun::spawn(node, &[entry.to_string_lossy().into_owned()]).await?;
    let endpoint = run.endpoint().to_owned();
    let outcome = tokio::time::timeout(PACKAGE_TURN_LIMIT, exchange_turn(&endpoint, body)).await;
    let stderr = run.finish().await.unwrap_or_default();
    match outcome {
        Ok(Ok(deltas)) => Ok(deltas),
        Ok(Err(error)) => Err(with_diagnostic(error, &stderr)),
        Err(_) => Err(with_diagnostic(PACKAGE_TIMED_OUT.to_owned(), &stderr)),
    }
}

pub(super) fn text_of_page(page: &Value) -> Result<Vec<String>, String> {
    let events = page
        .get("events")
        .and_then(Value::as_array)
        .ok_or_else(|| "agent runtime returned no events".to_owned())?;
    let mut deltas = Vec::new();
    for event in events {
        match event.get("type").and_then(Value::as_str) {
            Some("text_delta") => {
                if let Some(delta) = event.get("text").and_then(Value::as_str) {
                    if !delta.is_empty() {
                        deltas.push(delta.to_owned());
                    }
                }
            }
            Some("output") => {
                if let Some(whole) = event.get("text").and_then(Value::as_str) {
                    if !whole.is_empty() {
                        deltas = vec![whole.to_owned()];
                    }
                }
            }
            Some("failed") => {
                let message = event
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("agent runtime failed");
                return Err(message.to_owned());
            }
            _ => {}
        }
    }
    if deltas.is_empty() {
        return Err("agent runtime returned no text".into());
    }
    Ok(deltas)
}

pub fn start_body(prompt: &str, session_id: &str, run_id: &str, release: &str) -> Value {
    json!({
        "schema": START_SCHEMA,
        "request_id": run_id,
        "identity": {
            "schema": IDENTITY_SCHEMA,
            "protocol": PROTOCOL,
            "agent_release_identity": release,
            "session_id": session_id,
            "run_id": run_id
        },
        "prompt": prompt
    })
}

/// One `tasks/send` against a credential-free HTTPS agent card. The card is the remote runtime.
pub(super) async fn remote_card_text(
    card: &str,
    prompt: &str,
    parameters: Option<&Value>,
) -> Result<String, String> {
    let url = url::Url::parse(card)
        .map_err(|_| "a remote agent requires a credential-free https card".to_owned())?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.host_str().is_none()
    {
        return Err("a remote agent requires a credential-free https card".into());
    }
    let mut parts = vec![json!({ "type": "text", "text": prompt })];
    if let Some(parameters) = parameters {
        parts.push(json!({ "type": "data", "data": parameters }));
    }
    let payload = json!({
        "jsonrpc": "2.0",
        "id": "parameter-form",
        "method": "tasks/send",
        "params": {
            "id": "parameter-form",
            "message": { "role": "user", "parts": parts }
        }
    });
    let body = serde_json::to_string(&payload)
        .map_err(|_| "remote agent request is invalid".to_owned())?;
    let output = Command::new("curl")
        .args([
            "--silent",
            "--show-error",
            "--max-time",
            "30",
            "-H",
            "content-type: application/json",
            "-d",
            &body,
            "--",
            card,
        ])
        .output()
        .await
        .map_err(|_| "the remote agent card was not reached".to_owned())?;
    if !output.status.success() {
        return Err("the remote agent refused the task".into());
    }
    let parsed: Value = serde_json::from_slice(&output.stdout)
        .map_err(|_| "the remote agent returned no text".to_owned())?;
    parsed
        .get("result")
        .and_then(|result| result.get("text"))
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| "the remote agent returned no text".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_RELEASE: &str =
        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    #[tokio::test]
    async fn a_builtin_edge_run_streams_and_the_listener_closes() {
        let peer = BuiltinPeer::start().await.expect("peer");
        let endpoint = peer.endpoint();
        let refused = post_json(
            &endpoint,
            "/v1/agent/commands",
            &json!({ "schema": START_SCHEMA, "prompt": "审查" }),
        )
        .await;
        assert!(refused.is_err(), "a start without identity must be refused");
        post_json(
            &endpoint,
            "/v1/agent/commands",
            &start_body("审查这份合同", "session-1", "run-1", TEST_RELEASE),
        )
        .await
        .expect("start");
        let page = post_json(&endpoint, "/v1/agent/events:page", &json!({ "after": 0 }))
            .await
            .expect("page");
        let events = page
            .get("events")
            .and_then(Value::as_array)
            .expect("events");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0]["text"], "审查这份合同");
        assert_eq!(events[1]["type"], "completed");
        peer.stop().await;
        let gone = post_json(&endpoint, "/v1/agent/events:page", &json!({ "after": 0 })).await;
        assert!(gone.is_err(), "the listener must be gone after stop");
    }

    #[test]
    fn a_declared_entry_must_be_the_published_suffix() {
        let installed = Path::new("/pkg/g/1/agent/weather/agent/run.mjs");
        assert!(declared_entry_matches(installed, "agent/run.mjs"));
        assert!(declared_entry_matches(installed, "run.mjs"));
        assert!(!declared_entry_matches(installed, ""));
        assert!(!declared_entry_matches(installed, "../run.mjs"));
        assert!(!declared_entry_matches(
            Path::new("/pkg/not-agent/run.mjs"),
            "agent/run.mjs"
        ));
    }

    #[tokio::test]
    async fn a_builtin_start_refuses_an_unknown_field() {
        let peer = BuiltinPeer::start().await.expect("peer");
        let mut body = start_body("审查", "session-1", "run-1", TEST_RELEASE);
        body.as_object_mut()
            .expect("object")
            .insert("parameters".to_owned(), json!({ "city": "上海" }));
        let refused = post_json(&peer.endpoint(), "/v1/agent/commands", &body).await;
        assert!(
            refused.is_err(),
            "a start with an unknown field must be refused"
        );
        peer.stop().await;
    }

    #[tokio::test]
    async fn a_package_entry_is_killed_when_the_run_stops() {
        let node = Path::new("node");
        if Command::new(node).arg("--version").output().await.is_err() {
            return;
        }
        let script = r#"
const http = require('http')
const server = http.createServer((req, res) => {
  res.setHeader('content-type', 'application/json')
  res.end(JSON.stringify({ events: [{ sequence: 1, type: 'output', text: 'ok' }, { sequence: 2, type: 'completed' }] }))
})
server.listen(0, '127.0.0.1', () => {
  process.stdout.write('port ' + server.address().port + '\n')
})
"#;
        let run = PackageRun::spawn(node, &["-e".to_owned(), script.to_owned()])
            .await
            .expect("entry");
        let endpoint = run.endpoint().to_owned();
        let page = post_json(&endpoint, "/v1/agent/events:page", &json!({ "after": 0 }))
            .await
            .expect("page");
        assert_eq!(page["events"][0]["text"], "ok");
        run.stop().await.expect("stop");
        let gone = post_json(&endpoint, "/v1/agent/events:page", &json!({ "after": 0 })).await;
        assert!(gone.is_err(), "the entry process must be gone after stop");
    }

    #[tokio::test]
    async fn an_entry_that_does_not_listen_leaves_no_run() {
        let node = Path::new("node");
        if Command::new(node).arg("--version").output().await.is_err() {
            return;
        }
        match PackageRun::spawn(node, &["-e".to_owned(), "process.exit(1)".to_owned()]).await {
            Ok(_) => panic!("an entry that exits must not become a run"),
            Err(error) => assert!(error.contains("entry process"), "{error}"),
        }
    }

    #[test]
    fn a_socket_kind_uses_a_stable_phrase() {
        let reset = io_failure(std::io::Error::new(
            ErrorKind::ConnectionReset,
            "An existing connection was forcibly closed by the remote host.",
        ));
        assert!(reset.starts_with("connection reset: "), "{reset}");
        let refused = io_failure(std::io::Error::new(
            ErrorKind::ConnectionRefused,
            "No connection could be made because the target machine actively refused it.",
        ));
        assert!(refused.starts_with("connection refused: "), "{refused}");
        let aborted = io_failure(std::io::Error::new(
            ErrorKind::ConnectionAborted,
            "An established connection was aborted by the software in your host machine.",
        ));
        assert!(aborted.starts_with("connection reset: "), "{aborted}");
        let other = io_failure(std::io::Error::new(ErrorKind::Other, "disk full"));
        assert_eq!(other, "disk full");
    }

    #[test]
    fn a_start_body_keeps_the_supplied_release_and_run_ids() {
        let release = format!("sha256:{}", "cd".repeat(32));
        let body = start_body("上海天气", "session-1", "run-1", &release);
        assert_eq!(body["identity"]["agent_release_identity"], release);
        assert_eq!(body["identity"]["session_id"], "session-1");
        assert_eq!(body["identity"]["run_id"], "run-1");
        assert_ne!(
            body["identity"]["agent_release_identity"],
            format!("sha256:{:0>64}", "ab")
        );
        assert_ne!(body["identity"]["session_id"], "run");
        assert_ne!(body["identity"]["run_id"], "run");
    }

    #[test]
    fn declared_limits_are_the_box_budget() {
        let missing = json!({ "runtime": "edge" });
        assert_eq!(
            require_enforceable_limits(&missing).unwrap_err(),
            LIMITS_MISSING
        );
        let zero = json!({
            "resources": { "cpuMillis": 0, "memoryBytes": 268435456 }
        });
        assert_eq!(
            require_enforceable_limits(&zero).unwrap_err(),
            LIMITS_MISSING
        );
        let declared = json!({
            "resources": { "cpuMillis": 500, "memoryBytes": 268435456 }
        });
        assert_eq!(
            require_enforceable_limits(&declared).expect("limits"),
            EdgeResources {
                cpu_millis: 500,
                memory_bytes: 268_435_456
            }
        );
        let odd = json!({
            "resources": { "cpuMillis": 500, "memoryBytes": 1000 }
        });
        assert_eq!(
            require_enforceable_limits(&odd).unwrap_err(),
            super::super::edge_box::MEMORY_NOT_WHOLE_MIB
        );
    }

    #[tokio::test]
    async fn a_builtin_turn_does_not_echo_the_prompt() {
        let error = builtin_turn_text("上海天气").await.unwrap_err();
        assert_eq!(error, BUILTIN_UNAVAILABLE);
        assert!(!error.contains("上海天气"));
    }

    #[tokio::test]
    async fn a_package_turn_returns_each_delta_and_keeps_stderr_out_of_the_text() {
        let node = Path::new("node");
        if Command::new(node).arg("--version").output().await.is_err() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("a3s-package-deltas-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let marker = dir.join("starts");
        let entry = dir.join("run.cjs");
        let marker_js = serde_json::to_string(&marker.to_string_lossy()).unwrap();
        std::fs::write(
            &entry,
            format!(
                "const http=require('http');const fs=require('fs');fs.appendFileSync({marker_js},'1\\n');\
const server=http.createServer((req,res)=>{{req.on('data',()=>{{}});req.on('end',()=>{{const payload=String(req.url).includes('events:page')?JSON.stringify({{events:[{{sequence:1,type:'text_delta',text:'宿主'}},{{sequence:2,type:'text_delta',text:'晴'}},{{sequence:3,type:'completed'}}]}}):JSON.stringify({{accepted:true}});res.writeHead(200,{{'content-type':'application/json','content-length':Buffer.byteLength(payload),'connection':'close'}});res.end(payload);}});}});\
server.listen(0,'127.0.0.1',()=>{{process.stderr.write('诊断\\n');process.stdout.write('port '+server.address().port+'\\n');}});\n"
            ),
        )
        .unwrap();
        let deltas = package_turn_text(&entry, &json!({ "ok": true }))
            .await
            .expect("package");
        assert_eq!(deltas, vec!["宿主".to_owned(), "晴".to_owned()]);
        assert!(!deltas.join("").contains("诊断"));
        assert_eq!(std::fs::read_to_string(&marker).unwrap().lines().count(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn a_package_turn_stops_when_the_entry_does_not_answer() {
        let node = Path::new("node");
        if Command::new(node).arg("--version").output().await.is_err() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("a3s-package-hang-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let entry = dir.join("hang.cjs");
        std::fs::write(
            &entry,
            "const http=require('http');const server=http.createServer(()=>{});\
server.listen(0,'127.0.0.1',()=>{process.stderr.write('诊断\\n');process.stdout.write('port '+server.address().port+'\\n');});\n",
        )
        .unwrap();
        let error = package_turn_text(&entry, &json!({ "ok": true }))
            .await
            .expect_err("hang");
        assert!(error.contains(PACKAGE_TIMED_OUT), "{error}");
        assert_ne!(error, "诊断");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
