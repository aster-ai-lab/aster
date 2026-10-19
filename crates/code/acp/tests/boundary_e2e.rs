//! Six shipped-binary cases. Each one owns its mock, home, and workspace.
//! They stay out of the long pager tour in `shipped_tui_e2e.rs`.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use agent_client_protocol as acp;
use async_trait::async_trait;
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

const MODEL_ID: &str = "mock/gpt";
const FIXTURE_SECRET: &str = "hermetic-fixture-key";
const READ_LINE: &str = "boundary-read-line-7f3a";
const ASK_CHOSEN: &str = "cedar-left-7f3a";
const ASK_OTHER: &str = "maple-right-7f3a";

enum Arm {
    List,
    Read {
        sent: bool,
    },
    Edit {
        sent_outside: bool,
        sent_inside: bool,
    },
    Office {
        sent_outside: bool,
        sent_inside: bool,
    },
    Bash {
        sent: bool,
    },
    Ask {
        sent: bool,
    },
}

struct MockState {
    bodies: Vec<String>,
    seq: u64,
    arm: Arm,
}

struct Case {
    home: PathBuf,
    agent: AgentProc,
    session: acp::SessionId,
    state: Arc<Mutex<MockState>>,
}

struct AgentProc {
    child: tokio::process::Child,
    conn: Rc<acp::ClientSideConnection>,
}

impl Drop for AgentProc {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
    }
}

fn oracle(name: &str, detail: &str) {
    let detail = detail.replace(['\n', '\r'], " ");
    eprintln!("ORACLE test=boundary_e2e name={name} result=pass detail={detail}");
}

fn isolate(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "a3s-boundary-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&path).expect("temp dir");
    path
}

fn write_acl(workspace: &Path, base_url: &str) -> PathBuf {
    let dir = workspace.join(".a3s");
    std::fs::create_dir_all(&dir).expect("acl dir");
    let path = dir.join("config.acl");
    std::fs::write(
        &path,
        format!(
            r#"
providers "mock" {{
  base_url = "{base_url}"
  api_key = "test"
  models "gpt" {{
  }}
}}
default_model = "{MODEL_ID}"
"#
        ),
    )
    .expect("acl");
    path
}

fn spawn_mock(arm: Arm) -> (SocketAddr, Arc<Mutex<MockState>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock");
    let addr = listener.local_addr().expect("mock addr");
    let state = Arc::new(Mutex::new(MockState {
        bodies: Vec::new(),
        seq: 0,
        arm,
    }));
    let shared = Arc::clone(&state);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let _ = stream.set_read_timeout(Some(Duration::from_secs(20)));
            if let Some(body) = read_http_body(&mut stream) {
                let response = {
                    let mut guard = shared.lock().expect("mock");
                    guard.bodies.push(body.clone());
                    decide(&body, &mut guard)
                };
                let _ = write_http(&mut stream, &response);
            }
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

fn has_tools(body: &serde_json::Value) -> bool {
    body.get("tools")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|tools| !tools.is_empty())
}

fn streaming_of(raw: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(raw)
        .ok()
        .and_then(|value| value.get("stream").and_then(serde_json::Value::as_bool))
        .unwrap_or(false)
}

fn session_tools(raw: &str) -> bool {
    if raw.contains("emit_pre_analysis") || raw.contains("You are a pre-analysis assistant") {
        return false;
    }
    serde_json::from_str::<serde_json::Value>(raw)
        .ok()
        .is_some_and(|value| has_tools(&value))
}

fn pre_analysis_object() -> String {
    serde_json::json!({
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
        return serde_json::json!({
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
    let chunk = serde_json::json!({ "choices": [{ "delta": { "content": text } }] });
    let done = serde_json::json!({ "choices": [{ "delta": {}, "finish_reason": "stop" }] });
    format!("data: {chunk}\n\ndata: {done}\n\ndata: [DONE]\n\n")
}

fn reply_tool(name: &str, args: &str, state: &mut MockState, streaming: bool) -> String {
    state.seq = state.seq.saturating_add(1);
    let id = format!("call_{name}_{}", state.seq);
    if !streaming {
        return serde_json::json!({
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
                        "function": { "name": name, "arguments": args }
                    }]
                },
                "finish_reason": "tool_calls"
            }],
            "usage": { "prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2 }
        })
        .to_string();
    }
    let chunk = serde_json::json!({
        "choices": [{
            "delta": {
                "tool_calls": [{
                    "index": 0,
                    "id": id,
                    "function": { "name": name, "arguments": args }
                }]
            }
        }]
    });
    let done = serde_json::json!({ "choices": [{ "delta": {}, "finish_reason": "tool_calls" }] });
    format!("data: {chunk}\n\ndata: {done}\n\ndata: [DONE]\n\n")
}

fn decide(raw: &str, state: &mut MockState) -> String {
    let streaming = streaming_of(raw);
    if raw.contains("context-compaction engine") {
        return reply_text("The task is finished.", streaming);
    }
    // The classifier carries the user text. Answer it before the script arm,
    // or that call consumes the tool reply the session turn still needs.
    if raw.contains("emit_pre_analysis") || raw.contains("You are a pre-analysis assistant") {
        return reply_text(&pre_analysis_object(), streaming);
    }
    if raw.contains("You extract durable") {
        return reply_text("{\"items\":[]}", false);
    }
    let tools = session_tools(raw);
    match &mut state.arm {
        Arm::List => reply_text("ok", streaming),
        Arm::Read { sent } => {
            if !*sent && tools && raw.contains("E-READ-MARKER") {
                *sent = true;
                let args = serde_json::json!({ "file_path": "note.txt" }).to_string();
                return reply_tool("read", &args, state, streaming);
            }
            reply_text("ok", streaming)
        }
        Arm::Edit {
            sent_outside,
            sent_inside,
        } => {
            if !*sent_outside && tools && raw.contains("E-EDIT-MARKER") {
                *sent_outside = true;
                let args = serde_json::json!({
                    "file_path": "../outside.txt",
                    "old_string": "keep-outside",
                    "new_string": "CHANGED"
                })
                .to_string();
                return reply_tool("edit", &args, state, streaming);
            }
            if *sent_outside && !*sent_inside && tools && outside_refused(raw) {
                *sent_inside = true;
                let args = serde_json::json!({
                    "file_path": "note.txt",
                    "old_string": "alpha",
                    "new_string": "ALPHA"
                })
                .to_string();
                return reply_tool("edit", &args, state, streaming);
            }
            reply_text("ok", streaming)
        }
        Arm::Office {
            sent_outside,
            sent_inside,
        } => {
            if !*sent_outside && tools && raw.contains("E-OFFICE-MARKER") {
                *sent_outside = true;
                let args = office_args("../outside.docx", "nope");
                return reply_tool("office_file", &args, state, streaming);
            }
            if *sent_outside && !*sent_inside && tools && outside_refused(raw) {
                *sent_inside = true;
                let args = office_args("note.docx", "office-paragraph-7f3a");
                return reply_tool("office_file", &args, state, streaming);
            }
            reply_text("ok", streaming)
        }
        Arm::Bash { sent } => {
            if !*sent && tools && raw.contains("E-BASH-MARKER") {
                *sent = true;
                // No sandbox_permissions: the unqualified native command must
                // fail closed before a process exists.
                let args = serde_json::json!({ "command": "printf 'ran\\n' > side-effect.txt" })
                    .to_string();
                return reply_tool("bash", &args, state, streaming);
            }
            reply_text("ok", streaming)
        }
        Arm::Ask { sent } => {
            if !*sent && tools && raw.contains("T-ASK-MARKER") {
                *sent = true;
                let args = serde_json::json!({
                    "question": "Choose a token",
                    "options": [ASK_CHOSEN, ASK_OTHER]
                })
                .to_string();
                return reply_tool("ask_user", &args, state, streaming);
            }
            reply_text("ok", streaming)
        }
    }
}

/// `../` is refused before a write. The model sees either the boundary
/// sentence or the frozen-policy refusal that wraps a resolved path the
/// approval does not cover. Both leave the outside bytes untouched.
fn outside_refused(raw: &str) -> bool {
    raw.contains("Workspace boundary check failed")
        || raw.contains("tool_effect_effective_arguments_unapproved")
}

fn office_args(path: &str, text: &str) -> String {
    serde_json::json!({
        "file_path": path,
        "kind": "docx",
        "blocks": [{ "type": "paragraph", "text": text }]
    })
    .to_string()
}

fn function_names(body: &str) -> Vec<String> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(body) else {
        return Vec::new();
    };
    let Some(tools) = value.get("tools").and_then(serde_json::Value::as_array) else {
        return Vec::new();
    };
    tools
        .iter()
        .filter_map(|tool| {
            tool.pointer("/function/name")
                .and_then(serde_json::Value::as_str)
                .or_else(|| tool.get("name").and_then(serde_json::Value::as_str))
                .map(str::to_string)
        })
        .collect()
}

fn message_excerpt(body: &str) -> String {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(body) else {
        return "unparsed".into();
    };
    let Some(messages) = value.get("messages").and_then(serde_json::Value::as_array) else {
        return "no messages".into();
    };
    messages
        .iter()
        .map(|message| {
            let role = message
                .get("role")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("?");
            let content = message
                .get("content")
                .map(serde_json::Value::to_string)
                .unwrap_or_default();
            let excerpt: String = content.chars().take(320).collect();
            format!("{role}: {excerpt}")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn body_trace(state: &MockState) -> String {
    state
        .bodies
        .iter()
        .enumerate()
        .map(|(index, body)| {
            format!(
                "{index}:tools=[{}] bytes={}",
                function_names(body).join(","),
                body.len()
            )
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn tab_payloads(value: &serde_json::Value, out: &mut Vec<String>) {
    match value {
        serde_json::Value::String(text) => {
            for line in text.lines() {
                if let Some((_, payload)) = line.split_once('\t') {
                    out.push(payload.to_string());
                }
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                tab_payloads(item, out);
            }
        }
        serde_json::Value::Object(fields) => {
            for item in fields.values() {
                tab_payloads(item, out);
            }
        }
        _ => {}
    }
}

fn assert_no_leak(roots: &[PathBuf]) {
    let mut stack = roots.to_vec();
    while let Some(path) = stack.pop() {
        let Ok(meta) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_dir() {
            // Shipped user-guide pages name auth hosts. They are not requests.
            if path.file_name().and_then(|name| name.to_str()) == Some("docs") {
                continue;
            }
            if let Ok(entries) = std::fs::read_dir(&path) {
                for entry in entries.flatten() {
                    stack.push(entry.path());
                }
            }
            continue;
        }
        if meta.len() > 4_000_000 {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        assert!(
            !text.contains(FIXTURE_SECRET),
            "fixture secret in {}",
            path.display()
        );
        for line in text.lines() {
            if let Some(host) = live_request(line) {
                panic!("{host} in {}", path.display());
            }
        }
    }
}

/// A live provider request. The pager's local auth scope is
/// `https://auth.a3s::<uuid>` and is not a network call.
fn live_request(line: &str) -> Option<&'static str> {
    if line.contains("https://api.x.ai") || line.contains("://api.x.ai") {
        return Some("api.x.ai");
    }
    if line.contains("://grok.com") || line.contains("https://grok.com") {
        return Some("grok.com");
    }
    if line.contains("googleapis.com") {
        return Some("googleapis.com");
    }
    if line.contains("auth.a3s") && !line.contains("https://auth.a3s::") {
        return Some("auth.a3s");
    }
    None
}

struct TestClient;

#[async_trait(?Send)]
impl acp::Client for TestClient {
    async fn request_permission(
        &self,
        _args: acp::RequestPermissionRequest,
    ) -> Result<acp::RequestPermissionResponse, acp::Error> {
        Ok(acp::RequestPermissionResponse::new(
            acp::RequestPermissionOutcome::Selected(acp::SelectedPermissionOutcome::new(
                acp::PermissionOptionId::new("allow-once"),
            )),
        ))
    }

    async fn session_notification(
        &self,
        _args: acp::SessionNotification,
    ) -> Result<(), acp::Error> {
        Ok(())
    }
}

async fn spawn_agent(workspace: &Path, config: &Path, home: &Path) -> AgentProc {
    let bin = PathBuf::from(env!("CARGO_BIN_EXE_a3s-code-acp"));
    let stderr = std::fs::File::create(home.join("acp.stderr")).expect("acp stderr");
    let mut child = tokio::process::Command::new(bin)
        .arg("--cwd")
        .arg(workspace)
        .env("A3S_CONFIG", config)
        .env("A3S_DISABLE_CC_SWITCH", "1")
        .env("A3S_DISABLE_GROK_ACCOUNT", "1")
        .env("A3S_DISABLE_CURSOR_ACCOUNT", "1")
        .env("XAI_API_KEY", "sk-dummy-not-a-model")
        .env("HOME", home)
        .env("GROK_HOME", home.join("grok"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(stderr)
        .spawn()
        .expect("spawn a3s-code-acp");
    let stdout = child.stdout.take().expect("stdout").compat();
    let stdin = child.stdin.take().expect("stdin").compat_write();
    let (conn, io) = acp::ClientSideConnection::new(TestClient, stdin, stdout, |fut| {
        tokio::task::spawn_local(fut);
    });
    tokio::task::spawn_local(async move {
        let _ = io.await;
    });
    AgentProc {
        child,
        conn: Rc::new(conn),
    }
}

async fn open_case(workspace: &Path, arm: Arm) -> Case {
    std::fs::create_dir_all(workspace).expect("workspace");
    let home = isolate("home");
    let (addr, state) = spawn_mock(arm);
    let config = write_acl(workspace, &format!("http://{addr}/v1"));
    let agent = spawn_agent(workspace, &config, &home).await;
    acp::Agent::initialize(
        agent.conn.as_ref(),
        acp::InitializeRequest::new(acp::ProtocolVersion::V1),
    )
    .await
    .expect("initialize");
    let session = acp::Agent::new_session(
        agent.conn.as_ref(),
        acp::NewSessionRequest::new(workspace.to_path_buf()),
    )
    .await
    .expect("new session")
    .session_id;
    Case {
        home,
        agent,
        session,
        state,
    }
}

fn error_detail(error: &acp::Error) -> String {
    error
        .data
        .as_ref()
        .map(|value| match value {
            serde_json::Value::String(text) => text.clone(),
            other => other.to_string(),
        })
        .unwrap_or_default()
}

/// A successful mutation may stop on the completion gate. That error is the
/// settled turn. Any other prompt error is a failure.
async fn finish_prompt(case: &Case, text: &str, allow_gate: bool) -> String {
    let request = acp::PromptRequest::new(
        case.session.clone(),
        vec![acp::ContentBlock::Text(acp::TextContent::new(text))],
    );
    let result = tokio::time::timeout(
        Duration::from_secs(90),
        acp::Agent::prompt(case.agent.conn.as_ref(), request),
    )
    .await;
    let result = match result {
        Ok(result) => result,
        Err(_) => panic!(
            "prompt timed out\n{}",
            body_trace(&case.state.lock().expect("mock"))
        ),
    };
    match result {
        Ok(_) => String::new(),
        Err(error) => {
            let detail = error_detail(&error);
            if allow_gate && detail.contains("completion gate:") {
                return detail;
            }
            panic!(
                "prompt failed: {detail}\n{}",
                body_trace(&case.state.lock().expect("mock"))
            );
        }
    }
}

fn joined_bodies(state: &MockState) -> String {
    state.bodies.join("\n")
}

#[tokio::test(flavor = "current_thread")]
async fn e_list() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let workspace = isolate("list");
            let case = open_case(&workspace, Arm::List).await;
            finish_prompt(&case, "E-LIST-MARKER", false).await;
            let state = case.state.lock().expect("mock");
            let names = state
                .bodies
                .iter()
                .map(|body| function_names(body))
                .find(|names| names.iter().any(|name| name == "read"))
                .unwrap_or_else(|| panic!("no session tool list\n{}", body_trace(&state)));
            for required in ["read", "write", "bash", "office_file", "output_check"] {
                assert!(
                    names.iter().any(|name| name == required),
                    "session tools missing {required}: {names:?}"
                );
            }
            assert!(
                names.iter().all(|name| name != "parallel_task"),
                "parallel_task is model-visible: {names:?}"
            );
            drop(state);
            assert_no_leak(&[case.home.clone(), workspace]);
            oracle("E-LIST", "tools=read,write,bash,office_file,output_check");
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn e_read() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let workspace = isolate("read");
            let note = workspace.join("note.txt");
            std::fs::create_dir_all(&workspace).expect("workspace");
            std::fs::write(&note, READ_LINE.as_bytes()).expect("note");
            let case = open_case(&workspace, Arm::Read { sent: false }).await;
            finish_prompt(&case, "E-READ-MARKER", false).await;
            let state = case.state.lock().expect("mock");
            let mut payloads = Vec::new();
            for body in &state.bodies {
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(body) {
                    tab_payloads(&value, &mut payloads);
                }
            }
            assert!(
                payloads.iter().any(|payload| payload == READ_LINE),
                "read tab payload missing {READ_LINE:?}; saw {payloads:?}\n{}",
                body_trace(&state)
            );
            drop(state);
            assert_eq!(std::fs::read(&note).expect("note"), READ_LINE.as_bytes());
            assert_no_leak(&[case.home.clone(), workspace]);
            oracle("E-READ", "tab-payload=note.txt unchanged");
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn e_edit() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let root = isolate("edit");
            let workspace = root.join("workspace");
            let outside = root.join("outside.txt");
            std::fs::create_dir_all(&workspace).expect("workspace");
            std::fs::write(workspace.join("note.txt"), b"alpha beta").expect("note");
            std::fs::write(&outside, b"keep-outside").expect("outside");
            let case = open_case(
                &workspace,
                Arm::Edit {
                    sent_outside: false,
                    sent_inside: false,
                },
            )
            .await;
            let gate = finish_prompt(&case, "E-EDIT-MARKER", true).await;
            let state = case.state.lock().expect("mock");
            let observed = joined_bodies(&state);
            assert!(
                outside_refused(&observed),
                "outside edit was not refused\n{}\n{}",
                body_trace(&state),
                state
                    .bodies
                    .last()
                    .map(|body| message_excerpt(body))
                    .unwrap_or_default()
            );
            drop(state);
            assert_eq!(std::fs::read(&outside).expect("outside"), b"keep-outside");
            assert_eq!(
                std::fs::read(workspace.join("note.txt")).expect("note"),
                b"ALPHA beta"
            );
            assert_no_leak(&[case.home.clone(), root]);
            let gate = if gate.is_empty() {
                "end-turn"
            } else {
                "completion-gate"
            };
            oracle(
                "E-EDIT",
                &format!("span=ALPHA outside=unchanged turn={gate}"),
            );
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn e_office() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let root = isolate("office");
            let workspace = root.join("workspace");
            let outside = root.join("outside.docx");
            std::fs::create_dir_all(&workspace).expect("workspace");
            let case = open_case(
                &workspace,
                Arm::Office {
                    sent_outside: false,
                    sent_inside: false,
                },
            )
            .await;
            let gate = finish_prompt(&case, "E-OFFICE-MARKER", true).await;
            let state = case.state.lock().expect("mock");
            let observed = joined_bodies(&state);
            assert!(
                outside_refused(&observed),
                "outside office_file was not refused\n{}\n{}",
                body_trace(&state),
                state
                    .bodies
                    .last()
                    .map(|body| message_excerpt(body))
                    .unwrap_or_default()
            );
            drop(state);
            assert!(!outside.exists(), "office_file wrote outside the workspace");
            let bytes = std::fs::read(workspace.join("note.docx")).expect("note.docx");
            assert!(
                bytes.starts_with(b"PK\x03\x04"),
                "docx missing zip magic: {:02x?}",
                bytes.get(..4)
            );
            assert_no_leak(&[case.home.clone(), root]);
            let gate = if gate.is_empty() {
                "end-turn"
            } else {
                "completion-gate"
            };
            oracle("E-OFFICE", &format!("zip-magic outside=absent turn={gate}"));
        })
        .await;
}

#[cfg(target_os = "macos")]
#[tokio::test(flavor = "current_thread")]
async fn e_bash() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let workspace = isolate("bash");
            let case = open_case(&workspace, Arm::Bash { sent: false }).await;
            let detail = finish_prompt(&case, "E-BASH-MARKER", false).await;
            let state = case.state.lock().expect("mock");
            let observed = format!("{}\n{detail}", joined_bodies(&state));
            assert!(
                observed.contains("native_process_lifetime_unqualified"),
                "unescalated bash did not fail closed\n{}",
                body_trace(&state)
            );
            drop(state);
            assert!(
                !workspace.join("side-effect.txt").exists(),
                "unescalated bash created a file"
            );
            assert_no_leak(&[case.home.clone(), workspace]);
            oracle(
                "E-BASH",
                "refused=native_process_lifetime_unqualified file=absent",
            );
        })
        .await;
}

struct PtySession {
    parser: vt100::Parser,
    rx: std::sync::mpsc::Receiver<Vec<u8>>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
    raw: Vec<u8>,
    cpr_replied_through: usize,
}

impl Drop for PtySession {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn tui_binary() -> PathBuf {
    if let Some(path) = std::env::var_os("A3S_CODE_TUI_BIN") {
        return PathBuf::from(path);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../target/debug/aster")
}

fn open_pty(binary: &Path, args: &[&str], env: &[(&str, &str)], cwd: &Path) -> PtySession {
    let rows = 40;
    let cols = 120;
    let pair = portable_pty::native_pty_system()
        .openpty(portable_pty::PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("pty");
    let mut command = portable_pty::CommandBuilder::new(binary);
    for arg in args {
        command.arg(arg);
    }
    for (key, value) in env {
        command.env(key, value);
    }
    command.cwd(cwd);
    let child = pair.slave.spawn_command(command).expect("spawn tui");
    let mut reader = pair.master.try_clone_reader().expect("reader");
    let writer = pair.master.take_writer().expect("writer");
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = [0_u8; 8192];
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if tx.send(buf[..n].to_vec()).is_err() {
                        break;
                    }
                }
            }
        }
    });
    PtySession {
        parser: vt100::Parser::new(rows, cols, 1000),
        rx,
        writer,
        child,
        raw: Vec::new(),
        cpr_replied_through: 0,
    }
}

impl PtySession {
    fn drain(&mut self) {
        while let Ok(bytes) = self.rx.try_recv() {
            self.raw.extend_from_slice(&bytes);
            self.parser.process(&bytes);
        }
        let query = b"\x1b[6n";
        if let Some(at) = self
            .raw
            .windows(query.len())
            .rposition(|window| window == query)
        {
            if at >= self.cpr_replied_through {
                self.cpr_replied_through = self.raw.len();
                let _ = self.writer.write_all(b"\x1b[12;1R");
                let _ = self.writer.flush();
            }
        }
    }

    fn pump(&mut self, budget: Duration) -> String {
        let deadline = Instant::now() + budget;
        while Instant::now() < deadline {
            self.drain();
            std::thread::sleep(Duration::from_millis(30));
        }
        self.drain();
        self.screen()
    }

    fn screen(&mut self) -> String {
        self.drain();
        self.parser.screen().contents()
    }

    fn write_bytes(&mut self, bytes: &[u8]) {
        self.writer.write_all(bytes).expect("pty write");
        self.writer.flush().ok();
    }

    fn wait_for_frame(&mut self, timeout: Duration) -> String {
        let deadline = Instant::now() + timeout;
        let mut last = String::new();
        let mut stable_since: Option<Instant> = None;
        let mut welcome_since: Option<Instant> = None;
        while Instant::now() < deadline {
            self.drain();
            let screen = self.parser.screen().contents();
            let visible: String = screen.chars().filter(|ch| !ch.is_whitespace()).collect();
            if visible.len() > 12 && screen.contains("Aster") && screen.contains('❯') {
                if welcome_since.is_some_and(|start| start.elapsed() > Duration::from_millis(200)) {
                    return screen;
                }
                welcome_since.get_or_insert_with(Instant::now);
            } else {
                welcome_since = None;
            }
            if visible.len() > 12 {
                if screen == last {
                    if stable_since
                        .is_some_and(|start| start.elapsed() > Duration::from_millis(500))
                    {
                        return screen;
                    }
                    stable_since.get_or_insert_with(Instant::now);
                } else {
                    stable_since = Some(Instant::now());
                    last = screen;
                }
            }
            std::thread::sleep(Duration::from_millis(40));
        }
        panic!("timed out waiting for a product frame\n{last}")
    }
}

fn choice_rows(screen: &str) -> bool {
    let mut first = false;
    let mut second = false;
    for line in screen.lines() {
        if line.contains(ASK_CHOSEN) && line.contains('1') {
            first = true;
        }
        if line.contains(ASK_OTHER) && line.contains('2') {
            second = true;
        }
    }
    first && second
}

#[test]
fn t_ask() {
    let binary = tui_binary();
    assert!(
        binary.is_file(),
        "shipped aster is missing at {}",
        binary.display()
    );
    let workspace = isolate("ask");
    let home = isolate("ask-home");
    let (addr, state) = spawn_mock(Arm::Ask { sent: false });
    let config = write_acl(&workspace, &format!("http://{addr}/v1"));
    let acp_bin = env!("CARGO_BIN_EXE_a3s-code-acp");
    let cwd = workspace.to_str().expect("cwd");
    let config_path = config.to_str().expect("config").to_string();
    let home_path = home.to_str().expect("home").to_string();
    let grok_home = home.join("grok");
    let grok_home_path = grok_home.to_str().expect("grok home").to_string();
    let env = [
        ("A3S_ACP_AGENT_BIN", acp_bin),
        ("A3S_CONFIG", config_path.as_str()),
        ("A3S_DISABLE_CC_SWITCH", "1"),
        ("A3S_DISABLE_GROK_ACCOUNT", "1"),
        ("A3S_DISABLE_CURSOR_ACCOUNT", "1"),
        ("XAI_API_KEY", "sk-dummy-not-a-model"),
        ("HOME", home_path.as_str()),
        ("GROK_HOME", grok_home_path.as_str()),
        ("TERM", "xterm-256color"),
    ];
    let mut pty = open_pty(&binary, &["--cwd", cwd], &env, &workspace);
    pty.wait_for_frame(Duration::from_secs(45));
    pty.write_bytes(b"T-ASK-MARKER");
    pty.write_bytes(b"\r");
    let deadline = Instant::now() + Duration::from_secs(40);
    let mut last = String::new();
    while Instant::now() < deadline {
        last = pty.pump(Duration::from_millis(80));
        if choice_rows(&last) {
            break;
        }
    }
    assert!(
        choice_rows(&last),
        "both choices were not on one screen\n{last}"
    );
    pty.write_bytes(b"1");
    let answer_deadline = Instant::now() + Duration::from_secs(30);
    let mut answer = String::new();
    while Instant::now() < answer_deadline {
        if let Some(body) = state
            .lock()
            .expect("mock")
            .bodies
            .iter()
            .find(|body| body.contains(ASK_CHOSEN))
        {
            answer = body.clone();
            break;
        }
        pty.pump(Duration::from_millis(80));
    }
    assert!(
        answer.contains(ASK_CHOSEN),
        "next model request missed the chosen option\n{}",
        body_trace(&state.lock().expect("mock"))
    );
    assert!(
        !answer.contains(ASK_OTHER),
        "next model request still carried the other option"
    );
    pty.pump(Duration::from_millis(400));
    pty.write_bytes(b"\x15");
    pty.write_bytes(b"/quit");
    pty.write_bytes(b"\r");
    let quit_deadline = Instant::now() + Duration::from_secs(8);
    while Instant::now() < quit_deadline {
        if pty.child.try_wait().ok().flatten().is_some() {
            break;
        }
        pty.pump(Duration::from_millis(80));
    }
    let raw = String::from_utf8_lossy(&pty.raw);
    assert!(!raw.contains(FIXTURE_SECRET), "fixture secret on the pager");
    for line in raw.lines() {
        if let Some(host) = live_request(line) {
            panic!("{host} on the pager");
        }
    }
    assert_no_leak(&[home, workspace]);
    oracle("T-ASK", "screen=both next-request=cedar-left-7f3a");
}
