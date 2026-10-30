#![cfg(test)]

use super::*;
use a3s_code_core::llm::{
    ContentBlock, LlmClient, LlmResponse, Message, StreamEvent, TokenUsage, ToolDefinition,
};
use a3s_code_core::{Agent, AgentSession, HarnessComposeOptions, SessionOptions};
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::Semaphore;
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};
use tokio_util::sync::CancellationToken;

#[test]
fn acp_errors_use_structured_kinds_and_never_parse_error_text() {
    for (wire, expected) in [
        ("not_found", WorkspaceReadOverlayErrorKind::NotFound),
        ("unsupported", WorkspaceReadOverlayErrorKind::Unsupported),
        ("conflict", WorkspaceReadOverlayErrorKind::Conflict),
        ("timeout", WorkspaceReadOverlayErrorKind::Timeout),
        ("foreign", WorkspaceReadOverlayErrorKind::Foreign),
        ("transport", WorkspaceReadOverlayErrorKind::Transport),
    ] {
        let error = agent_client_protocol::Error::invalid_params()
            .data(json!({"a3sDocument":{"kind":wire}}));
        assert_eq!(map_acp_read_error(error).kind(), expected);
    }
    let misleading = agent_client_protocol::Error::internal_error().data("not_found");
    assert_eq!(
        map_acp_read_error(misleading).kind(),
        WorkspaceReadOverlayErrorKind::Transport
    );
    assert_eq!(
        map_acp_read_error(agent_client_protocol::Error::resource_not_found(None)).kind(),
        WorkspaceReadOverlayErrorKind::NotFound
    );
    assert_eq!(
        map_acp_read_error(agent_client_protocol::Error::method_not_found()).kind(),
        WorkspaceReadOverlayErrorKind::Unsupported
    );
}

#[test]
fn document_read_contract_distinguishes_text_paths_from_binary_documents() {
    let root = tempfile::tempdir().unwrap();
    let tool = DocumentBridge::new().tool(root.path(), "contract").unwrap();
    let description = tool.description();
    assert!(description.contains("UTF-8"));
    assert!(description.contains("Office"));
    assert!(description.contains("PDF"));
    assert!(description.contains("office_read"));
    assert!(description.contains("absolute file path"));
    let schema = tool.parameters();
    let path = schema["properties"]["file_path"]["description"]
        .as_str()
        .unwrap();
    assert!(path.contains("absolute"));
    assert!(path.contains("relative to the current workspace"));
    assert!(path.contains("not a document ID"));
}

struct Model {
    calls: AtomicUsize,
    result: Mutex<Option<String>>,
}
impl Model {
    fn response(content: Vec<ContentBlock>, stop: &str) -> LlmResponse {
        LlmResponse {
            message: Message {
                content,
                ..Message::assistant("")
            },
            usage: TokenUsage {
                prompt_tokens: 1,
                completion_tokens: 1,
                total_tokens: 2,
                cache_read_tokens: None,
                cache_write_tokens: None,
            },
            stop_reason: Some(stop.into()),
            token_logprobs: vec![],
            meta: None,
        }
    }
}
#[async_trait]
impl LlmClient for Model {
    async fn complete(
        &self,
        messages: &[Message],
        _: Option<&str>,
        tools: &[ToolDefinition],
    ) -> Result<LlmResponse> {
        if !tools.iter().any(|tool| tool.name == "studio_document_read") {
            return Ok(Self::response(
                vec![ContentBlock::Text {
                    text: "auxiliary".into(),
                }],
                "end_turn",
            ));
        }
        if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
            Ok(Self::response(
                vec![ContentBlock::ToolUse {
                    id: "read-1".into(),
                    name: "studio_document_read".into(),
                    input: json!({"file_path":"note.txt"}),
                }],
                "tool_use",
            ))
        } else {
            let content = messages
                .iter()
                .rev()
                .flat_map(|message| message.content.iter().rev())
                .find_map(|block| match block {
                    ContentBlock::ToolResult { content, .. } => Some(content.as_text()),
                    _ => None,
                })
                .context("read result missing from actual Harness inference")?;
            *self.result.lock().unwrap() = Some(content);
            Ok(Self::response(
                vec![ContentBlock::Text {
                    text: "done".into(),
                }],
                "end_turn",
            ))
        }
    }
    async fn complete_streaming(
        &self,
        _: &[Message],
        _: Option<&str>,
        _: &[ToolDefinition],
        _: CancellationToken,
    ) -> Result<mpsc::Receiver<StreamEvent>> {
        anyhow::bail!("non-streaming contract")
    }
}
struct Host {
    value: Value,
    seen: Semaphore,
    release: Option<Arc<Semaphore>>,
    probe: Option<Mutex<Probe>>,
}
#[async_trait(?Send)]
impl agent_client_protocol::Client for Host {
    async fn request_permission(
        &self,
        _: agent_client_protocol::RequestPermissionRequest,
    ) -> agent_client_protocol::Result<agent_client_protocol::RequestPermissionResponse> {
        Err(agent_client_protocol::Error::internal_error())
    }
    async fn session_notification(
        &self,
        _: agent_client_protocol::SessionNotification,
    ) -> agent_client_protocol::Result<()> {
        Ok(())
    }
    async fn read_text_file(
        &self,
        args: agent_client_protocol::ReadTextFileRequest,
    ) -> agent_client_protocol::Result<agent_client_protocol::ReadTextFileResponse> {
        let params = serde_json::to_value(&args).unwrap();
        assert_eq!(params["sessionId"], "document-session");
        self.seen.add_permits(1);
        if let Some(release) = &self.release {
            release.acquire().await.unwrap().forget();
        }
        if let Some(probe) = &self.probe {
            let value = probe
                .lock()
                .unwrap()
                .call(json!({"harness":{"operation":"acp_read","params":params}}));
            return Ok(acp_response(value));
        }
        let mut observation = self.value.clone();
        let content = observation["snapshot"]["text"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        observation["snapshot"]
            .as_object_mut()
            .unwrap()
            .remove("text");
        Ok(
            agent_client_protocol::ReadTextFileResponse::new(content).meta(
                serde_json::Map::from_iter([("a3sDocument".to_owned(), observation)]),
            ),
        )
    }
}
fn acp_response(mut value: Value) -> agent_client_protocol::ReadTextFileResponse {
    let content = value["content"].as_str().unwrap_or_default().to_owned();
    let meta = value
        .get_mut("_meta")
        .and_then(Value::as_object_mut)
        .cloned()
        .unwrap_or_default();
    agent_client_protocol::ReadTextFileResponse::new(content).meta(meta)
}

fn observation(root: &Path) -> Value {
    json!({"path":root.join("note.txt"),"released":true,"snapshot":{"documentId":"native-model","uri":url::Url::from_file_path(root.join("note.txt")).unwrap().to_string(),"version":2,"text":"unsaved text","dirty":true,"diskConflict":false}})
}
fn connect(bridge: &DocumentBridge, host: Rc<Host>) -> Vec<tokio::task::JoinHandle<()>> {
    let (left, right) = tokio::io::duplex(1024 * 1024);
    let (ar, aw) = tokio::io::split(left);
    let (cr, cw) = tokio::io::split(right);
    let agent = crate::A3sCodeAgent::new(crate::agent::test_launch(None));
    let (connection, agent_io) = agent_client_protocol::AgentSideConnection::new(
        agent,
        aw.compat_write(),
        ar.compat(),
        |future| {
            tokio::task::spawn_local(future);
        },
    );
    let (_client, client_io) = agent_client_protocol::ClientSideConnection::new(
        host,
        cw.compat_write(),
        cr.compat(),
        |future| {
            tokio::task::spawn_local(future);
        },
    );
    bridge.connect(Rc::new(connection));
    vec![
        tokio::task::spawn_local(async move {
            let _ = agent_io.await;
        }),
        tokio::task::spawn_local(async move {
            let _ = client_io.await;
        }),
    ]
}
async fn session(
    root: &Path,
    bridge: &Arc<DocumentBridge>,
    read: bool,
) -> (Arc<AgentSession>, Arc<Model>) {
    let model = Arc::new(Model {
        calls: AtomicUsize::new(0),
        result: Mutex::new(None),
    });
    let agent = Agent::from_config(crate::agent::test_launch(Some(root.join("sessions"))).config)
        .await
        .unwrap();
    let base = a3s_code_core::workspace::WorkspaceServices::local(root);
    let services = a3s_code_core::workspace::WorkspaceServicesBuilder::new(
        base.workspace_ref().clone(),
        base.fs(),
    )
    .capabilities(a3s_code_core::workspace::WorkspaceCapabilities {
        read,
        write: false,
        exec: false,
        search: false,
        git: false,
        code_intelligence: false,
    })
    .build();
    let options = SessionOptions::new()
        .with_session_id("document-session")
        .with_llm_client(model.clone())
        .with_workspace_backend(services)
        .with_planning_mode(a3s_code_core::PlanningMode::Disabled)
        .with_permission_policy(
            a3s_code_core::permissions::PermissionPolicy::new().allow("studio_document_read"),
        )
        .with_harness(
            HarnessComposeOptions::compose(
                ["system", "tools", "budget", "compact", "infer"]
                    .into_iter()
                    .map(str::to_owned)
                    .collect(),
                None,
                None,
                vec![],
            )
            .unwrap(),
        );
    let session = Arc::new(
        agent
            .session_async(root.to_string_lossy().into_owned(), Some(options))
            .await
            .unwrap(),
    );
    session
        .register_dynamic_tool(bridge.tool(root, "document-session").unwrap())
        .unwrap();
    (session, model)
}
fn successful_tool_json(text: &str) -> Value {
    let payload = text
        .strip_prefix("Tool exit code: 0\n")
        .expect("successful tool result must expose its zero exit status");
    serde_json::from_str(payload).expect("tool payload must remain JSON")
}

async fn run(host: Rc<Host>, root: &Path, read: bool) -> String {
    let bridge = DocumentBridge::new();
    let io = connect(&bridge, host);
    let (session, model) = session(root, &bridge, read).await;
    session
        .send("Read the current editor document", None)
        .await
        .unwrap();
    let result = model.result.lock().unwrap().clone().unwrap();
    session.close().await;
    bridge.stop();
    for task in io {
        task.abort();
    }
    result
}
#[tokio::test]
async fn actual_harness_reads_through_bidirectional_acp_and_refuses_invalid_observations() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let workspace = tempfile::tempdir().unwrap();
            let root = std::fs::canonicalize(workspace.path()).unwrap();
            std::fs::write(root.join("note.txt"), "disk text").unwrap();
            let value = observation(&root);
            let host = Rc::new(Host {
                value: value.clone(),
                seen: Semaphore::new(0),
                release: None,
                probe: None,
            });
            let text = run(host, &root, true).await;
            assert_eq!(
                crate::test_support::successful_tool_json(&text)["text"],
                "unsaved text"
            );
            assert_eq!(
                std::fs::read_to_string(root.join("note.txt")).unwrap(),
                "disk text"
            );
            for invalid in [
                json!({"released":false}),
                json!({"path":"/foreign/note.txt"}),
                json!({"snapshot":{"version":0}}),
                json!({"snapshot":{"diskConflict":true}}),
                json!({"snapshot":{"uri":"file:///foreign/note.txt"}}),
                json!({"snapshot":{"text":"x".repeat(TEXT_LIMIT + 1)}}),
            ] {
                let mut response = value.clone();
                for (key, replacement) in invalid.as_object().unwrap() {
                    if key == "snapshot" {
                        for (field, v) in replacement.as_object().unwrap() {
                            response[key][field] = v.clone();
                        }
                    } else {
                        response[key] = replacement.clone();
                    }
                }
                let host = Rc::new(Host {
                    value: response,
                    seen: Semaphore::new(0),
                    release: None,
                    probe: None,
                });
                let text = run(host, &root, true).await;
                crate::test_support::assert_failed_tool_text(&text);
            }
        })
        .await;
}
#[tokio::test]
async fn actual_run_without_read_authority_never_calls_document_host() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let workspace = tempfile::tempdir().unwrap();
            let root = std::fs::canonicalize(workspace.path()).unwrap();
            let host = Rc::new(Host {
                value: observation(&root),
                seen: Semaphore::new(0),
                release: None,
                probe: None,
            });
            let text = run(host.clone(), &root, false).await;
            assert!(text.contains("Workspace operation not found"), "{text}");
            assert_eq!(host.seen.available_permits(), 0);
        })
        .await;
}
#[tokio::test]
async fn cancelled_actual_turn_cannot_accept_late_document_reply() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let workspace = tempfile::tempdir().unwrap();
            let root = std::fs::canonicalize(workspace.path()).unwrap();
            let release = Arc::new(Semaphore::new(0));
            let host = Rc::new(Host {
                value: observation(&root),
                seen: Semaphore::new(0),
                release: Some(release.clone()),
                probe: None,
            });
            let bridge = DocumentBridge::new();
            let io = connect(&bridge, host.clone());
            let (session, model) = session(&root, &bridge, true).await;
            let run = {
                let session = session.clone();
                tokio::spawn(async move { session.send("read", None).await })
            };
            tokio::time::timeout(Duration::from_secs(3), host.seen.acquire())
                .await
                .unwrap()
                .unwrap()
                .forget();
            assert!(session.cancel().await);
            release.add_permits(1);
            let _ = tokio::time::timeout(Duration::from_secs(3), run)
                .await
                .unwrap()
                .unwrap();
            assert!(
                model.result.lock().unwrap().is_none(),
                "cancelled reply reached a new model inference"
            );
            session.close().await;
            bridge.stop();
            for task in io {
                task.abort();
            }
        })
        .await;
}
#[tokio::test]
async fn document_read_timeout_fails_without_accepting_late_host_text() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let workspace = tempfile::tempdir().unwrap();
            let root = std::fs::canonicalize(workspace.path()).unwrap();
            std::fs::write(root.join("note.txt"), "disk").unwrap();
            let host = Rc::new(Host {
                value: observation(&root),
                seen: Semaphore::new(0),
                release: Some(Arc::new(Semaphore::new(0))),
                probe: None,
            });
            let text = tokio::time::timeout(Duration::from_secs(8), run(host, &root, true))
                .await
                .unwrap();
            assert!(text.contains("timeout"), "{text}");
            assert!(!text.contains("unsaved text"), "{text}");
        })
        .await;
}

#[tokio::test]
async fn plain_tool_context_cannot_use_bridge() {
    let workspace = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(workspace.path()).unwrap();
    let bridge = DocumentBridge::new();
    let tool = bridge.tool(&root, "document-session").unwrap();
    let result = tool
        .execute(&json!({"file_path":"note.txt"}), &ToolContext::new(root))
        .await
        .unwrap();
    assert!(result.content.contains("Workspace operation not found"));
}

struct ObserveTool {
    inner: Arc<dyn Tool>,
    output: Arc<Mutex<Option<String>>>,
}

#[async_trait]
impl Tool for ObserveTool {
    fn name(&self) -> &str {
        self.inner.name()
    }
    fn description(&self) -> &str {
        self.inner.description()
    }
    fn parameters(&self) -> Value {
        self.inner.parameters()
    }
    fn capabilities(&self, args: &Value) -> ToolCapabilities {
        self.inner.capabilities(args)
    }
    async fn execute(&self, args: &Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let output = self.inner.execute(args, ctx).await?;
        *self.output.lock().unwrap() = Some(output.content.clone());
        Ok(output)
    }
}

#[tokio::test]
async fn workspace_replaced_during_read_cannot_accept_old_host_observation() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let workspace = tempfile::tempdir().unwrap();
            let root = std::fs::canonicalize(workspace.path()).unwrap();
            std::fs::write(root.join("note.txt"), "original disk").unwrap();
            let release = Arc::new(Semaphore::new(0));
            let host = Rc::new(Host {
                value: observation(&root),
                seen: Semaphore::new(0),
                release: Some(release.clone()),
                probe: None,
            });
            let bridge = DocumentBridge::new();
            let io = connect(&bridge, host.clone());
            let (session, model) = session(&root, &bridge, true).await;
            let observed = Arc::new(Mutex::new(None));
            session
                .register_dynamic_tool(Arc::new(ObserveTool {
                    inner: bridge.tool(&root, "document-session").unwrap(),
                    output: observed.clone(),
                }))
                .unwrap();
            let run = {
                let session = session.clone();
                tokio::spawn(async move { session.send("read", None).await })
            };
            tokio::time::timeout(Duration::from_secs(3), host.seen.acquire())
                .await
                .unwrap()
                .unwrap()
                .forget();
            let retired = root.with_extension("retired");
            let moved = std::fs::rename(&root, &retired);
            #[cfg(windows)]
            if let Err(error) = &moved {
                // The live Windows fact-log handles can refuse the rename
                // before a replacement workspace or stale reply can exist.
                assert!(matches!(error.raw_os_error(), Some(5 | 32)), "{error}");
                assert!(!retired.exists());
                assert_eq!(std::fs::read_to_string(root.join("note.txt")).unwrap(), "original disk");
                release.add_permits(1);
                let result = tokio::time::timeout(Duration::from_secs(3), run).await.unwrap().unwrap();
                assert!(result.is_ok(), "the unchanged workspace must remain usable: {result:?}");
                let text = model.result.lock().unwrap().clone().unwrap();
                assert_eq!(crate::test_support::successful_tool_json(&text)["text"], "unsaved text");
                assert_eq!(std::fs::read_to_string(root.join("note.txt")).unwrap(), "original disk");
                session.close().await;
                bridge.stop();
                for task in io { task.abort(); }
                return;
            }
            moved.unwrap();
            std::fs::create_dir(&root).unwrap();
            std::fs::write(root.join("note.txt"), "replacement disk").unwrap();
            release.add_permits(1);
            let result = tokio::time::timeout(Duration::from_secs(3), run)
                .await
                .unwrap()
                .unwrap();
            assert!(
                result.is_err(),
                "a replaced fact-log directory must refuse completion: result={result:?}, output={:?}, model={:?}",
                observed.lock().unwrap(), model.result.lock().unwrap()
            );
            let output = observed.lock().unwrap().clone().unwrap();
            assert!(output.contains("workspace is not admitted"), "{output}");
            assert!(!output.contains("unsaved text"));
            assert!(model.result.lock().unwrap().is_none());
            assert_eq!(
                std::fs::read_to_string(root.join("note.txt")).unwrap(),
                "replacement disk"
            );
            std::fs::remove_dir_all(&root).unwrap();
            std::fs::rename(retired, &root).unwrap();
            session.close().await;
            bridge.stop();
            for task in io {
                task.abort();
            }
        })
        .await;
}

#[tokio::test]
async fn stopping_transport_releases_waiting_tool_without_accepting_host_text() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let workspace = tempfile::tempdir().unwrap();
            let root = std::fs::canonicalize(workspace.path()).unwrap();
            std::fs::write(root.join("note.txt"), "disk").unwrap();
            let release = Arc::new(Semaphore::new(0));
            let host = Rc::new(Host {
                value: observation(&root),
                seen: Semaphore::new(0),
                release: Some(release.clone()),
                probe: None,
            });
            let bridge = DocumentBridge::new();
            let io = connect(&bridge, host.clone());
            let (session, model) = session(&root, &bridge, true).await;
            let run = {
                let session = session.clone();
                tokio::spawn(async move { session.send("read", None).await })
            };
            tokio::time::timeout(Duration::from_secs(3), host.seen.acquire())
                .await
                .unwrap()
                .unwrap()
                .forget();
            bridge.stop();
            tokio::time::timeout(Duration::from_secs(3), run)
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            let result = model.result.lock().unwrap().clone().unwrap();
            assert!(result.contains("document host unavailable"), "{result}");
            assert!(!result.contains("unsaved text"));
            release.add_permits(1);
            session.close().await;
            for task in io {
                task.abort();
            }
        })
        .await;
}
struct Probe {
    child: std::process::Child,
    input: std::process::ChildStdin,
    output: std::io::BufReader<std::process::ChildStdout>,
}
impl Probe {
    fn new() -> Self {
        let path = std::env::var_os("A3S_STUDIO_DOCUMENT_PROBE").expect("build the native workspace_document_stdio test executable and set A3S_STUDIO_DOCUMENT_PROBE");
        let mut child = std::process::Command::new(path)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .unwrap();
        let input = child.stdin.take().unwrap();
        let output = std::io::BufReader::new(child.stdout.take().unwrap());
        Self {
            child,
            input,
            output,
        }
    }
    fn call(&mut self, mut value: Value) -> Value {
        use std::io::{BufRead, Write};
        value["id"] = json!(1);
        serde_json::to_writer(&mut self.input, &value).unwrap();
        self.input.write_all(b"\n").unwrap();
        self.input.flush().unwrap();
        let mut line = String::new();
        self.output.read_line(&mut line).unwrap();
        let response: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(response["status"], 200, "{response}");
        response["value"].clone()
    }
}
impl Drop for Probe {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
#[tokio::test]
#[ignore = "requires the separately built Studio native document probe"]
async fn actual_harness_acp_reads_native_unsaved_document_across_process_boundary() {
    tokio::task::LocalSet::new().run_until(async {
        let workspace = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(workspace.path()).unwrap();
        std::fs::write(root.join("note.txt"), "disk text").unwrap();
        let mut probe = Probe::new();
        let opened = probe.call(json!({"request":{"operation":"open","root":root,"path":root.join("note.txt")}}));
        probe.call(json!({"request":{"operation":"edit","lease":opened["lease"],"sequence":1,"version":opened["snapshot"]["version"],"text":"native unsaved editor text"}}));
        let publisher = probe.call(json!({"request":{"operation":"delegate_diagnostics","lease":opened["lease"]}}));
        let publication = probe.call(json!({"bearer":publisher["lease"],"request":{"operation":"publish_diagnostics","lease":publisher["lease"],"sequence":1,"version":2,"items":[{"start":{"line":0,"character":0},"end":{"line":0,"character":6},"severity":"warning","message":"native unsaved diagnostic"}]}}));
        probe.call(json!({"harness":{"operation":"bind","root":root,"session":"document-session"}}));
        let host = Rc::new(Host { value:Value::Null, seen:Semaphore::new(0), release:None, probe:Some(Mutex::new(probe)) });
        let text = run(host, &root, true).await;
        let value = crate::test_support::successful_tool_json(&text);
        assert_eq!(value["text"], "native unsaved editor text");
        assert_eq!(value["documentId"], opened["snapshot"]["documentId"]);
        assert_eq!(value["version"], 2);
        assert_eq!(value["dirty"], true);
        assert_eq!(value["diagnostics"][0]["publisherId"], publication["publisherId"]);
        assert_eq!(value["diagnostics"][0]["items"][0]["message"], "native unsaved diagnostic");
        assert_eq!(value["diagnostics"][0]["version"], value["version"]);
        assert_eq!(value["diagnosticsComplete"], false);
        assert!(!text.contains(publisher["lease"].as_str().unwrap()));
        assert_eq!(std::fs::read_to_string(root.join("note.txt")).unwrap(), "disk text");
    }).await;
}

#[tokio::test]
async fn actual_harness_accepts_bound_advisory_diagnostics_and_refuses_forged_publications() {
    tokio::task::LocalSet::new().run_until(async {
        let workspace = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(workspace.path()).unwrap();
        std::fs::write(root.join("note.txt"), "disk text").unwrap();
        let mut response = observation(&root);
        let version = response["snapshot"]["version"].clone();
        let publication = json!({"publisherId":uuid::Uuid::new_v4().to_string(),"sequence":1,"version":version,"items":[{"start":{"line":0,"character":0},"end":{"line":0,"character":7},"severity":"error","message":"bounded finding"}]});
        response["diagnostics"] = json!([publication]);
        let host = |value| Rc::new(Host { value, seen:Semaphore::new(0), release:None, probe:None });
        let accepted = run(host(response.clone()), &root, true).await;
        let accepted = crate::test_support::successful_tool_json(&accepted);
        assert_eq!(accepted["diagnostics"][0]["items"][0]["message"], "bounded finding");
        assert_eq!(accepted["diagnosticsComplete"], false);
        let mut invalid = Vec::new();
        for (field, value) in [("version", json!(0)), ("publisherId", json!("invented")), ("sequence", json!(0)), ("sequence", json!(9_007_199_254_740_992u64)), ("authority", json!("forged"))] {
            let mut next = response.clone(); next["diagnostics"][0][field] = value; invalid.push(next);
        }
        for (field, value) in [("end", json!({"line":0,"character":200})), ("start", json!({"line":0,"character":8})), ("severity", json!("fatal")), ("message", json!("x".repeat(2049)))] {
            let mut next = response.clone(); next["diagnostics"][0]["items"][0][field] = value; invalid.push(next);
        }
        let mut duplicate = response.clone(); duplicate["diagnostics"] = json!([publication.clone(), publication.clone()]); invalid.push(duplicate);
        let mut many = response.clone(); many["diagnostics"] = json!(vec![publication; 17]); invalid.push(many);
        let mut split = response.clone(); split["snapshot"]["text"] = json!("🙂x"); split["diagnostics"][0]["items"][0]["end"]["character"] = json!(1); invalid.push(split);
        for value in invalid {
            let refused = run(host(value), &root, true).await;
            crate::test_support::assert_failed_tool_text(&refused);
            assert!(!refused.contains("bounded finding"), "{refused}");
        }
    }).await;
}
