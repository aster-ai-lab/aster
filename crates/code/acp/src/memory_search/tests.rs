use super::*;
use aster_core::llm::{
    ContentBlock, LlmClient, LlmResponse, Message, StreamEvent, TokenUsage, ToolDefinition,
};
use std::{
    rc::Rc,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

#[derive(Default)]
struct Model {
    skip_retrieval: AtomicBool,
    calls: AtomicUsize,
    results: Mutex<Vec<String>>,
}
#[async_trait]
impl LlmClient for Model {
    async fn complete(
        &self,
        messages: &[Message],
        _: Option<&str>,
        tools: &[ToolDefinition],
    ) -> anyhow::Result<LlmResponse> {
        let content = if self.skip_retrieval.load(Ordering::SeqCst)
            || !tools.iter().any(|tool| tool.name == TOOL_NAME)
        {
            vec![ContentBlock::Text {
                text: "auxiliary".into(),
            }]
        } else if self.calls.fetch_add(1, Ordering::SeqCst) % 2 == 0 {
            vec![ContentBlock::ToolUse {
                id: format!("memory-{}", self.calls.load(Ordering::SeqCst)),
                name: TOOL_NAME.into(),
                input: json!({"query":"库存"}),
            }]
        } else {
            let text = messages
                .iter()
                .rev()
                .flat_map(|message| message.content.iter().rev())
                .find_map(|block| match block {
                    ContentBlock::ToolResult { content, .. } => Some(content.as_text()),
                    _ => None,
                })
                .ok_or_else(|| anyhow::anyhow!("tool result missing"))?;
            self.results.lock().unwrap().push(text);
            vec![ContentBlock::Text {
                text: "done".into(),
            }]
        };
        let stop = if matches!(content.first(), Some(ContentBlock::ToolUse { .. })) {
            "tool_use"
        } else {
            "end_turn"
        };
        Ok(LlmResponse {
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
        })
    }
    async fn complete_streaming(
        &self,
        _: &[Message],
        _: Option<&str>,
        _: &[ToolDefinition],
        _: tokio_util::sync::CancellationToken,
    ) -> anyhow::Result<tokio::sync::mpsc::Receiver<StreamEvent>> {
        anyhow::bail!("non-streaming fixture")
    }
}

fn response(session: &str) -> Value {
    json!({"sessionId":session,"query":"库存","scope":"all","phase":"ready","searched":1,"pending":0,"complete":true,"nextCursor":null,"liveLayersOnly":true,"hits":[{"taskId":"task-b","taskName":"Inventory task","workspaceId":"workspace-b","workspaceName":"Inventory project","sourceRoot":"/saved/workspace-b","id":"stock-rule","passage":"先核对库存原始记录。","layer":"durable","memoryType":"semantic","revision":"saved-revision","timestamp":"2026-10-27T00:00:00Z","importance":0.9,"truncated":false}]})
}

struct Host {
    calls: AtomicUsize,
    foreign: AtomicBool,
    revoke: AtomicBool,
    denies: Arc<Mutex<Vec<String>>>,
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
    async fn ext_method(
        &self,
        request: agent_client_protocol::ExtRequest,
    ) -> agent_client_protocol::Result<agent_client_protocol::ExtResponse> {
        assert_eq!(request.method.as_ref(), "aster/memory/search-workspaces");
        let params: Value = serde_json::from_str(request.params.get()).unwrap();
        assert_eq!(params["sessionId"], "origin-session");
        assert_eq!(params["query"], "库存");
        assert_eq!(params["scope"], "all");
        assert!(params["cursor"].is_null());
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.revoke.load(Ordering::SeqCst) {
            self.denies.lock().unwrap().push(TOOL_NAME.into());
        }
        let value = response(if self.foreign.load(Ordering::SeqCst) {
            "foreign-session"
        } else {
            "origin-session"
        });
        Ok(agent_client_protocol::ExtResponse::new(
            serde_json::value::to_raw_value(&value).unwrap().into(),
        ))
    }
}

#[tokio::test]
async fn search_memory_round_trip_uses_governed_harness_and_original_acp_transport() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let origin = tempfile::tempdir().unwrap();
            let denies = Arc::new(Mutex::new(Vec::new()));
            let host = Rc::new(Host {
                calls: AtomicUsize::new(0),
                foreign: AtomicBool::new(false),
                revoke: AtomicBool::new(false),
                denies: Arc::clone(&denies),
            });
            let bridge = crate::document_read::DocumentBridge::new();
            let (left, right) = tokio::io::duplex(1024 * 1024);
            let (ar, aw) = tokio::io::split(left);
            let (cr, cw) = tokio::io::split(right);
            let (connection, agent_io) = agent_client_protocol::AgentSideConnection::new(
                crate::AsterCodeAgent::new(crate::agent::test_launch(None)),
                aw.compat_write(),
                ar.compat(),
                |f| {
                    tokio::task::spawn_local(f);
                },
            );
            let (_client, client_io) = agent_client_protocol::ClientSideConnection::new(
                Rc::clone(&host),
                cw.compat_write(),
                cr.compat(),
                |f| {
                    tokio::task::spawn_local(f);
                },
            );
            bridge.connect(Rc::new(connection));
            let io = [
                tokio::task::spawn_local(async move {
                    let _ = agent_io.await;
                }),
                tokio::task::spawn_local(async move {
                    let _ = client_io.await;
                }),
            ];
            let tool = Arc::new(
                SearchMemoryTool::new(
                    Arc::clone(&bridge),
                    "origin-session",
                    origin.path(),
                    Arc::clone(&denies),
                )
                .unwrap(),
            );
            let model = Arc::new(Model::default());
            let agent = aster_core::Agent::from_config(
                crate::agent::test_launch(Some(origin.path().join("sessions"))).config,
            )
            .await
            .unwrap();
            let options = aster_core::SessionOptions::new()
                .with_session_id("origin-session")
                .with_llm_client(model.clone())
                .with_planning_mode(aster_core::PlanningMode::Disabled)
                .with_permission_policy(
                    aster_core::permissions::PermissionPolicy::new().allow(TOOL_NAME),
                )
                .with_harness(
                    aster_core::HarnessComposeOptions::compose(
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
            let session = agent
                .session_async(origin.path().to_string_lossy().into_owned(), Some(options))
                .await
                .unwrap();
            session.register_dynamic_tool(tool.clone()).unwrap();
            model.skip_retrieval.store(true, Ordering::SeqCst);
            session
                .send("Answer directly when retrieval is unnecessary", None)
                .await
                .unwrap();
            assert_eq!(host.calls.load(Ordering::SeqCst), 0);
            assert!(model.results.lock().unwrap().is_empty());
            model.skip_retrieval.store(false, Ordering::SeqCst);
            session
                .send("Find remembered inventory guidance", None)
                .await
                .unwrap();
            let text = model.results.lock().unwrap()[0].clone();
            let body: Value =
                serde_json::from_str(text.strip_prefix("Tool exit code: 0\n").unwrap()).unwrap();
            assert_eq!(body["hits"][0]["id"], "stock-rule");
            assert_eq!(body["hits"][0]["workspaceName"], "Inventory project");
            assert_eq!(body["hits"][0]["layer"], "durable");
            host.foreign.store(true, Ordering::SeqCst);
            session.send("Find again", None).await.unwrap();
            assert!(model.results.lock().unwrap()[1].contains("binding mismatch"));
            denies.lock().unwrap().push(TOOL_NAME.into());
            session.send("Find while denied", None).await.unwrap();
            assert!(model.results.lock().unwrap()[2].contains("denied"));
            assert_eq!(host.calls.load(Ordering::SeqCst), 2);
            denies.lock().unwrap().clear();
            denies.lock().unwrap().push("search_*".into());
            assert!(tool.denied(&json!({"query":"库存"})));
            denies.lock().unwrap().clear();
            host.foreign.store(false, Ordering::SeqCst);
            host.revoke.store(true, Ordering::SeqCst);
            session
                .send("Find while revoked in transit", None)
                .await
                .unwrap();
            assert!(model.results.lock().unwrap()[3].contains("denied"));
            assert!(!model.results.lock().unwrap()[3].contains("stock-rule"));
            assert_eq!(host.calls.load(Ordering::SeqCst), 3);
            session.close().await;
            denies.lock().unwrap().clear();
            std::fs::rename(origin.path(), origin.path().with_extension("moved")).unwrap();
            std::fs::create_dir(origin.path()).unwrap();
            assert!(tool
                .execute(
                    &json!({"query":"库存"}),
                    &ToolContext::new(origin.path().into())
                )
                .await
                .is_err());
            assert_eq!(host.calls.load(Ordering::SeqCst), 3);
            std::fs::remove_dir_all(origin.path().with_extension("moved")).unwrap();
            bridge.stop();
            for task in io {
                task.abort();
            }
        })
        .await;
}

#[test]
fn search_memory_refuses_malformed_attribution_and_over_budget_results() {
    validate_reply(&response("s"), "s", "库存", Scope::All).unwrap();
    for (key, bad) in [
        ("layer", json!("unknown")),
        ("memoryType", json!("unknown")),
        ("id", json!("")),
        ("importance", json!(2)),
        ("passage", json!("x".repeat(8193))),
    ] {
        let mut value = response("s");
        value["hits"][0][key] = bad;
        assert!(validate_reply(&value, "s", "库存", Scope::All).is_err());
    }
    let mut value = response("s");
    value["hits"] = json!(vec![value["hits"][0].clone(); 13]);
    assert!(validate_reply(&value, "s", "库存", Scope::All).is_err());
    assert!(validate_reply(&response("other"), "s", "库存", Scope::All).is_err());
    let mut incomplete = response("s");
    incomplete["pending"] = json!(1);
    assert!(validate_reply(&incomplete, "s", "库存", Scope::All).is_err());
}

#[test]
fn search_memory_host_admission_is_explicit_and_strict() {
    for (input, expected) in [
        (json!({}), false),
        (json!({"workspaceMemorySearch":true}), true),
        (json!({"workspaceMemorySearch":false}), false),
    ] {
        let meta = json!({"asterHost":input});
        assert_eq!(
            crate::host_context::studio_host_from_meta(meta.as_object())
                .unwrap()
                .workspace_memory_search,
            expected
        );
    }
    let meta = json!({"asterHost":{"workspaceMemorySearch":"true"}});
    assert!(crate::host_context::studio_host_from_meta(meta.as_object()).is_err());
}
