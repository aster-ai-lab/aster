use super::*;
use a3s_code_core::llm::{
    ContentBlock, LlmClient, LlmResponse, Message, StreamEvent, TokenUsage, ToolDefinition,
};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
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
        let has_tool = tools.iter().any(|tool| tool.name == TOOL_NAME)
            && !self.skip_retrieval.load(Ordering::SeqCst);
        let turn = if has_tool {
            self.calls.fetch_add(1, Ordering::SeqCst)
        } else {
            0
        };
        let content = if !has_tool {
            vec![ContentBlock::Text {
                text: "auxiliary".into(),
            }]
        } else if turn % 2 == 0 {
            vec![ContentBlock::ToolUse {
                id: format!("wiki-{turn}"),
                name: TOOL_NAME.into(),
                input: json!({"query":"workspaceanswer42marker"}),
            }]
        } else {
            let result = messages
                .iter()
                .rev()
                .flat_map(|message| message.content.iter().rev())
                .find_map(|block| match block {
                    ContentBlock::ToolResult { content, .. } => Some(content.as_text()),
                    _ => None,
                })
                .ok_or_else(|| anyhow::anyhow!("Harness tool result missing"))?;
            self.results.lock().unwrap().push(result);
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
        anyhow::bail!("non-streaming contract")
    }
}

struct Host {
    wiki: std::path::PathBuf,
    calls: AtomicUsize,
    foreign_session: AtomicBool,
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
        assert_eq!(request.method.as_ref(), "a3s/wiki/search-workspaces");
        let params: Value = serde_json::from_str(request.params.get()).unwrap();
        assert_eq!(params["sessionId"], "origin-session");
        assert_eq!(params["scope"], "all");
        assert!(params["cursor"].is_null());
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.revoke.load(Ordering::SeqCst) {
            self.denies.lock().unwrap().push(TOOL_NAME.into());
        }
        let indexed = search_admitted_body(&self.wiki, params["query"].as_str().unwrap()).await;
        assert_eq!(indexed["phase"], "published");
        let hits: Vec<Value> = indexed["hits"]
            .as_array()
            .unwrap()
            .iter()
            .map(|hit| {
                let mut hit = hit.clone();
                for (key, value) in [
                    ("taskId", "task-b"),
                    ("taskName", "Task B"),
                    ("workspaceId", "workspace-b"),
                    ("workspaceName", "Project B"),
                ] {
                    hit[key] = json!(value);
                }
                hit["sourceRoot"] = json!(self
                    .wiki
                    .parent()
                    .unwrap()
                    .parent()
                    .unwrap()
                    .parent()
                    .unwrap());
                hit["outputRoot"] = json!(self.wiki);
                hit
            })
            .collect();
        let value = json!({
            "sessionId":if self.foreign_session.load(Ordering::SeqCst) {"foreign-session"} else {"origin-session"},
            "scope":"all", "query":params["query"], "phase":"published",
            "searched":1,"pending":0,"complete":true,"nextCursor":null,"hits":hits
        });
        Ok(agent_client_protocol::ExtResponse::new(
            serde_json::value::to_raw_value(&value).unwrap().into(),
        ))
    }
}

#[tokio::test]
async fn search_wiki_cross_workspace_round_trip_uses_the_original_acp_transport_and_index() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let origin = tempfile::tempdir().unwrap();
            let (_foreign, wiki) = super::tests::fixture();
            let denies = Arc::new(Mutex::new(Vec::new()));
            let host = Rc::new(Host {
                wiki,
                calls: AtomicUsize::new(0),
                foreign_session: AtomicBool::new(false),
                revoke: AtomicBool::new(false),
                denies: Arc::clone(&denies),
            });
            let bridge = crate::document_read::DocumentBridge::new();
            let (left, right) = tokio::io::duplex(1024 * 1024);
            let (ar, aw) = tokio::io::split(left);
            let (cr, cw) = tokio::io::split(right);
            let (connection, agent_io) = agent_client_protocol::AgentSideConnection::new(
                crate::A3sCodeAgent::new(crate::agent::test_launch(None)),
                aw.compat_write(),
                ar.compat(),
                |future| {
                    tokio::task::spawn_local(future);
                },
            );
            let (_client, client_io) = agent_client_protocol::ClientSideConnection::new(
                Rc::clone(&host),
                cw.compat_write(),
                cr.compat(),
                |future| {
                    tokio::task::spawn_local(future);
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
                SearchWikiTool::new(None, Arc::clone(&denies))
                    .with_host(Arc::clone(&bridge), "origin-session", origin.path())
                    .unwrap(),
            );
            let model = Arc::new(Model::default());
            let agent = a3s_code_core::Agent::from_config(
                crate::agent::test_launch(Some(origin.path().join("sessions"))).config,
            )
            .await
            .unwrap();
            let options = a3s_code_core::SessionOptions::new()
                .with_session_id("origin-session")
                .with_llm_client(model.clone())
                .with_planning_mode(a3s_code_core::PlanningMode::Disabled)
                .with_permission_policy(
                    a3s_code_core::permissions::PermissionPolicy::new().allow(TOOL_NAME),
                )
                .with_harness(
                    a3s_code_core::HarnessComposeOptions::compose(
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
            let ctx = ToolContext::new(origin.path().into());
            let args = json!({"query":"workspaceanswer42marker"});
            session.send("Find the saved evidence", None).await.unwrap();
            let content = model.results.lock().unwrap()[0].clone();
            let body: Value =
                serde_json::from_str(content.strip_prefix("Tool exit code: 0\n").unwrap()).unwrap();
            assert_eq!(body["hits"].as_array().unwrap().len(), 1);
            assert_eq!(body["hits"][0]["workspaceName"], "Project B");
            assert_eq!(body["hits"][0]["path"], "concepts/overview.md");
            host.foreign_session.store(true, Ordering::SeqCst);
            session.send("Find again", None).await.unwrap();
            let second = model.results.lock().unwrap()[1].clone();
            assert!(
                second.contains("binding mismatch"),
                "result={second}, calls={}",
                host.calls.load(Ordering::SeqCst)
            );
            assert_eq!(host.calls.load(Ordering::SeqCst), 2);
            denies.lock().unwrap().push("search_wiki".into());
            session.send("Find while denied", None).await.unwrap();
            assert!(model.results.lock().unwrap()[2].contains("denied"));
            assert_eq!(host.calls.load(Ordering::SeqCst), 2);
            denies.lock().unwrap().clear();
            host.foreign_session.store(false, Ordering::SeqCst);
            host.revoke.store(true, Ordering::SeqCst);
            session
                .send("Find while permission is revoked in transit", None)
                .await
                .unwrap();
            assert!(model.results.lock().unwrap()[3].contains("denied"));
            assert!(!model.results.lock().unwrap()[3].contains("workspaceanswer42marker"));
            assert_eq!(host.calls.load(Ordering::SeqCst), 3);
            session.close().await;
            denies.lock().unwrap().clear();
            std::fs::rename(origin.path(), origin.path().with_extension("moved")).unwrap();
            std::fs::create_dir(origin.path()).unwrap();
            assert!(tool.execute(&args, &ctx).await.is_err());
            assert_eq!(host.calls.load(Ordering::SeqCst), 3);
            std::fs::remove_dir_all(origin.path().with_extension("moved")).unwrap();
            bridge.stop();
            for task in io {
                task.abort();
            }
        })
        .await;
}
