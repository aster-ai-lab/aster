//! End-to-end coverage of the shipped pager and the shipped ACP agent.
//!
//! The model is a local OpenAI-compatible server, except the ignored
//! `shipped_bound_goal_real_llm`, which runs the operator's live ACL. The agent process is
//! `aster-acp`. Palette and launch cases spawn `aster` on a PTY.
//! Nothing here substitutes a scripted `LlmClient` or the pager's content mock.

// These fixtures execute POSIX shell scripts and require Unix executable modes.
#![cfg(unix)]

#[path = "support/binaries.rs"]
mod binaries;
#[path = "support/tui_readiness.rs"]
mod tui_readiness;

#[path = "../../../../apps/aster/tests/support/python.rs"]
mod python;

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use agent_client_protocol as acp;
use async_trait::async_trait;
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

const MODEL_ID: &str = "mock/gpt";

#[path = "shipped_tui_e2e/reasoning.rs"]
mod reasoning;
#[path = "shipped_tui_e2e/retrieval.rs"]
mod retrieval;
#[path = "shipped_tui_e2e/session_controls.rs"]
mod session_controls;
const COMPACT_SUMMARY: &str = "COMPACT-SUMMARY-7f3a";
const DURABLE_TOKEN: &str = "DURABLE-TOKEN-42";
const GOAL_OBJECTIVE: &str = "准备好测试工作区 pty-goal-7b2e";
const WIRE_TEST: &str = "shipped_agent_session_wire";
const PTY_TEST: &str = "shipped_tui_launch_resume_and_visible_commands";
const NATIVE_FORM: &str = r#"{"root":"root","elements":{"root":{"type":"Card","props":{"title":"Aster form"},"children":["name","amount","ready","submit"]},"name":{"type":"Input","props":{"label":"Name","type":"text","required":true,"value":{"$bindState":"/form/name"}}},"amount":{"type":"Input","props":{"label":"Amount","type":"number","min":1,"max":5,"value":{"$bindState":"/form/amount"}}},"ready":{"type":"Checkbox","props":{"label":"Ready","required":true,"checked":{"$bindState":"/form/ready"}}},"submit":{"type":"Button","props":{"action":"submit","label":"Continue"}}},"state":{"form":{"name":"","amount":3,"ready":false}}}"#;

fn oracle(test: &str, name: &str, detail: &str) {
    let detail = detail.replace(['\n', '\r'], " ");
    eprintln!("ORACLE test={test} name={name} result=pass detail={detail}");
}

/// Unsupported local handlers and their palette descriptions; a current backend command may share the name.
const UNAVAILABLE: &[(&str, &str)] = &[
    ("usage", "View usage"),
    ("view-plan", "View the current plan"),
    ("plan", "Enter plan mode"),
    ("tutorial", "Quick tips to get the most out of Aster Code"),
    ("docs", "Open How-to Guides"),
    ("privacy", "coding data, retention"),
    ("login", "Log in or re-authenticate"),
    ("logout", "Log out and return to the login screen"),
    ("btw", "side question without interrupting"),
    ("feedback", "Send feedback about the current session"),
    ("queue", "prompts queued behind"),
    ("rename", "Rename the current session"),
    ("transcript", "conversation transcript"),
    ("rewind", "Rewind to a previous turn"),
    ("workflows", "Browse installed workflows"),
    ("workflow", "Launch a saved workflow"),
    ("personas", "Manage personas"),
    ("config-agents", "Manage agent definitions"),
    ("plugins", "View plugins"),
    ("skills", "View skills"),
    ("mcps", "MCP server status"),
    ("hooks", "View hooks"),
    ("marketplace", "View marketplace"),
    ("import-claude", "Claude settings import"),
    ("remember", "Save a memory note"),
    // `memory` / `flush` / `dream` / `tasks` / Core `/history` are ACP-advertised
    // (see FULLSCREEN_PALETTE); do not list them here — dual oracles false-pass.
    ("imagine", "imagine"),
    ("imagine-video", "imagine-video"),
    ("voice", "voice"),
    ("dashboard", "Agent Dashboard"),
    ("recap", "Summarize the session so far"),
    ("share", "Share this session via URL"),
    ("release-notes", "release notes"),
    ("announcements", "announcements"),
    ("loop", "recurring interval"),
    ("auto", "classifier approves safe tools"),
];

/// Offered in the fullscreen agent session, with the palette description.
/// Keep aligned with pager `ASTER_EFFECTIVE_COMMANDS` plus ACP-advertised
/// `memory` / `tasks`. `cd` is dashboard-only (asserted absent below).
const FULLSCREEN_PALETTE: &[(&str, &str)] = &[
    ("fork", "Branch the current session"),
    ("memory", "Inspect and manage stored memories"),
    ("flush", "Save this session's memory to disk now"),
    (
        "dream",
        "Consolidate memory now (merge observations into topics)",
    ),
    ("history", "Show conversation stats"),
    ("tasks", "Inspect background tasks or stop a task"),
    ("settings", "Open the settings modal"),
    ("config", "Add and configure providers and models"),
    ("new", "Start a new session"),
    ("resume", "Resume a previous session"),
    ("effort", "Set reasoning effort"),
    ("goal", "durable goal"),
    ("compact", "Compact conversation history"),
    ("model", "Switch the active model"),
    ("context", "View context usage"),
    ("jump", "Jump to a turn"),
    ("edit-prompt", "external editor"),
    ("session-info", "Show session info"),
    ("export", "Export the current conversation"),
    ("copy", "Copy last response"),
    ("find", "Search the conversation scrollback"),
    ("theme", "Switch the color theme"),
    ("yolo", "skip all permission prompts"),
    ("vim-mode", "vim-style scrollback"),
    ("multiline", "multiline input mode"),
    ("compact-mode", "less padding, more content"),
    ("timestamps", "message timestamps"),
    ("toggle-mouse-reporting", "terminal mouse reporting"),
    ("minimal", "scrollback-native"),
    ("timeline", "timeline sidebar"),
    ("doctor", "show available fixes"),
    ("home", "welcome screen"),
    ("delete", "Delete this session"),
    ("help", "keyboard shortcuts"),
    ("quit", "Quit the application"),
];

struct MockState {
    bodies: Vec<String>,
    tool_seq: u64,
    write_tool_replies: u64,
    edit_tool_replies: u64,
    expand_replies: u64,
    ask_tool_replies: u64,
    queue_release: Option<std::sync::mpsc::Receiver<()>>,
    write_path: String,
    late_path: String,
}

fn spawn_mock(write_path: &Path, late_path: &Path) -> (SocketAddr, Arc<Mutex<MockState>>) {
    spawn_mock_with_catalog(write_path, late_path, || {})
}

fn spawn_mock_with_catalog(
    write_path: &Path,
    late_path: &Path,
    mut on_catalog: impl FnMut() + Send + 'static,
) -> (SocketAddr, Arc<Mutex<MockState>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock");
    let addr = listener.local_addr().expect("mock addr");
    let state = Arc::new(Mutex::new(MockState {
        bodies: Vec::new(),
        tool_seq: 0,
        write_tool_replies: 0,
        edit_tool_replies: 0,
        expand_replies: 0,
        ask_tool_replies: 0,
        queue_release: None,
        write_path: write_path.display().to_string(),
        late_path: late_path.display().to_string(),
    }));
    let shared = Arc::clone(&state);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let _ = stream.set_read_timeout(Some(Duration::from_secs(20)));
            if let Some(body) = read_http_body(&mut stream) {
                if body.is_empty() {
                    on_catalog();
                    let catalog = serde_json::json!({"data":[{"id":"gpt","context_window":262144,"effort":{"supported_levels":["none","low","high"],"default_level":"high"}},{"id":"alt","context_window":131072,"effort":{"supported_levels":["low"],"default_level":"low"}}]});
                    let _ = write_http(&mut stream, &catalog.to_string());
                    continue;
                }
                let response = {
                    let mut guard = shared.lock().expect("mock");
                    guard.bodies.push(body.clone());
                    let response = decide(&body, &mut guard);
                    if session_controls::hold_provider(
                        &body,
                        &stream,
                        &response,
                        &mut guard.queue_release,
                    ) {
                        continue;
                    }
                    response
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

fn decide(raw: &str, state: &mut MockState) -> String {
    let parsed: serde_json::Value = serde_json::from_str(raw).unwrap_or(serde_json::Value::Null);
    let streaming = parsed
        .get("stream")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    if raw.contains("context-compaction engine") {
        return reply_text(COMPACT_SUMMARY, streaming);
    }
    // The classifier call carries the user text and a tool named
    // emit_pre_analysis. Answer it before the oracle arms, or that tool
    // call consumes the write/bash reply the product turn still needs.
    if raw.contains("emit_pre_analysis")
        || raw.contains("You are a pre-analysis assistant")
        || raw.contains("You plan a durable Goal.")
    {
        return reply_text(
            &pre_analysis_object(raw.contains("execution_plan.stages")),
            streaming,
        );
    }
    let tools = has_tools(&parsed);
    if let Some(response) = retrieval::response(&parsed, state, streaming) {
        return response;
    }
    if raw.contains("FORM-ORACLE") {
        // Cancel / invalid arms first so a prior submit's values cannot short-circuit
        // a later FORM-ORACLE-* turn in the same session.
        if raw.contains("FORM-ORACLE-CANCEL") {
            if raw.contains("form was cancelled by the user") {
                return reply_text("FORM-CANCELLED", streaming);
            }
            if parsed["tools"].as_array().is_some_and(|tools| {
                tools
                    .iter()
                    .any(|tool| tool["function"]["name"] == "ask_user_form")
            }) {
                let args =
                    serde_json::json!({"form":NATIVE_FORM,"prompt":"Cancel this form"}).to_string();
                return reply_tool("ask_user_form", &args, state, streaming);
            }
            return reply_text("FORM-UNAVAILABLE", streaming);
        }
        if let Some(values) = returned_form_values(&parsed["messages"]) {
            let valid = values["name"] == "Ada"
                && values["amount"].as_f64() == Some(4.0)
                && values["ready"] == true;
            return reply_text(
                if valid {
                    "FORM-SUBMITTED"
                } else {
                    "FORM-BAD-VALUES"
                },
                streaming,
            );
        }
        if parsed["tools"].as_array().is_some_and(|tools| {
            tools
                .iter()
                .any(|tool| tool["function"]["name"] == "ask_user_form")
        }) {
            let args =
                serde_json::json!({"form":NATIVE_FORM,"prompt":"Fill in details"}).to_string();
            return reply_tool("ask_user_form", &args, state, streaming);
        }
        return reply_text("FORM-UNAVAILABLE", streaming);
    }
    if raw.contains("WRITE-ORACLE") && tools {
        if state.write_tool_replies >= 1 {
            return reply_text("ok", streaming);
        }
        state.write_tool_replies = state.write_tool_replies.saturating_add(1);
        let args = serde_json::json!({
            "file_path": state.write_path,
            "content": "wrote-marker"
        })
        .to_string();
        return reply_tool("write", &args, state, streaming);
    }
    if raw.contains("EDIT-ORACLE") && tools {
        if state.edit_tool_replies >= 1 {
            return reply_text("edit-done", streaming);
        }
        state.edit_tool_replies = state.edit_tool_replies.saturating_add(1);
        let args = serde_json::json!({
            "file_path": state.write_path,
            "old_string": "seed-before-edit",
            "new_string": "edited-marker"
        })
        .to_string();
        return reply_tool("edit", &args, state, streaming);
    }
    if raw.contains("ASK-WRITE") && tools {
        if raw.contains("wrote-marker") || raw.contains("rejected via ACP request_permission") {
            return reply_text("ok", streaming);
        }
        let args = serde_json::json!({
            "file_path": state.write_path,
            "content": "wrote-marker"
        })
        .to_string();
        return reply_tool("write", &args, state, streaming);
    }
    if raw.contains("EXPAND-ORACLE") && tools {
        if state.expand_replies >= 1 {
            return reply_text("ok", streaming);
        }
        state.expand_replies = state.expand_replies.saturating_add(1);
        // Ask posture and explicit execution escalation have separate approvals.
        let args = serde_json::json!({
            "command": "printf 'EXPAND-MARKER\\n'",
            "sandbox_permissions": "require_escalated",
            "justification": "Explicit approval for the expansion test command"
        })
        .to_string();
        return reply_tool("bash", &args, state, streaming);
    }
    if raw.contains("ASK-ORACLE") && tools {
        if state.ask_tool_replies >= 1 {
            return reply_text("ASK-ANSWER-DONE", streaming);
        }
        state.ask_tool_replies = state.ask_tool_replies.saturating_add(1);
        let args = serde_json::json!({
            "question": "Pick one",
            "options": ["alpha", "beta"]
        })
        .to_string();
        return reply_tool("ask_user", &args, state, streaming);
    }
    if raw.contains("CANCEL-ORACLE") && tools {
        let command = format!(
            "sleep 4; printf LATE > '{}'",
            state.late_path.replace('\'', "")
        );
        let args = serde_json::json!({
            "command": command,
            "sandbox_permissions": "require_escalated",
            "justification": "Explicitly approved host command cancellation fixture"
        })
        .to_string();
        return reply_tool("bash", &args, state, streaming);
    }
    if raw.contains("REPEAT-LIST-ORACLE") && tools {
        return reply_tool("ls", r#"{"path":"probe"}"#, state, streaming);
    }
    if raw.contains("LIST-FILES") {
        if let Some(output) = parsed["messages"].as_array().and_then(|messages| {
            messages
                .iter()
                .rev()
                .take_while(|message| message["role"] != "user")
                .find(|message| message["role"] == "tool")
                .and_then(|message| message["content"].as_str())
        }) {
            return reply_text(
                if output.starts_with("Tool exit code: 0\nDirectory: ") {
                    "listed"
                } else {
                    "LIST-FAILED"
                },
                streaming,
            );
        }
        if !tools {
            return reply_text("LIST-UNAVAILABLE", streaming);
        }
        let args = serde_json::json!({ "path": "." }).to_string();
        return reply_tool("ls", &args, state, streaming);
    }
    if raw.contains("CONTROL-NEXT") {
        return reply_text("CONTROL-NEXT-DONE", streaming);
    }
    session_controls::response(&parsed, streaming)
        .or_else(|| reasoning::response(&parsed, streaming))
        .unwrap_or_else(|| reply_text("ok", streaming))
}

fn has_tools(body: &serde_json::Value) -> bool {
    body.get("tools")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|tools| !tools.is_empty())
}

fn returned_form_values(value: &serde_json::Value) -> Option<serde_json::Value> {
    returned_result(value, "aster.code.user-form.v1")?
        .get("values")
        .cloned()
}

fn returned_result(value: &serde_json::Value, schema: &str) -> Option<serde_json::Value> {
    if value["schema"] == schema {
        return Some(value.clone());
    }
    match value {
        serde_json::Value::Object(fields) => fields
            .values()
            .find_map(|value| returned_result(value, schema)),
        serde_json::Value::Array(items) => items
            .iter()
            .find_map(|value| returned_result(value, schema)),
        serde_json::Value::String(text) => {
            serde_json::from_str(text.strip_prefix("Tool exit code: 0\n").unwrap_or(text))
                .ok()
                .and_then(|nested| returned_result(&nested, schema))
        }
        _ => None,
    }
}

#[test]
fn submitted_form_values_survive_the_tool_exit_status_prefix() {
    let values = serde_json::json!({"name":"Ada","amount":4.0,"ready":true});
    let result = serde_json::json!({"schema":"aster.code.user-form.v1","values":values});
    let messages =
        serde_json::json!([{"role":"tool","content":format!("Tool exit code: 0\n{result}")}]);
    assert_eq!(returned_form_values(&messages), Some(values));
}

fn budget_number(body: &str, marker: &str) -> Option<u64> {
    let rest = body.split_once(marker)?.1;
    let digits: String = rest.chars().take_while(|ch| ch.is_ascii_digit()).collect();
    digits.parse().ok()
}

fn pre_analysis_object(stage_plan: bool) -> String {
    // Empty optimized_input keeps the original user sentence on the product turn.
    let mut value = serde_json::json!({
        "intent": "GeneralPurpose",
        "requires_planning": false,
        "goal": {
            "description": "ok",
            "success_criteria": []
        },
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
    });
    if stage_plan {
        let plan = value["execution_plan"].as_object_mut().unwrap();
        let stages = plan.remove("steps").unwrap();
        plan.insert("stages".into(), stages);
    }
    value.to_string()
}

#[test]
fn pre_analysis_fixture_keeps_goal_stages_and_ordinary_steps_distinct() {
    for stage_plan in [false, true] {
        let value: serde_json::Value =
            serde_json::from_str(&pre_analysis_object(stage_plan)).unwrap();
        let (present, absent) = if stage_plan {
            ("stages", "steps")
        } else {
            ("steps", "stages")
        };
        assert_eq!(
            value["execution_plan"][present].as_array().unwrap().len(),
            1
        );
        assert!(value["execution_plan"][absent].is_null());
    }
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
    let chunk = serde_json::json!({
        "choices": [{ "delta": { "content": text } }]
    });
    let done = serde_json::json!({
        "choices": [{ "delta": {}, "finish_reason": "stop" }],
        "usage": { "prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2 }
    });
    format!("data: {chunk}\n\ndata: {done}\n\ndata: [DONE]\n\n")
}

fn reply_tool(name: &str, args: &str, state: &mut MockState, streaming: bool) -> String {
    state.tool_seq = state.tool_seq.saturating_add(1);
    let id = format!("call_{name}_{}", state.tool_seq);
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
    let done = serde_json::json!({
        "choices": [{ "delta": {}, "finish_reason": "tool_calls" }],
        "usage": { "prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2 }
    });
    format!("data: {chunk}\n\ndata: {done}\n\ndata: [DONE]\n\n")
}

struct Gate {
    option_ids: Vec<String>,
    reply: tokio::sync::oneshot::Sender<String>,
}

struct TestClient {
    gates: tokio::sync::mpsc::UnboundedSender<Gate>,
    tool_names: Rc<std::cell::RefCell<Vec<String>>>,
    /// `(session id, prompt id)` stamped on session updates.
    stamps: Rc<std::cell::RefCell<Vec<(String, String)>>>,
    /// `(session id, is_replay, text)` from user and agent message chunks.
    replayed: Rc<std::cell::RefCell<Vec<(String, bool, String)>>>,
}

#[async_trait(?Send)]
impl acp::Client for TestClient {
    async fn request_permission(
        &self,
        args: acp::RequestPermissionRequest,
    ) -> Result<acp::RequestPermissionResponse, acp::Error> {
        let option_ids = args
            .options
            .iter()
            .map(|option| option.option_id.0.to_string())
            .collect();
        let (tx, rx) = tokio::sync::oneshot::channel();
        let _ = self.gates.send(Gate {
            option_ids,
            reply: tx,
        });
        let chosen = rx.await.unwrap_or_else(|_| "cancel".to_string());
        if chosen == "cancel" {
            return Ok(acp::RequestPermissionResponse::new(
                acp::RequestPermissionOutcome::Cancelled,
            ));
        }
        Ok(acp::RequestPermissionResponse::new(
            acp::RequestPermissionOutcome::Selected(acp::SelectedPermissionOutcome::new(
                acp::PermissionOptionId::new(chosen),
            )),
        ))
    }

    async fn session_notification(&self, args: acp::SessionNotification) -> Result<(), acp::Error> {
        if let Some(prompt_id) = args
            .meta
            .as_ref()
            .and_then(|meta| meta.get("promptId"))
            .and_then(serde_json::Value::as_str)
        {
            self.stamps
                .borrow_mut()
                .push((args.session_id.0.to_string(), prompt_id.to_string()));
        }
        let is_replay = args
            .meta
            .as_ref()
            .and_then(|meta| meta.get("isReplay"))
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        if let Some(text) = chunk_text(&args.update) {
            self.replayed
                .borrow_mut()
                .push((args.session_id.0.to_string(), is_replay, text));
        }
        if let acp::SessionUpdate::ToolCall(call) = &args.update {
            if !call.title.is_empty() {
                self.tool_names.borrow_mut().push(call.title.clone());
            }
        }
        if let acp::SessionUpdate::ToolCallUpdate(update) = &args.update {
            if let Some(title) = update.fields.title.as_ref() {
                if !title.is_empty() {
                    self.tool_names.borrow_mut().push(title.clone());
                }
            }
        }
        Ok(())
    }
}

struct AgentProc {
    child: tokio::process::Child,
    conn: Rc<acp::ClientSideConnection>,
    gates: tokio::sync::mpsc::UnboundedReceiver<Gate>,
    tool_names: Rc<std::cell::RefCell<Vec<String>>>,
    stamps: Rc<std::cell::RefCell<Vec<(String, String)>>>,
    replayed: Rc<std::cell::RefCell<Vec<(String, bool, String)>>>,
}

fn isolate_dir(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "aster-shipped-e2e-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&path).expect("temp dir");
    path
}

fn write_acl(home: &Path, workspace: &Path, base_url: &str) -> PathBuf {
    let config_dir = workspace.join(".aster");
    std::fs::create_dir_all(&config_dir).expect("acl dir");
    let path = config_dir.join("config.acl");
    std::fs::write(
        &path,
        format!(
            r#"
providers "mock" {{
  base_url = "{base_url}"
  api_key = "test"
  models "gpt" {{
    reasoning_control = {{ mode = "effort", levels = ["none", "low", "high"] }}
  }}
  models "alt" {{
  }}
}}
default_model = "{MODEL_ID}"
"#
        ),
    )
    .expect("acl");
    let _ = home;
    path
}

async fn spawn_agent(workspace: &Path, config: &Path, home: &Path) -> AgentProc {
    spawn_agent_with_env(workspace, config, home, &[]).await
}

async fn spawn_agent_with_env(
    workspace: &Path,
    config: &Path,
    home: &Path,
    extra_env: &[(&str, &Path)],
) -> AgentProc {
    let bin = PathBuf::from(binaries::acp_binary());
    let stderr_path = home.join("acp.stderr");
    let stderr = std::fs::File::create(&stderr_path).expect("acp stderr");
    let mut child = tokio::process::Command::new(bin)
        .arg("--cwd")
        .arg(workspace)
        .env("ASTER_CONFIG", config)
        .env("ASTER_DISABLE_GROK_ACCOUNT", "1")
        .env("ASTER_DISABLE_CURSOR_ACCOUNT", "1")
        .env("XAI_API_KEY", "sk-dummy-not-a-model")
        .env("HOME", home)
        .env("GROK_HOME", home.join("grok"))
        .envs(extra_env.iter().map(|(key, value)| (*key, *value)))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::from(stderr))
        .kill_on_drop(true)
        .spawn()
        .expect("spawn aster-acp");
    let stdout = child.stdout.take().expect("stdout").compat();
    let stdin = child.stdin.take().expect("stdin").compat_write();
    let (gate_tx, gate_rx) = tokio::sync::mpsc::unbounded_channel();
    let tool_names = Rc::new(std::cell::RefCell::new(Vec::new()));
    let stamps = Rc::new(std::cell::RefCell::new(Vec::new()));
    let replayed = Rc::new(std::cell::RefCell::new(Vec::new()));
    let client = TestClient {
        gates: gate_tx,
        tool_names: Rc::clone(&tool_names),
        stamps: Rc::clone(&stamps),
        replayed: Rc::clone(&replayed),
    };
    let (conn, io) = acp::ClientSideConnection::new(client, stdin, stdout, |fut| {
        tokio::task::spawn_local(fut);
    });
    tokio::task::spawn_local(async move {
        let _ = io.await;
    });
    AgentProc {
        child,
        conn: Rc::new(conn),
        gates: gate_rx,
        tool_names,
        stamps,
        replayed,
    }
}

fn chunk_text(update: &acp::SessionUpdate) -> Option<String> {
    let chunk = match update {
        acp::SessionUpdate::UserMessageChunk(chunk)
        | acp::SessionUpdate::AgentMessageChunk(chunk) => chunk,
        _ => return None,
    };
    match &chunk.content {
        acp::ContentBlock::Text(text) => Some(text.text.clone()),
        _ => None,
    }
}

fn user_prompt(session: &acp::SessionId, text: &str) -> acp::PromptRequest {
    acp::PromptRequest::new(
        session.clone(),
        vec![acp::ContentBlock::Text(acp::TextContent::new(text))],
    )
}

fn user_prompt_id(session: &acp::SessionId, text: &str, prompt_id: &str) -> acp::PromptRequest {
    let mut meta = acp::Meta::new();
    meta.insert(
        "promptId".to_string(),
        serde_json::Value::String(prompt_id.to_string()),
    );
    user_prompt(session, text).meta(Some(meta))
}

async fn session_info_body(
    conn: &acp::ClientSideConnection,
    session: &acp::SessionId,
) -> serde_json::Value {
    let params = serde_json::value::to_raw_value(&serde_json::json!({
        "sessionId": session.0.as_ref(),
    }))
    .expect("session info params");
    let response = acp::Agent::ext_method(
        conn,
        acp::ExtRequest::new("aster/session/info", std::sync::Arc::from(params)),
    )
    .await
    .expect("session info");
    serde_json::from_str(response.0.get()).expect("session info json")
}

fn effort_meta(level: &str) -> acp::Meta {
    let mut meta = acp::Meta::new();
    meta.insert(
        "reasoningEffort".to_string(),
        serde_json::Value::String(level.to_string()),
    );
    meta
}

fn append_jsonl_dir(dir: &Path, out: &mut String) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if entry.path().extension().and_then(|ext| ext.to_str()) != Some("jsonl") {
            continue;
        }
        if let Ok(text) = std::fs::read_to_string(entry.path()) {
            out.push_str(&text);
            out.push('\n');
        }
    }
}

fn log_text(workspace: &Path) -> String {
    let mut out = String::new();
    append_jsonl_dir(&workspace.join(".aster").join("effect-log"), &mut out);
    let Ok(dates) = std::fs::read_dir(workspace.join(".aster").join("sessions")) else {
        return out;
    };
    for date in dates.flatten() {
        let Ok(homes) = std::fs::read_dir(date.path()) else {
            continue;
        };
        for home in homes.flatten() {
            append_jsonl_dir(&home.path(), &mut out);
        }
    }
    out
}

fn bodies_since(state: &MockState, start: usize) -> Vec<String> {
    state.bodies.iter().skip(start).cloned().collect()
}

fn first_tool_request(bodies: &[String]) -> &str {
    bodies
        .iter()
        .find(|body| {
            serde_json::from_str::<serde_json::Value>(body).is_ok_and(|value| has_tools(&value))
        })
        .map(String::as_str)
        .expect("actual tool-bearing model request")
}

async fn answer_gate(agent: &mut AgentProc, option_id: &str) {
    let gate = tokio::time::timeout(Duration::from_secs(30), agent.gates.recv())
        .await
        .expect("permission timed out")
        .expect("agent closed permission channel");
    assert!(
        gate.option_ids.iter().any(|id| id == option_id),
        "missing {option_id} in {:?}",
        gate.option_ids
    );
    gate.reply.send(option_id.to_string()).expect("reply");
}

#[tokio::test(flavor = "current_thread")]
async fn shipped_agent_refuses_repeated_reads_without_new_progress() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let root = tempfile::tempdir().unwrap();
            let workspace = root.path().join("workspace");
            let home = root.path().join("home");
            std::fs::create_dir_all(workspace.join("probe")).unwrap();
            std::fs::create_dir_all(&home).unwrap();
            let source = workspace.join("probe/stable.txt");
            std::fs::write(&source, b"unchanged").unwrap();
            let (addr, _state) = spawn_mock(&workspace.join("unused"), &workspace.join("late"));
            let config = write_acl(&home, &workspace, &format!("http://{addr}/v1"));
            let agent = connect_agent(&workspace, &config, &home).await;
            let session = acp::Agent::new_session(
                agent.conn.as_ref(),
                acp::NewSessionRequest::new(&workspace),
            )
            .await
            .unwrap();
            let error = tokio::time::timeout(
                Duration::from_secs(30),
                acp::Agent::prompt(
                    agent.conn.as_ref(),
                    user_prompt(&session.session_id, "REPEAT-LIST-ORACLE"),
                ),
            )
            .await
            .expect("a repeated read must terminate")
            .expect_err("no fresh original progress");
            assert!(
                error
                    .data
                    .as_ref()
                    .and_then(|data| data.as_str())
                    .unwrap_or("")
                    .contains("run.tool_refusal_cycle"),
                "unexpected error: {error:?}"
            );
            let thread =
                aster_core::fact_control::thread_for_session(session.session_id.0.as_ref());
            let directory = aster_core::session_folder::effect_log_dir(
                &workspace,
                session.session_id.0.as_ref(),
            );
            let facts = std::fs::read_to_string(directory.join(format!("{thread}.jsonl"))).unwrap();
            let refusal = facts
                .lines()
                .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
                .find(|fact| {
                    fact["type"] == "tool.result"
                        && fact["payload"]["receipt"]["outcome"] == "denied"
                })
                .expect("original duplicate-call denial receipt");
            assert_eq!(
                refusal["payload"]["receipt"]["effects"]["changes"],
                serde_json::json!([])
            );
            assert_eq!(std::fs::read(source).unwrap(), b"unchanged");
            stop_agent(agent).await;
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn shipped_agent_session_wire() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let workspace = isolate_dir("wire");
            let home = isolate_dir("wire-home");
            let write_path = workspace.join("side-effect.txt");
            let late_path = workspace.join("late.txt");
            let (addr, state) = spawn_mock(&write_path, &late_path);
            let config = write_acl(&home, &workspace, &format!("http://{addr}/v1"));
            let mut agent = spawn_agent(&workspace, &config, &home).await;

            let init = acp::Agent::initialize(
                agent.conn.as_ref(),
                acp::InitializeRequest::new(acp::ProtocolVersion::V1),
            )
            .await
            .expect("initialize");
            let meta = init.meta.expect("initialize meta");
            assert_eq!(
                meta.get("asterCode").and_then(serde_json::Value::as_bool),
                Some(true)
            );
            let current = meta
                .get("modelState")
                .and_then(|value| value.get("currentModelId"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            assert_eq!(current, MODEL_ID, "XAI_API_KEY must not select the model");
            oracle(WIRE_TEST, "initialize-acl-model", current);

            let created = acp::Agent::new_session(
                agent.conn.as_ref(),
                acp::NewSessionRequest::new(workspace.clone()),
            )
            .await
            .expect("new session");
            let session_model = created
                .models
                .as_ref()
                .map(|models| models.current_model_id.0.to_string())
                .unwrap_or_default();
            assert_eq!(session_model, MODEL_ID);
            oracle(WIRE_TEST, "new-session-model", &session_model);
            let session_id = created.session_id.clone();

            acp::Agent::set_session_model(
                agent.conn.as_ref(),
                acp::SetSessionModelRequest::new(session_id.clone(), acp::ModelId::new(MODEL_ID))
                    .meta(Some(effort_meta("low"))),
            )
            .await
            .expect("effort low");

            let start = state.lock().expect("mock").bodies.len();
            let mut product_prompts = 0u64;
            let listed = tokio::task::spawn_local({
                let conn = agent.conn.clone();
                let session_id = session_id.clone();
                async move {
                    acp::Agent::prompt(
                        conn.as_ref(),
                        user_prompt(&session_id, "LIST-FILES the workspace"),
                    )
                    .await
                }
            });
            let listed = listed.await.expect("join").expect("low prompt");
            assert_eq!(listed.stop_reason, acp::StopReason::EndTurn);
            assert!(agent.replayed.borrow().iter().any(|(_, _, text)| text == "listed"), "the model must observe a successful directory listing");
            product_prompts += 1;
            let info = session_info_body(agent.conn.as_ref(), &session_id).await;
            let turn_index = info["result"]["turnIndex"].as_u64().unwrap_or(0);
            let turns = info["result"]["turns"].as_u64().unwrap_or(0);
            let turn_count = info["result"]["context"]["turnCount"].as_u64().unwrap_or(0);
            let used = info["result"]["context"]["used"].as_u64().unwrap_or(0);
            assert_eq!(
                (turn_index, turns, turn_count),
                (product_prompts, product_prompts, product_prompts),
                "session-info turn does not match the prompts submitted on this session: {info}"
            );
            assert!(
                used >= 1,
                "completed turn still reports no token usage: {info}"
            );
            let low_bodies = bodies_since(&state.lock().expect("mock"), start);
            assert_eq!(first_tool_request(&low_bodies).matches("Engineering discipline").count(), 1,
                "the actual model request must carry the default policy exactly once");
            oracle(WIRE_TEST, "engineering-default", "policy present once without selection metadata");
            assert!(
                !low_bodies.is_empty(),
                "low effort produced no model request"
            );
            let low_thinking = budget_number(first_tool_request(&low_bodies), "thinking_budget=")
                .expect("thinking budget in the request the model receives");
            assert_eq!(budget_number(first_tool_request(&low_bodies), "tool_rounds=").unwrap_or(0), 0,
                "effort must not introduce a finite tool-round ceiling");
            oracle(WIRE_TEST, "session-info-turn-usage", &format!("turn={turn_index} used={used} prompts={product_prompts}"));
            assert!(
                agent
                    .tool_names
                    .borrow()
                    .iter()
                    .any(|name| !name.is_empty() && name.contains("ls")),
                "tool updates need a non-empty tool name, saw {:?}",
                agent.tool_names.borrow()
            );
            oracle(
                WIRE_TEST,
                "tool-call-name",
                &format!("low_thinking={low_thinking}"),
            );

            acp::Agent::set_session_model(
                agent.conn.as_ref(),
                acp::SetSessionModelRequest::new(session_id.clone(), acp::ModelId::new(MODEL_ID))
                    .meta(Some(effort_meta("high"))),
            )
            .await
            .expect("effort high");
            let start = state.lock().expect("mock").bodies.len();
            let high = tokio::task::spawn_local({
                let conn = agent.conn.clone();
                let session_id = session_id.clone();
                async move {
                    acp::Agent::prompt(conn.as_ref(), user_prompt(&session_id, "LIST-FILES again"))
                        .await
                }
            });
            let high = high.await.expect("join").expect("high prompt");
            assert_eq!(high.stop_reason, acp::StopReason::EndTurn);
            let high_bodies = bodies_since(&state.lock().expect("mock"), start);
            let high_thinking = budget_number(first_tool_request(&high_bodies), "thinking_budget=")
                .expect("high thinking");
            assert!(high_thinking > low_thinking);
            assert_eq!(budget_number(first_tool_request(&high_bodies), "tool_rounds=").unwrap_or(0), 0,
                "higher effort must not introduce a finite tool-round ceiling");
            oracle(WIRE_TEST, "effort-budget", &format!("low={low_thinking} high={high_thinking}"));

            let none_session = acp::Agent::new_session(
                agent.conn.as_ref(),
                acp::NewSessionRequest::new(workspace.clone()),
            )
            .await
            .expect("none session")
            .session_id;
            acp::Agent::set_session_model(
                agent.conn.as_ref(),
                acp::SetSessionModelRequest::new(none_session.clone(), acp::ModelId::new(MODEL_ID))
                    .meta(Some(effort_meta("none"))),
            )
            .await
            .expect("effort none");
            let start = state.lock().expect("mock").bodies.len();
            let budgeted = tokio::task::spawn_local({
                let conn = agent.conn.clone();
                let none_session = none_session.clone();
                async move {
                    acp::Agent::prompt(
                        conn.as_ref(),
                        user_prompt(&none_session, "LIST-FILES small budget"),
                    )
                    .await
                }
            });
            let budgeted = budgeted.await.expect("join").expect("none prompt");
            assert_eq!(budgeted.stop_reason, acp::StopReason::EndTurn);
            let none_bodies = bodies_since(&state.lock().expect("mock"), start);
            let none_thinking = budget_number(first_tool_request(&none_bodies), "thinking_budget=").expect("none thinking budget");
            assert!(none_thinking <= low_thinking, "effort none must not exceed low effort's thinking budget");
            assert_eq!(budget_number(first_tool_request(&none_bodies), "tool_rounds=").unwrap_or(0), 0,
                "effort none must not introduce a finite tool-round ceiling");
            oracle(WIRE_TEST, "effort-none", &format!("thinking={none_thinking}; no finite tool-round ceiling"));

            // Permission blocks until AllowOnce. The side-effect file stays absent.
            let perm_session = acp::Agent::new_session(
                agent.conn.as_ref(),
                acp::NewSessionRequest::new(workspace.clone()),
            )
            .await
            .expect("perm session")
            .session_id;
            let writing = tokio::task::spawn_local({
                let conn = agent.conn.clone();
                let perm_session = perm_session.clone();
                async move {
                    acp::Agent::prompt(
                        conn.as_ref(),
                        user_prompt(&perm_session, "WRITE-ORACLE side-effect.txt"),
                    )
                    .await
                }
            });
            let gate = tokio::time::timeout(Duration::from_secs(30), agent.gates.recv())
                .await
                .expect("permission")
                .expect("gate");
            assert!(
                gate.option_ids.iter().any(|id| id == "allow-once"),
                "AllowOnce id missing: {:?}",
                gate.option_ids
            );
            assert!(
                !write_path.exists(),
                "write ran before the permission was answered"
            );
            gate.reply.send("allow-once".to_string()).expect("allow");
            let wrote = writing.await.expect("join");
            // The file is the side effect. An unverified mutation must not
            // EndTurn; the pager shows this completion gate as TurnFailed.
            let detail = match &wrote {
                Ok(response) => panic!(
                    "unverified write must stay behind the completion gate, got {response:?}"
                ),
                Err(error) => error
                    .data
                    .as_ref()
                    .and_then(|value| value.as_str())
                    .unwrap_or("")
                    .to_string(),
            };
            assert!(
                detail.contains("completion gate:")
                    && detail.contains("no bound Passed verification"),
                "write turn ended without the completion gate: {detail}"
            );
            let written = std::fs::read_to_string(&write_path).expect("side effect file");
            assert_eq!(written, "wrote-marker");
            assert!(
                agent
                    .tool_names
                    .borrow()
                    .iter()
                    .any(|name| name.contains("write")),
                "write tool name missing: {:?}",
                agent.tool_names.borrow()
            );
            oracle(
                WIRE_TEST,
                "permission-blocks-until-allow-once",
                "completion-gate file=wrote-marker",
            );

            // Question stays parked until question.answered for the selected option.
            let ask_session = acp::Agent::new_session(
                agent.conn.as_ref(),
                acp::NewSessionRequest::new(workspace.clone()),
            )
            .await
            .expect("ask session")
            .session_id;
            let asking = tokio::task::spawn_local({
                let conn = agent.conn.clone();
                let ask_session = ask_session.clone();
                async move {
                    acp::Agent::prompt(
                        conn.as_ref(),
                        user_prompt(&ask_session, "ASK-ORACLE a choice"),
                    )
                    .await
                }
            });
            let gate = tokio::time::timeout(Duration::from_secs(30), agent.gates.recv())
                .await
                .expect("question")
                .expect("gate");
            assert!(
                gate.option_ids.iter().any(|id| id == "question-0"),
                "question options: {:?}",
                gate.option_ids
            );
            let parked = log_text(&workspace);
            assert!(
                !parked.contains("\"question.answered\""),
                "question settled before the answer"
            );
            gate.reply.send("question-0".to_string()).expect("answer");
            let asked = asking.await.expect("join").expect("ask prompt");
            assert_eq!(asked.stop_reason, acp::StopReason::EndTurn);
            let answered = log_text(&workspace);
            assert!(
                answered.contains("\"kind\":\"question.answered\"")
                    || answered.contains("\"question.answered\""),
                "missing question.answered fact: {answered}"
            );
            assert!(
                answered.contains("alpha"),
                "the answer fact must record the selected option"
            );
            oracle(WIRE_TEST, "question-answered", "option=alpha");

            // Cancel ends Cancelled and does not leave the late file.
            let cancel_session = acp::Agent::new_session(
                agent.conn.as_ref(),
                acp::NewSessionRequest::new(workspace.clone()),
            )
            .await
            .expect("cancel session")
            .session_id;
            let cancelling = tokio::task::spawn_local({
                let conn = agent.conn.clone();
                let cancel_session = cancel_session.clone();
                async move {
                    acp::Agent::prompt(
                        conn.as_ref(),
                        user_prompt(&cancel_session, "/goal CANCEL-ORACLE run the slow command"),
                    )
                    .await
                }
            });
            answer_gate(&mut agent, "allow-once").await;
            let saw_bash = tokio::time::timeout(Duration::from_secs(20), async {
                loop {
                    if agent
                        .tool_names
                        .borrow()
                        .iter()
                        .any(|name| name.contains("bash"))
                    {
                        return;
                    }
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
            })
            .await;
            assert!(saw_bash.is_ok(), "bash tool name was not reported");
            acp::Agent::cancel(
                agent.conn.as_ref(),
                acp::CancelNotification::new(cancel_session.clone()),
            )
            .await
            .expect("cancel");
            let cancelled = tokio::time::timeout(Duration::from_secs(20), cancelling)
                .await
                .expect("cancel prompt hung")
                .expect("join")
                .expect("cancel result");
            assert_eq!(cancelled.stop_reason, acp::StopReason::Cancelled);
            tokio::time::sleep(Duration::from_secs(5)).await;
            let cancel_thread = aster_core::fact_control::thread_for_session(cancel_session.0.as_ref());
            let cancel_facts = aster_core::fact_control::read_workspace_facts(&workspace, &cancel_thread).expect("cancel facts");
            assert_eq!(cancel_facts.iter().filter(|fact| fact.kind == "aster.goal.slice.admitted").count(), 1,
                "cancelling the goal wake must not admit another slice");
            assert!(
                !late_path.exists(),
                "cancelled turn wrote the late side-effect file"
            );
            oracle(WIRE_TEST, "cancel-no-late-file", "stop=Cancelled");

            // Compact appends compaction.done and the next turn keeps the summary.
            let message_bytes = |bodies: &[String]| {
                bodies
                    .iter()
                    .rev()
                    .map(|body| serde_json::from_str::<serde_json::Value>(body).unwrap())
                    .find(has_tools)
                    .expect("an actual request with tools")
                    ["messages"]
                    .to_string()
                    .len()
            };
            let compact_session = acp::Agent::new_session(
                agent.conn.as_ref(),
                acp::NewSessionRequest::new(workspace.clone()),
            )
            .await
            .expect("compact session")
            .session_id;
            for text in ["first note", "second note"] {
                let response =
                    acp::Agent::prompt(agent.conn.as_ref(), user_prompt(&compact_session, text))
                        .await
                        .expect("seed");
                assert_eq!(response.stop_reason, acp::StopReason::EndTurn);
            }
            let before_compaction_bytes = message_bytes(&state.lock().expect("mock").bodies);
            let params = serde_json::value::to_raw_value(&serde_json::json!({
                "sessionId": compact_session.0.as_ref(),
            }))
            .expect("params");
            acp::Agent::ext_method(
                agent.conn.as_ref(),
                acp::ExtRequest::new("aster/compact_conversation", std::sync::Arc::from(params)),
            )
            .await
            .expect("compact");
            let facts = log_text(&workspace);
            assert!(
                facts.contains("compaction.done"),
                "missing compaction.done: {facts}"
            );
            assert!(facts.contains(COMPACT_SUMMARY));
            let start = state.lock().expect("mock").bodies.len();
            acp::Agent::prompt(
                agent.conn.as_ref(),
                user_prompt(&compact_session, "continue after compact"),
            )
            .await
            .expect("post compact");
            let after = bodies_since(&state.lock().expect("mock"), start);
            assert!(
                after.iter().any(|body| body.contains(COMPACT_SUMMARY)),
                "next turn dropped the compaction summary"
            );
            oracle(WIRE_TEST, "compact-summary-kept", COMPACT_SUMMARY);
            oracle(
                WIRE_TEST,
                "compact-tool-request-message-bytes",
                &format!(
                    "before={before_compaction_bytes} after={} scope=last-request-with-tools token-count=unmeasured",
                    message_bytes(&after)
                ),
            );

            // Focus-note compact: seed a new session, compact with userContext, keep summary.
            let focus_session = acp::Agent::new_session(
                agent.conn.as_ref(),
                acp::NewSessionRequest::new(workspace.clone()),
            )
            .await
            .expect("focus compact session")
            .session_id;
            for text in [
                "auth bug in login.rs needs the middleware fix",
                "keep the session middleware patch after compact",
            ] {
                acp::Agent::prompt(agent.conn.as_ref(), user_prompt(&focus_session, text))
                    .await
                    .expect("focus seed");
            }
            let focus_params = serde_json::value::to_raw_value(&serde_json::json!({
                "sessionId": focus_session.0.as_ref(),
                "userContext": "auth",
            }))
            .expect("focus params");
            acp::Agent::ext_method(
                agent.conn.as_ref(),
                acp::ExtRequest::new(
                    "aster/compact_conversation",
                    std::sync::Arc::from(focus_params),
                ),
            )
            .await
            .expect("focus compact");
            let focus_facts = log_text(&workspace);
            assert!(
                focus_facts.contains("compaction.done"),
                "focus compact missing compaction.done: {focus_facts}"
            );
            let focus_start = state.lock().expect("mock").bodies.len();
            acp::Agent::prompt(
                agent.conn.as_ref(),
                user_prompt(&focus_session, "continue after focus compact"),
            )
            .await
            .expect("post focus compact");
            let focus_after = bodies_since(&state.lock().expect("mock"), focus_start);
            assert!(
                focus_after
                    .iter()
                    .any(|body| body.contains(COMPACT_SUMMARY)),
                "focus compact next turn dropped the summary"
            );
            oracle(WIRE_TEST, "compact-focus-note-kept", COMPACT_SUMMARY);

            // Flush/dream host methods on the live session (typed pager dispositions).
            // Seeded list/read/delete are covered by PTY + hermetic ACP deep tests.
            let flush_session = acp::Agent::new_session(
                agent.conn.as_ref(),
                acp::NewSessionRequest::new(workspace.clone()),
            )
            .await
            .expect("flush session")
            .session_id;
            let flush_params = serde_json::value::to_raw_value(&serde_json::json!({
                "session_id": flush_session.0.as_ref(),
            }))
            .expect("flush params");
            let flush = acp::Agent::ext_method(
                agent.conn.as_ref(),
                acp::ExtRequest::new("aster/memory/flush", std::sync::Arc::from(flush_params)),
            )
            .await
            .expect("memory flush");
            let flush_body: serde_json::Value =
                serde_json::from_str(flush.0.get()).expect("flush json");
            assert_eq!(flush_body["disposition"], "flushed");
            oracle(WIRE_TEST, "memory-flush", "disposition=flushed");
            let dream_params = serde_json::value::to_raw_value(&serde_json::json!({
                "session_id": flush_session.0.as_ref(),
            }))
            .expect("dream params");
            let dream = acp::Agent::ext_method(
                agent.conn.as_ref(),
                acp::ExtRequest::new("aster/memory/dream", std::sync::Arc::from(dream_params)),
            )
            .await
            .expect("memory dream");
            let dream_body: serde_json::Value =
                serde_json::from_str(dream.0.get()).expect("dream json");
            assert_eq!(dream_body["disposition"], "no_work");
            oracle(WIRE_TEST, "memory-dream", "disposition=no_work");

            // status / pause / clear do not start a model turn. set stores the objective.
            let goal_session = acp::Agent::new_session(
                agent.conn.as_ref(),
                acp::NewSessionRequest::new(workspace.clone()),
            )
            .await
            .expect("goal session")
            .session_id;
            for command in ["/goal status", "/goal pause", "/goal clear"] {
                let start = state.lock().expect("mock").bodies.len();
                acp::Agent::prompt(agent.conn.as_ref(), user_prompt(&goal_session, command))
                    .await
                    .expect(command);
                let added = state.lock().expect("mock").bodies.len() - start;
                assert_eq!(added, 0, "{command} started a model turn");
            }
            acp::Agent::prompt(
                agent.conn.as_ref(),
                user_prompt(&goal_session, "/goal ship the durable objective"),
            )
            .await
            .expect("goal set");
            let goal_folder = aster_core::session_folder::find_session_dir(
                &workspace,
                goal_session.0.as_ref(),
            )
            .expect("session folder lookup")
            .expect("goal session folder");
            let bound = goal_state(&agent, &goal_session).await;
            assert_eq!(bound["definition"]["goal"]["description"], "ship the durable objective");
            assert_eq!(bound["definition"]["plan_accepted"], true);
            assert!(bound["work_policy_digest"].is_string());
            assert!(!goal_folder.join("durable-goal.json").exists(), "new goals use original facts");
            assert!(
                !workspace.join(".aster/durable-goal.json").exists(),
                "a session goal must not become workspace-wide state"
            );
            oracle(
                WIRE_TEST,
                "goal-binding-and-quiet-verbs",
                "status/pause/clear added 0 bodies; set bound the objective and policy in original facts",
            );

            // Two live sessions must stamp their own prompt ids. A shared
            // first-session id would land on the other session's updates.
            agent.stamps.borrow_mut().clear();
            let stamp_left = acp::Agent::new_session(
                agent.conn.as_ref(),
                acp::NewSessionRequest::new(workspace.clone()),
            )
            .await
            .expect("stamp left")
            .session_id;
            let stamp_right = acp::Agent::new_session(
                agent.conn.as_ref(),
                acp::NewSessionRequest::new(workspace.clone()),
            )
            .await
            .expect("stamp right")
            .session_id;
            acp::Agent::prompt(
                agent.conn.as_ref(),
                user_prompt_id(&stamp_left, "STAMP-LEFT", "stamp-left"),
            )
            .await
            .expect("stamp left prompt");
            acp::Agent::prompt(
                agent.conn.as_ref(),
                user_prompt_id(&stamp_right, "STAMP-RIGHT", "stamp-right"),
            )
            .await
            .expect("stamp right prompt");
            let stamps = agent.stamps.borrow().clone();
            let left_key = stamp_left.0.to_string();
            let right_key = stamp_right.0.to_string();
            let left_ids: Vec<_> = stamps
                .iter()
                .filter(|(session, _)| session == &left_key)
                .map(|(_, prompt_id)| prompt_id.as_str())
                .collect();
            let right_ids: Vec<_> = stamps
                .iter()
                .filter(|(session, _)| session == &right_key)
                .map(|(_, prompt_id)| prompt_id.as_str())
                .collect();
            assert!(
                !left_ids.is_empty() && left_ids.iter().all(|id| *id == "stamp-left"),
                "left session updates were stamped with another prompt id: {stamps:?}"
            );
            assert!(
                !right_ids.is_empty() && right_ids.iter().all(|id| *id == "stamp-right"),
                "right session updates were stamped with another prompt id: {stamps:?}"
            );
            oracle(
                WIRE_TEST,
                "prompt-id-per-session",
                "left=stamp-left right=stamp-right",
            );

            // A second process reloads the same session id and still sees the token.
            acp::Agent::prompt(agent.conn.as_ref(), user_prompt(&session_id, DURABLE_TOKEN))
                .await
                .expect("seed token");
            drop(agent.conn);
            let _ = agent.child.start_kill();
            let _ = agent.child.wait().await;

            let mut resumed = spawn_agent(&workspace, &config, &home).await;
            acp::Agent::initialize(
                resumed.conn.as_ref(),
                acp::InitializeRequest::new(acp::ProtocolVersion::V1),
            )
            .await
            .expect("reinitialize");
            let loaded = acp::Agent::load_session(
                resumed.conn.as_ref(),
                acp::LoadSessionRequest::new(session_id.clone(), workspace.clone()),
            )
            .await
            .expect("load session");
            let replayed = resumed.replayed.borrow().clone();
            let loaded_key = session_id.0.to_string();
            assert!(
                replayed.iter().any(|(sid, is_replay, text)| {
                    sid == &loaded_key && *is_replay && text.contains(DURABLE_TOKEN)
                }),
                "load_session did not replay the durable transcript: {replayed:?}"
            );
            let loaded_model = loaded
                .models
                .as_ref()
                .map(|models| models.current_model_id.0.to_string())
                .unwrap_or_default();
            assert_eq!(loaded_model, MODEL_ID);
            let start = state.lock().expect("mock").bodies.len();
            acp::Agent::prompt(
                resumed.conn.as_ref(),
                user_prompt(&session_id, "what was stored"),
            )
            .await
            .expect("resumed prompt");
            let resumed_bodies = bodies_since(&state.lock().expect("mock"), start);
            assert!(
                resumed_bodies
                    .iter()
                    .any(|body| body.contains(DURABLE_TOKEN)),
                "resume opened a blank session"
            );
            oracle(WIRE_TEST, "load-session-replay", DURABLE_TOKEN);

            drop(resumed.conn);
            let _ = resumed.child.start_kill();
            let _ = resumed.child.wait().await;
            let _ = std::fs::remove_dir_all(&workspace);
            let _ = std::fs::remove_dir_all(&home);
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn shipped_agent_sends_aster_identity_to_the_model() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let root = tempfile::tempdir().expect("fixture");
            let workspace = root.path().join("workspace");
            let home = root.path().join("home");
            std::fs::create_dir_all(&workspace).expect("workspace");
            std::fs::create_dir_all(&home).expect("home");
            let (addr, state) = spawn_mock(&workspace.join("unused"), &workspace.join("late"));
            let config = write_acl(&home, &workspace, &format!("http://{addr}/v1"));
            let agent = connect_agent(&workspace, &config, &home).await;
            let session = acp::Agent::new_session(
                agent.conn.as_ref(),
                acp::NewSessionRequest::new(workspace),
            )
            .await
            .expect("session");
            tokio::time::timeout(
                Duration::from_secs(30),
                acp::Agent::prompt(
                    agent.conn.as_ref(),
                    user_prompt(&session.session_id, "Hello"),
                ),
            )
            .await
            .expect("bounded turn")
            .expect("prompt");
            let bodies = state.lock().expect("mock").bodies.clone();
            let body = bodies
                .iter()
                .rev()
                .find_map(|request| {
                    let body: serde_json::Value = serde_json::from_str(request).ok()?;
                    (body["messages"].as_array()?.last()?["content"] == "Hello").then_some(body)
                })
                .expect("actual product model request");
            let system = body["messages"]
                .as_array()
                .expect("messages")
                .iter()
                .filter(|message| message["role"] == "system")
                .map(|message| message["content"].to_string())
                .collect::<Vec<_>>()
                .join("\n");
            assert!(
                system.contains("You are Aster, an expert AI coding agent"),
                "Aster identity must reach the model"
            );
            assert!(
                !system.contains("You are Aster Code"),
                "the inherited identity must be replaced"
            );
            assert!(
                system.contains("## Boundaries") && system.contains("## Runtime Contract"),
                "branding must retain safety and runtime instructions"
            );
            stop_agent(agent).await;
        })
        .await;
}

#[test]
fn shipped_tui_reports_configuration_failure_before_the_connect_budget() {
    let root = tempfile::tempdir().expect("fixture");
    let home = root.path().join("home");
    let workspace = root.path().join("workspace");
    std::fs::create_dir_all(&home).expect("home");
    std::fs::create_dir_all(&workspace).expect("workspace");
    let config = home.join("host.acl");
    std::fs::write(&config, "default_model = \"missing/model\"\n").expect("host ACL");
    let binary = tui_binary();
    let home_text = home.to_string_lossy().to_string();
    let config_text = config.to_string_lossy().to_string();
    let acp_binary = binaries::acp_binary();
    let env = [
        ("HOME", home_text.as_str()),
        ("ASTER_HOME", home_text.as_str()),
        ("ASTER_CONFIG", config_text.as_str()),
        ("ASTER_ACP_AGENT_BIN", acp_binary.as_str()),
        ("ASTER_CONNECT_TIMEOUT_SECS", "60"),
    ];
    let started = Instant::now();
    let mut pty = open_pty_cleared(&binary, &[], &env, &workspace);
    let status = loop {
        if let Some(status) = pty.child.try_wait().expect("child status") {
            break status;
        }
        if started.elapsed() >= Duration::from_secs(15) {
            let screen = pty.screen();
            let _ = pty.child.kill();
            let _ = pty.child.wait();
            panic!("an invalid ACP configuration must not consume the connect budget\n{screen}");
        }
        pty.pump(Duration::from_millis(40));
    };
    pty.drain();
    let raw = String::from_utf8_lossy(&pty.raw);
    assert!(!status.success(), "an invalid host catalog cannot start");
    assert!(
        raw.contains("Cannot load trusted ACL configuration") && raw.contains("missing/model"),
        "{raw}"
    );
    assert!(
        raw.contains("trusted ACL") && raw.contains("ASTER_CONFIG"),
        "{raw}"
    );
    assert!(!raw.contains("startup timed out"), "{raw}");
    oracle(
        "shipped_tui_reports_configuration_failure_before_the_connect_budget",
        "agent-exit",
        "configuration refusal closes startup with trusted-ACL guidance before the connect budget",
    );
}

#[test]
fn shipped_tui_keeps_a_bounded_wait_for_a_running_unresponsive_agent() {
    let root = tempfile::tempdir().expect("fixture");
    let agent = root.path().join("unresponsive-acp");
    std::fs::write(&agent, "#!/bin/sh\nexec /bin/sleep 30\n").expect("agent fixture");
    std::fs::set_permissions(&agent, std::fs::Permissions::from_mode(0o755)).expect("executable");
    let agent_text = agent.to_string_lossy().to_string();
    let binary = tui_binary();
    for variable in ["ASTER_CONNECT_TIMEOUT_SECS", "GROK_CONNECT_UI_TIMEOUT_SECS"] {
        let home = root.path().join(variable);
        std::fs::create_dir_all(&home).expect("home");
        let home_text = home.to_string_lossy().to_string();
        let env = [
            ("HOME", home_text.as_str()),
            ("ASTER_HOME", home_text.as_str()),
            ("ASTER_ACP_AGENT_BIN", agent_text.as_str()),
            (variable, "1"),
        ];
        let started = Instant::now();
        let mut pty = open_pty_cleared(&binary, &[], &env, root.path());
        let status = loop {
            if let Some(status) = pty.child.try_wait().expect("child status") {
                break status;
            }
            if started.elapsed() >= Duration::from_secs(22) {
                let screen = pty.screen();
                let _ = pty.child.kill();
                let _ = pty.child.wait();
                panic!("{variable} must retain the bounded startup wait\n{screen}");
            }
            pty.pump(Duration::from_millis(40));
        };
        pty.drain();
        let raw = String::from_utf8_lossy(&pty.raw);
        assert!(!status.success(), "an unresponsive agent cannot start");
        assert!(raw.contains("startup timed out"), "{raw}");
        assert!(raw.contains("ASTER_CONNECT_TIMEOUT_SECS=60 aster"), "{raw}");
    }
    oracle(
        "shipped_tui_keeps_a_bounded_wait_for_a_running_unresponsive_agent",
        "live-agent-timeout",
        "native and legacy overrides preserve the timeout and show the native retry command",
    );
}

struct PtySession {
    parser: vt100::Parser,
    rx: std::sync::mpsc::Receiver<Vec<u8>>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
    raw: Vec<u8>,
    /// Raw offset already answered with a cursor-position report.
    cpr_replied_through: usize,
}

impl Drop for PtySession {
    fn drop(&mut self) {
        if matches!(self.child.try_wait(), Ok(None)) {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
    }
}

fn tui_binary() -> PathBuf {
    if let Some(path) = std::env::var_os("ASTER_TUI_BIN") {
        return PathBuf::from(path);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../target/debug/aster")
}

fn open_pty(binary: &Path, args: &[&str], env: &[(&str, &str)], cwd: &Path) -> PtySession {
    open_pty_inner(binary, args, env, cwd, false)
}

/// Same as [`open_pty`], after dropping the parent environment.
/// portable-pty still injects `SHELL` from the password database.
fn open_pty_cleared(binary: &Path, args: &[&str], env: &[(&str, &str)], cwd: &Path) -> PtySession {
    open_pty_inner(binary, args, env, cwd, true)
}

fn open_pty_inner(
    binary: &Path,
    args: &[&str],
    env: &[(&str, &str)],
    cwd: &Path,
    clear_env: bool,
) -> PtySession {
    let rows = 40;
    let cols = 120;
    let system = native_pty_system();
    let pair = system
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("pty");
    let mut command = CommandBuilder::new(binary);
    if clear_env {
        command.env_clear();
    }
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
        // Inline mode asks the terminal where the cursor is (`CSI 6 n`).
        // A real terminal answers; this PTY has to do the same or the switch aborts.
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
        if let Err(error) = self.writer.write_all(bytes) {
            let screen = self.screen();
            let status = self.child.try_wait();
            panic!("pty write {bytes:?}: {error}; child={status:?}\n{screen}");
        }
        self.writer.flush().ok();
    }

    fn type_text(&mut self, text: &str) {
        self.write_bytes(text.as_bytes());
    }

    fn enter(&mut self) {
        self.write_bytes(b"\r");
    }

    fn escape(&mut self) {
        self.write_bytes(b"\x1b");
    }

    /// Close a modal or scrollback search.
    /// A lone Escape is held until the sequence timeout, so the next byte
    /// has to wait or it is swallowed as part of that sequence.
    fn dismiss(&mut self) {
        for _ in 0..3 {
            let screen = self.screen();
            let open = screen.contains("[✗]")
                || screen.contains("search:")
                || screen.contains("Esc close")
                || screen.contains("Esc:cancel");
            if !open {
                return;
            }
            self.escape();
            std::thread::sleep(Duration::from_millis(400));
            self.pump(Duration::from_millis(80));
        }
    }

    /// Vim scrollback keeps `/` as search. `i` returns to the prompt; on the
    /// prompt it is just a letter, which Ctrl-U clears.
    fn focus_composer(&mut self) {
        self.dismiss();
        self.write_bytes(b"i");
        std::thread::sleep(Duration::from_millis(80));
        self.write_bytes(b"\x15");
        self.pump(Duration::from_millis(80));
    }

    fn wait_for_frame(&mut self, timeout: Duration) -> String {
        let deadline = Instant::now() + timeout;
        let mut last = String::new();
        let mut welcome_since: Option<Instant> = None;
        while Instant::now() < deadline {
            self.drain();
            let screen = self.parser.screen().contents();
            if tui_readiness::is_input_frame(&screen) {
                if welcome_since.is_some_and(|start| start.elapsed() > Duration::from_millis(200)) {
                    return screen;
                }
                welcome_since.get_or_insert_with(Instant::now);
            } else {
                welcome_since = None;
            }
            last = screen;
            std::thread::sleep(Duration::from_millis(40));
        }
        panic!("timed out waiting for a product frame\n{last}")
    }

    fn wait_for(&mut self, needle: &str, timeout: Duration) -> String {
        let deadline = Instant::now() + timeout;
        let mut last = String::new();
        while Instant::now() < deadline {
            self.drain();
            last = self.parser.screen().contents();
            if last.contains(needle) {
                return last;
            }
            std::thread::sleep(Duration::from_millis(40));
        }
        panic!("timed out waiting for {needle:?}\n{last}")
    }

    fn open_session_info(&mut self, timeout: Duration) -> String {
        let deadline = Instant::now() + timeout;
        self.focus_composer();
        self.type_text("/session-info");
        self.pump(Duration::from_millis(100));
        self.enter();
        let mut last = String::new();
        while Instant::now() < deadline {
            last = self.pump(Duration::from_millis(80));
            if last.contains("Session ID") {
                return last;
            }
            // The welcome frame precedes the asynchronous session response.
            if last.contains("No active session") {
                self.focus_composer();
                self.type_text("/session-info");
                self.enter();
            } else if last
                .lines()
                .any(|line| line.trim_start().starts_with("→ /session-info"))
            {
                self.enter();
            }
        }
        panic!("session info did not become available\n{last}")
    }
}

/// `default_model` as written into the ACL file, not a hard-coded id.
fn acl_default_model(config: &Path) -> String {
    aster_core::CodeConfig::from_file(config)
        .unwrap_or_else(|_| panic!("invalid trusted ACL configuration"))
        .default_model
        .expect("ACL must declare its default model")
}

/// Model id on the session-info row. `Model Hash` is a different label.
fn model_field(screen: &str) -> Option<String> {
    screen.lines().find_map(|line| {
        let rest = line.split_once("Model:")?.1;
        rest.split_whitespace()
            .map(|word| {
                word.trim_matches(|ch: char| {
                    !ch.is_ascii_alphanumeric() && ch != '/' && ch != '-' && ch != '_' && ch != '.'
                })
            })
            .find(|word| word.contains('/'))
            .map(str::to_string)
    })
}

/// Launch necessities plus one credential. No model selector and no importer switch.
fn key_only_env(acp: &str, config: &Path, home: &Path) -> Vec<(String, String)> {
    vec![
        ("ASTER_ACP_AGENT_BIN".into(), acp.into()),
        ("ASTER_CONFIG".into(), config.display().to_string()),
        ("XAI_API_KEY".into(), "sk-dummy-not-a-model".into()),
        ("HOME".into(), home.display().to_string()),
        ("GROK_HOME".into(), home.join("grok").display().to_string()),
        ("TERM".into(), "xterm-256color".into()),
    ]
}

const KEY_ONLY_FORBIDDEN: &[&str] = &[
    "ASTER_DEFAULT_MODEL",
    "ASTER_PREFER_GROK",
    "ASTER_DISABLE_GROK_ACCOUNT",
    "ASTER_GROK_HOME",
    "ANTHROPIC_API_KEY",
    "ANTHROPIC_MODEL",
    "OPENAI_API_KEY",
    "OPENAI_MODEL",
];

fn child_env(acp: &str, config: &Path, home: &Path) -> Vec<(String, String)> {
    vec![
        ("ASTER_ACP_AGENT_BIN".into(), acp.into()),
        ("ASTER_CONFIG".into(), config.display().to_string()),
        ("ASTER_DISABLE_GROK_ACCOUNT".into(), "1".into()),
        ("ASTER_DISABLE_CURSOR_ACCOUNT".into(), "1".into()),
        ("XAI_API_KEY".into(), "sk-dummy-not-a-model".into()),
        ("HOME".into(), home.display().to_string()),
        ("GROK_HOME".into(), home.join("grok").display().to_string()),
        ("TERM".into(), "xterm-256color".into()),
    ]
}

fn line_session_id(line: &str) -> Option<String> {
    line.split_whitespace().find_map(|word| {
        let token = word.trim_matches(|ch: char| !ch.is_ascii_hexdigit() && ch != '-');
        looks_like_session_id(token).then(|| token.to_string())
    })
}

fn session_id_from_screen(screen: &str) -> String {
    let lines: Vec<&str> = screen.lines().collect();
    for (index, line) in lines.iter().enumerate() {
        if !line.contains("Session ID") {
            continue;
        }
        if let Some(id) = line_session_id(line) {
            return id;
        }
        // The modal puts the label and the id on separate rows, with a box
        // glyph as the first word of the value row.
        for next in lines.iter().skip(index + 1).take(4) {
            if next.contains("Working directory") || next.contains("Model:") {
                break;
            }
            if let Some(id) = line_session_id(next) {
                return id;
            }
        }
    }
    String::new()
}

fn palette_offers(screen: &str, name: &str, description: &str) -> bool {
    screen.lines().any(|line| {
        let trimmed = line.trim();
        let has_description = description != name && !description.eq_ignore_ascii_case(name);
        // Palette rows are `❯ /name description`. The composer echoes the
        // typed command inside a box (`│ ❯ /name │`), so that line's only
        // alphanumeric token is the name. A row still counts when the name
        // shares the line with any other word.
        let tokens: Vec<&str> = trimmed
            .split_whitespace()
            .map(|word| word.trim_matches(|ch: char| !ch.is_ascii_alphanumeric() && ch != '-'))
            .filter(|word| !word.is_empty())
            .collect();
        let name_hits = tokens.iter().filter(|word| **word == name).count();
        name_hits > 0
            && tokens.len() > name_hits
            && (!has_description || trimmed.contains(description))
    })
}

#[test]
fn palette_rows_keep_the_declared_handler_description() {
    let screen = "❯ /tasks Inspect background tasks or stop a task\n│ ❯ /tasks │";
    assert!(palette_offers(
        screen,
        "tasks",
        "Inspect background tasks or stop a task"
    ));
    assert!(!palette_offers(
        screen,
        "tasks",
        "List background tasks, subagents, and scheduled tasks"
    ));
    assert!(!palette_offers(
        "│ ❯ /tasks │",
        "tasks",
        "Inspect background tasks or stop a task"
    ));
    assert!(!palette_offers(
        screen,
        "memory",
        "Inspect background tasks or stop a task"
    ));
}

fn config_texts(home: &Path) -> String {
    [
        home.join("grok").join("config.toml"),
        home.join(".grok").join("config.toml"),
    ]
    .iter()
    .filter_map(|path| std::fs::read_to_string(path).ok())
    .collect::<Vec<_>>()
    .join("\n")
}

fn wait_config(home: &Path, needle: &str, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if config_texts(home).contains(needle) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(40));
    }
    false
}

fn summary_for(home: &Path, session_id: &str) -> Option<PathBuf> {
    fn walk(dir: &Path, session_id: &str, depth: usize, out: &mut Option<PathBuf>) {
        if depth > 6 || out.is_some() {
            return;
        }
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            if path.file_name().and_then(|name| name.to_str()) == Some(session_id) {
                let summary = path.join("summary.json");
                if summary.is_file() {
                    *out = Some(summary);
                    return;
                }
            }
            walk(&path, session_id, depth + 1, out);
        }
    }
    let mut out = None;
    walk(home, session_id, 0, &mut out);
    out
}

fn wait_goal_fact(pty: &mut PtySession, workspace: &Path, kind: &str) -> serde_json::Value {
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        pty.pump(Duration::from_millis(100));
        if let Some(fact) = log_text(workspace)
            .lines()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .find(|fact| fact["type"] == kind)
        {
            return fact;
        }
    }
    panic!("missing {kind} in original goal facts\n{}", pty.screen());
}

fn wait_file(path: &Path, timeout: Duration) -> String {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if let Ok(text) = std::fs::read_to_string(path) {
            if !text.is_empty() {
                return text;
            }
        }
        std::thread::sleep(Duration::from_millis(40));
    }
    std::fs::read_to_string(path).unwrap_or_default()
}

fn wait_body(state: &Arc<Mutex<MockState>>, needle: &str, timeout: Duration) -> String {
    wait_body_all(state, &[needle], timeout, false)
}

fn wait_body_all(
    state: &Arc<Mutex<MockState>>,
    needles: &[&str],
    timeout: Duration,
    tools_only: bool,
) -> String {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if let Some(body) = state
            .lock()
            .expect("mock")
            .bodies
            .iter()
            .rev()
            .find(|body| {
                needles.iter().all(|needle| body.contains(needle))
                    && (!tools_only
                        || serde_json::from_str::<serde_json::Value>(body)
                            .is_ok_and(|value| has_tools(&value)))
            })
        {
            return body.clone();
        }
        std::thread::sleep(Duration::from_millis(40));
    }
    let last = state
        .lock()
        .expect("mock")
        .bodies
        .last()
        .cloned()
        .unwrap_or_default();
    let excerpt: String = last.chars().take(400).collect();
    panic!("no model request contained {needles:?}\nlast body: {excerpt}");
}

fn wait_prompt_response(pty: &mut PtySession, trace: &Path, prompt: &str) {
    let digest =
        aster_core::content_digest::digest_bytes("aster.test.tui-prompt.v1", prompt.as_bytes());
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        if let Ok(text) = std::fs::read_to_string(trace) {
            for row in text
                .lines()
                .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            {
                if row["kind"] == "prompt-response" && row["prompt_digest"] == digest {
                    assert!(row["error_code"].is_null(), "prompt failed: {row}");
                    assert_eq!(
                        row["stop_reason"], "end_turn",
                        "prompt did not settle: {row}"
                    );
                    return;
                }
            }
        }
        pty.pump(Duration::from_millis(50));
    }
    panic!("no original ACP response for {prompt}: {}", pty.screen());
}

fn looks_like_session_id(token: &str) -> bool {
    token.len() >= 8 && token.chars().all(|ch| ch.is_ascii_hexdigit() || ch == '-')
}

fn launch_cleared_session(env: &[(&str, &str)], cwd: &Path) -> (String, String, String) {
    let binary = tui_binary();
    assert!(
        binary.is_file(),
        "shipped aster is missing at {}",
        binary.display()
    );
    let args = ["--cwd", cwd.to_str().expect("cwd")];
    let mut pty = open_pty_cleared(&binary, &args, env, cwd);
    let ready = pty.wait_for_frame(Duration::from_secs(45));
    assert!(
        ready.chars().any(|ch| !ch.is_whitespace()),
        "cleared launch produced an empty product frame"
    );
    let info = pty.open_session_info(Duration::from_secs(15));
    let session_id = session_id_from_screen(&info);
    let model = model_field(&info).unwrap_or_default();
    pty.escape();
    pty.pump(Duration::from_millis(200));
    pty.type_text("/quit");
    pty.enter();
    let _ = pty.child.wait();
    (session_id, model, info)
}

fn launch_once(continue_last: bool, env: &[(&str, &str)], cwd: &Path) -> (String, String) {
    let binary = tui_binary();
    assert!(
        binary.is_file(),
        "shipped aster is missing at {}",
        binary.display()
    );
    let mut args = vec!["--cwd", cwd.to_str().expect("cwd")];
    if continue_last {
        args.push("--continue");
    }
    let mut pty = open_pty(&binary, &args, env, cwd);
    let ready = pty.wait_for_frame(Duration::from_secs(45));
    assert!(
        ready.chars().any(|ch| !ch.is_whitespace()),
        "launch produced an empty product frame"
    );
    let info = pty.open_session_info(Duration::from_secs(15));
    assert!(
        info.contains(MODEL_ID),
        "session info did not show the ACL model\n{info}"
    );
    let session_id = session_id_from_screen(&info);
    assert!(
        !session_id.is_empty(),
        "session info had no session id\n{info}"
    );
    pty.escape();
    pty.pump(Duration::from_millis(200));
    pty.type_text("/quit");
    pty.enter();
    let _ = pty.child.wait();
    (session_id, info)
}

fn session_info_turn_and_used(screen: &str) -> Option<(u64, u64)> {
    let mut turn = None;
    let mut used = None;
    for line in screen.lines() {
        // The session-info modal paints a box glyph before the label.
        if let Some(rest) = line.split_once("Turn:").map(|(_, rest)| rest) {
            if let Some(parsed) = rest
                .split_whitespace()
                .next()
                .and_then(|token| token.parse().ok())
            {
                turn = Some(parsed);
            }
        }
        if let Some(rest) = line.split_once("Context:").map(|(_, rest)| rest) {
            if let Some(parsed) = rest
                .split(|ch: char| !ch.is_ascii_digit())
                .find(|token| !token.is_empty())
                .and_then(|token| token.parse().ok())
            {
                used = Some(parsed);
            }
        }
    }
    Some((turn?, used?))
}

/// One real turn, then quit. Returns the session id the pager persisted.
fn seed_marked_session(
    env: &[(&str, &str)],
    cwd: &Path,
    state: &Arc<Mutex<MockState>>,
    marker: &str,
) -> String {
    let binary = tui_binary();
    let cwd_arg = cwd.to_str().expect("cwd");
    let args = ["--cwd", cwd_arg];
    let mut pty = open_pty(&binary, &args, env, cwd);
    pty.wait_for_frame(Duration::from_secs(45));
    pty.type_text(marker);
    pty.enter();
    let _ = wait_body(state, marker, Duration::from_secs(25));
    let seeded = pty.wait_for(marker, Duration::from_secs(15));
    assert!(
        seeded.contains(marker),
        "seeded session did not keep the transcript token\n{seeded}"
    );
    pty.write_bytes(b"\x15");
    let info = pty.open_session_info(Duration::from_secs(15));
    assert!(
        info.contains(MODEL_ID),
        "seeded session did not show the ACL model\n{info}"
    );
    let session_id = session_id_from_screen(&info);
    assert!(
        !session_id.is_empty(),
        "seeded session info had no session id\n{info}"
    );
    pty.escape();
    pty.pump(Duration::from_millis(200));
    pty.type_text("/quit");
    pty.enter();
    let _ = pty.child.wait();
    session_id
}

fn launch_resumed(
    session_id: &str,
    env: &[(&str, &str)],
    cwd: &Path,
    marker: &str,
) -> (String, String) {
    let binary = tui_binary();
    let cwd_arg = cwd.to_str().expect("cwd");
    let args = ["--cwd", cwd_arg, "--resume", session_id];
    let mut pty = open_pty(&binary, &args, env, cwd);
    let resumed = pty.wait_for(marker, Duration::from_secs(30));
    assert!(
        resumed.contains(marker),
        "--resume did not reopen the prior transcript\n{resumed}"
    );
    assert!(
        resumed.contains(MODEL_ID),
        "--resume did not show the ACL model\n{resumed}"
    );
    let info = pty.open_session_info(Duration::from_secs(15));
    let resumed_id = session_id_from_screen(&info);
    assert_eq!(
        resumed_id, session_id,
        "--resume opened a different session\n{info}"
    );
    pty.escape();
    pty.pump(Duration::from_millis(200));
    pty.type_text("/quit");
    pty.enter();
    let _ = pty.child.wait();
    (resumed_id, resumed)
}

fn exercise_marker_navigation(
    env: &[(&str, &str)],
    cwd: &Path,
    session_id: &str,
    marker: &str,
    state: &Arc<Mutex<MockState>>,
) {
    let binary = tui_binary();
    let cwd_arg = cwd.to_str().expect("cwd");
    let args = ["--cwd", cwd_arg, "--resume", session_id];
    let home = PathBuf::from(
        env.iter()
            .find(|(key, _)| *key == "HOME")
            .expect("fixture home")
            .1,
    );
    let relay = home.join("navigation-acp-relay");
    let trace = home.join("navigation-acp-trace.jsonl");
    std::fs::write(
        &relay,
        include_str!("../../../../apps/aster/tests/tui_913_real_llm/acp_trace_proxy.py").replacen(
            "#!/usr/bin/python3",
            &format!("#!{}", python::executable().display()),
            1,
        ),
    )
    .expect("ACP relay");
    std::fs::set_permissions(&relay, std::fs::Permissions::from_mode(0o700)).expect("relay mode");
    let real_acp = env
        .iter()
        .find(|(key, _)| *key == "ASTER_ACP_AGENT_BIN")
        .expect("selected ACP")
        .1;
    let mut owned: Vec<(String, String)> = env
        .iter()
        .filter(|(key, _)| *key != "ASTER_ACP_AGENT_BIN")
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect();
    owned.extend([
        ("ASTER_ACP_AGENT_BIN".into(), relay.display().to_string()),
        ("ASTER_TUI_TEST_ACP_REAL".into(), real_acp.into()),
        (
            "ASTER_TUI_TEST_ACP_TRACE".into(),
            trace.display().to_string(),
        ),
    ]);
    let env: Vec<_> = owned
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect();
    let mut pty = open_pty(&binary, &args, &env, cwd);
    let opened = pty.wait_for(marker, Duration::from_secs(30));
    assert!(
        opened.contains(marker),
        "marked session did not show the transcript\n{opened}"
    );

    pty.write_bytes(b"\x15");
    pty.type_text(&format!("/find {marker}"));
    pty.enter();
    let found = pty.wait_for("search:", Duration::from_secs(6));
    assert!(
        found.contains("search:") && found.contains(marker),
        "find did not show the marker\n{found}"
    );
    oracle(PTY_TEST, "find-marker", marker);

    // The picker opens only when the timeline has two turns. One turn toasts
    // "Nothing to jump to yet" and never paints the title.
    pty.focus_composer();
    pty.type_text("JUMP-SECOND-7c1a");
    pty.enter();
    let _ = wait_body(state, "JUMP-SECOND-7c1a", Duration::from_secs(25));
    let _second = pty.wait_for("JUMP-SECOND-7c1a", Duration::from_secs(15));

    pty.focus_composer();
    pty.type_text("/jump");
    pty.enter();
    let jump = pty.wait_for("Jump to which turn?", Duration::from_secs(6));
    assert!(
        jump.contains(marker),
        "jump preview omitted the marker\n{jump}"
    );
    // Oldest row is the marked turn. Up is a no-op when the picker already opened there.
    pty.write_bytes(b"k");
    pty.pump(Duration::from_millis(200));
    let selected = pty.screen();
    assert!(
        selected.contains("Jump to which turn?") && selected.contains(marker),
        "jump lost the marker row\n{selected}"
    );
    pty.enter();
    pty.pump(Duration::from_millis(300));
    pty.focus_composer();
    let info = pty.open_session_info(Duration::from_secs(15));
    let (turn, _) = session_info_turn_and_used(&info).unwrap_or((0, 0));
    // Turn counts user.message facts. The marker plus this second prompt is 2.
    // It is not the viewport row the picker selected.
    assert_eq!(
        turn, 2,
        "session-info turn is the product prompt count\n{info}"
    );
    oracle(PTY_TEST, "jump-turn", "preview=marker prompts=2");
    pty.escape();
    pty.pump(Duration::from_millis(200));
    pty.focus_composer();

    // One scoped host approval covers this exact execution request.
    let before_expand = state.lock().expect("mock").bodies.len();
    pty.type_text("EXPAND-ORACLE");
    pty.enter();
    let allow_expand = pty.wait_for("Yes, run on host", Duration::from_secs(20));
    assert!(
        allow_expand.contains("Allow Execute?") && allow_expand.contains("Ctrl+o:YOLO"),
        "execution approval must own the keyboard\n{allow_expand}"
    );
    assert!(
        !log_text(cwd)
            .lines()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .any(|fact| fact["type"] == "tool.result"),
        "execution must stay parked until approval"
    );
    pty.write_bytes(b"1");
    let expand_deadline = Instant::now() + Duration::from_secs(25);
    while Instant::now() < expand_deadline {
        if state.lock().expect("mock").bodies.len() > before_expand.saturating_add(1) {
            break;
        }
        pty.pump(Duration::from_millis(100));
    }
    assert!(
        state.lock().expect("mock").bodies.len() > before_expand.saturating_add(1),
        "expand bash did not finish\n{}",
        pty.screen()
    );
    let requests: Vec<serde_json::Value> = std::fs::read_to_string(&trace)
        .expect("approval trace")
        .lines()
        .map(|line| serde_json::from_str(line).expect("trace record"))
        .filter(|event: &serde_json::Value| event["kind"] == "permission-request")
        .collect();
    assert_eq!(
        requests.len(),
        1,
        "one scoped request must settle the original call"
    );
    assert!(requests
        .first()
        .is_some_and(|request| !request["rpc_id"].is_null()));
    let receipt = log_text(cwd)
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .find(|fact| fact["type"] == "tool.result")
        .expect("the expansion command must have an original execution receipt")["payload"]
        ["receipt"]
        .clone();
    assert_eq!(receipt["outcome"], "succeeded", "{receipt}");
    let output: serde_json::Value =
        serde_json::from_str(receipt["output"].as_str().expect("wire result"))
            .expect("tool result");
    assert_eq!(output["metadata"]["exit_code"], 0, "{output}");
    assert!(
        output["text"]
            .as_str()
            .is_some_and(|text| text.contains("EXPAND-MARKER")),
        "{output}"
    );
    pty.write_bytes(b"\x15");
    pty.type_text("/minimal");
    pty.enter();
    let _minimal = pty.wait_for("Switched to minimal mode", Duration::from_secs(6));
    pty.pump(Duration::from_millis(800));
    pty.write_bytes(b"\x15");
    pty.type_text("/expand");
    pty.enter();
    let expanded = pty.wait_for("EXPAND-MARKER", Duration::from_secs(8));
    assert!(
        expanded.contains("EXPAND-MARKER") && !expanded.contains("Nothing to expand"),
        "expand did not reprint the folded tool output\n{expanded}"
    );
    oracle(PTY_TEST, "expand-marker", "EXPAND-MARKER");

    // Minimal mode quits on a second Ctrl-C while the prompt is idle and empty.
    // A slash `/quit` clears the composer and leaves the process up.
    pty.write_bytes(b"\x15");
    pty.pump(Duration::from_millis(80));
    pty.write_bytes(b"\x03");
    pty.pump(Duration::from_millis(200));
    pty.write_bytes(b"\x03");
    let quit_deadline = Instant::now() + Duration::from_secs(8);
    loop {
        pty.pump(Duration::from_millis(80));
        match pty.child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() >= quit_deadline => {
                pty.child.kill().ok();
                let _ = pty.child.wait();
                break;
            }
            Ok(None) => {}
            Err(error) => panic!("quit wait failed: {error}"),
        }
    }
}

fn launch_continued(
    expected_id: &str,
    env: &[(&str, &str)],
    cwd: &Path,
    marker: &str,
) -> (String, String) {
    let binary = tui_binary();
    let cwd_arg = cwd.to_str().expect("cwd");
    let args = ["--cwd", cwd_arg, "--continue"];
    let mut pty = open_pty(&binary, &args, env, cwd);
    let continued = pty.wait_for(marker, Duration::from_secs(30));
    assert!(
        continued.contains(marker),
        "--continue did not reopen the prior transcript\n{continued}"
    );
    assert!(
        continued.contains(MODEL_ID),
        "--continue did not show the ACL model\n{continued}"
    );
    let info = pty.open_session_info(Duration::from_secs(15));
    let continued_id = session_id_from_screen(&info);
    assert_eq!(
        continued_id, expected_id,
        "--continue opened a different session\n{info}"
    );
    pty.escape();
    pty.pump(Duration::from_millis(200));
    pty.type_text("/quit");
    pty.enter();
    let _ = pty.child.wait();
    (continued_id, continued)
}

#[test]
fn shipped_tui_xai_key_keeps_acl_model() {
    let binary = tui_binary();
    assert!(
        binary.is_file(),
        "build aster before this test: {}",
        binary.display()
    );
    let workspace = isolate_dir("xai-key");
    let home = isolate_dir("xai-key-home");
    let grok = home.join("grok");
    std::fs::create_dir_all(&grok).expect("grok home");
    assert!(
        !grok.join("auth.json").is_file(),
        "hermetic home must not contain grok auth"
    );
    let (addr, _state) = spawn_mock(
        &workspace.join("side-effect.txt"),
        &workspace.join("late.txt"),
    );
    let config = write_acl(&home, &workspace, &format!("http://{addr}/v1"));
    let acl_model = acl_default_model(&config);
    assert!(
        acl_model.contains('/'),
        "ACL default_model must be provider/model, got {acl_model}"
    );
    let owned = key_only_env(&binaries::acp_binary(), &config, &home);
    assert!(
        owned
            .iter()
            .any(|(key, value)| key == "XAI_API_KEY" && !value.is_empty()),
        "XAI_API_KEY missing from the cleared launch"
    );
    for (key, _) in &owned {
        assert!(
            !KEY_ONLY_FORBIDDEN.contains(&key.as_str()),
            "cleared launch set a model selector: {key}"
        );
    }
    let credential_keys = owned
        .iter()
        .filter(|(key, _)| key.ends_with("API_KEY") || key.contains("API_KEY"))
        .count();
    assert_eq!(
        credential_keys, 1,
        "only XAI_API_KEY may be set among credentials: {owned:?}"
    );
    let env: Vec<(&str, &str)> = owned
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect();
    let (session_id, shown, info) = launch_cleared_session(&env, &workspace);
    assert!(
        !session_id.is_empty(),
        "session info had no session id\n{info}"
    );
    assert_eq!(
        shown, acl_model,
        "XAI_API_KEY replaced the merged ACL default\n{info}"
    );
    oracle(
        "shipped_tui_xai_key_keeps_acl_model",
        "xai-key-keeps-acl-model",
        &format!("model={shown} session={session_id}"),
    );
    let _ = std::fs::remove_dir_all(&workspace);
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn shipped_tui_prefix_fork_inherits_history_and_resumes_the_child() {
    let root = tempfile::tempdir().expect("fork fixture");
    let workspace = root.path().join("workspace");
    let home = root.path().join("home");
    std::fs::create_dir_all(&workspace).unwrap();
    std::fs::create_dir_all(&home).unwrap();
    let (addr, state) = spawn_mock(&workspace.join("write.txt"), &workspace.join("late.txt"));
    let config = write_acl(&home, &workspace, &format!("http://{addr}/v1"));
    let owned = child_env(&binaries::acp_binary(), &config, &home);
    let env: Vec<_> = owned
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect();
    let marker = "FORK-SOURCE-HISTORY-72f1";
    let source = seed_marked_session(&env, &workspace, &state, marker);
    let binary = tui_binary();
    let cwd = workspace.to_str().unwrap();
    let mut pty = open_pty(
        &binary,
        &["--cwd", cwd, "--resume", &source],
        &env,
        &workspace,
    );
    pty.wait_for(marker, Duration::from_secs(30));
    let source_info = pty.open_session_info(Duration::from_secs(15));
    assert_eq!(session_id_from_screen(&source_info), source);
    pty.escape();
    pty.focus_composer();
    pty.type_text("/fork --worktree");
    pty.enter();
    pty.wait_for("same workspace", Duration::from_secs(6));
    let refused = pty.open_session_info(Duration::from_secs(15));
    assert_eq!(session_id_from_screen(&refused), source);
    pty.escape();
    let calls = state.lock().unwrap().bodies.len();
    pty.focus_composer();
    pty.type_text("/fork");
    pty.enter();
    pty.wait_for(&format!("forked from {source}"), Duration::from_secs(30));
    let info = pty.open_session_info(Duration::from_secs(15));
    let child = session_id_from_screen(&info);
    assert!(!child.is_empty() && child != source, "{info}");
    assert_eq!(
        state.lock().unwrap().bodies.len(),
        calls,
        "fork must not invoke the model"
    );
    pty.escape();
    let export = workspace.join("child.md");
    pty.focus_composer();
    pty.type_text(&format!("/export {}", export.display()));
    pty.enter();
    pty.pump(Duration::from_millis(300));
    assert!(wait_file(&export, Duration::from_secs(5)).contains(marker));
    assert!(
        summary_for(&home, &child).is_some(),
        "fork missing from the resume catalog"
    );
    let child_log = aster_core::session_folder::effect_log_dir(&workspace, &child);
    let completed_turns = || {
        let mut raw = String::new();
        append_jsonl_dir(&child_log, &mut raw);
        raw.lines()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .filter(|fact| fact["type"] == "turn.complete")
            .count()
    };
    let before = completed_turns();
    pty.focus_composer();
    pty.type_text("CHILD-ONLY-QUESTION-83a2");
    pty.enter();
    let body = wait_body_all(
        &state,
        &["CHILD-ONLY-QUESTION-83a2"],
        Duration::from_secs(25),
        true,
    );
    assert!(
        body.contains(marker),
        "fork lost the original model context; observed request: {body}"
    );
    let deadline = Instant::now() + Duration::from_secs(15);
    while completed_turns() == before && Instant::now() < deadline {
        pty.pump(Duration::from_millis(40));
    }
    assert!(
        completed_turns() > before,
        "child did not settle its own turn\n{}",
        pty.screen()
    );
    pty.focus_composer();
    pty.type_text("/quit");
    pty.enter();
    assert!(pty.child.wait().unwrap().success());
    drop(pty);
    let (_, replay) = launch_resumed(&child, &env, &workspace, "CHILD-ONLY-QUESTION-83a2");
    assert!(replay.contains("CHILD-ONLY-QUESTION-83a2"));
    let (_, source_replay) = launch_resumed(&source, &env, &workspace, marker);
    assert!(!source_replay.contains("CHILD-ONLY-QUESTION-83a2"));
    let target = uuid::Uuid::new_v4().to_string();
    let mut pty = open_pty(
        &binary,
        &[
            "--cwd",
            cwd,
            "--resume",
            &source,
            "--fork-session",
            "--session-id",
            &target,
        ],
        &env,
        &workspace,
    );
    pty.wait_for(marker, Duration::from_secs(30));
    let startup_info = pty.open_session_info(Duration::from_secs(15));
    assert_eq!(session_id_from_screen(&startup_info), target);
    pty.escape();
    pty.focus_composer();
    pty.type_text("/quit");
    pty.enter();
    assert!(pty.child.wait().unwrap().success());
    oracle(
        "shipped_tui_prefix_fork_inherits_history_and_resumes_the_child",
        "history-and-restart",
        &format!("source={source} child={child} startup={target}"),
    );
}

#[test]
fn shipped_tui_launch_resume_and_visible_commands() {
    let binary = tui_binary();
    if !binary.is_file() {
        panic!("build aster before this test: {}", binary.display());
    }
    let workspace = isolate_dir("pty");
    let home = isolate_dir("pty-home");
    let write_path = workspace.join("side-effect.txt");
    let late_path = workspace.join("late.txt");
    // Seed durable memory before any ACP process opens the collection lock.
    {
        let memory_dir = aster_core::state_root::workspace_state_dir(&workspace).join("memory");
        let memory_store = aster_core::durable_memory_vec::VecMemoryStore::open(&memory_dir)
            .expect("pty memory store");
        memory_store
            .put(&aster_core::durable_memory_vec::MemoryRecord {
                id: "pty-deep-note".into(),
                content: "pty-seeded durable memory for deep inspect".into(),
                tags: vec!["pty".into()],
                importance: 0.6,
                memory_type: "\"procedural\"".into(),
                timestamp: "2026-11-09T00:00:00Z".into(),
                meta: "{}".into(),
            })
            .expect("seed pty memory");
    }
    let (addr, state) = spawn_mock(&write_path, &late_path);
    let config = write_acl(&home, &workspace, &format!("http://{addr}/v1"));
    let grok_home = home.join("grok");
    std::fs::create_dir_all(&grok_home).expect("grok home");
    std::fs::write(
        grok_home.join("config.toml"),
        "[ui]\nmouse_reporting_toggle = true\n",
    )
    .expect("mouse flag");
    let editor = home.join("editor.sh");
    std::fs::write(
        &editor,
        format!(
            "#!/bin/sh\nprintf edited > '{}'\nexit 0\n",
            workspace.join("editor-ran.txt").display()
        ),
    )
    .expect("editor script");
    std::fs::set_permissions(&editor, std::fs::Permissions::from_mode(0o755)).expect("editor mode");
    let acp_bin = binaries::acp_binary();
    let mut owned = child_env(&acp_bin, &config, &home);
    owned.push(("EDITOR".into(), editor.display().to_string()));
    owned.push(("VISUAL".into(), editor.display().to_string()));
    let env: Vec<(&str, &str)> = owned
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect();

    let (first_id, first_frame) = launch_once(false, &env, &workspace);
    assert!(first_frame.contains(MODEL_ID));
    let (second_id, second_frame) = launch_once(true, &env, &workspace);
    assert_eq!(first_id, second_id, "continue opened a different session");
    assert!(second_frame.contains(MODEL_ID));
    assert_ne!(first_frame.trim(), "", "first frame blank");
    assert_ne!(second_frame.trim(), "", "second frame blank");
    oracle(
        PTY_TEST,
        "continue-same-session",
        &format!("session={first_id} model={MODEL_ID}"),
    );
    eprintln!("---LAUNCH-1---\n{first_frame}\n---END-LAUNCH-1---");
    eprintln!("---LAUNCH-2---\n{second_frame}\n---END-LAUNCH-2---");

    // The same continue path with the agent found beside the pager, not via
    // ASTER_ACP_AGENT_BIN. A separate directory keeps this summary out of the
    // launch pair above.
    let discover_ws = isolate_dir("pty-discover");
    let discover_config = write_acl(&home, &discover_ws, &format!("http://{addr}/v1"));
    let mut discover_owned = child_env(&acp_bin, &discover_config, &home);
    discover_owned.retain(|(key, _)| key != "ASTER_ACP_AGENT_BIN");
    let discover_env: Vec<(&str, &str)> = discover_owned
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect();
    let (discovered_id, discovered_frame) = launch_once(false, &discover_env, &discover_ws);
    assert!(
        discovered_frame.contains(MODEL_ID),
        "discovered agent did not report the ACL model\n{discovered_frame}"
    );
    let (continued_id, continued_frame) = launch_once(true, &discover_env, &discover_ws);
    assert_eq!(
        discovered_id, continued_id,
        "continue without ASTER_ACP_AGENT_BIN opened a different session"
    );
    assert!(continued_frame.contains(MODEL_ID));
    oracle(
        PTY_TEST,
        "continue-without-agent-env",
        &format!("session={discovered_id} model={MODEL_ID}"),
    );

    let resume_marker = "RESUME-MARKER-9e4b";
    let resume_ws = isolate_dir("pty-resume");
    let seeded_id = seed_marked_session(&env, &resume_ws, &state, resume_marker);
    let (resumed_id, resumed_frame) = launch_resumed(&seeded_id, &env, &resume_ws, resume_marker);
    assert_eq!(
        resumed_id, seeded_id,
        "--resume reopened a different session"
    );
    assert!(
        resumed_frame.contains(MODEL_ID) && resumed_frame.contains(resume_marker),
        "--resume frame lost the model or the prior transcript\n{resumed_frame}"
    );
    oracle(
        PTY_TEST,
        "resume-durable-transcript",
        &format!("session={resumed_id} marker={resume_marker}"),
    );
    let (continued_marked_id, continued_marked) =
        launch_continued(&seeded_id, &env, &resume_ws, resume_marker);
    assert_eq!(
        continued_marked_id, seeded_id,
        "--continue reopened a different session"
    );
    assert!(
        continued_marked.contains(MODEL_ID) && continued_marked.contains(resume_marker),
        "--continue frame lost the model or the prior transcript\n{continued_marked}"
    );
    oracle(
        PTY_TEST,
        "continue-durable-transcript",
        &format!("session={continued_marked_id} marker={resume_marker}"),
    );
    exercise_marker_navigation(&env, &resume_ws, &seeded_id, resume_marker, &state);

    let proxy = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../apps/aster/tests/tui_913_real_llm/acp_trace_proxy.py");
    let relay = home.join("interactive-acp-relay");
    std::fs::write(
        &relay,
        format!(
            "#!/bin/sh\nexec '{}' '{}' \"$@\"\n",
            python::executable()
                .display()
                .to_string()
                .replace('\'', "'\\''"),
            proxy.display().to_string().replace('\'', "'\\''")
        ),
    )
    .expect("trace relay");
    std::fs::set_permissions(&relay, std::fs::Permissions::from_mode(0o755))
        .expect("relay executable");
    let trace = home.join("interactive-acp-trace.jsonl");
    let mut interactive_owned = owned.clone();
    interactive_owned.retain(|(key, _)| key != "ASTER_ACP_AGENT_BIN");
    interactive_owned.extend([
        ("ASTER_ACP_AGENT_BIN".into(), relay.display().to_string()),
        ("ASTER_TUI_TEST_ACP_REAL".into(), acp_bin.into()),
        (
            "ASTER_TUI_TEST_ACP_TRACE".into(),
            trace.display().to_string(),
        ),
    ]);
    let env: Vec<(&str, &str)> = interactive_owned
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect();
    let args = vec!["--cwd", workspace.to_str().expect("cwd")];
    let mut pty = open_pty(&binary, &args, &env, &workspace);
    pty.wait_for_frame(Duration::from_secs(45));

    for (name, description) in UNAVAILABLE {
        pty.escape();
        pty.pump(Duration::from_millis(80));
        pty.write_bytes(b"\x15");
        pty.type_text(&format!("/{name}"));
        let screen = pty.pump(Duration::from_millis(350));
        assert!(
            !palette_offers(&screen, name, description),
            "/{name} is still offered\n{screen}"
        );
        oracle(PTY_TEST, &format!("hidden-{name}"), "absent from palette");
    }
    // `cd` stays in the effective list and is dashboard-only, so the agent session does not offer it.
    pty.escape();
    pty.pump(Duration::from_millis(80));
    pty.write_bytes(b"\x15");
    pty.type_text("/cd");
    let cd_screen = pty.pump(Duration::from_millis(350));
    assert!(
        !palette_offers(&cd_screen, "cd", "working directory for new agents"),
        "/cd is offered outside the dashboard\n{cd_screen}"
    );
    oracle(PTY_TEST, "cd-agent-palette", "absent outside the dashboard");

    for (name, description) in FULLSCREEN_PALETTE {
        pty.escape();
        pty.pump(Duration::from_millis(80));
        pty.write_bytes(b"\x15");
        pty.type_text(&format!("/{name}"));
        let screen = pty.pump(Duration::from_millis(400));
        assert!(
            palette_offers(&screen, name, description),
            "/{name} missing from the palette\n{screen}"
        );
        oracle(PTY_TEST, &format!("palette-{name}"), description);
    }

    // Each still-visible command's own effect. Escape dismisses the palette
    // left open by the last probe; the line clear has to land after that.
    pty.escape();
    pty.pump(Duration::from_millis(80));
    pty.write_bytes(b"\x15");

    let before_inspection = state.lock().expect("mock").bodies.len();
    pty.write_bytes(b"\x15");
    pty.type_text("/memory list pty-seeded");
    pty.enter();
    let memory_list = pty.wait_for("Stored memories:", Duration::from_secs(8));
    assert!(
        memory_list.contains("Stored memories: 1") && memory_list.contains("pty-deep-note"),
        "seeded /memory list missing note\n{memory_list}"
    );
    assert_eq!(
        state.lock().expect("mock").bodies.len(),
        before_inspection,
        "/memory list must use the host contract without a model request"
    );
    oracle(PTY_TEST, "/memory", "seeded list, zero model requests");

    pty.write_bytes(b"\x15");
    pty.type_text("/memory read pty-deep-note");
    pty.enter();
    // Read output includes a Revision line; list preview does not.
    let memory_read = pty.wait_for("Revision:", Duration::from_secs(8));
    assert!(
        memory_read.contains("pty-seeded durable memory for deep inspect")
            && memory_read.contains("Revision:"),
        "seeded /memory read missing content\n{memory_read}"
    );
    // TUI wrap can put the 64-hex revision on the line after `Revision:`.
    let revision = {
        let lines: Vec<&str> = memory_read.lines().collect();
        let mut found = None;
        for (index, line) in lines.iter().enumerate() {
            if let Some(rest) = line.split("Revision:").nth(1) {
                let same_line: String = rest.chars().filter(|ch| ch.is_ascii_hexdigit()).collect();
                if same_line.len() == 64 {
                    found = Some(same_line);
                    break;
                }
                if let Some(next) = lines.get(index + 1) {
                    let next_line: String =
                        next.chars().filter(|ch| ch.is_ascii_hexdigit()).collect();
                    if next_line.len() == 64 {
                        found = Some(next_line);
                        break;
                    }
                }
            }
        }
        found.unwrap_or_else(|| {
            panic!("revision missing from /memory read screen\n{memory_read}");
        })
    };
    oracle(PTY_TEST, "/memory-read", "seeded content");

    pty.write_bytes(b"\x15");
    pty.type_text(&format!("/memory delete pty-deep-note {revision}"));
    pty.enter();
    let deleted = pty.wait_for("Deleted memory", Duration::from_secs(8));
    assert!(
        deleted.contains("Deleted memory"),
        "seeded /memory delete missing confirmation\n{deleted}"
    );
    oracle(PTY_TEST, "/memory-delete", "deleted seeded note");

    pty.write_bytes(b"\x15");
    pty.type_text("/tasks");
    pty.enter();
    let tasks = pty.wait_for("No background tasks.", Duration::from_secs(8));
    assert!(tasks.contains("No background tasks."), "{tasks}");
    assert_eq!(
        state.lock().expect("mock").bodies.len(),
        before_inspection,
        "/tasks must use the host contract without a model request"
    );
    oracle(
        PTY_TEST,
        "/tasks",
        "current host inspection, zero model requests",
    );

    pty.write_bytes(b"\x15");
    pty.type_text("/flush");
    pty.enter();
    let flush = pty.wait_for("Memory flushed", Duration::from_secs(12));
    assert!(
        flush.contains("Memory flushed"),
        "/flush did not report a typed outcome\n{flush}"
    );
    oracle(PTY_TEST, "/flush", "Memory flushed");

    pty.write_bytes(b"\x15");
    pty.type_text("/dream");
    pty.enter();
    let dream = pty.wait_for("Nothing to consolidate", Duration::from_secs(12));
    assert!(
        dream.contains("Nothing to consolidate"),
        "/dream did not report a typed outcome\n{dream}"
    );
    oracle(PTY_TEST, "/dream", "Nothing to consolidate");
    pty.write_bytes(b"\x15");
    let bodies_before_doctor = state.lock().expect("mock").bodies.len();
    pty.type_text("/doctor");
    pty.enter();
    let doctor = pty.wait_for("terminal", Duration::from_secs(8));
    assert!(
        doctor.to_ascii_lowercase().contains("terminal")
            || doctor.to_ascii_lowercase().contains("color")
            || doctor.to_ascii_lowercase().contains("clipboard"),
        "doctor did not report\n{doctor}"
    );
    assert_eq!(
        state.lock().expect("mock").bodies.len(),
        bodies_before_doctor,
        "/doctor started a model turn"
    );
    oracle(PTY_TEST, "doctor", "terminal report, zero model requests");

    // Goal and compact before any user turn: status must not call the model,
    // and an empty session reports that there is nothing to compact.
    let bodies_before_goal = state.lock().expect("mock").bodies.len();
    pty.write_bytes(b"\x15");
    pty.type_text("/goal status");
    pty.enter();
    let goal = pty.wait_for("No goal is set.", Duration::from_secs(8));
    assert!(
        goal.contains("No goal is set."),
        "goal status did not report the empty objective\n{goal}"
    );
    assert_eq!(
        state.lock().expect("mock").bodies.len(),
        bodies_before_goal,
        "/goal status started a model turn"
    );
    oracle(
        PTY_TEST,
        "goal-status",
        "empty objective, zero model requests",
    );
    pty.write_bytes(b"\x15");
    pty.type_text("/compact");
    pty.enter();
    let compact = pty.wait_for("Nothing to compact yet.", Duration::from_secs(8));
    assert!(
        compact.contains("Nothing to compact yet."),
        "compact on an empty session did not report its own result\n{compact}"
    );
    oracle(PTY_TEST, "compact-empty", "Nothing to compact yet");

    let bodies_before_context = state.lock().expect("mock").bodies.len();
    pty.write_bytes(b"\x15");
    pty.type_text("/context");
    pty.enter();
    let context = pty.wait_for("Usage not reported", Duration::from_secs(8));
    assert!(context.contains("token window"), "{context}");
    assert!(!context.contains("0 /"), "{context}");
    assert!(!context.contains("100%"), "{context}");
    assert_eq!(
        state.lock().expect("mock").bodies.len(),
        bodies_before_context,
        "/context started a model turn"
    );
    oracle(
        PTY_TEST,
        "context-empty",
        "usage unavailable, no fabricated free budget, zero model requests",
    );
    pty.dismiss();

    // A new explicit goal uses Core facts and the default bounded driver.
    pty.write_bytes(b"\x15");
    pty.type_text(&format!("/goal {GOAL_OBJECTIVE}"));
    pty.enter();
    let created = wait_goal_fact(&mut pty, &workspace, "aster.goal.created");
    assert!(created.to_string().contains(GOAL_OBJECTIVE));
    let stopped = pty.wait_for("需要你的输入后才能继续。", Duration::from_secs(30));
    assert!(stopped.contains("需要你的输入后才能继续。"), "{stopped}");
    let first_goal_body = wait_body(&state, GOAL_OBJECTIVE, Duration::from_secs(5));
    assert!(first_goal_body.contains("Engineering discipline"));
    assert!(first_goal_body.contains("Use zh-CN only"));
    assert!(stopped.contains("目标尚未完成"), "{stopped}");
    assert!(stopped.contains("已完成 0/1 项。"), "{stopped}");
    for internal in ["Durable goal", "acp-tick-", "Plan r1", "needs_input"] {
        assert!(!stopped.contains(internal), "{stopped}");
    }
    assert!(!stopped.contains("目标已完成"), "{stopped}");
    assert_eq!(
        stopped.matches("需要你的输入后才能继续。").count(),
        1,
        "{stopped}"
    );
    assert!(!log_text(&workspace).contains("aster.goal.completed"));
    oracle(
        PTY_TEST,
        "goal-set",
        "default policy and native goal; stopped at the no-progress bound",
    );

    let bodies_before_pause = state.lock().expect("mock").bodies.len();
    pty.write_bytes(b"\x15");
    pty.type_text("/goal pause");
    pty.enter();
    wait_goal_fact(&mut pty, &workspace, "aster.goal.paused");
    let paused = pty.wait_for("目标已暂停", Duration::from_secs(8));
    assert!(paused.contains("已完成 0/1 项。"), "{paused}");
    assert_eq!(
        state.lock().expect("mock").bodies.len(),
        bodies_before_pause
    );
    oracle(
        PTY_TEST,
        "goal-pause",
        "persisted pause, zero model requests",
    );

    pty.write_bytes(b"\x15");
    pty.type_text("/goal resume");
    pty.enter();
    wait_goal_fact(&mut pty, &workspace, "aster.goal.resumed");
    let resumed = pty.wait_for("需要你的输入后才能继续。", Duration::from_secs(8));
    assert!(resumed.contains("需要你的输入后才能继续。"), "{resumed}");
    assert_eq!(
        state.lock().expect("mock").bodies.len(),
        bodies_before_pause,
        "resume must not reset the no-progress count"
    );
    oracle(
        PTY_TEST,
        "goal-resume",
        "resumed posture; the existing no-progress bound still stops execution",
    );

    pty.write_bytes(b"\x15");
    pty.type_text("/goal clear");
    pty.enter();
    wait_goal_fact(&mut pty, &workspace, "aster.goal.cancelled");
    let cleared = pty.wait_for("目标已取消", Duration::from_secs(8));
    assert!(cleared.contains("已完成 0/1 项。"), "{cleared}");
    assert_eq!(
        state.lock().expect("mock").bodies.len(),
        bodies_before_pause
    );
    assert!(
        log_text(&workspace).contains("aster.goal.created"),
        "clear retains the original history"
    );
    oracle(
        PTY_TEST,
        "goal-clear",
        "cancelled in original facts; history retained, zero model requests",
    );

    // A terminal bound goal stays bound; ordinary work starts in a fresh session.
    pty.write_bytes(b"\x15");
    pty.type_text("/new");
    pty.enter();
    pty.pump(Duration::from_millis(600));
    let mut product_prompts = 0u64;

    // Export skips system chrome. A user turn has to exist or the command
    // reports an empty transcript and writes nothing. This stays before
    // vim-mode so the composer is still in insert mode.
    pty.write_bytes(b"\x15");
    pty.pump(Duration::from_millis(80));
    pty.type_text("EXPORT-SOURCE");
    let typed = pty.pump(Duration::from_millis(300));
    assert!(
        typed.contains("EXPORT-SOURCE"),
        "composer did not accept the export prompt\n{typed}"
    );
    pty.enter();
    let _export_turn = wait_body(&state, "EXPORT-SOURCE", Duration::from_secs(25));
    product_prompts += 1;
    pty.pump(Duration::from_secs(1));
    let export_path = workspace.join("export-out").join("exported-session.md");
    pty.write_bytes(b"\x15");
    pty.type_text(&format!("/export {}", export_path.display()));
    pty.pump(Duration::from_millis(200));
    pty.escape();
    pty.pump(Duration::from_millis(80));
    pty.enter();
    let export_screen = pty.pump(Duration::from_millis(400));
    let exported = wait_file(&export_path, Duration::from_secs(4));
    assert!(
        exported.contains("EXPORT-SOURCE"),
        "export did not write the conversation: {exported:?}\n{export_screen}"
    );
    oracle(PTY_TEST, "export", "file contains EXPORT-SOURCE");

    pty.write_bytes(b"\x15");
    let after_turn = pty.open_session_info(Duration::from_secs(15));
    let (turn, used) = session_info_turn_and_used(&after_turn).unwrap_or((0, 0));
    assert_eq!(
        turn, product_prompts,
        "session-info turn {turn} is not the {product_prompts} prompts submitted in this session\n{after_turn}"
    );
    assert!(
        used >= 1,
        "session-info after a completed turn still shows {used} tokens\n{after_turn}"
    );
    let tour_id = session_id_from_screen(&after_turn);
    assert!(
        !tour_id.is_empty(),
        "session-info had no session id\n{after_turn}"
    );
    oracle(
        PTY_TEST,
        "session-info",
        &format!("session={tour_id} turn={turn} prompts={product_prompts} used={used}"),
    );
    pty.escape();
    pty.pump(Duration::from_millis(200));
    pty.dismiss();

    pty.escape();
    pty.write_bytes(b"\x15");
    let bodies_before_theme = state.lock().expect("mock").bodies.len();
    let before_theme = config_texts(&home);
    assert!(
        !before_theme.contains("theme = \"aster-day\""),
        "theme was already aster-day before /theme\n{before_theme}"
    );
    pty.type_text("/theme aster-day");
    pty.enter();
    assert!(
        wait_config(&home, "theme = \"aster-day\"", Duration::from_secs(3)),
        "theme did not persist aster-day\n{}",
        config_texts(&home)
    );
    assert_eq!(
        state.lock().expect("mock").bodies.len(),
        bodies_before_theme,
        "/theme started a model turn"
    );
    oracle(PTY_TEST, "theme", "aster-day, zero model requests");

    pty.write_bytes(b"\x15");
    let before_vim = config_texts(&home);
    assert!(
        !before_vim.contains("vim_mode = true"),
        "vim mode was already on\n{before_vim}"
    );
    pty.type_text("/vim-mode");
    pty.enter();
    assert!(
        wait_config(&home, "vim_mode = true", Duration::from_secs(3)),
        "vim-mode did not persist vim_mode = true\n{}",
        config_texts(&home)
    );
    oracle(PTY_TEST, "vim-mode", "vim_mode = true");

    pty.write_bytes(b"\x15");
    let before_timestamps = config_texts(&home);
    assert!(
        !before_timestamps.contains("show_timestamps = false"),
        "timestamps were already off\n{before_timestamps}"
    );
    pty.type_text("/timestamps");
    pty.enter();
    assert!(
        wait_config(&home, "show_timestamps = false", Duration::from_secs(3)),
        "timestamps did not persist show_timestamps = false\n{}",
        config_texts(&home)
    );
    oracle(PTY_TEST, "timestamps", "show_timestamps = false");
    pty.write_bytes(b"\x15");
    pty.type_text("/multiline");
    pty.enter();
    let multiline = pty.wait_for("Multiline:", Duration::from_secs(4));
    assert!(
        multiline.contains("Multiline:"),
        "multiline toggle did not report its session flag\n{multiline}"
    );
    oracle(PTY_TEST, "multiline-on", "Multiline:");
    pty.write_bytes(b"\x15");
    let before_compact_mode = config_texts(&home);
    assert!(
        !before_compact_mode.contains("compact_mode = true"),
        "compact mode was already on\n{before_compact_mode}"
    );
    pty.type_text("/compact-mode");
    pty.enter();
    assert!(
        wait_config(&home, "compact_mode = true", Duration::from_secs(3)),
        "compact-mode did not persist compact_mode = true\n{}",
        config_texts(&home)
    );
    oracle(PTY_TEST, "compact-mode", "compact_mode = true");
    pty.write_bytes(b"\x15");
    let before_timeline = config_texts(&home);
    assert!(
        !before_timeline.contains("show_timeline = true"),
        "timeline was already on\n{before_timeline}"
    );
    pty.type_text("/timeline");
    pty.enter();
    assert!(
        wait_config(&home, "show_timeline = true", Duration::from_secs(3)),
        "timeline did not persist show_timeline = true\n{}",
        config_texts(&home)
    );
    oracle(PTY_TEST, "timeline", "show_timeline = true");

    pty.write_bytes(b"\x15");
    let bodies_before_help = state.lock().expect("mock").bodies.len();
    pty.type_text("/help");
    pty.enter();
    let help = pty.wait_for("Commands", Duration::from_secs(5));
    assert!(
        help.contains("Settings") && (help.contains("Quit") || help.contains("Keyboard Shortcuts")),
        "help did not open the command palette\n{help}"
    );
    assert_eq!(
        state.lock().expect("mock").bodies.len(),
        bodies_before_help,
        "/help started a model turn"
    );
    oracle(PTY_TEST, "help", "Commands palette, zero model requests");
    pty.escape();
    pty.pump(Duration::from_millis(120));
    // One Escape can land while the palette is still opening. Close it
    // before the next slash command, or Enter selects a palette row.
    if pty.screen().contains("Esc close") {
        pty.escape();
        pty.pump(Duration::from_millis(120));
    }

    pty.write_bytes(b"\x15");
    pty.type_text("/settings");
    pty.enter();
    let settings = pty.wait_for("Space toggle", Duration::from_secs(5));
    assert!(
        settings.contains("Settings") && settings.contains("Space toggle"),
        "settings modal did not open\n{settings}"
    );
    oracle(PTY_TEST, "settings", "Settings modal");
    pty.dismiss();

    pty.write_bytes(b"\x15");
    pty.type_text("/config");
    pty.enter();
    let config_modal = pty.wait_for("Add provider", Duration::from_secs(6));
    assert!(
        config_modal.contains("Providers")
            && config_modal.contains("Models")
            && config_modal.contains("Add provider"),
        "config modal did not open\n{config_modal}"
    );
    oracle(PTY_TEST, "config", "Providers and Models");
    pty.dismiss();

    pty.write_bytes(b"\x15");
    pty.type_text("/context");
    pty.enter();
    let context = pty.wait_for("Reported input", Duration::from_secs(8));
    assert!(
        !context.contains("System prompt")
            && !context.contains("Reasoning/overhead")
            && !context.contains("Auto-compact"),
        "context report missing\n{context}"
    );
    let context_turns = context.lines().find_map(|line| {
        // The context modal paints a box glyph before the footer label.
        let rest = line.split_once("Turns:")?.1;
        let digits: String = rest
            .trim()
            .chars()
            .take_while(|ch| ch.is_ascii_digit())
            .collect();
        digits.parse::<u64>().ok()
    });
    assert_eq!(
        context_turns,
        Some(product_prompts),
        "context Turns is not the {product_prompts} prompts submitted in this session\n{context}"
    );
    let context_used = context.lines().find_map(|line| {
        if !line.contains("tokens") {
            return None;
        }
        line.split_once(" / ")?
            .0
            .split_whitespace()
            .last()?
            .parse::<u64>()
            .ok()
    });
    assert_eq!(
        context_used,
        Some(1),
        "context must show the owned provider fixture's one reported prompt token\n{context}"
    );
    oracle(
        PTY_TEST,
        "context",
        "actual turns, one reported input token, no unmeasured categories or threshold",
    );
    pty.dismiss();

    let before_compact = log_text(&workspace);
    assert!(
        !before_compact.contains("compaction.done"),
        "compaction.done was already in the log before /compact"
    );
    pty.write_bytes(b"\x15");
    pty.type_text("/compact");
    pty.enter();
    let compact_deadline = Instant::now() + Duration::from_secs(25);
    let mut compact_facts = String::new();
    let mut compact_screen = String::new();
    while Instant::now() < compact_deadline {
        compact_screen = pty.pump(Duration::from_millis(150));
        compact_facts = log_text(&workspace);
        if compact_facts.contains("compaction.done") && compact_facts.contains(COMPACT_SUMMARY) {
            break;
        }
    }
    assert!(
        compact_facts.contains("compaction.done") && compact_facts.contains(COMPACT_SUMMARY),
        "compact did not append compaction.done\n{compact_facts}\n{compact_screen}"
    );
    oracle(PTY_TEST, "compact-done", COMPACT_SUMMARY);
    pty.dismiss();

    pty.write_bytes(b"\x15");
    pty.type_text("/find marker");
    pty.enter();
    let find = pty.wait_for("search:", Duration::from_secs(6));
    assert!(
        find.contains("search:"),
        "find did not open the scrollback search row\n{find}"
    );
    oracle(PTY_TEST, "find", "search:");
    // Find leaves vim scrollback focused, where `/` opens search again.
    pty.focus_composer();
    pty.type_text("/jump");
    pty.enter();
    let jump_deadline = Instant::now() + Duration::from_secs(6);
    let mut jump = String::new();
    while Instant::now() < jump_deadline {
        jump = pty.pump(Duration::from_millis(150));
        if jump.contains("Jump to which turn?") || jump.contains("Nothing to jump to yet") {
            break;
        }
    }
    assert!(
        jump.contains("Jump to which turn?") || jump.contains("Nothing to jump to yet"),
        "jump picker missing\n{jump}"
    );
    oracle(PTY_TEST, "jump", "picker");
    pty.escape();
    pty.pump(Duration::from_millis(150));
    pty.dismiss();

    // Multiline swaps Enter and Shift+Enter. The on-toast already landed.
    // Find left scrollback focused; return to the prompt before the toggle.
    pty.focus_composer();
    pty.type_text("/multiline");
    pty.enter();
    let multiline_deadline = Instant::now() + Duration::from_secs(4);
    let mut multiline_screen = String::new();
    let mut multiline_off = false;
    while Instant::now() < multiline_deadline {
        multiline_screen = pty.pump(Duration::from_millis(120));
        let footer_on = multiline_screen
            .lines()
            .any(|line| line.contains("mock/gpt") && line.contains("multiline"));
        if multiline_screen.contains("mock/gpt") && !footer_on {
            multiline_off = true;
            break;
        }
    }
    assert!(multiline_off, "multiline stayed on\n{multiline_screen}");
    oracle(PTY_TEST, "multiline-off", "footer cleared");

    pty.write_bytes(b"\x15");
    pty.type_text("/effort low");
    pty.enter();
    pty.pump(Duration::from_millis(400));
    pty.write_bytes(b"\x15");
    pty.type_text("EFFORT-LOW-PROBE");
    let typed_effort = pty.pump(Duration::from_millis(200));
    assert!(
        typed_effort.contains("EFFORT-LOW-PROBE"),
        "composer did not accept the effort probe\n{typed_effort}"
    );
    pty.enter();
    let low_body = wait_body_all(
        &state,
        &["EFFORT-LOW-PROBE", "thinking_budget=", COMPACT_SUMMARY],
        Duration::from_secs(20),
        true,
    );
    let low_thinking = budget_number(&low_body, "thinking_budget=").expect("tui low thinking");
    assert_eq!(budget_number(&low_body, "tool_rounds=").unwrap_or(0), 0);
    wait_prompt_response(&mut pty, &trace, "EFFORT-LOW-PROBE");
    pty.write_bytes(b"\x15");
    pty.type_text("/effort high");
    pty.enter();
    pty.pump(Duration::from_millis(400));
    pty.write_bytes(b"\x15");
    pty.type_text("EFFORT-HIGH-PROBE");
    pty.enter();
    let high_body = wait_body_all(
        &state,
        &["EFFORT-HIGH-PROBE", "thinking_budget="],
        Duration::from_secs(20),
        true,
    );
    let high_thinking = budget_number(&high_body, "thinking_budget=").expect("tui high thinking");
    assert_eq!(budget_number(&high_body, "tool_rounds=").unwrap_or(0), 0);
    wait_prompt_response(&mut pty, &trace, "EFFORT-HIGH-PROBE");
    assert!(
        high_thinking > low_thinking,
        "tui effort did not change thinking budget: low={low_thinking}, high={high_thinking}"
    );

    oracle(
        PTY_TEST,
        "effort",
        &format!("low={low_thinking} high={high_thinking}; no finite tool-round ceiling"),
    );

    // The session already uses mock/gpt, so repeating that id is a no-op.
    // mock/alt is the other catalog entry. A reasoning model row ends with a
    // space, so Enter would open the effort menu instead of submitting.
    // Escape closes that menu and leaves the typed id; Enter then persists it.
    pty.write_bytes(b"\x15");
    pty.type_text("/model mock/alt");
    let typed_model = pty.pump(Duration::from_millis(250));
    assert!(
        typed_model.contains("mock/alt"),
        "composer did not show the model id\n{typed_model}"
    );
    pty.escape();
    std::thread::sleep(Duration::from_millis(450));
    let armed = pty.pump(Duration::from_millis(80));
    assert!(
        armed.contains("mock/alt"),
        "escape cleared the model command\n{armed}"
    );
    pty.enter();
    let model_deadline = Instant::now() + Duration::from_secs(6);
    let mut persisted = acl_default_model(&config);
    while Instant::now() < model_deadline {
        persisted = acl_default_model(&config);
        if persisted == "mock/alt" {
            break;
        }
        pty.pump(Duration::from_millis(150));
    }
    assert_eq!(
        persisted, "mock/alt",
        "model did not persist the selected catalog id"
    );
    oracle(PTY_TEST, "model", "mock/alt");

    // Ask keeps the write parked until Allow once. The file and the gate
    // sentence are the observations; the assistant sentence is not.
    pty.write_bytes(b"\x15");
    pty.type_text("ASK-WRITE please");
    pty.enter();
    let ask_prompt = pty.wait_for("Yes, proceed", Duration::from_secs(20));
    assert!(
        !write_path.exists(),
        "write ran before Allow once\n{ask_prompt}"
    );
    pty.write_bytes(b"1");
    let ask_gate = pty.wait_for("completion gate:", Duration::from_secs(20));
    let asked = std::fs::read_to_string(&write_path).unwrap_or_default();
    assert_eq!(asked, "wrote-marker", "Allow once did not write the marker");
    assert!(
        ask_gate.contains("Assistant text does not count") && ask_gate.contains("side-effect.txt"),
        "Ask turn hid the completion gate\n{ask_gate}"
    );
    oracle(PTY_TEST, "ask-allow-once-gate", "side-effect.txt");

    // yolo lets a write through without a permission key.
    pty.write_bytes(b"\x15");
    pty.type_text("/yolo");
    pty.enter();
    pty.write_bytes(b"\x15");
    let yolo_frame = pty.pump(Duration::from_millis(400));
    assert!(
        yolo_frame.contains("YOLO"),
        "info line did not show the yolo flag\n{yolo_frame}"
    );
    assert!(
        !yolo_frame.contains("/yolo"),
        "cleared composer still shows the slash\n{yolo_frame}"
    );
    assert!(
        !yolo_frame.contains("always-approve"),
        "info line still shows always-approve\n{yolo_frame}"
    );
    oracle(PTY_TEST, "yolo-flag", "YOLO");
    // Allow once already wrote this path and left the completion gate on
    // screen. Remove the file so this turn has to create it again.
    std::fs::remove_file(&write_path).expect("remove allow-once side effect");
    assert!(
        !write_path.exists(),
        "side effect still present before yolo"
    );
    pty.write_bytes(b"\x15");
    pty.type_text("WRITE-ORACLE please");
    pty.enter();
    let yolo_deadline = Instant::now() + Duration::from_secs(20);
    let mut yolo_screen = String::new();
    while Instant::now() < yolo_deadline {
        yolo_screen = pty.pump(Duration::from_millis(80));
        assert!(
            !yolo_screen.contains("Yes, proceed"),
            "yolo showed a permission prompt\n{yolo_screen}"
        );
        if write_path.is_file() {
            break;
        }
    }
    assert!(
        write_path.is_file(),
        "yolo did not create side-effect.txt\n{yolo_screen}"
    );
    let wrote = std::fs::read_to_string(&write_path).unwrap_or_default();
    assert_eq!(wrote, "wrote-marker", "yolo did not write the marker");
    yolo_screen = pty.pump(Duration::from_millis(200));
    assert!(
        !yolo_screen.contains("Yes, proceed"),
        "yolo showed a permission prompt after the write\n{yolo_screen}"
    );
    oracle(
        PTY_TEST,
        "yolo",
        "new side-effect.txt, no permission prompt",
    );

    pty.write_bytes(b"\x15");
    let copy_path = workspace.join("copied-reply.txt");
    pty.type_text(&format!("/copy {}", copy_path.display()));
    pty.enter();
    let copied = wait_file(&copy_path, Duration::from_secs(4));
    assert!(
        copied.contains("ok") || copied.contains("wrote-marker"),
        "copy did not write the assistant reply: {copied:?}"
    );
    oracle(PTY_TEST, "copy", "assistant reply file");

    let editor_marker = workspace.join("editor-ran.txt");
    pty.write_bytes(b"\x15");
    pty.type_text("/edit-prompt");
    pty.enter();
    // The editor starts only after the frame writer drains. Leave the PTY
    // unread and the handoff times out before the script runs.
    let editor_deadline = Instant::now() + Duration::from_secs(8);
    let mut edited = String::new();
    while Instant::now() < editor_deadline {
        pty.pump(Duration::from_millis(50));
        if let Ok(text) = std::fs::read_to_string(&editor_marker) {
            if !text.is_empty() {
                edited = text;
                break;
            }
        }
    }
    assert!(
        edited.contains("edited"),
        "edit-prompt did not launch EDITOR: {edited:?}"
    );
    oracle(PTY_TEST, "edit-prompt", "editor-ran");

    let mouse_at = pty.raw.len();
    pty.write_bytes(b"\x15");
    pty.type_text("/toggle-mouse-reporting");
    pty.enter();
    let mouse_deadline = Instant::now() + Duration::from_secs(3);
    let mut mouse_changed = false;
    while Instant::now() < mouse_deadline {
        pty.pump(Duration::from_millis(100));
        let tail = String::from_utf8_lossy(&pty.raw[mouse_at.min(pty.raw.len())..]);
        if tail.contains("1000l")
            || tail.contains("1002l")
            || tail.contains("1003l")
            || tail.contains("1006l")
            || tail.contains("1000h")
            || tail.contains("1002h")
        {
            mouse_changed = true;
            break;
        }
    }
    assert!(
        mouse_changed,
        "toggle-mouse-reporting did not change mouse capture"
    );
    oracle(PTY_TEST, "toggle-mouse-reporting", "mouse capture changed");

    pty.write_bytes(b"\x15");
    pty.type_text("/home");
    pty.enter();
    let welcome = pty.wait_for("Resume session", Duration::from_secs(6));
    assert!(
        welcome.contains("Resume session"),
        "home did not return to the welcome screen\n{welcome}"
    );
    pty.write_bytes(b"\x15");
    pty.type_text("/session-info");
    pty.enter();
    let home_info = pty.wait_for("No active session", Duration::from_secs(6));
    assert!(
        home_info.contains("No active session") && session_id_from_screen(&home_info) != tour_id,
        "home left session {tour_id} active\n{home_info}"
    );
    oracle(PTY_TEST, "home", "welcome, no active session");

    pty.escape();
    pty.write_bytes(b"\x15");
    pty.type_text("/new");
    pty.enter();
    pty.pump(Duration::from_secs(2));
    pty.write_bytes(b"\x15");
    let created = pty.open_session_info(Duration::from_secs(15));
    let created_id = session_id_from_screen(&created);
    assert!(
        !created_id.is_empty(),
        "new did not open a session\n{created}"
    );
    assert_ne!(created_id, tour_id, "new reused the session home just left");
    oracle(PTY_TEST, "new", &format!("session={created_id}"));
    pty.focus_composer();
    pty.type_text("/minimal");
    pty.enter();
    let minimal = pty.wait_for("Switched to minimal mode", Duration::from_secs(6));
    assert!(minimal.contains("Switched to minimal mode"));
    oracle(PTY_TEST, "minimal", "Switched to minimal mode");
    pty.write_bytes(b"\x15");
    pty.type_text("/expand");
    let expand = pty.pump(Duration::from_millis(400));
    assert!(
        palette_offers(&expand, "expand", "fully expanded"),
        "/expand is not offered in minimal mode\n{expand}"
    );
    pty.enter();
    let expanded = pty.wait_for("Nothing to expand", Duration::from_secs(6));
    assert!(
        expanded.contains("Nothing to expand"),
        "/expand did not report its empty state\n{expanded}"
    );
    oracle(PTY_TEST, "expand", "Nothing to expand");
    pty.write_bytes(b"\x15");
    pty.type_text("/fullscreen");
    pty.enter();
    let fullscreen = pty.wait_for("Switched to fullscreen mode", Duration::from_secs(6));
    assert!(fullscreen.contains("Switched to fullscreen mode"));
    oracle(PTY_TEST, "fullscreen", "Switched to fullscreen mode");
    let created_summary = {
        let deadline = Instant::now() + Duration::from_secs(6);
        loop {
            if let Some(path) = summary_for(&home, &created_id) {
                break path;
            }
            if Instant::now() >= deadline {
                panic!(
                    "new session {created_id} was not persisted under {}",
                    home.display()
                );
            }
            pty.pump(Duration::from_millis(100));
        }
    };
    pty.write_bytes(b"\x15");
    pty.type_text("/delete");
    pty.enter();
    let question = pty.wait_for("Delete this session permanently?", Duration::from_secs(6));
    assert!(
        question.contains("Delete this session permanently?"),
        "delete did not ask\n{question}"
    );
    pty.pump(Duration::from_millis(200));
    pty.write_bytes(b"1");
    let delete_deadline = Instant::now() + Duration::from_secs(8);
    while created_summary.exists() && Instant::now() < delete_deadline {
        pty.pump(Duration::from_millis(100));
    }
    let delete_screen = pty.screen();
    assert!(
        !created_summary.exists(),
        "delete left {}\n{delete_screen}",
        created_summary.display()
    );
    pty.write_bytes(b"\x15");
    pty.type_text("/session-info");
    pty.enter();
    let deleted = pty.wait_for("No active session", Duration::from_secs(6));
    assert!(
        deleted.contains("No active session") && !deleted.contains(&created_id),
        "delete left session {created_id} active\n{deleted}"
    );
    oracle(PTY_TEST, "delete", &format!("removed {created_id}"));

    pty.write_bytes(b"\x15");
    pty.type_text("/quit");
    pty.enter();
    let status = pty.child.wait().expect("quit");
    assert!(status.success(), "/quit exited {status:?}");
    oracle(PTY_TEST, "quit", "exit 0");
    let _ = args;
    let _ = late_path;
    let _ = std::fs::remove_dir_all(&workspace);
    let _ = std::fs::remove_dir_all(&home);
}

/// Ask-user choice and edit tool mutate through the real pager under yolo.
#[test]
fn shipped_tui_ask_user_and_edit_tool_oracles() {
    let binary = tui_binary();
    assert!(
        binary.is_file(),
        "build aster before this test: {}",
        binary.display()
    );
    let workspace = isolate_dir("pty-ask-edit");
    let home = isolate_dir("pty-ask-edit-home");
    let target = workspace.join("edit-target.txt");
    std::fs::write(&target, "seed-before-edit\n").expect("seed");
    let (addr, state) = spawn_mock(&target, &workspace.join("late.txt"));
    let config = write_acl(&home, &workspace, &format!("http://{addr}/v1"));
    let owned = child_env(&binaries::acp_binary(), &config, &home);
    let env: Vec<(&str, &str)> = owned
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect();
    let cwd = workspace.to_str().expect("cwd");
    let mut pty = open_pty(&binary, &["--cwd", cwd], &env, &workspace);
    let ready = pty.wait_for("mock/gpt", Duration::from_secs(45));
    assert!(
        ready.contains("Aster"),
        "welcome did not show Aster\n{ready}"
    );

    pty.type_text("ASK-ORACLE a choice");
    pty.enter();
    let card = pty.wait_for("alpha", Duration::from_secs(30));
    assert!(card.contains("beta"), "ask_user options missing\n{card}");
    pty.write_bytes(b"2");
    pty.wait_for("ASK-ANSWER-DONE", Duration::from_secs(30));
    let schema = aster_core::ask_user::USER_ANSWER_SCHEMA;
    let body = wait_body_all(
        &state,
        &["ASK-ORACLE a choice", schema],
        Duration::from_secs(10),
        true,
    );
    let request: serde_json::Value = serde_json::from_str(&body).unwrap();
    let answer = returned_result(&request["messages"], schema).expect("typed question answer");
    assert_eq!(answer["answer"], "beta");
    assert_eq!(answer["permission_grant"], false);
    let answers: Vec<_> = log_text(&workspace)
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|fact| fact["type"] == "question.answered")
        .collect();
    assert_eq!(answers.len(), 1);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(answers[0]["payload"]["text"].as_str().unwrap())
            .unwrap(),
        answer
    );
    oracle(
        "shipped_tui_ask_user_and_edit_tool_oracles",
        "ask-user-select",
        "option=beta",
    );

    pty.write_bytes(b"\x15");
    pty.type_text("/yolo");
    pty.enter();
    pty.pump(Duration::from_millis(400));
    pty.write_bytes(b"\x15");
    pty.type_text("EDIT-ORACLE please");
    pty.enter();
    let edit_deadline = Instant::now() + Duration::from_secs(30);
    let mut edited = String::new();
    while Instant::now() < edit_deadline {
        edited = std::fs::read_to_string(&target).unwrap_or_default();
        if edited.contains("edited-marker") {
            break;
        }
        pty.pump(Duration::from_millis(100));
    }
    assert!(
        edited.contains("edited-marker") && !edited.contains("seed-before-edit"),
        "edit tool did not replace the seed\ncurrent={edited}"
    );
    oracle(
        "shipped_tui_ask_user_and_edit_tool_oracles",
        "edit-tool",
        "edited-marker",
    );

    pty.write_bytes(b"\x15");
    pty.type_text("/quit");
    pty.enter();
    let status = pty.child.wait().expect("quit");
    assert!(status.success(), "/quit exited {status:?}");
    let _ = std::fs::remove_dir_all(&workspace);
    let _ = std::fs::remove_dir_all(&home);
}

fn form_keys(pty: &mut PtySession, sequence: &[&[u8]]) {
    for key in sequence {
        pty.write_bytes(key);
        pty.pump(Duration::from_millis(100));
    }
}

/// Native ask_user_form card: required/range rejection, submit, cancel, invalid schema.
#[test]
fn shipped_native_form_submit_cancel_and_reject() {
    let binary = tui_binary();
    assert!(
        binary.is_file(),
        "build aster before this test: {}",
        binary.display()
    );
    let workspace = isolate_dir("pty-native-form");
    let home = isolate_dir("pty-native-form-home");
    let (addr, _state) = spawn_mock(&workspace.join("unused.txt"), &workspace.join("late.txt"));
    let config = write_acl(&home, &workspace, &format!("http://{addr}/v1"));
    let owned = child_env(&binaries::acp_binary(), &config, &home);
    let env: Vec<(&str, &str)> = owned
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect();
    let cwd = workspace.to_str().expect("cwd");
    let mut pty = open_pty(&binary, &["--cwd", cwd], &env, &workspace);
    let ready = pty.wait_for("mock/gpt", Duration::from_secs(45));
    assert!(
        ready.contains("Aster"),
        "welcome did not show Aster\n{ready}"
    );

    pty.type_text("FORM-ORACLE fill the card");
    pty.enter();
    let card = pty.wait_for("Aster form", Duration::from_secs(30));
    assert!(
        card.contains("Name") && card.contains("Amount"),
        "native form fields missing\n{card}"
    );
    form_keys(&mut pty, &[b"\t", b"\t", b"\t", b"\r"]);
    let required = pty.wait_for("required", Duration::from_secs(10));
    assert!(
        required.to_ascii_lowercase().contains("required"),
        "empty submit skipped required validation\n{required}"
    );
    oracle(
        "shipped_native_form_submit_cancel_and_reject",
        "empty-field",
        "required",
    );

    form_keys(
        &mut pty,
        &[
            b"\x1b[A", b"\x1b[A", b"\x1b[A", b"\r", b"Ada", b"\r", b"\t", b"\r", b"\x7f", b"\x7f",
            b"\x7f", b"6", b"\r", b"\t", b"\x1b[C", b"\t", b"\r",
        ],
    );
    let range = pty.wait_for("max 5", Duration::from_secs(10));
    assert!(
        range.contains("max 5"),
        "amount 6 skipped range validation\n{range}"
    );
    oracle(
        "shipped_native_form_submit_cancel_and_reject",
        "range",
        "max 5",
    );

    form_keys(
        &mut pty,
        &[
            b"\x1b[A", b"\x1b[A", b"\r", b"\x7f", b"4", b"\r", b"\t", b"\t", b"\r",
        ],
    );
    let submitted = pty.wait_for("FORM-SUBMITTED", Duration::from_secs(30));
    assert!(
        submitted.contains("FORM-SUBMITTED"),
        "valid submit did not settle\n{submitted}"
    );
    oracle(
        "shipped_native_form_submit_cancel_and_reject",
        "FORM-SUBMITTED",
        "name=Ada amount=4 ready=true",
    );

    pty.write_bytes(b"\x15");
    pty.type_text("FORM-ORACLE-CANCEL cancel the card");
    pty.enter();
    pty.wait_for("Aster form", Duration::from_secs(30));
    pty.write_bytes(b"\x03");
    let cancelled = pty.wait_for("FORM-CANCELLED", Duration::from_secs(30));
    assert!(
        cancelled.contains("FORM-CANCELLED"),
        "cancel did not settle\n{cancelled}"
    );
    oracle(
        "shipped_native_form_submit_cancel_and_reject",
        "FORM-CANCELLED",
        "ctrl-c",
    );

    // Schema-refusal text is covered by Core ask_user_form unit tests and the
    // ignored live form-schema wave; keep this hermetic case on native card UX.

    pty.write_bytes(b"\x15");
    pty.type_text("/quit");
    pty.enter();
    let status = pty.child.wait().expect("quit");
    assert!(status.success(), "/quit exited {status:?}");
    let _ = std::fs::remove_dir_all(&workspace);
    let _ = std::fs::remove_dir_all(&home);
}

/// Reject and cancel answer the permission card and leave the write unrun.
#[test]
fn shipped_tui_permission_reject_and_cancel_skip_the_side_effect() {
    let binary = tui_binary();
    assert!(
        binary.is_file(),
        "build aster before this test: {}",
        binary.display()
    );
    let workspace = isolate_dir("pty-deny");
    let home = isolate_dir("pty-deny-home");
    let write_path = workspace.join("side-effect.txt");
    let (addr, _state) = spawn_mock(&write_path, &workspace.join("late.txt"));
    let config = write_acl(&home, &workspace, &format!("http://{addr}/v1"));
    let owned = child_env(&binaries::acp_binary(), &config, &home);
    let env: Vec<(&str, &str)> = owned
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect();
    let cwd = workspace.to_str().expect("cwd");
    let mut pty = open_pty(&binary, &["--cwd", cwd], &env, &workspace);
    let ready = pty.wait_for("mock/gpt", Duration::from_secs(45));
    assert!(
        ready.contains("Aster"),
        "welcome did not show Aster\n{ready}"
    );

    pty.type_text("ASK-WRITE reject this");
    pty.enter();
    let reject_prompt = pty.wait_for("Yes, proceed", Duration::from_secs(25));
    assert!(
        reject_prompt.contains("No"),
        "reject option missing\n{reject_prompt}"
    );
    assert!(
        !write_path.exists(),
        "write ran before the permission was rejected"
    );
    pty.write_bytes(b"2");
    let rejected = pty.wait_for("denied", Duration::from_secs(25));
    assert!(
        !write_path.exists(),
        "reject wrote the side effect\n{rejected}"
    );
    oracle(
        "shipped_tui_permission_reject_and_cancel_skip_the_side_effect",
        "permission-reject",
        "side-effect.txt absent",
    );

    pty.write_bytes(b"\x15");
    pty.type_text("/quit");
    pty.enter();
    let rejected_status = pty.child.wait().expect("quit after reject");
    assert!(
        rejected_status.success(),
        "/quit after reject exited {rejected_status:?}"
    );

    let mut pty = open_pty(&binary, &["--cwd", cwd], &env, &workspace);
    let ready = pty.wait_for("mock/gpt", Duration::from_secs(45));
    assert!(
        ready.contains("Aster"),
        "cancel launch did not show Aster\n{ready}"
    );
    pty.type_text("ASK-WRITE cancel this");
    pty.enter();
    let cancel_prompt = pty.wait_for("Yes, proceed", Duration::from_secs(25));
    assert!(
        !write_path.exists(),
        "write ran before cancel\n{cancel_prompt}"
    );
    pty.write_bytes(b"\x03");
    let cancel_deadline = Instant::now() + Duration::from_secs(20);
    let mut cancel_screen = String::new();
    while Instant::now() < cancel_deadline {
        cancel_screen = pty.pump(Duration::from_millis(150));
        if !cancel_screen.contains("Yes, proceed") {
            break;
        }
    }
    assert!(
        !cancel_screen.contains("Yes, proceed"),
        "cancel left the permission prompt\n{cancel_screen}"
    );
    assert!(
        !write_path.exists(),
        "cancel wrote the side effect\n{cancel_screen}"
    );
    oracle(
        "shipped_tui_permission_reject_and_cancel_skip_the_side_effect",
        "permission-cancel",
        "side-effect.txt absent",
    );

    pty.write_bytes(b"\x15");
    pty.type_text("/quit");
    pty.enter();
    let status = pty.child.wait().expect("quit");
    assert!(status.success(), "/quit exited {status:?}");
    let _ = std::fs::remove_dir_all(&workspace);
    let _ = std::fs::remove_dir_all(&home);
}

const REAL_GOAL_TEST: &str = "shipped_bound_goal_real_llm";
/// Live bound-goal pin: operator DeepSeek from `./.aster/config.acl`, not boyue.
const REAL_GOAL_MODEL: &str = "deepseek/deepseek-flash";

/// Explicit host ACL: the operator's live file plus trusted goal bounds.
fn write_real_goal_acl(home: &Path) -> PathBuf {
    let source =
        std::env::var_os("ASTER_CONFIG_FILE").expect("ASTER_CONFIG_FILE names the live ACL");
    let mut acl = std::fs::read_to_string(source).expect("live ACL");
    let deadline = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_millis()
        + 3_600_000;
    acl.push_str(&format!(
        "\ngoal_control {{\n  automatic_enabled = true\n  max_slices = \"4\"\n  \
         max_no_progress_slices = \"2\"\n  deadline_unix_ms = \"{deadline}\"\n}}\n",
    ));
    let path = home.join("goal-host.acl");
    std::fs::write(&path, acl).expect("goal host acl");
    path
}

async fn goal_ext(
    agent: &AgentProc,
    method: &'static str,
    params: serde_json::Value,
) -> serde_json::Value {
    let raw = serde_json::value::to_raw_value(&params).expect("goal params");
    let response = acp::Agent::ext_method(
        agent.conn.as_ref(),
        acp::ExtRequest::new(method, std::sync::Arc::from(raw)),
    )
    .await
    .unwrap_or_else(|error| panic!("{method}: {error:?}"));
    serde_json::from_str(response.0.get()).expect("goal json")
}

async fn goal_state(agent: &AgentProc, id: &acp::SessionId) -> serde_json::Value {
    goal_ext(
        agent,
        "aster/goal/state",
        serde_json::json!({"session_id": id.0.as_ref()}),
    )
    .await
}

fn goal_command(kind: &str, operation_id: &str, view: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "kind": kind, "operation_id": operation_id,
        "goal_id": view["binding"]["goal_id"], "session_id": view["binding"]["session_id"],
        "workspace": view["binding"]["workspace"],
        "work_policy_digest": view["binding"]["work_policy_digest"],
        "expected_goal_revision": view["binding"]["goal_revision"],
        "expected_plan_revision": view["plan_revision"],
        "expected_goal_sequence": view["goal_sequence"],
    })
}

/// Agent text since `from`, for one session, excluding replay.
fn agent_text(agent: &AgentProc, session: &acp::SessionId, from: usize) -> String {
    agent.replayed.borrow()[from..]
        .iter()
        .filter(|(sid, replay, _)| sid == session.0.as_ref() && !replay)
        .map(|(_, _, text)| text.as_str())
        .collect()
}

async fn slash(agent: &AgentProc, session: &acp::SessionId, text: &str) -> String {
    let from = agent.replayed.borrow().len();
    acp::Agent::prompt(agent.conn.as_ref(), user_prompt(session, text))
        .await
        .unwrap_or_else(|error| panic!("{text}: {error:?}"));
    agent_text(agent, session, from)
}

async fn connect_agent(workspace: &Path, config: &Path, home: &Path) -> AgentProc {
    let agent = spawn_agent(workspace, config, home).await;
    acp::Agent::initialize(
        agent.conn.as_ref(),
        acp::InitializeRequest::new(acp::ProtocolVersion::V1),
    )
    .await
    .expect("initialize");
    agent
}

async fn stop_agent(mut agent: AgentProc) {
    drop(agent.conn);
    let _ = agent.child.start_kill();
    let _ = agent.child.wait().await;
}

fn create_goal_command(
    session: &acp::SessionId,
    workspace: &Path,
    goal_id: &str,
    description: &str,
    criterion: &str,
) -> serde_json::Value {
    let root = workspace.display().to_string();
    serde_json::json!({
        "kind": "create", "operation_id": format!("{goal_id}-create"), "goal_id": goal_id,
        "session_id": session.0.as_ref(), "workspace": {"id": root, "display_root": root},
        "goal": {"description": description, "success_criteria": [criterion],
            "progress": 0.0, "achieved": false, "created_at": 1, "achieved_at": null},
        "non_goals": ["Do not modify release.txt"], "scope_fact_sequences": [],
        "work_policy_digest": null,
        "expected_goal_revision": null, "expected_plan_revision": null,
        "expected_goal_sequence": null,
    })
}

/// A policy session admits engineering-v1 (at create and/or first Run); a goal
/// binds only to that digest, and review admission works on the bound session.
async fn policy_goal_review(agent: &AgentProc, workspace: &Path) {
    let meta = serde_json::json!({"asterEngineering": {"policy": "engineering-v1"}});
    let id = acp::Agent::new_session(
        agent.conn.as_ref(),
        acp::NewSessionRequest::new(workspace.to_path_buf()).meta(meta.as_object().cloned()),
    )
    .await
    .expect("policy session")
    .session_id;
    let unbound = goal_state(agent, &id).await;
    assert_eq!(unbound["definition"], serde_json::Value::Null);
    let pre_digest = unbound["work_policy_digest"].clone();
    let answer = slash(
        agent,
        &id,
        "Reply with the word ADMITTED and nothing else. Do not use tools.",
    )
    .await;
    assert!(answer.contains("ADMITTED"), "policy turn: {answer}");
    let digest = goal_state(agent, &id).await["work_policy_digest"].clone();
    assert!(
        digest
            .as_str()
            .is_some_and(|value| value.starts_with("sha256:")),
        "the session admitted the policy: {digest}"
    );
    if !pre_digest.is_null() {
        assert_eq!(
            digest, pre_digest,
            "an already-admitted policy digest must stay stable across the turn"
        );
    }
    let mut create = create_goal_command(
        &id,
        workspace,
        "policy-goal",
        "Review release.txt",
        "Findings name release.txt",
    );
    let raw = serde_json::value::to_raw_value(&create).expect("create");
    assert!(
        acp::Agent::ext_method(
            agent.conn.as_ref(),
            acp::ExtRequest::new("aster/goal/definition", std::sync::Arc::from(raw)),
        )
        .await
        .is_err(),
        "a goal must bind the admitted policy"
    );
    create["work_policy_digest"] = digest.clone();
    let bound = goal_ext(agent, "aster/goal/definition", create).await;
    assert_eq!(bound["binding"]["work_policy_digest"], digest);
    let target = goal_ext(
        agent,
        "aster/review/target",
        serde_json::json!({"sessionId": id.0.as_ref(), "paths": ["release.txt"]}),
    )
    .await["digest"]
        .clone();
    let admitted = goal_ext(
        agent,
        "aster/review/admit",
        serde_json::json!({
            "sessionId": id.0.as_ref(), "scenarioId": "engineering.change",
            "sourceReviewId": "pager-review-1", "workPolicyDigest": digest,
            "target": {"paths": ["release.txt"], "digest": target},
            "scope": {"type": "sessionTarget"},
            "findings": [{"findingId": "f-1", "severity": "warning", "category": "correctness",
                "claim": "The release code has no checksum", "evidence": "release.txt:1",
                "subject": {"type": "opaque", "kind": "engineering.target", "uri": target}}],
        }),
    )
    .await;
    assert_eq!(admitted["admitted"], 1);
    oracle(
        REAL_GOAL_TEST,
        "stdio-policy-goal-review",
        &format!("policy={digest} findings=1"),
    );
}

/// Bind a goal and plan to the pager's session, then status and pause.
async fn bind_pager_session(agent: &AgentProc, workspace: &Path, id: &acp::SessionId) {
    acp::Agent::load_session(
        agent.conn.as_ref(),
        acp::LoadSessionRequest::new(id.clone(), workspace.to_path_buf()),
    )
    .await
    .expect("load the pager session");
    let prior = goal_state(agent, id).await;
    assert_eq!(prior["definition"], serde_json::Value::Null);
    let mut create = create_goal_command(
        id,
        workspace,
        "release-report",
        "Record the release code stored in release.txt in report.txt",
        "report.txt contains exactly the release code read from release.txt",
    );
    // Bind the digest the pager session already admitted on its seed turn.
    create["work_policy_digest"] = prior["work_policy_digest"].clone();
    let created = goal_ext(agent, "aster/goal/definition", create).await;
    // The shipped host verifies a write plus a passing shell check of that
    // path; a read-only step has no host report and is never accepted.
    let mut propose = goal_command("propose_plan", "pager-propose", &created);
    propose["proposal_sequence"] = serde_json::Value::Null;
    propose["plan"] = serde_json::json!({
        "goal": "Record the release code", "complexity": "Simple", "required_tools": [],
        "estimated_steps": 1,
        "steps": [{"id": "report",
                   "content": "Write the exact release code from release.txt (the code only) to a new file report.txt, then run the shell command `test -f report.txt`",
                   "status": "pending", "priority": "medium",
                   "success_criteria": "report.txt holds exactly the release code from release.txt and `test -f report.txt` passed after the write"}],
    });
    let proposed = goal_ext(agent, "aster/goal/plan", propose).await;
    let mut accept = goal_command("accept_plan", "pager-accept", &proposed);
    accept["plan"] = serde_json::Value::Null;
    accept["proposal_sequence"] = proposed["plan_fact_sequence"].clone();
    let accepted = goal_ext(agent, "aster/goal/plan", accept).await;
    assert_eq!(accepted["plan_accepted"], true);
    let status = slash(agent, id, "/goal status").await;
    assert!(
        status.contains("Goal in progress.")
            && status.contains("Completed 0 of 1 steps.")
            && status.contains("Next: Write the exact release code"),
        "{status}"
    );
    let paused = slash(agent, id, "/goal pause").await;
    assert!(
        paused.contains("Goal paused.") && paused.contains("Use /goal resume to continue."),
        "{paused}"
    );
    assert_eq!(goal_state(agent, id).await["definition"]["state"], "paused");
    oracle(
        REAL_GOAL_TEST,
        "stdio-bind-status-pause",
        "create+plan, active 0/1 next advance, paused",
    );
}

#[test]
#[ignore = "requires ASTER_CONFIG_FILE with deepseek/deepseek-flash and a built aster"]
fn shipped_bound_goal_real_llm() {
    let binary = tui_binary();
    assert!(binary.is_file(), "build aster: {}", binary.display());
    let started = Instant::now();
    let workspace = std::fs::canonicalize(isolate_dir("bound-goal")).expect("workspace");
    let home = isolate_dir("bound-goal-home");
    let code = format!("RC-{}", uuid::Uuid::new_v4().simple());
    std::fs::write(
        workspace.join("release.txt"),
        format!("release code: {code}\n"),
    )
    .expect("release file");
    let config = write_real_goal_acl(&home);
    // A policy session refuses a model switch before its first Run admits the
    // policy, so every agent starts on the model. This ignored test runs alone.
    std::env::set_var("ASTER_DEFAULT_MODEL", REAL_GOAL_MODEL);
    let acp_bin = binaries::acp_binary();
    let owned = child_env(&acp_bin, &config, &home);
    let env: Vec<(&str, &str)> = owned
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect();
    let cwd = workspace.to_str().expect("cwd");

    // The shipped pager creates and registers the session with one real turn.
    let number = std::process::id() % 9000 + 1000;
    let reply = format!("SEED-{number}");
    let mut pty = open_pty(&binary, &["--cwd", cwd], &env, &workspace);
    pty.wait_for_frame(Duration::from_secs(45));
    pty.type_text(&format!(
        "Reply with the word SEED, a hyphen, then the number {number}, and nothing else. Do not use tools."
    ));
    pty.enter();
    pty.wait_for(&reply, Duration::from_secs(180));
    pty.write_bytes(b"\x15");
    let info = pty.open_session_info(Duration::from_secs(15));
    let session_id = session_id_from_screen(&info);
    assert!(!session_id.is_empty(), "{info}");
    pty.escape();
    pty.pump(Duration::from_millis(200));
    pty.type_text("/quit");
    pty.enter();
    let _ = pty.child.wait();
    oracle(
        REAL_GOAL_TEST,
        "pager-seed",
        &format!("session={session_id}"),
    );

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    let local = tokio::task::LocalSet::new();
    let id = acp::SessionId::new(session_id.clone());
    local.block_on(&runtime, async {
        let agent = connect_agent(&workspace, &config, &home).await;
        policy_goal_review(&agent, &workspace).await;
        bind_pager_session(&agent, &workspace, &id).await;
        stop_agent(agent).await;
    });

    // The pager resumes the bound session; `/goal resume` runs one real slice.
    let mut pty = open_pty(
        &binary,
        &["--cwd", cwd, "--resume", &session_id],
        &env,
        &workspace,
    );
    pty.wait_for(&reply, Duration::from_secs(60));
    pty.write_bytes(b"\x15");
    pty.type_text("/goal status");
    pty.enter();
    let status = pty.wait_for("Goal paused.", Duration::from_secs(30));
    assert!(status.contains("Completed 0 of 1 steps."), "{status}");
    oracle(REAL_GOAL_TEST, "pager-reopen-status", "paused 0/1");
    let slice_started = Instant::now();
    pty.write_bytes(b"\x15");
    pty.type_text("/goal resume");
    pty.enter();
    let deadline = Instant::now() + Duration::from_secs(300);
    let mut screen = String::new();
    while Instant::now() < deadline {
        screen = pty.pump(Duration::from_millis(500));
        if screen.contains("Goal complete.") {
            break;
        }
        if screen.contains("Yes, proceed") {
            pty.write_bytes(b"1");
        }
    }
    assert!(screen.contains("Goal complete."), "{screen}");
    let report = std::fs::read_to_string(workspace.join("report.txt")).expect("report.txt");
    assert_eq!(report.trim(), code, "{screen}");
    let slice_ms = slice_started.elapsed().as_millis();
    oracle(
        REAL_GOAL_TEST,
        "pager-resume-completes",
        &format!("slice_ms={slice_ms}"),
    );
    pty.write_bytes(b"\x15");
    pty.type_text("/quit");
    pty.enter();
    let _ = pty.child.wait();

    // A new agent process reopens the completed goal: nothing resumes or runs.
    local.block_on(&runtime, async {
        let agent = connect_agent(&workspace, &config, &home).await;
        acp::Agent::load_session(
            agent.conn.as_ref(),
            acp::LoadSessionRequest::new(id.clone(), workspace.clone()),
        )
        .await
        .expect("reopen");
        let before = goal_state(&agent, &id).await;
        assert_eq!(before["definition"]["state"], "completed");
        assert_eq!(before["definition"]["plan"]["steps"][0]["status"], "completed");
        let resumed = slash(&agent, &id, "/goal resume").await;
        assert!(
            resumed.contains("Goal complete.") && !resumed.contains("Goal slice"),
            "{resumed}"
        );
        assert_eq!(
            goal_state(&agent, &id).await,
            before,
            "a completed goal records nothing"
        );
        let info = session_info_body(agent.conn.as_ref(), &id).await;
        oracle(REAL_GOAL_TEST, "reopen-completed", "no slice, no fact");

        // Clear on the shipped binary cancels an unstarted goal.
        let other = acp::Agent::new_session(
            agent.conn.as_ref(),
            acp::NewSessionRequest::new(workspace.clone()),
        )
        .await
        .expect("second session")
        .session_id;
        let mut clear_create =
            create_goal_command(&other, &workspace, "clear-me", "Never started", "Cancelled");
        clear_create["work_policy_digest"] =
            goal_state(&agent, &other).await["work_policy_digest"].clone();
        let created = goal_ext(&agent, "aster/goal/definition", clear_create).await;
        assert_eq!(created["state"], "active");
        let cleared = slash(&agent, &other, "/goal clear").await;
        assert!(cleared.contains("Goal cancelled."), "{cleared}");
        oracle(REAL_GOAL_TEST, "stdio-clear", "active→cancelled");
        eprintln!(
            "BOUND-GOAL-REAL model={REAL_GOAL_MODEL} total_ms={} slice_ms={slice_ms} session_info={info}",
            started.elapsed().as_millis()
        );
        stop_agent(agent).await;
    });
    let _ = std::fs::remove_dir_all(&workspace);
    let _ = std::fs::remove_dir_all(&home);
}
