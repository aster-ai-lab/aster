//! Interactive permission modes through the real ACP process, without unattended approval.
use super::*;
use serde_json::json;

type PendingCall = Arc<Mutex<Option<(String, serde_json::Value)>>>;

fn provider() -> (SocketAddr, PendingCall) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let next = Arc::new(Mutex::new(None::<(String, serde_json::Value)>));
    let pending = Arc::clone(&next);
    std::thread::spawn(move || {
        let mut sequence = 0;
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            stream
                .set_read_timeout(Some(Duration::from_secs(10)))
                .unwrap();
            let Some(raw) = read_http_body(&mut stream) else {
                continue;
            };
            let request: serde_json::Value = serde_json::from_str(&raw).unwrap();
            let streaming = request["stream"].as_bool().unwrap_or(false);
            let body = if let Some(reply) = provider_failures::planning_reply(&request) {
                reply
            } else if request["tools"].as_array().is_none_or(Vec::is_empty) {
                reply_text("No further context.", streaming)
            } else if let Some((name, args)) = pending.lock().unwrap().take() {
                sequence += 1;
                reply_tool(&name, args, sequence, streaming)
            } else {
                reply_text("The requested operation has settled.", streaming)
            };
            write_http(&mut stream, &body).unwrap();
        }
    });
    (address, next)
}

#[tokio::test(flavor = "current_thread")]
async fn interactive_permission_modes_apply_to_each_prompt_and_preserve_denies() {
    tokio::task::LocalSet::new().run_until(async {
        let workspace = isolate("permission modes 工作区");
        let state = isolate("permission-modes-state");
        let home = isolate("permission-modes-home");
        let tmp = isolate("permission-modes-tmp");
        std::fs::write(workspace.join("source.txt"), "original\n").unwrap();
        let (address, next) = provider();
        let config = write_acl(&state, &format!("http://{address}/v1"));
        let agent = spawn_agent(&workspace, &config, &home, &tmp, &[]).await;
        acp::Agent::initialize(agent.conn.as_ref(), acp::InitializeRequest::new(acp::ProtocolVersion::V1)
            .meta(json!({"permissionRules":{"deny":["Write(protected.txt)"]}}).as_object().cloned())).await.unwrap();
        let session = acp::Agent::new_session(agent.conn.as_ref(), acp::NewSessionRequest::new(&workspace)).await.unwrap().session_id;
        for (turn, mode) in ["default", "auto", "yolo", "plan", "default", "yolo"].into_iter().enumerate() {
            for operation in ["read", "write", "host", "denied"] {
                let file = format!("{turn}-{mode}-{operation}.txt");
                let marker = format!("permission-mode-marker-{turn}");
                let (tool, args) = match operation {
                    "read" => ("read", json!({"file_path":"source.txt"})),
                    "write" => ("write", json!({"file_path":file,"content":"changed\n"})),
                    "host" => ("bash", json!({"command":format!("printf '{marker}'"), "sandbox_permissions":"require_escalated", "justification":"Read-only interactive permission regression"})),
                    _ => ("write", json!({"file_path":"protected.txt","content":"forbidden"})),
                };
                *next.lock().unwrap() = Some((tool.into(), args));
                agent.approve_permissions.set(true);
                let before = agent.permission_asks.get();
                let request = acp::PromptRequest::new(session.clone(), vec![acp::ContentBlock::Text(acp::TextContent::new(format!("Perform the {operation} operation once.")))])
                    .meta(json!({"permissionPosture":mode}).as_object().cloned());
                let response = tokio::time::timeout(Duration::from_secs(15), acp::Agent::prompt(agent.conn.as_ref(), request)).await.expect("interactive mode must settle");
                let asks = agent.permission_asks.get() - before;
                let expected = u32::from(matches!((mode, operation), ("default", "write" | "host") | ("auto", "host")));
                assert_eq!(asks, expected, "{mode}/{operation}: {response:?}");
                if operation == "write" {
                    assert_eq!(workspace.join(&file).exists(), mode != "plan", "{mode}/{operation}: {response:?}");
                    if mode != "plan" {
                        assert_eq!(std::fs::read(workspace.join(&file)).unwrap(), b"changed\n");
                    }
                }
                if operation == "host" {
                    let facts = private_fixture_facts(&workspace);
                    let executed = facts.iter().any(|fact| fact["type"] == "tool.result"
                        && fact["payload"]["receipt"]["outcome"] == "succeeded"
                        && fact["payload"]["receipt"]["exit_code"] == 0
                        && fact["payload"]["receipt"]["output"].as_str().is_some_and(|output| output.contains(&marker)));
                    assert_eq!(executed, mode != "plan", "{mode}/{operation}: actual command receipt");
                }
                assert!(!workspace.join("protected.txt").exists());
                eprintln!("PERMISSION_ORACLE mode={mode} operation={operation} asks={asks} response={response:?}");
            }
        }
        assert_eq!(std::fs::read(workspace.join("source.txt")).unwrap(), b"original\n");
    }).await;
}

#[tokio::test(flavor = "current_thread")]
async fn rejecting_a_command_does_not_execute_or_learn_an_approval() {
    tokio::task::LocalSet::new().run_until(async {
        let workspace = isolate("permission-rejection");
        let state = isolate("permission-rejection-state");
        let home = isolate("permission-rejection-home");
        let tmp = isolate("permission-rejection-tmp");
        let (address, next) = provider();
        let config = write_acl(&state, &format!("http://{address}/v1"));
        let agent = spawn_agent(&workspace, &config, &home, &tmp, &[]).await;
        let session = open(&agent, &workspace).await;
        for mode in ["default", "auto", "default"] {
            let before = agent.permission_asks.get();
            *next.lock().unwrap() = Some(("bash".into(), json!({
                "command":"printf forbidden > rejected.txt", "sandbox_permissions":"require_escalated",
                "justification":"The regression client rejects this command"
            })));
            let request = acp::PromptRequest::new(session.clone(), vec![acp::ContentBlock::Text(acp::TextContent::new("Run the requested command once."))])
                .meta(json!({"permissionPosture":mode}).as_object().cloned());
            let response = tokio::time::timeout(Duration::from_secs(15), acp::Agent::prompt(agent.conn.as_ref(), request)).await.expect("rejection must settle");
            assert_eq!(agent.permission_asks.get()-before, 1, "{mode}: {response:?}");
            assert!(!workspace.join("rejected.txt").exists());
            eprintln!("PERMISSION_REJECTION mode={mode} asks=1 executed=false");
        }
    }).await;
}
