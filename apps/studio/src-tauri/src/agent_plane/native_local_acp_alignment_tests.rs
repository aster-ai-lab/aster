#![cfg(test)]

use super::*;
use futures_util::FutureExt;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

struct Provider {
    base: String,
    stop: Arc<AtomicBool>,
    streams: Arc<AtomicU64>,
    thread: Option<std::thread::JoinHandle<Result<(), String>>>,
}

impl Provider {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}/v1", listener.local_addr().unwrap());
        listener.set_nonblocking(true).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let streams = Arc::new(AtomicU64::new(0));
        let count = streams.clone();
        let thread = std::thread::spawn(move || {
            while !stopped.load(Ordering::SeqCst) {
                let mut socket = match listener.accept() {
                    Ok((socket, _)) => socket,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(5));
                        continue;
                    }
                    Err(error) => return Err(error.to_string()),
                };
                let (route, body) = request(&mut socket)?;
                eprintln!(
                    "STUDIO_FIXTURE_REQUEST route={route} stream={}",
                    body["stream"] == true
                );
                if route == "/v1/systemone" {
                    let response=json!({"model":"apofasi-lexical-fixture","answers":{"surface":{"type":"choice","choice":"prose"}}}).to_string();
                    write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}",response.len()).map_err(|error|error.to_string())?;
                    continue;
                }
                let streaming = body["stream"] == true;
                let (content_type, response) = if streaming {
                    let index = count.fetch_add(1, Ordering::SeqCst);
                    let delta = match index {
                        0 | 2 => call(
                            index,
                            "write",
                            json!({"file_path":"result.txt","content":"STUDIO_REAL_ACP_OK\n"}),
                        ),
                        3 => call(index, "read", json!({"file_path":"result.txt"})),
                        1 => json!({"content":"已提供计划，等待确认。"}),
                        4 => json!({"content":"STUDIO_REAL_ACP_OK"}),
                        _ => return Err(format!("unexpected main stream #{}", index + 1)),
                    };
                    let reason = if delta.get("tool_calls").is_some() {
                        "tool_calls"
                    } else {
                        "stop"
                    };
                    let chunk =
                        json!({"choices":[{"index":0,"delta":delta,"finish_reason":reason}]});
                    (
                        "text/event-stream",
                        format!("data: {chunk}\n\ndata: [DONE]\n\n"),
                    )
                } else {
                    let text = if body
                        .pointer("/response_format/json_schema/name")
                        .and_then(Value::as_str)
                        == Some("pre_analysis")
                    {
                        json!({"intent":"GeneralPurpose","requires_planning":false,
                            "goal":{"description":"Complete this synthetic fixture turn","success_criteria":[]},
                            "execution_plan":{"complexity":"Simple","steps":[{"id":"step-1","description":"Complete this synthetic fixture turn","dependencies":[],"success_criteria":"Return the fixture marker"}],"required_tools":[]},"optimized_input":""}).to_string()
                    } else if body["messages"][0]["content"].as_str().is_some_and(|text| {
                        text.starts_with(
                            "You extract durable, reusable memory for a coding agent.\n",
                        )
                    }) {
                        "{\"items\":[]}".into()
                    } else {
                        return Err("unexpected auxiliary provider request".into());
                    };
                    ("application/json", json!({"choices":[{"index":0,"message":{"role":"assistant","content":text},"finish_reason":"stop"}],"usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}}).to_string())
                };
                write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}",response.len()).map_err(|error|error.to_string())?;
            }
            Ok(())
        });
        Self {
            base,
            stop,
            streams,
            thread: Some(thread),
        }
    }
}

impl Drop for Provider {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let result = self.thread.take().unwrap().join();
        eprintln!("STUDIO_FIXTURE_SERVER {result:?}");
        if !std::thread::panicking() {
            assert!(matches!(result, Ok(Ok(()))), "provider fixture: {result:?}");
        }
    }
}

fn call(index: u64, name: &str, arguments: Value) -> Value {
    json!({"tool_calls":[{"index":0,"id":format!("synthetic-{index}-{name}"),"type":"function","function":{"name":name,"arguments":arguments.to_string()}}]})
}

fn request(socket: &mut TcpStream) -> Result<(String, Value), String> {
    socket
        .set_nonblocking(false)
        .map_err(|error| error.to_string())?;
    socket
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|error| error.to_string())?;
    socket
        .set_write_timeout(Some(Duration::from_secs(5)))
        .map_err(|error| error.to_string())?;
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let read = socket
            .read(&mut buffer)
            .map_err(|error| error.to_string())?;
        if read == 0 || bytes.len() + read > 2 * 1024 * 1024 {
            return Err("incomplete or oversized provider request".into());
        }
        bytes.extend_from_slice(&buffer[..read]);
        let Some(end) = bytes.windows(4).position(|value| value == b"\r\n\r\n") else {
            continue;
        };
        let header = String::from_utf8_lossy(&bytes[..end]);
        let route = header
            .lines()
            .next()
            .and_then(|line| line.strip_prefix("POST "))
            .and_then(|line| line.split_once(' ').map(|(route, _)| route))
            .ok_or("unexpected method")?;
        if !matches!(route, "/v1/chat/completions" | "/v1/systemone") {
            return Err(format!("unexpected provider route: {route}"));
        }
        let length = header
            .lines()
            .find_map(|line| {
                line.to_ascii_lowercase()
                    .strip_prefix("content-length:")
                    .and_then(|value| value.trim().parse::<usize>().ok())
            })
            .ok_or("missing content length")?;
        if bytes.len() >= end + 4 + length {
            return serde_json::from_slice(&bytes[end + 4..end + 4 + length])
                .map(|value| (route.to_owned(), value))
                .map_err(|error| error.to_string());
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires just build-studio workspace ACP companion; uses only a private loopback provider"]
async fn native_alignment_real_local_acp_planning_then_execution() {
    real_local_acp_flow(false).await;
}

#[cfg(feature = "tenant-aster")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires workspace ACP companion; synthetic anonymous BYOK loopback only"]
async fn default_aster_real_anonymous_byok_planning_then_execution() {
    real_local_acp_flow(true).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires workspace ACP identity; synthetic managed denial must never launch it"]
async fn default_aster_managed_product_refuses_the_same_local_model_turn() {
    let root = tempfile::tempdir().unwrap();
    let provider = Provider::start();
    let binary = PathBuf::from(
        std::env::var_os("A3S_STUDIO_ALIGNMENT_TEST_ACP").expect("exact workspace ACP path"),
    );
    assert!(binary.is_file());
    let store = Arc::new(PlaneStore::open(Some(root.path().join("state"))));
    let agents = Arc::new(AgentCatalog::new(store.clone()));
    let sessions = Arc::new(SessionCatalog::new(store.clone()));
    let hub = Arc::new(EventHub::new());
    let managed = crate::providers::local_tenant_tests::managed_fixture(&provider.base);
    let config = Arc::new(PlaneConfig {
        root: Some(root.path().join("state")),
        credentials: Arc::new(move |id| managed.execution_credentials(id, None)),
        binary: Some(binary),
        workspace_documents: None,
        schedule_mcp: None,
        mail_mcp: None,
        slug: String::new(),
        personal_wiki: None,
        cloud_agent_runtime: None,
    });
    let agent = agents.create(&json!({"type":"a3s-code","name":"Synthetic managed rejection","model":"openai::gpt-test"})).unwrap();
    let session = sessions
        .create(&json!({"agentId":agent.id,"workspace":{"type":"system"}}))
        .unwrap();
    assert!(PathBuf::from(&session.workspace.path).starts_with(root.path()));
    let turns = Arc::new(TurnService::new(
        store.clone(),
        agents,
        sessions,
        config,
        hub.clone(),
    ));
    let mut events = hub.subscribe();
    turns.open(&session.id,&json!({"userMessageParts":[{"type":"text","text":"Reply with the synthetic marker."}]})).unwrap();
    let seen = super::tests::until_done(&mut events, |_| {}).await;
    assert_eq!(
        seen.last().unwrap()["payload"]["status"],
        "error",
        "{seen:?}"
    );
    assert_eq!(provider.streams.load(Ordering::SeqCst), 0);
    assert!(turns.runtimes.lock().await.is_empty());
    assert!(!root.path().join("state/runtime").exists());
    eprintln!(
        "STUDIO_MANAGED_LOCAL_DENIAL {}",
        json!({"compiled_tenant":crate::tenant::HostIdentity::compiled().as_str(),"provider_requests":0,"runtime_spawned":false,"terminal":"error"})
    );
    turns.close_test_runtimes().await;
}

async fn real_local_acp_flow(local_byok: bool) {
    let root = tempfile::tempdir().unwrap();
    let provider = Provider::start();
    let binary = std::env::var_os("A3S_STUDIO_ALIGNMENT_TEST_ACP")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../target/debug/a3s-code-acp")
        })
        .canonicalize()
        .expect("build workspace ACP first");
    let store = Arc::new(PlaneStore::open(Some(root.path().join("state"))));
    let agents = Arc::new(AgentCatalog::new(store.clone()));
    let sessions = Arc::new(SessionCatalog::new(store.clone()));
    let hub = Arc::new(EventHub::new());
    let base = provider.base.clone();
    let credentials: super::super::CredentialLookup = if local_byok {
        #[cfg(feature = "tenant-aster")]
        {
            let local = crate::providers::local_tenant_tests::local_fixture(&base);
            assert!(local.execution_credentials("a3s", None).is_none());
            Arc::new(move |id| local.execution_credentials(id, None))
        }
        #[cfg(not(feature = "tenant-aster"))]
        {
            panic!("local BYOK flow cannot exist in this build");
        }
    } else {
        Arc::new(move |provider| {
            (provider == "a3s").then(|| crate::providers::ProviderCredentials {
                base_url: base.clone(),
                api_key: "synthetic-invalid-member-token".into(),
            })
        })
    };
    let config = Arc::new(PlaneConfig {
        root: Some(root.path().join("state")),
        credentials,
        binary: Some(binary.clone()),
        workspace_documents: None,
        schedule_mcp: None,
        mail_mcp: None,
        slug: String::new(),
        personal_wiki: None,
        cloud_agent_runtime: None,
    });
    let model = if local_byok {
        "openai::gpt-test"
    } else {
        "a3s::gpt-test"
    };
    let agent=agents.create(&json!({"type":"a3s-code","name":"Synthetic local ACP","model":model,"configuration":{"permission_mode":"auto"}})).unwrap();
    let session = sessions
        .create(&json!({"agentId":agent.id,"workspace":{"type":"system"}}))
        .unwrap();
    let turns = Arc::new(TurnService::new(
        store.clone(),
        agents,
        sessions,
        config,
        hub.clone(),
    ));
    let result=std::panic::AssertUnwindSafe(async {
        let path=PathBuf::from(&session.workspace.path).join("result.txt");
        let mut events=hub.subscribe();
        turns.open(&session.id,&json!({"userMessageParts":[{"type":"text","text":"Create result.txt only after reviewing this plan."}],"turnContext":{"planFirst":true}})).unwrap();
        let planned=super::tests::until_done(&mut events, |event|{eprintln!("STUDIO_FIXTURE_EVENT phase=plan channel={} status={}",event["channel"],event["payload"]["status"]);}).await;
        assert_eq!(planned.last().unwrap()["payload"]["status"],"success", "{planned:?}");
        assert!(!path.exists(),"per-turn planning gate prevents the model's write");
        assert_eq!(provider.streams.load(Ordering::SeqCst),2);
        let goal=turns.goal_snapshot(&session.id).await.unwrap();
        assert_eq!(goal["status"],"known","{goal}");
        turns.open(&session.id,&json!({"userMessageParts":[{"type":"text","text":"Execute the approved synthetic task: write result.txt and read it back."}]})).unwrap();
        let mut approvals=0;
        let mut observed=false;
        let executed=super::tests::until_done(&mut events, |event|{
            eprintln!("STUDIO_FIXTURE_EVENT phase=execute channel={} status={}",event["channel"],event["payload"]["status"]);
            observed |= event.pointer("/payload/status/nativeExecution/runId").and_then(Value::as_str).is_some();
            if let Some(pending)=event.pointer("/payload/status/pendingApprovals").and_then(Value::as_array) {
                for pending in pending {
                    let binding:ApprovalBinding=serde_json::from_value(pending.clone()).unwrap();
                    let mut wrong=binding.clone();wrong.execution_id="foreign".into();
                    assert!(!turns.respond_bound_approval(&session.id,&wrong,true));
                    assert!(turns.respond_bound_approval(&session.id,&binding,true));
                    assert!(!turns.respond_bound_approval(&session.id,&binding,true));
                    approvals+=1;
                }
            }
        }).await;
        assert!(approvals>0,"the actual session posture is respected through a live bound approval");
        assert_eq!(executed.last().unwrap()["payload"]["status"],"success", "{executed:?}");
        assert_eq!(std::fs::read_to_string(&path).unwrap(),"STUDIO_REAL_ACP_OK\n");
        assert!(store.messages(&session.id).last().unwrap().searchable_text.contains("STUDIO_REAL_ACP_OK"));
        assert_eq!(provider.streams.load(Ordering::SeqCst),5);
        let runtime=turns.runtimes.lock().await.get(&session.id).unwrap().clone();
        let methods=runtime.client.raw_notification_methods();
        eprintln!("STUDIO_ACTUAL_RAW_NOTIFICATION_METHODS {}",json!(methods));
        assert!(methods.iter().any(|method|method=="__a3s/execution/observe"));
        assert!(methods.iter().any(|method|method=="_x.ai/turn/outcome"));
        assert!(observed,"actual Core Run observations must reach native status");
        let raw=runtime.client.request("_a3s/goal/state",json!({"session_id":runtime.acp_session})).await.unwrap();
        let workspace=PathBuf::from(&session.workspace.path).canonicalize().unwrap().to_string_lossy().into_owned();
        let mut definition=runtime.client.request("_a3s/goal/definition",json!({"kind":"create","operation_id":"fixture-create","goal_id":"fixture-goal","session_id":runtime.acp_session,
            "workspace":{"id":workspace,"display_root":workspace},"goal":{"description":"合成目标","success_criteria":["人工检查"],"progress":0.0,"achieved":false,"created_at":0,"achieved_at":null},
            "non_goals":[],"scope_fact_sequences":[],"work_policy_digest":raw["work_policy_digest"],"expected_goal_revision":null,"expected_plan_revision":null,"expected_goal_sequence":null})).await.unwrap();
        let active=turns.goal_snapshot(&session.id).await.unwrap();
        assert_eq!(active["goal"]["state"],"active","{active}");
        assert_eq!(active["goal"]["tick"],"needs_input","unaccepted plan cannot advance");
        for (kind,state,tick) in [("pause","paused",json!("paused")),("cancel","cancelled",Value::Null)] {
            definition=runtime.client.request("_a3s/goal/control",json!({"kind":kind,"operation_id":format!("fixture-{kind}"),"goal_id":definition["binding"]["goal_id"],
                "session_id":runtime.acp_session,"workspace":definition["binding"]["workspace"],"work_policy_digest":definition["binding"]["work_policy_digest"],
                "expected_goal_revision":definition["binding"]["goal_revision"],"expected_plan_revision":definition["plan_revision"],"expected_goal_sequence":definition["goal_sequence"]})).await.unwrap();
            let snapshot=turns.goal_snapshot(&session.id).await.unwrap();
            assert_eq!(snapshot["goal"]["state"],state,"{snapshot}");
            assert_eq!(snapshot["goal"]["tick"],tick);
        }
        eprintln!("STUDIO_REAL_ACP_RESULT {}",json!({"binary":binary,"anonymous_byok":local_byok,"planning_mutation":false,"execution_marker":true,"main_streams":5,"bound_approvals":approvals,"native_goal_states":["active","paused","cancelled"]}));
    }).catch_unwind().await;
    turns.close_test_runtimes().await;
    if let Err(error) = result {
        std::panic::resume_unwind(error);
    }
}
