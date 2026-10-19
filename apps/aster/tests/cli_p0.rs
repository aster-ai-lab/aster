//! P0 process oracles for the shipped `aster` binary.
//!
//! Every check launches `CARGO_BIN_EXE_aster` against an isolated home and a
//! local OpenAI-compatible fixture. Nothing here talks to a public provider
//! or to `127.0.0.1:9`.

use std::collections::BTreeSet;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

const FIXTURE_KEY: &str = "p0-fixture-key-do-not-leak";
const MODEL_ID: &str = "fixture/local";
const SHARED_TITLE: &str = "shared fixture title";

struct MockState {
    tool_seq: u64,
    shell_sent: bool,
}

struct Isolated {
    home: PathBuf,
    grok_home: PathBuf,
    workspace: PathBuf,
    config: PathBuf,
    /// Read only by the macOS shell-refusal oracle.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    marker: PathBuf,
}

fn main_aster() -> &'static str {
    env!("CARGO_BIN_EXE_aster")
}

fn isolate() -> (PathBuf, PathBuf, PathBuf) {
    let stamp = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or(0);
    let root = std::env::temp_dir().join(format!("aster-cli-p0-{}-{stamp}", std::process::id()));
    let home = root.join("home");
    let workspace = root.join("workspace");
    let grok_home = home.join("grok");
    fs::create_dir_all(&home).expect("home");
    fs::create_dir_all(&workspace).expect("workspace");
    (home, grok_home, workspace)
}

fn spawn_mock(marker: &Path) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind fixture");
    let addr = listener.local_addr().expect("fixture addr");
    let marker_text = marker.display().to_string();
    let state = Arc::new(Mutex::new(MockState {
        tool_seq: 0,
        shell_sent: false,
    }));
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
            let Some(body) = read_http_body(&mut stream) else {
                continue;
            };
            let response = {
                let mut guard = state.lock().expect("mock");
                decide(&body, &marker_text, &mut guard)
            };
            let _ = write_http(&mut stream, &response);
        }
    });
    format!("http://{addr}/v1")
}

fn excerpt_around(body: &str, needle: &str, radius: usize) -> String {
    let Some(at) = body.find(needle) else {
        return String::new();
    };
    let start = at.saturating_sub(radius);
    let end = (at + needle.len() + radius).min(body.len());
    body[start..end].replace(['\n', '\r'], " ")
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

fn decide(raw: &str, marker: &str, state: &mut MockState) -> String {
    let parsed: serde_json::Value = serde_json::from_str(raw).unwrap_or(serde_json::Value::Null);
    let streaming = parsed
        .get("stream")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    if raw.contains("emit_pre_analysis") || raw.contains("You are a pre-analysis assistant") {
        return reply_text(&pre_analysis(), streaming);
    }
    if raw.contains("generating the session title") {
        let title = if raw.contains("TITLE-SHARED") {
            SHARED_TITLE.to_string()
        } else if raw.contains("SHELL-ORACLE") {
            "shell refusal check".to_string()
        } else if raw.contains("SANDBOX-ORIGIN") {
            "sandbox origin".to_string()
        } else if let Some(token) = ping_token(raw) {
            format!("ping {token}")
        } else {
            "fixture session".to_string()
        };
        let args = serde_json::json!({ "session_title": title }).to_string();
        return reply_tool("session_title", &args, state, streaming);
    }
    if raw.contains("SHELL-ORACLE") && has_tools(&parsed) {
        if state.shell_sent || raw.contains("native_process_lifetime_unqualified") {
            return reply_text("SHELL-DONE", streaming);
        }
        state.shell_sent = true;
        let command = format!("printf side-effect > '{marker}'");
        let args = serde_json::json!({
            "command": command,
            "description": "write the shell side-effect marker"
        })
        .to_string();
        return reply_tool("run_terminal_command", &args, state, streaming);
    }
    if let Some(token) = ping_token(raw) {
        return reply_text(&format!("PONG-{token}"), streaming);
    }
    reply_text("ok", streaming)
}

fn ping_token(raw: &str) -> Option<String> {
    // A continued session repeats earlier PING tokens. Answer the latest one.
    let rest = raw.rsplit_once("PING-")?.1;
    let token: String = rest
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '-')
        .collect();
    (!token.is_empty()).then_some(token)
}

fn has_tools(body: &serde_json::Value) -> bool {
    body.get("tools")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|tools| !tools.is_empty())
}

fn pre_analysis() -> String {
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
            "usage": { "prompt_tokens": 3, "completion_tokens": 2, "total_tokens": 5 }
        })
        .to_string();
    }
    let chunk = serde_json::json!({
        "id": "chatcmpl-p0",
        "object": "chat.completion.chunk",
        "created": 1,
        "model": "local",
        "choices": [{
            "index": 0,
            "delta": { "role": "assistant", "content": text },
            "finish_reason": null
        }]
    });
    let done = serde_json::json!({
        "id": "chatcmpl-p0",
        "object": "chat.completion.chunk",
        "created": 1,
        "model": "local",
        "choices": [{ "index": 0, "delta": {}, "finish_reason": "stop" }],
        "usage": { "prompt_tokens": 3, "completion_tokens": 2, "total_tokens": 5 }
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
            "usage": { "prompt_tokens": 3, "completion_tokens": 2, "total_tokens": 5 }
        })
        .to_string();
    }
    let chunk = serde_json::json!({
        "id": "chatcmpl-p0",
        "object": "chat.completion.chunk",
        "created": 1,
        "model": "local",
        "choices": [{
            "index": 0,
            "delta": {
                "role": "assistant",
                "content": null,
                "tool_calls": [{
                    "index": 0,
                    "id": id,
                    "type": "function",
                    "function": { "name": name, "arguments": args }
                }]
            },
            "finish_reason": null
        }]
    });
    let done = serde_json::json!({
        "id": "chatcmpl-p0",
        "object": "chat.completion.chunk",
        "created": 1,
        "model": "local",
        "choices": [{ "index": 0, "delta": {}, "finish_reason": "tool_calls" }],
        "usage": { "prompt_tokens": 3, "completion_tokens": 2, "total_tokens": 5 }
    });
    format!("data: {chunk}\n\ndata: {done}\n\ndata: [DONE]\n\n")
}

struct Captured {
    status: i32,
    stdout: String,
    stderr: String,
}

impl Captured {
    fn combined(&self) -> String {
        format!("{}\n{}", self.stdout, self.stderr)
    }

    fn success(&self) -> bool {
        self.status == 0
    }
}

fn with_fixture_model(args: &[&str]) -> Vec<String> {
    let mut owned: Vec<String> = args.iter().map(|arg| (*arg).to_string()).collect();
    let prompts = args.iter().any(|arg| *arg == "-p" || *arg == "--single");
    let pinned = args.iter().any(|arg| *arg == "-m" || *arg == "--model");
    if prompts && !pinned {
        owned.insert(0, MODEL_ID.to_string());
        owned.insert(0, "-m".to_string());
    }
    owned
}

fn write_shell_config(isolated: &Isolated, base_url: &str) {
    let toml = format!(
        r#"
[models]
default = "{MODEL_ID}"

[model."{MODEL_ID}"]
model = "local"
base_url = "{base_url}"
api_key = "{FIXTURE_KEY}"
api_backend = "chat_completions"
context_window = 8192
supports_reasoning_effort = false
"#
    );
    fs::write(isolated.grok_home.join("config.toml"), toml).expect("shell config");
}

fn run(isolated: &Isolated, args: &[&str], timeout: Duration) -> Captured {
    let path = std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".to_string());
    let args = with_fixture_model(args);
    let mut command = Command::new(main_aster());
    command
        .args(&args)
        .current_dir(&isolated.workspace)
        .env_clear()
        .env("HOME", &isolated.home)
        .env("GROK_HOME", &isolated.grok_home)
        .env("A3S_CONFIG", &isolated.config)
        .env("A3S_DISABLE_CC_SWITCH", "1")
        .env("A3S_DISABLE_GROK_ACCOUNT", "1")
        .env("A3S_DISABLE_CURSOR_ACCOUNT", "1")
        .env("GROK_MANAGED_CONFIG", "0")
        .env("PATH", path)
        .env("TERM", "xterm-256color")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(tmpdir) = std::env::var_os("TMPDIR") {
        command.env("TMPDIR", tmpdir);
    }
    let mut child = command.spawn().expect("spawn aster");
    let mut stdout = child.stdout.take().expect("stdout");
    let mut stderr = child.stderr.take().expect("stderr");
    let stdout_thread = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stdout.read_to_end(&mut buf);
        String::from_utf8_lossy(&buf).into_owned()
    });
    let stderr_thread = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stderr.read_to_end(&mut buf);
        String::from_utf8_lossy(&buf).into_owned()
    });
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().expect("wait aster") {
            break status;
        }
        if started.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            panic!("aster timed out after {timeout:?}: {args:?}");
        }
        std::thread::sleep(Duration::from_millis(30));
    };
    Captured {
        status: status.code().unwrap_or(1),
        stdout: stdout_thread.join().expect("stdout thread"),
        stderr: stderr_thread.join().expect("stderr thread"),
    }
}

fn oracle(name: &str, detail: &str) {
    let detail = detail.replace(['\n', '\r'], " ");
    println!("ORACLE name={name} detail={detail}");
}

fn assert_no_fixture_key(captured: &Captured, name: &str) {
    let combined = captured.combined();
    assert!(
        !combined.contains(FIXTURE_KEY),
        "{name} leaked the fixture key:\n{combined}"
    );
}

fn assert_no_installs(grok_home: &Path, name: &str) {
    for file in INSTALL_FILES {
        assert!(
            !grok_home.join(file).exists(),
            "{name} wrote install file {file}"
        );
    }
}

fn assert_no_public_host(captured: &Captured, name: &str) {
    let combined = captured.combined();
    for host in ["api.x.ai", "grok.com", "auth.a3s", "googleapis.com"] {
        assert!(
            !combined.contains(host),
            "{name} contacted or named {host}:\n{combined}"
        );
    }
}

/// A utility command must finish with a local result or a fail-closed status.
fn assert_utility(captured: &Captured, grok_home: &Path, name: &str) {
    assert_no_fixture_key(captured, name);
    assert_no_installs(grok_home, name);
    assert_no_public_host(captured, name);
}

fn assert_closed_utilities(isolated: &Isolated, quick: Duration) {
    let sessions = run(isolated, &["sessions", "list"], quick);
    assert!(sessions.success(), "{}", sessions.combined());
    assert_utility(&sessions, &isolated.grok_home, "sessions");
    oracle("sessions", "local list");

    let usage = run(isolated, &["usage", "missing-session"], quick);
    assert_ne!(usage.status, 0, "{}", usage.combined());
    assert_utility(&usage, &isolated.grok_home, "usage");
    oracle("usage-missing", "fail closed");

    let exported = run(isolated, &["export", "missing-session"], quick);
    assert_ne!(exported.status, 0, "{}", exported.combined());
    assert_utility(&exported, &isolated.grok_home, "export");
    oracle("export-missing", "fail closed");

    let memory = run(isolated, &["memory", "clear"], quick);
    assert!(memory.success(), "{}", memory.combined());
    assert!(
        memory.stdout.contains("Nothing to clear") || memory.stdout.contains("Cancelled"),
        "{}",
        memory.stdout
    );
    assert_utility(&memory, &isolated.grok_home, "memory");
    oracle("memory", memory.stdout.trim());

    let mcp = run(isolated, &["mcp", "list", "--json"], quick);
    assert!(mcp.success(), "{}", mcp.combined());
    assert!(parse_json_stdout(&mcp.stdout).is_array(), "{}", mcp.stdout);
    assert_utility(&mcp, &isolated.grok_home, "mcp");
    oracle("mcp", "local server list");

    let plugin = run(isolated, &["plugin", "list", "--json"], quick);
    assert!(plugin.success(), "{}", plugin.combined());
    assert!(
        parse_json_stdout(&plugin.stdout).is_array(),
        "{}",
        plugin.stdout
    );
    assert_utility(&plugin, &isolated.grok_home, "plugin");
    oracle("plugin", "local plugin list");

    let worktree = run(isolated, &["worktree", "list", "--json"], quick);
    if worktree.success() {
        let listed = parse_json_stdout(&worktree.stdout);
        assert!(
            listed.is_array() || listed.is_object(),
            "{}",
            worktree.stdout
        );
    }
    assert_utility(&worktree, &isolated.grok_home, "worktree");
    oracle(
        "worktree",
        &format!("exit={} {}", worktree.status, worktree.stdout.trim()),
    );

    let du = run(isolated, &["du", "--json"], quick);
    assert!(du.success(), "{}", du.combined());
    let report = parse_json_stdout(&du.stdout);
    assert!(report["schema_version"].is_number(), "{}", du.stdout);
    assert!(
        report["grok_home"]
            .as_str()
            .is_some_and(|path| path.contains("aster-cli-p0")),
        "{}",
        du.stdout
    );
    assert_utility(&du, &isolated.grok_home, "du");
    oracle("du", "local home report");

    let leader = run(isolated, &["leader", "list", "--json"], quick);
    assert!(leader.success(), "{}", leader.combined());
    assert!(
        parse_json_stdout(&leader.stdout).is_array(),
        "{}",
        leader.stdout
    );
    assert_utility(&leader, &isolated.grok_home, "leader");
    oracle("leader", "local leader list");

    let logout = run(isolated, &["logout"], quick);
    assert!(logout.success(), "{}", logout.combined());
    assert!(
        logout.combined().contains("No cached session") || logout.combined().contains("Logged out"),
        "{}",
        logout.combined()
    );
    assert_utility(&logout, &isolated.grok_home, "logout");
    oracle("logout", logout.combined().trim());

    let trace = run(
        isolated,
        &["trace", "--local", "--json", "missing-session"],
        quick,
    );
    assert_ne!(trace.status, 0, "{}", trace.combined());
    assert!(
        trace.combined().contains("not found"),
        "{}",
        trace.combined()
    );
    assert_utility(&trace, &isolated.grok_home, "trace");
    oracle("trace", "missing session fail closed");
}

fn parse_json_stdout(stdout: &str) -> serde_json::Value {
    let trimmed = stdout.trim();
    if let Ok(value) = serde_json::from_str(trimmed) {
        return value;
    }
    let bytes = trimmed.as_bytes();
    for (index, byte) in bytes.iter().enumerate() {
        if *byte != b'{' {
            continue;
        }
        if let Ok(value) = serde_json::from_str(&trimmed[index..]) {
            return value;
        }
    }
    panic!("stdout is not JSON:\n{stdout}");
}

fn session_id_of(captured: &Captured) -> String {
    let value = parse_json_stdout(&captured.stdout);
    value["sessionId"]
        .as_str()
        .unwrap_or_else(|| panic!("missing sessionId in {}", captured.stdout))
        .to_string()
}

const INSTALL_FILES: &[&str] = &[
    "managed_config.toml",
    "requirements.toml",
    "managed_config_cache.json",
    "managed_config.sig.json",
    "managed_identity.sig.json",
];

fn install_snapshot(home: &Path) -> String {
    INSTALL_FILES
        .iter()
        .map(|name| {
            let path = home.join(name);
            match fs::metadata(&path) {
                Ok(meta) => format!("{name} {}", meta.len()),
                Err(_) => format!("{name} <absent>"),
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn fingerprint_a3s(path: &Path) -> String {
    if !path.exists() {
        return "<missing>".to_string();
    }
    let mut rows = Vec::new();
    walk_a3s(path, path, &mut rows);
    rows.sort();
    rows.join("\n")
}

fn a3s_net_heartbeat(relative: &str) -> bool {
    let mut parts = relative.split('/');
    if parts.next() != Some("boxes") || parts.next().is_none() {
        return false;
    }
    matches!(
        (parts.next(), parts.next(), parts.next()),
        (Some("sockets"), None, None) | (Some("sockets"), Some("net.stats.json"), None)
    )
}

fn walk_a3s(root: &Path, dir: &Path, rows: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let relative = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .display()
            .to_string();
        // A live box netproxy rewrites this heartbeat and its directory mtime.
        if a3s_net_heartbeat(&relative) {
            rows.push(format!("{relative} <live>"));
            if path.is_dir() {
                walk_a3s(root, &path, rows);
            }
            continue;
        }
        let meta = entry.metadata().ok();
        let len = meta.as_ref().map(|meta| meta.len()).unwrap_or(0);
        let modified = meta
            .and_then(|meta| meta.modified().ok())
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|time| time.as_secs())
            .unwrap_or(0);
        rows.push(format!("{relative} {len} {modified}"));
        if path.is_dir() {
            walk_a3s(root, &path, rows);
        }
    }
}

fn assert_same_tree(label: &str, before: &str, after: &str) {
    if before == after {
        return;
    }
    let before_lines: BTreeSet<&str> = before.lines().collect();
    let after_lines: BTreeSet<&str> = after.lines().collect();
    let mut delta = String::new();
    for line in before_lines.difference(&after_lines) {
        delta.push_str("\n- ");
        delta.push_str(line);
    }
    for line in after_lines.difference(&before_lines) {
        delta.push_str("\n+ ");
        delta.push_str(line);
    }
    panic!("{label} changed{delta}");
}

fn local_session_ids(grok_home: &Path) -> Vec<String> {
    let mut ids = Vec::new();
    fn walk(dir: &Path, ids: &mut Vec<String>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            if path.join("summary.json").is_file()
                && let Some(name) = path.file_name().and_then(|name| name.to_str())
            {
                ids.push(name.to_string());
            }
            walk(&path, ids);
        }
    }
    walk(&grok_home.join("sessions"), &mut ids);
    ids
}

fn path_has_component(root: &Path, name: &str) -> bool {
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            if entry.file_name() == name {
                return true;
            }
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            }
        }
    }
    false
}

fn summary_profile(grok_home: &Path, session_id: &str) -> String {
    let sessions = grok_home.join("sessions");
    let mut found = None;
    walk_summaries(&sessions, session_id, &mut found);
    found.unwrap_or_else(|| {
        panic!(
            "no summary.json for {session_id} under {}",
            sessions.display()
        )
    })
}

fn walk_summaries(dir: &Path, session_id: &str, found: &mut Option<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_summaries(&path, session_id, found);
            continue;
        }
        if path.file_name().and_then(|name| name.to_str()) != Some("summary.json") {
            continue;
        }
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        if !text.contains(session_id) {
            continue;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
            continue;
        };
        if let Some(profile) = value.get("sandbox_profile").and_then(|v| v.as_str()) {
            *found = Some(profile.to_string());
        }
    }
}

#[test]
fn shipped_cli_p0_oracles() {
    let developer_home = PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/".into()));
    let developer_a3s = developer_home.join(".a3s");
    let developer_grok = developer_home.join(".grok");
    let before_a3s = fingerprint_a3s(&developer_a3s);
    let binary = PathBuf::from(main_aster());
    let binary_before = fs::metadata(&binary).expect("aster binary").len();

    let marker_slot =
        std::env::temp_dir().join(format!("aster-cli-p0-marker-{}", std::process::id()));
    let _ = fs::remove_file(&marker_slot);
    let base_url = spawn_mock(&marker_slot);
    let (home, grok_home, workspace) = isolate();
    let config = workspace.join("config.acl");
    fs::write(
        &config,
        format!(
            r#"
providers "fixture" {{
  base_url = "{base_url}"
  api_key = "{FIXTURE_KEY}"
  models "local" {{
  }}
}}
default_model = "{MODEL_ID}"
"#
        ),
    )
    .expect("acl");
    let isolated = Isolated {
        home,
        grok_home,
        workspace,
        config,
        marker: marker_slot,
    };

    let quick = Duration::from_secs(60);
    let turn = Duration::from_secs(180);

    let version = run(&isolated, &["version", "--json"], quick);
    assert!(version.success(), "{}", version.combined());
    let version_json = parse_json_stdout(&version.stdout);
    assert_eq!(version_json["product"], "Aster", "{}", version.stdout);
    assert!(version.stdout.contains("Aster"), "{}", version.stdout);
    assert_no_fixture_key(&version, "version");
    oracle("version", version.stdout.trim());

    let doctor = run(&isolated, &["doctor", "--json"], quick);
    assert!(doctor.success(), "{}", doctor.combined());
    let doctor_json = parse_json_stdout(&doctor.stdout);
    assert!(
        doctor_json.is_object() || doctor_json.is_array(),
        "{}",
        doctor.stdout
    );
    assert_no_fixture_key(&doctor, "doctor");
    oracle("doctor", "json diagnostic report");

    let inspect = run(&isolated, &["inspect", "--json"], quick);
    assert!(inspect.success(), "{}", inspect.combined());
    let _inspect_json = parse_json_stdout(&inspect.stdout);
    assert_no_fixture_key(&inspect, "inspect");
    oracle("inspect", "json configuration report");

    let installs_before_setup = install_snapshot(&isolated.grok_home);
    let setup = run(&isolated, &["setup", "--json"], quick);
    assert!(setup.success(), "{}", setup.combined());
    let setup_json = parse_json_stdout(&setup.stdout);
    assert!(setup_json.is_object(), "{}", setup.stdout);
    assert_eq!(setup_json["configured"], false, "{}", setup.stdout);
    assert_eq!(
        install_snapshot(&isolated.grok_home),
        installs_before_setup,
        "setup --json wrote install files"
    );
    for name in INSTALL_FILES {
        assert!(
            !isolated.grok_home.join(name).exists(),
            "setup --json left install file {name}"
        );
    }
    assert_no_fixture_key(&setup, "setup");
    oracle("setup", setup.stdout.trim());

    let update = run(&isolated, &["update", "--check", "--json"], quick);
    assert!(update.success(), "{}", update.combined());
    let update_json = parse_json_stdout(&update.stdout);
    assert_eq!(update_json["updateAvailable"], false, "{}", update.stdout);
    let binary_after = fs::metadata(&binary).expect("aster binary").len();
    assert_eq!(binary_before, binary_after, "update --check replaced aster");
    assert_no_fixture_key(&update, "update");
    oracle("update-check", update.stdout.trim());

    let share = run(
        &isolated,
        &["share", "00000000-0000-4000-8000-000000000001"],
        quick,
    );
    assert_ne!(
        share.status,
        0,
        "share must fail closed: {}",
        share.combined()
    );
    let share_text = share.combined();
    assert!(
        share_text.contains("temporarily disabled") || share_text.contains("disabled"),
        "{share_text}"
    );
    assert!(
        !share_text.contains("http://") && !share_text.contains("https://"),
        "{share_text}"
    );
    assert_no_fixture_key(&share, "share");
    oracle(
        "share",
        &format!("exit={} {}", share.status, share_text.trim()),
    );

    let completions = run(&isolated, &["completions", "zsh"], quick);
    assert!(completions.success(), "{}", completions.combined());
    assert!(
        completions.stdout.contains("aster"),
        "completions did not register aster:\n{}",
        completions.stdout
    );
    assert_no_fixture_key(&completions, "completions");
    oracle("completions", "zsh script registers aster");

    write_shell_config(&isolated, &base_url);
    assert_closed_utilities(&isolated, quick);

    let models = run(&isolated, &["models"], turn);
    assert!(models.success(), "{}", models.combined());
    assert!(
        models.stdout.contains(MODEL_ID),
        "models did not name {MODEL_ID}:\n{}",
        models.stdout
    );
    assert_no_fixture_key(&models, "models");
    oracle("models", models.stdout.trim());

    let created = run(
        &isolated,
        &[
            "-p",
            "PING-ORIGIN reply with the token",
            "--output-format",
            "json",
            "--yolo",
            "--trust",
            "--no-auto-update",
            "--disable-web-search",
        ],
        turn,
    );
    assert!(created.success(), "{}", created.combined());
    assert!(created.stdout.contains("PONG-ORIGIN"), "{}", created.stdout);
    let origin = session_id_of(&created);
    assert_no_fixture_key(&created, "headless-create");
    oracle(
        "headless-create",
        &format!("session={origin} token=PONG-ORIGIN"),
    );

    let listed = run(&isolated, &["sessions", "list"], quick);
    assert!(listed.success(), "{}", listed.combined());
    assert!(listed.stdout.contains(&origin), "{}", listed.stdout);
    oracle("sessions-list", "origin is listed");

    let usage = run(&isolated, &["usage", &origin], quick);
    assert!(usage.success(), "{}", usage.combined());
    let _usage_json = parse_json_stdout(&usage.stdout);
    assert!(usage.stdout.contains(&origin), "{}", usage.stdout);
    oracle("usage", "persisted usage json");

    let exported = run(&isolated, &["export", &origin], quick);
    assert!(exported.success(), "{}", exported.combined());
    assert!(
        exported.stdout.contains("PING-ORIGIN") || exported.stdout.contains("PONG-ORIGIN"),
        "{}",
        exported.stdout
    );
    let trace_path = isolated.workspace.join("trace.tar.gz");
    let traced = run(
        &isolated,
        &[
            "trace",
            "--local",
            "--json",
            &origin,
            "-o",
            trace_path.to_str().expect("trace path"),
        ],
        quick,
    );
    assert!(traced.success(), "{}", traced.combined());
    let trace_json = parse_json_stdout(&traced.stdout);
    assert_eq!(trace_json["status"], "exported", "{}", traced.stdout);
    assert!(trace_json["url"].is_null(), "{}", traced.stdout);
    let bundle_len = fs::metadata(&trace_path).expect("trace bundle").len();
    assert!(
        trace_path.is_file() && bundle_len > 0,
        "local trace bundle was not written"
    );
    assert_no_fixture_key(&traced, "trace-local");
    assert_no_public_host(&traced, "trace-local");
    oracle(
        "trace-local",
        &format!("status=exported url=null bytes={bundle_len}"),
    );
    oracle("export", "transcript contains the turn");

    let continued = run(
        &isolated,
        &[
            "-c",
            "-p",
            "PING-CONTINUE reply with the token",
            "--output-format",
            "json",
            "--yolo",
            "--no-auto-update",
            "--disable-web-search",
        ],
        turn,
    );
    assert!(continued.success(), "{}", continued.combined());
    assert_eq!(session_id_of(&continued), origin, "{}", continued.stdout);
    assert!(
        continued.stdout.contains("PONG-CONTINUE"),
        "{}",
        continued.stdout
    );
    oracle(
        "continue",
        &format!("reattached {origin} token=PONG-CONTINUE"),
    );

    let resumed = run(
        &isolated,
        &[
            "--resume",
            &origin,
            "-p",
            "PING-RESUME reply with the token",
            "--output-format",
            "json",
            "--yolo",
            "--no-auto-update",
            "--disable-web-search",
        ],
        turn,
    );
    assert!(resumed.success(), "{}", resumed.combined());
    assert_eq!(session_id_of(&resumed), origin, "{}", resumed.stdout);
    assert!(resumed.stdout.contains("PONG-RESUME"), "{}", resumed.stdout);
    oracle(
        "resume-id",
        &format!("reattached {origin} token=PONG-RESUME"),
    );

    for prompt in ["TITLE-SHARED alpha", "TITLE-SHARED beta"] {
        let titled = run(
            &isolated,
            &[
                "-p",
                prompt,
                "--output-format",
                "json",
                "--yolo",
                "--no-auto-update",
                "--disable-web-search",
            ],
            turn,
        );
        assert!(titled.success(), "{prompt}: {}", titled.combined());
    }
    let ambiguous = run(
        &isolated,
        &[
            "--resume",
            SHARED_TITLE,
            "-p",
            "PING-AMBIGUOUS reply with the token",
            "--output-format",
            "json",
            "--yolo",
            "--no-auto-update",
        ],
        quick,
    );
    assert_ne!(ambiguous.status, 0, "{}", ambiguous.combined());
    let ambiguous_text = ambiguous.combined();
    assert!(
        ambiguous_text.contains("Multiple sessions match title"),
        "{ambiguous_text}"
    );
    assert!(
        !ambiguous_text.contains("PONG-AMBIGUOUS"),
        "ambiguous title resume chose a session:\n{ambiguous_text}"
    );
    oracle("resume-title", "ambiguous title refused");

    let forked = run(
        &isolated,
        &[
            "--resume",
            &origin,
            "--fork-session",
            "-p",
            "PING-FORK reply with the token",
            "--output-format",
            "json",
            "--yolo",
            "--no-auto-update",
            "--disable-web-search",
        ],
        turn,
    );
    assert!(forked.success(), "{}", forked.combined());
    assert!(forked.stdout.contains("PONG-FORK"), "{}", forked.stdout);
    let fork_id = session_id_of(&forked);
    assert_ne!(fork_id, origin, "{}", forked.stdout);
    let listed_after_fork = run(&isolated, &["sessions", "list"], quick);
    assert!(
        listed_after_fork.stdout.contains(&origin),
        "{}",
        listed_after_fork.stdout
    );
    assert!(
        listed_after_fork.stdout.contains(&fork_id),
        "{}",
        listed_after_fork.stdout
    );
    oracle(
        "fork",
        &format!("new={fork_id} original={origin} token=PONG-FORK"),
    );

    let deleted = run(&isolated, &["sessions", "delete", &origin], quick);
    assert!(deleted.success(), "{}", deleted.combined());
    assert!(deleted.stdout.contains(&origin), "{}", deleted.stdout);
    let listed_after_delete = run(&isolated, &["sessions", "list"], quick);
    assert!(
        !listed_after_delete.stdout.contains(&origin),
        "{}",
        listed_after_delete.stdout
    );
    let export_missing = run(&isolated, &["export", &origin], quick);
    assert_ne!(export_missing.status, 0, "{}", export_missing.combined());
    assert!(
        export_missing.combined().contains("not found"),
        "{}",
        export_missing.combined()
    );
    let usage_missing = run(&isolated, &["usage", &origin], quick);
    assert_ne!(usage_missing.status, 0, "{}", usage_missing.combined());
    assert!(
        usage_missing.combined().contains("not found"),
        "{}",
        usage_missing.combined()
    );
    let resume_missing = run(
        &isolated,
        &[
            "--resume",
            &origin,
            "-p",
            "PING-GONE reply with the token",
            "--output-format",
            "json",
            "--yolo",
            "--no-auto-update",
        ],
        turn,
    );
    assert_ne!(resume_missing.status, 0, "{}", resume_missing.combined());
    assert!(
        !resume_missing.stdout.contains("PONG-GONE"),
        "deleted session was resumed:\n{}",
        resume_missing.combined()
    );
    let resume_missing_text = resume_missing.combined().to_ascii_lowercase();
    assert!(
        resume_missing_text.contains("not found")
            || resume_missing_text.contains("no session")
            || resume_missing_text.contains("does not exist")
            || resume_missing_text.contains("could not")
            || resume_missing_text.contains("failed"),
        "{}",
        resume_missing.combined()
    );
    oracle(
        "delete",
        &format!("{origin} is gone from list, export, usage, and resume"),
    );

    let sandboxed = run(
        &isolated,
        &[
            "--sandbox",
            "workspace",
            "-p",
            "PING-SANDBOX-ORIGIN reply with the token",
            "--output-format",
            "json",
            "--yolo",
            "--trust",
            "--no-auto-update",
            "--disable-web-search",
        ],
        turn,
    );
    assert!(sandboxed.success(), "{}", sandboxed.combined());
    let sandbox_id = session_id_of(&sandboxed);
    assert_eq!(
        summary_profile(&isolated.grok_home, &sandbox_id),
        "workspace",
        "session was not stored with the workspace profile"
    );
    let refused = run(
        &isolated,
        &[
            "--resume",
            &sandbox_id,
            "--sandbox",
            "read-only",
            "-p",
            "PING-SANDBOX-CHANGE reply with the token",
            "--output-format",
            "json",
            "--yolo",
            "--no-auto-update",
        ],
        quick,
    );
    assert_ne!(refused.status, 0, "{}", refused.combined());
    assert!(
        refused
            .combined()
            .contains("cannot resume this session under sandbox profile"),
        "{}",
        refused.combined()
    );
    assert_eq!(
        summary_profile(&isolated.grok_home, &sandbox_id),
        "workspace",
        "refused resume changed the stored profile"
    );
    oracle(
        "sandbox-resume",
        &format!("stored profile workspace; {}", refused.combined().trim()),
    );

    // Ubuntu CI compiles this test. The refusal text and the absent marker are
    // a macOS product gate, so that host is the only one that runs this oracle.
    #[cfg(target_os = "macos")]
    assert_macos_shell_refusal(&isolated, turn);

    assert_same_tree(
        "developer ~/.a3s",
        &before_a3s,
        &fingerprint_a3s(&developer_a3s),
    );
    let mut session_ids = local_session_ids(&isolated.grok_home);
    if !session_ids.iter().any(|id| id == &origin) {
        session_ids.push(origin);
    }
    for id in session_ids {
        assert!(
            !path_has_component(&developer_a3s, &id),
            "session {id} appeared under {}",
            developer_a3s.display()
        );
        assert!(
            !path_has_component(&developer_grok, &id),
            "session {id} appeared under {}",
            developer_grok.display()
        );
    }
    oracle("isolated-home", "developer homes unchanged");
}

/// Requires the shipped macOS shell refusal. Not compiled on Linux, so Ubuntu
/// CI does not spawn the marker command.
#[cfg(target_os = "macos")]
fn assert_macos_shell_refusal(isolated: &Isolated, turn: Duration) {
    let shell = run(
        isolated,
        &[
            "-p",
            "SHELL-ORACLE run a shell command",
            "--output-format",
            "streaming-json",
            "--yolo",
            "--permission-mode",
            "bypassPermissions",
            "--trust",
            "--no-auto-update",
            "--disable-web-search",
        ],
        turn,
    );
    let shell_text = shell.combined();
    assert!(
        shell_text.contains("native_process_lifetime_unqualified"),
        "shell tool was not refused:\n{shell_text}"
    );
    assert!(
        !isolated.marker.exists(),
        "shell command left a side effect at {}",
        isolated.marker.display()
    );
    oracle(
        "shell-refusal",
        &format!(
            "marker file was not created; {}",
            excerpt_around(&shell_text, "native_process_lifetime_unqualified", 120)
        ),
    );
}
