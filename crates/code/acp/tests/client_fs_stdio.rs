//! Client file-system reads through the shipped `aster-acp` binary
//! (ACP-CAPS1).
//!
//! A client that declares `fs.readTextFile` serves the model's `read` tool
//! from its unsaved buffer. A client that does not declare it gets no
//! `fs/read_text_file` request, and the model sees the saved file.

use std::cell::RefCell;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use agent_client_protocol as acp;
use async_trait::async_trait;
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

const MODEL_ID: &str = "mock/gpt";
const SAVED: &str = "saved on disk\n";
const UNSAVED: &str = "UNSAVED BUFFER LINE\n";

/// Tool results the model received, in order.
type Seen = Arc<Mutex<Vec<String>>>;

fn spawn_mock(seen: Seen, rich: bool) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock");
    let addr = listener.local_addr().expect("mock addr");
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let seen = Arc::clone(&seen);
            std::thread::spawn(move || {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(20)));
                if let Some(body) = read_http_body(&mut stream) {
                    let _ = write_http(&mut stream, &decide(&body, &seen, rich));
                }
            });
        }
    });
    addr
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
    };
    let headers = String::from_utf8_lossy(&buf[..header_end]).to_ascii_lowercase();
    let length = headers
        .lines()
        .find_map(|line| line.strip_prefix("content-length:"))
        .and_then(|value| value.trim().parse::<usize>().ok())
        .unwrap_or(0);
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

fn reply(delta: serde_json::Value, finish: &str, streaming: bool) -> String {
    if !streaming {
        let mut message = delta;
        message["role"] = "assistant".into();
        return serde_json::json!({
            "id": "cmpl",
            "object": "chat.completion",
            "choices": [{ "index": 0, "message": message, "finish_reason": finish }],
            "usage": { "prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2 }
        })
        .to_string();
    }
    let chunk = serde_json::json!({ "choices": [{ "delta": delta }] });
    let done = serde_json::json!({ "choices": [{ "delta": {}, "finish_reason": finish }] });
    format!("data: {chunk}\n\ndata: {done}\n\ndata: [DONE]\n\n")
}

fn message_text(message: &serde_json::Value) -> String {
    match &message["content"] {
        serde_json::Value::String(text) => text.clone(),
        serde_json::Value::Array(parts) => parts
            .iter()
            .filter_map(|part| part["text"].as_str())
            .collect::<Vec<_>>()
            .join(""),
        _ => String::new(),
    }
}

/// Reads `notes.txt` once, records the tool result, then answers with text.
fn decide(raw: &str, seen: &Seen, rich: bool) -> String {
    let parsed: serde_json::Value = serde_json::from_str(raw).unwrap_or(serde_json::Value::Null);
    let streaming = parsed["stream"].as_bool().unwrap_or(false);
    let has_read = parsed["tools"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|tool| tool["function"]["name"] == "read");
    let messages = parsed["messages"].as_array().cloned().unwrap_or_default();
    if let Some(last) = messages.last().filter(|m| m["role"] == "tool") {
        seen.lock().expect("seen").push(message_text(last));
        let tool_results = messages.iter().filter(|m| m["role"] == "tool").count();
        if rich && tool_results == 1 {
            let args = serde_json::json!({ "file_path": "notes.txt" }).to_string();
            let call = serde_json::json!({
                "content": null,
                "tool_calls": [{
                    "index": 0,
                    "id": "call_document_read",
                    "type": "function",
                    "function": { "name": "studio_document_read", "arguments": args }
                }]
            });
            return reply(call, "tool_calls", streaming);
        }
        return reply(serde_json::json!({ "content": "done" }), "stop", streaming);
    }
    if !has_read {
        return reply(serde_json::json!({ "content": "done" }), "stop", streaming);
    }
    let args = serde_json::json!({ "file_path": "notes.txt" }).to_string();
    let call = serde_json::json!({
        "content": null,
        "tool_calls": [{
            "index": 0,
            "id": "call_read",
            "type": "function",
            "function": { "name": "read", "arguments": args }
        }]
    });
    reply(call, "tool_calls", streaming)
}

#[derive(Clone, Copy)]
enum ReadMode {
    Buffer,
    NotFound,
    Unsupported,
    Conflict,
    Timeout,
    Foreign,
    Transport,
}

struct TestClient {
    reads: Rc<RefCell<Vec<PathBuf>>>,
    mode: ReadMode,
}

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

    async fn read_text_file(
        &self,
        args: acp::ReadTextFileRequest,
    ) -> Result<acp::ReadTextFileResponse, acp::Error> {
        self.reads.borrow_mut().push(args.path.clone());
        if !args.path.ends_with("notes.txt") || matches!(self.mode, ReadMode::NotFound) {
            return Err(acp::Error::resource_not_found(None));
        }
        if matches!(self.mode, ReadMode::Unsupported) {
            return Err(acp::Error::method_not_found());
        }
        if matches!(self.mode, ReadMode::Transport) {
            return Err(acp::Error::internal_error().data("host transport failed"));
        }
        if matches!(self.mode, ReadMode::Timeout) {
            tokio::time::sleep(Duration::from_secs(6)).await;
        }
        let mut observation = serde_json::json!({
            "path": args.path,
            "snapshot": {
                "documentId": "native-model",
                "uri": url::Url::from_file_path(&args.path).unwrap().to_string(),
                "version": 2,
                "dirty": true,
                "diskConflict": matches!(self.mode, ReadMode::Conflict)
            },
            "released": true
        });
        if matches!(self.mode, ReadMode::Foreign) {
            observation["path"] = serde_json::json!("/foreign/notes.txt");
        }
        Ok(
            acp::ReadTextFileResponse::new(UNSAVED).meta(serde_json::Map::from_iter([(
                "asterDocument".to_owned(),
                observation,
            )])),
        )
    }

    async fn session_notification(
        &self,
        _args: acp::SessionNotification,
    ) -> Result<(), acp::Error> {
        Ok(())
    }
}

struct Fixture {
    _root: tempfile::TempDir,
    workspace: PathBuf,
    home: PathBuf,
    state: PathBuf,
    tmp: PathBuf,
    config: PathBuf,
}

fn isolate(root: &Path, label: &str) -> PathBuf {
    let path = root.join(label);
    std::fs::create_dir_all(&path).expect("temp dir");
    path
}

impl Fixture {
    fn new(addr: SocketAddr) -> Self {
        let root = tempfile::tempdir().expect("root");
        let workspace = isolate(root.path(), "repo");
        std::fs::write(workspace.join("notes.txt"), SAVED).expect("notes");
        let outside = isolate(root.path(), "outside");
        let config = outside.join("config.acl");
        std::fs::write(
            &config,
            format!(
                r#"
providers "mock" {{
  base_url = "http://{addr}/v1"
  api_key = "test"
  models "gpt" {{
  }}
}}
default_model = "{MODEL_ID}"
"#
            ),
        )
        .expect("acl");
        Self {
            home: isolate(root.path(), "home"),
            state: isolate(root.path(), "state"),
            tmp: isolate(root.path(), "tmp"),
            _root: root,
            workspace,
            config,
        }
    }
}

/// Runs one `read notes.txt` turn and returns the client's
/// `fs/read_text_file` paths.
async fn read_turn(
    fixture: &Fixture,
    client_reads: bool,
    studio_documents: bool,
    rich: bool,
    mode: ReadMode,
) -> Vec<PathBuf> {
    let bin = PathBuf::from(env!("CARGO_BIN_EXE_aster-acp"));
    let mut child = tokio::process::Command::new(bin)
        .arg("--cwd")
        .arg(&fixture.workspace)
        .env("ASTER_CONFIG", &fixture.config)
        .env("ASTER_STATE_HOME", &fixture.state)
        .env("ASTER_DISABLE_CC_SWITCH", "1")
        .env("ASTER_DISABLE_GROK_ACCOUNT", "1")
        .env("ASTER_DISABLE_CURSOR_ACCOUNT", "1")
        .env("XAI_API_KEY", "sk-dummy-not-a-model")
        .env("HOME", &fixture.home)
        .env("GROK_HOME", fixture.home.join("grok"))
        .env("TMPDIR", &fixture.tmp)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn aster-acp");
    let stdout = child.stdout.take().expect("stdout").compat();
    let stdin = child.stdin.take().expect("stdin").compat_write();
    let reads = Rc::new(RefCell::new(Vec::new()));
    let client = TestClient {
        reads: Rc::clone(&reads),
        mode,
    };
    let (conn, io) = acp::ClientSideConnection::new(client, stdin, stdout, |fut| {
        tokio::task::spawn_local(fut);
    });
    tokio::task::spawn_local(async move {
        let _ = io.await;
    });
    let capabilities = acp::ClientCapabilities::new()
        .fs(acp::FileSystemCapabilities::new().read_text_file(client_reads));
    acp::Agent::initialize(
        &conn,
        acp::InitializeRequest::new(acp::ProtocolVersion::V1).client_capabilities(capabilities),
    )
    .await
    .expect("initialize");
    let mut new_session = acp::NewSessionRequest::new(fixture.workspace.clone());
    if studio_documents {
        new_session = new_session.meta(serde_json::Map::from_iter([(
            "asterStudioDocuments".to_owned(),
            serde_json::Value::Bool(true),
        )]));
    }
    let session = acp::Agent::new_session(&conn, new_session)
        .await
        .expect("new session")
        .session_id;
    let request = acp::PromptRequest::new(
        session,
        vec![acp::ContentBlock::Text(acp::TextContent::new(if rich {
            "read both notes.txt"
        } else {
            "read notes.txt"
        }))],
    );
    tokio::time::timeout(Duration::from_secs(120), acp::Agent::prompt(&conn, request))
        .await
        .expect("prompt timed out")
        .expect("prompt");
    let _ = child.start_kill();
    let paths = reads.borrow().clone();
    paths
}

#[tokio::test(flavor = "current_thread")]
async fn declared_client_reads_serve_the_unsaved_buffer() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let seen: Seen = Arc::default();
            let fixture = Fixture::new(spawn_mock(Arc::clone(&seen), false));
            let reads = read_turn(&fixture, true, true, false, ReadMode::Buffer).await;

            let expected = fixture.workspace.join("notes.txt");
            assert!(
                reads.iter().any(|path| path == &expected),
                "no fs/read_text_file for {expected:?}: {reads:?}"
            );
            let seen = seen.lock().expect("seen").clone();
            assert_eq!(seen.len(), 1, "{seen:?}");
            assert!(seen[0].contains("UNSAVED BUFFER LINE"), "{seen:?}");
            assert!(!seen[0].contains("saved on disk"), "{seen:?}");
            assert_eq!(
                std::fs::read_to_string(&expected).expect("notes"),
                SAVED,
                "a read changed the file"
            );
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn plain_and_rich_document_reads_share_one_unsaved_observation() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let seen: Seen = Arc::default();
            let fixture = Fixture::new(spawn_mock(Arc::clone(&seen), true));
            let reads = read_turn(&fixture, true, true, true, ReadMode::Buffer).await;
            let expected = std::fs::canonicalize(fixture.workspace.join("notes.txt")).unwrap();
            let seen = seen.lock().expect("seen").clone();
            let matched = reads
                .iter()
                .filter(|path| path.ends_with("notes.txt"))
                .collect::<Vec<_>>();
            assert_eq!(matched.len(), 2, "reads={reads:?}, seen={seen:?}");
            assert!(
                matched
                    .iter()
                    .all(|path| std::fs::canonicalize(path).unwrap() == expected),
                "reads={reads:?}"
            );
            assert_eq!(seen.len(), 2, "{seen:?}");
            assert!(seen[0].contains("UNSAVED BUFFER LINE"), "{seen:?}");
            assert!(seen[1].contains("UNSAVED BUFFER LINE"), "{seen:?}");
            assert!(seen[1].contains("\"version\":2"), "{seen:?}");
            assert!(seen[1].contains("\"dirty\":true"), "{seen:?}");
            assert!(seen.iter().all(|text| !text.contains(SAVED)), "{seen:?}");
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn declared_client_reads_without_document_bridge_stay_on_the_workspace() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let seen: Seen = Arc::default();
            let fixture = Fixture::new(spawn_mock(Arc::clone(&seen), false));
            let reads = read_turn(&fixture, true, false, false, ReadMode::Buffer).await;
            assert!(reads.is_empty(), "plain session enabled overlay: {reads:?}");
            let seen = seen.lock().expect("seen").clone();
            assert_eq!(seen.len(), 1, "{seen:?}");
            assert!(seen[0].contains("saved on disk"), "{seen:?}");
        })
        .await;
}

async fn exercise_read_mode(mode: ReadMode) -> (Seen, Fixture) {
    let seen: Seen = Arc::default();
    let fixture = Fixture::new(spawn_mock(Arc::clone(&seen), false));
    tokio::task::LocalSet::new()
        .run_until(read_turn(&fixture, true, true, false, mode))
        .await;
    (seen, fixture)
}

#[tokio::test(flavor = "current_thread")]
async fn failed_client_reads_never_fall_back_to_saved_disk_text() {
    for (mode, kind) in [
        (ReadMode::Conflict, "conflict"),
        (ReadMode::Timeout, "timeout"),
        (ReadMode::Foreign, "foreign"),
        (ReadMode::Transport, "transport"),
    ] {
        let (seen, fixture) = exercise_read_mode(mode).await;
        let seen = seen.lock().expect("seen").clone();
        assert_eq!(seen.len(), 1, "{kind}: {seen:?}");
        assert!(seen[0].contains(kind), "{kind}: {seen:?}");
        assert!(!seen[0].contains(SAVED), "{kind}: {seen:?}");
        assert!(!seen[0].contains(UNSAVED), "{kind}: {seen:?}");
        assert_eq!(
            std::fs::read_to_string(fixture.workspace.join("notes.txt")).unwrap(),
            SAVED
        );
    }
}

#[tokio::test(flavor = "current_thread")]
async fn absent_or_unsupported_client_reads_fall_back_to_saved_disk_text() {
    for mode in [ReadMode::NotFound, ReadMode::Unsupported] {
        let (seen, fixture) = exercise_read_mode(mode).await;
        let seen = seen.lock().expect("seen").clone();
        assert_eq!(seen.len(), 1, "{seen:?}");
        assert!(seen[0].contains(SAVED), "{seen:?}");
        assert!(!seen[0].contains(UNSAVED), "{seen:?}");
        assert_eq!(
            std::fs::read_to_string(fixture.workspace.join("notes.txt")).unwrap(),
            SAVED
        );
    }
}

#[tokio::test(flavor = "current_thread")]
async fn undeclared_client_reads_stay_on_the_workspace() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let seen: Seen = Arc::default();
            let fixture = Fixture::new(spawn_mock(Arc::clone(&seen), false));
            let reads = read_turn(&fixture, false, true, false, ReadMode::Buffer).await;

            assert!(
                reads.is_empty(),
                "undeclared capability was used: {reads:?}"
            );
            let seen = seen.lock().expect("seen").clone();
            assert_eq!(seen.len(), 1, "{seen:?}");
            assert!(seen[0].contains("saved on disk"), "{seen:?}");
        })
        .await;
}
