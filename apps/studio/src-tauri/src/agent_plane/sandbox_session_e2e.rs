//! Desktop `AcpClient::spawn` of the real `a3s-code-acp`, one scripted bash,
//! with no companion directory. ToWork's spawn writes gateway env, so the
//! refusal is checked here too. The HVF success cases stay on AsterDream.
//!
//! Ignored so a normal lib run does not start the agent. Run with `--ignored`
//! and `--test-threads=1`. A missing agent binary is a failure.

use std::ffi::{OsStr, OsString};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tokio::sync::mpsc;

use super::{AcpClient, Inbound};

const MARKER: &str = "E-BASH-MARKER";
const REFUSE_COMMAND: &str = "printf 'ran\\n' > side-effect.txt";

fn lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

struct EnvGuard {
    saved: Vec<(&'static str, Option<OsString>)>,
}

impl EnvGuard {
    fn replace(pairs: &[(&'static str, Option<OsString>)]) -> Self {
        let saved = pairs
            .iter()
            .map(|(key, _)| (*key, std::env::var_os(key)))
            .collect();
        for (key, value) in pairs {
            write_env(key, value.as_deref());
        }
        Self { saved }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (key, value) in self.saved.drain(..).rev() {
            write_env(key, value.as_deref());
        }
    }
}

fn write_env(key: &str, value: Option<&OsStr>) {
    // SAFETY: every sandbox_session test holds `lock` until this guard drops.
    match value {
        Some(value) => unsafe { std::env::set_var(key, value) },
        None => unsafe { std::env::remove_var(key) },
    }
}

fn isolate_env(home: &Path, config: &Path) -> EnvGuard {
    let grok = home.join("grok");
    std::fs::create_dir_all(&grok).expect("grok home");
    EnvGuard::replace(&[
        ("HOME", Some(home.as_os_str().to_os_string())),
        ("GROK_HOME", Some(grok.as_os_str().to_os_string())),
        ("A3S_CONFIG", Some(config.as_os_str().to_os_string())),
        ("A3S_DISABLE_CC_SWITCH", Some(OsString::from("1"))),
        ("A3S_DISABLE_GROK_ACCOUNT", Some(OsString::from("1"))),
        ("A3S_DISABLE_CURSOR_ACCOUNT", Some(OsString::from("1"))),
        ("XAI_API_KEY", Some(OsString::from("sk-dummy-not-a-model"))),
        ("ASTER_SANDBOX_DIR", None),
    ])
}

struct MockState {
    bodies: Vec<String>,
    seq: u64,
    sent: bool,
    command: String,
}

fn spawn_mock(command: &str) -> (SocketAddr, Arc<Mutex<MockState>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock");
    let addr = listener.local_addr().expect("mock addr");
    let state = Arc::new(Mutex::new(MockState {
        bodies: Vec::new(),
        seq: 0,
        sent: false,
        command: command.to_owned(),
    }));
    let shared = Arc::clone(&state);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let shared = Arc::clone(&shared);
            std::thread::spawn(move || {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(20)));
                if let Some(body) = read_http_body(&mut stream) {
                    let response = {
                        let mut guard = shared.lock().expect("mock");
                        guard.bodies.push(body.clone());
                        decide(&body, &mut guard)
                    };
                    let _ = write_http(&mut stream, &response);
                }
            });
        }
    });
    (addr, state)
}

fn read_http_body(stream: &mut TcpStream) -> Option<String> {
    let mut buf = Vec::new();
    let mut tmp = [0_u8; 8192];
    let header_end = loop {
        let n = stream.read(&mut tmp).ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&tmp[..n]);
        if let Some(pos) = buf.windows(4).position(|window| window == b"\r\n\r\n") {
            break pos + 4;
        }
        if buf.len() > 16_000_000 {
            return None;
        }
    };
    let headers = String::from_utf8_lossy(&buf[..header_end]);
    let mut length = 0_usize;
    for line in headers.lines() {
        let lower = line.to_ascii_lowercase();
        if let Some(value) = lower.strip_prefix("content-length:") {
            length = value.trim().parse().unwrap_or(0);
        }
    }
    while buf.len() < header_end + length {
        let n = stream.read(&mut tmp).ok()?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&tmp[..n]);
    }
    let end = (header_end + length).min(buf.len());
    Some(String::from_utf8_lossy(&buf[header_end..end]).into_owned())
}

fn write_http(stream: &mut TcpStream, body: &str) -> std::io::Result<()> {
    let content_type = if body.starts_with("data:") {
        "text/event-stream"
    } else {
        "application/json"
    };
    let header = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(header.as_bytes())?;
    stream.write_all(body.as_bytes())?;
    stream.flush()
}

fn session_tools(raw: &str) -> bool {
    if raw.contains("emit_pre_analysis") || raw.contains("You are a pre-analysis assistant") {
        return false;
    }
    serde_json::from_str::<Value>(raw)
        .ok()
        .and_then(|value| value.get("tools").cloned())
        .and_then(|tools| tools.as_array().cloned())
        .is_some_and(|tools| !tools.is_empty())
}

fn streaming_of(raw: &str) -> bool {
    serde_json::from_str::<Value>(raw)
        .ok()
        .and_then(|value| value.get("stream").and_then(Value::as_bool))
        .unwrap_or(false)
}

fn pre_analysis_object() -> String {
    json!({
        "intent": "GeneralPurpose",
        "requires_planning": false,
        "goal": { "description": "ok", "success_criteria": [] },
        "execution_plan": {
            "complexity": "Simple",
            "steps": [{
                "id": "step-1",
                "description": "ok",
                "dependencies": [],
                "success_criteria": "Complete the request"
            }],
            "required_tools": []
        },
        "optimized_input": ""
    })
    .to_string()
}

fn reply_text(text: &str, streaming: bool) -> String {
    if !streaming {
        return json!({
            "id": "cmpl",
            "object": "chat.completion",
            "choices": [{
                "index": 0,
                "message": { "role": "assistant", "content": text },
                "finish_reason": "stop"
            }],
            "usage": { "prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2 }
        })
        .to_string();
    }
    let chunk = json!({ "choices": [{ "delta": { "content": text } }] });
    let done = json!({ "choices": [{ "delta": {}, "finish_reason": "stop" }] });
    format!("data: {chunk}\n\ndata: {done}\n\ndata: [DONE]\n\n")
}

fn reply_bash(command: &str, state: &mut MockState, streaming: bool) -> String {
    state.seq = state.seq.saturating_add(1);
    let id = format!("call_bash_{}", state.seq);
    let args = json!({ "command": command }).to_string();
    if !streaming {
        return json!({
            "id": "cmpl",
            "object": "chat.completion",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": id,
                        "type": "function",
                        "function": { "name": "bash", "arguments": args }
                    }]
                },
                "finish_reason": "tool_calls"
            }],
            "usage": { "prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2 }
        })
        .to_string();
    }
    let chunk = json!({
        "choices": [{
            "delta": {
                "tool_calls": [{
                    "index": 0,
                    "id": id,
                    "function": { "name": "bash", "arguments": args }
                }]
            }
        }]
    });
    let done = json!({ "choices": [{ "delta": {}, "finish_reason": "tool_calls" }] });
    format!("data: {chunk}\n\ndata: {done}\n\ndata: [DONE]\n\n")
}

fn decide(raw: &str, state: &mut MockState) -> String {
    let streaming = streaming_of(raw);
    if raw.contains("context-compaction engine") {
        return reply_text("The task is finished.", streaming);
    }
    if raw.contains("emit_pre_analysis") || raw.contains("You are a pre-analysis assistant") {
        return reply_text(&pre_analysis_object(), streaming);
    }
    if raw.contains("You extract durable") {
        return reply_text("{\"items\":[]}", false);
    }
    if !state.sent && session_tools(raw) && raw.contains(MARKER) {
        state.sent = true;
        let command = state.command.clone();
        return reply_bash(&command, state, streaming);
    }
    reply_text("ok", streaming)
}

fn after_bash_request(bodies: &[String]) -> String {
    let index = bodies
        .iter()
        .position(|body| session_tools(body) && body.contains(MARKER));
    match index {
        Some(index) => bodies[index + 1..].join("\n"),
        None => String::new(),
    }
}

fn json_scalars(body: &str, key: &str) -> Vec<String> {
    let pattern = format!("\"{key}\"");
    let mut values = Vec::new();
    let mut rest = body;
    while let Some(index) = rest.find(&pattern) {
        rest = &rest[index + pattern.len()..];
        let trimmed = rest.trim_start();
        let Some(trimmed) = trimmed.strip_prefix(':') else {
            continue;
        };
        let trimmed = trimmed.trim_start();
        let end = trimmed.find([',', '}', '\n', ' ']).unwrap_or(trimmed.len());
        let value = trimmed[..end].trim().trim_matches('"').to_owned();
        if !value.is_empty() {
            values.push(value);
        }
    }
    values
}

fn excerpt(text: &str) -> String {
    let max = 4000;
    if text.len() <= max {
        return text.to_owned();
    }
    text[text.len() - max..].to_owned()
}

fn agent_binary() -> PathBuf {
    // `launcher::workspace_debug_binary` is private. This is that function's
    // manifest-relative path.
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../target/debug/a3s-code-acp");
    let path = std::fs::canonicalize(&path).unwrap_or(path);
    if !path.is_file() {
        panic!(
            "build the workspace companion with cargo build -p a3s-code-acp --locked ({})",
            path.display()
        );
    }
    path
}

fn place_binary(source: &Path, dest: &Path, root: &Path) {
    let source = std::fs::canonicalize(source)
        .unwrap_or_else(|error| panic!("agent binary {}: {error}", source.display()));
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).expect("bin dir");
    }
    if std::fs::hard_link(&source, dest).is_err() {
        std::fs::copy(&source, dest)
            .unwrap_or_else(|error| panic!("copy agent {}: {error}", dest.display()));
        let mut permissions = std::fs::metadata(dest).expect("copied agent").permissions();
        use std::os::unix::fs::PermissionsExt;
        permissions.set_mode(0o755);
        std::fs::set_permissions(dest, permissions).expect("agent mode");
    }
    let placed = std::fs::canonicalize(dest).expect("placed agent");
    let root = std::fs::canonicalize(root).expect("temp root");
    assert!(
        placed.starts_with(&root),
        "placed binary resolved outside the temp root: {}",
        placed.display()
    );
}

fn write_acl(path: &Path, base_url: &str) {
    std::fs::write(
        path,
        format!(
            r#"
providers "mock" {{
  base_url = "{base_url}"
  api_key = "test"
  models "gpt" {{
  }}
}}
default_model = "mock/gpt"
"#
        ),
    )
    .expect("acl");
}

fn private_workspace(root: &Path) -> PathBuf {
    let workspace = root.join("workspace");
    std::fs::create_dir(&workspace).expect("workspace");
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&workspace, std::fs::Permissions::from_mode(0o700))
        .expect("workspace mode");
    workspace
}

fn hvf_host_count() -> usize {
    let output = std::process::Command::new("/bin/ps")
        .args(["-ax", "-o", "command="])
        .output()
        .expect("ps");
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| line.contains("macos-hvf-host-service"))
        .count()
}

struct HostWatch {
    baseline: usize,
    max_seen: Arc<AtomicUsize>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl HostWatch {
    fn start() -> Self {
        let baseline = hvf_host_count();
        let max_seen = Arc::new(AtomicUsize::new(baseline));
        let stop = Arc::new(AtomicBool::new(false));
        let max_flag = Arc::clone(&max_seen);
        let stop_flag = Arc::clone(&stop);
        let thread = std::thread::spawn(move || {
            while !stop_flag.load(Ordering::Relaxed) {
                let count = hvf_host_count();
                max_flag.fetch_max(count, Ordering::Relaxed);
                std::thread::sleep(Duration::from_millis(30));
            }
        });
        Self {
            baseline,
            max_seen,
            stop,
            thread: Some(thread),
        }
    }

    fn max_seen(&self) -> usize {
        self.max_seen.load(Ordering::Relaxed)
    }
}

impl Drop for HostWatch {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn allow_once(params: &Value) -> Option<Value> {
    let offered = params
        .get("options")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|option| option.get("optionId").and_then(Value::as_str))
        .any(|id| id == "allow-once");
    offered.then(|| json!({ "outcome": { "outcome": "selected", "optionId": "allow-once" } }))
}

async fn round_trip(
    client: &AcpClient,
    inbound: &mut mpsc::UnboundedReceiver<Inbound>,
    session_id: &str,
) -> Result<Value, String> {
    let timeout = Duration::from_secs(90);
    let params = json!({
        "sessionId": session_id,
        "prompt": [{ "type": "text", "text": MARKER }],
    });
    let mut request = Box::pin(client.request("session/prompt", params));
    let mut inbound_open = true;
    let mut unknown = Vec::new();
    let result = tokio::time::timeout(timeout, async {
        loop {
            tokio::select! {
                result = &mut request => return result,
                incoming = inbound.recv(), if inbound_open => {
                    match incoming {
                        None => inbound_open = false,
                        Some(Inbound::Request { id, method, params })
                            if method == "session/request_permission" =>
                        {
                            match allow_once(&params) {
                                Some(outcome) => client.respond(id, outcome).await?,
                                None => {
                                    let _ = client
                                        .respond(
                                            id,
                                            json!({ "outcome": { "outcome": "cancelled" } }),
                                        )
                                        .await;
                                    return Err(format!(
                                        "permission options had no allow-once: {params}"
                                    ));
                                }
                            }
                        }
                        Some(Inbound::Request { id, method, .. }) => {
                            unknown.push(method);
                            client
                                .respond_error(id, -32601, "method not supported")
                                .await?;
                        }
                        Some(Inbound::Notification { .. }) => {}
                    }
                }
            }
        }
    })
    .await;
    if !unknown.is_empty() {
        eprintln!("sandbox_session unknown_requests={unknown:?}");
    }
    match result {
        Ok(result) => result,
        Err(_) => Err("prompt timed out after 90s".to_owned()),
    }
}

async fn open_session(
    binary: &Path,
    workspace: &Path,
    config: &Path,
) -> (Arc<AcpClient>, mpsc::UnboundedReceiver<Inbound>, String) {
    let args = vec![
        "--cwd".to_owned(),
        workspace.display().to_string(),
        "--model".to_owned(),
        "mock/gpt".to_owned(),
        "--config".to_owned(),
        config.display().to_string(),
    ];
    let (client, mut inbound) = AcpClient::spawn(binary, &args, workspace)
        .unwrap_or_else(|error| panic!("spawn a3s-code-acp: {error}"));
    tokio::time::timeout(
        Duration::from_secs(30),
        client.request(
            "initialize",
            json!({
                "protocolVersion": 1,
                "clientCapabilities": {},
                "clientInfo": {
                    "name": "a3s-studio",
                    "version": env!("CARGO_PKG_VERSION"),
                },
            }),
        ),
    )
    .await
    .expect("initialize timed out")
    .unwrap_or_else(|error| panic!("initialize: {error}"));
    let opened = tokio::time::timeout(
        Duration::from_secs(30),
        client.request(
            "session/new",
            json!({
                "cwd": workspace.display().to_string(),
                "mcpServers": [],
            }),
        ),
    )
    .await
    .expect("session/new timed out")
    .unwrap_or_else(|error| panic!("session/new: {error}"));
    let session_id = opened
        .get("sessionId")
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("session/new missing sessionId: {opened}"))
        .to_owned();
    while inbound.try_recv().is_ok() {}
    (client, inbound, session_id)
}

#[tokio::test]
#[ignore = "starts the real a3s-code-acp and must not boot HVF"]
async fn sandbox_session_without_companions_refuses_one_bash() {
    let _lock = lock();
    let root = tempfile::tempdir_in("/private/var/tmp").expect("temp root");
    let workspace = private_workspace(root.path());
    let home = root.path().join("home");
    std::fs::create_dir_all(&home).expect("home");
    let bin_dir = root.path().join("bin");
    std::fs::create_dir_all(&bin_dir).expect("bin");
    let binary = bin_dir.join("a3s-code-acp");
    place_binary(&agent_binary(), &binary, root.path());
    let config = root.path().join("config.acl");
    let escape = root
        .path()
        .join(format!("sandbox-session-escape-{}", std::process::id()));
    let (addr, state) = spawn_mock(REFUSE_COMMAND);
    write_acl(&config, &format!("http://{addr}/v1"));
    let _env = isolate_env(&home, &config);
    let watch = HostWatch::start();
    let started = Instant::now();
    let (client, mut inbound, session_id) = open_session(&binary, &workspace, &config).await;
    let result = round_trip(&client, &mut inbound, &session_id).await;
    let bodies = state.lock().expect("mock").bodies.clone();
    let sent = state.lock().expect("mock").sent;
    let observed = after_bash_request(&bodies);
    assert!(
        sent,
        "scripted model never sent bash\n{}",
        excerpt(&bodies.join("\n"))
    );
    match result {
        Ok(_) => {}
        Err(detail) if detail.contains("completion gate:") => {}
        Err(detail) => panic!("prompt failed: {detail}\n{}", excerpt(&observed)),
    }
    assert!(
        observed.contains("native_process_lifetime_unqualified"),
        "unescalated bash did not fail closed\n{}",
        excerpt(&observed)
    );
    assert!(
        !workspace.join("side-effect.txt").exists(),
        "unescalated bash created a file"
    );
    let started_flags = json_scalars(&observed, "command_started");
    if !started_flags.is_empty() {
        assert!(
            started_flags.iter().all(|value| value == "false"),
            "command_started was not false: {started_flags:?}"
        );
    }
    let exits = json_scalars(&observed, "exit_code");
    if !exits.is_empty() {
        assert!(
            exits.iter().all(|value| value == "null"),
            "exit_code was not null: {exits:?}"
        );
    }
    let max_seen = watch.max_seen();
    let baseline = watch.baseline;
    assert_eq!(
        max_seen, baseline,
        "macos-hvf-host-service rose from {baseline} to {max_seen}"
    );
    drop(watch);
    drop(client);
    assert!(
        !escape.exists(),
        "escape file appeared at {}",
        escape.display()
    );
    eprintln!(
        "ORACLE test=sandbox_session name=without_companions result=pass detail=refused=native_process_lifetime_unqualified file=absent hvf_baseline={baseline} hvf_max={max_seen} elapsed_ms={}",
        started.elapsed().as_millis()
    );
}
