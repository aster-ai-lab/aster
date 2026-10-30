//! The actual ACP process carries local gateway identity through a Responses tool round.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::net::TcpListener;
use tokio::process::{Child, ChildStdin, ChildStdout};

const ANSWER: &str = "本地路由已确认。";
const FILE_CONTENT: &str = "work-order: local-responses-stdio\n";
const SOURCE_SESSION: &str = "studio-session-fixture";
const MODEL: &str = "openai-responses/model-a";
const GATEWAY_PATH: &str = "/api/local-models/internal/providers/codex/responses";

#[tokio::test]
async fn local_responses_model_completes_a_real_acp_tool_round_with_gateway_identity() {
    let root = tempfile::tempdir().unwrap();
    let workspace = root.path().join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    let file = workspace.join("work-order.txt");
    std::fs::write(&file, FILE_CONTENT).unwrap();
    let gateway = Gateway::start().await;
    let config = root.path().join("config.acl");
    std::fs::write(
        &config,
        format!(
            r#"
default_model = "{MODEL}"
import_accounts = []
sessions_dir = {sessions}
project_doc_max_bytes = 0
providers "openai-responses" {{
  api_key = "synthetic-proxy-token"
  base_url = "http://{}/api/local-models/internal/providers/codex"
  models "model-a" {{
    headers = {{
      "authorization" = "Bearer synthetic-proxy-token"
      "x-aster-source" = "aster-chat"
      "x-aster-session" = "{SOURCE_SESSION}"
      "chatgpt-account-id" = "account-fixture"
    }}
  }}
}}
"#,
            gateway.address,
            sessions = json!(root.path().join("sessions")),
        ),
    )
    .unwrap();
    let mut agent = Agent::launch(root.path(), &workspace, &config, file.clone());
    let initialized = agent
        .rpc(json!({"jsonrpc":"2.0","id":1,"method":"initialize",
            "params":{"protocolVersion":1,"clientCapabilities":{"fs":{"readTextFile":true}}}}))
        .await;
    assert!(initialized.get("result").is_some(), "{initialized}");
    let opened = agent
        .rpc(json!({"jsonrpc":"2.0","id":2,"method":"session/new",
            "params":{"cwd":workspace,"mcpServers":[],"_meta":{"a3sStudioDocuments":true}}}))
        .await;
    assert_eq!(opened["result"]["models"]["currentModelId"], MODEL);
    assert!(opened["result"]["models"]["availableModels"]
        .as_array()
        .unwrap()
        .iter()
        .any(|model| model["modelId"] == MODEL));
    let session = opened["result"]["sessionId"].as_str().unwrap();
    let result = agent
        .rpc(json!({"jsonrpc":"2.0","id":3,"method":"session/prompt",
            "params":{"sessionId":session,"prompt":[{"type":"text",
                "text":"Read work-order.txt and confirm the local route."}]}}))
        .await;
    assert_eq!(result["result"]["stopReason"], "end_turn", "{result}");
    let usage = &result["result"]["_meta"]["usage"];
    assert_eq!(usage["modelCalls"], 2, "{result}");
    assert_eq!(usage["inputTokens"], 20);
    assert_eq!(usage["outputTokens"], 12);
    assert_eq!(usage["totalTokens"], 32);
    assert_eq!(usage["usageIsIncomplete"], true);
    assert_eq!(usage["costIsPartial"], true);
    assert_eq!(agent.reads, [std::fs::canonicalize(file).unwrap()]);
    let text: String = agent
        .seen
        .iter()
        .filter(|event| event["params"]["update"]["sessionUpdate"] == "agent_message_chunk")
        .filter_map(|event| event["params"]["update"]["content"]["text"].as_str())
        .collect();
    assert_eq!(text, ANSWER, "{:?}", agent.seen);
    agent.child.kill().await.unwrap();
    agent.child.wait().await.unwrap();

    let requests = gateway.requests.lock().unwrap();
    let main: Vec<_> = requests
        .iter()
        .filter(|request| request.has_read())
        .collect();
    assert_eq!(main.len(), 2, "{requests:?}");
    for request in requests.iter() {
        assert_eq!(request.path, GATEWAY_PATH);
        assert_eq!(
            request.headers["authorization"],
            "Bearer synthetic-proxy-token"
        );
        assert_eq!(request.headers["x-aster-source"], "aster-chat");
        assert_eq!(request.headers["x-aster-session"], SOURCE_SESSION);
        assert_eq!(request.headers["chatgpt-account-id"], "account-fixture");
        assert_eq!(request.body["model"], "model-a");
        assert_eq!(request.body["stream"], true);
        assert!(request.body["input"].is_array());
        assert!(request.body.get("messages").is_none());
    }
    let call = main[1].body["input"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["type"] == "function_call")
        .unwrap();
    let output = main[1].body["input"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["type"] == "function_call_output")
        .unwrap();
    assert!(!call["call_id"].as_str().unwrap().is_empty());
    assert_eq!(output["call_id"], call["call_id"]);
    assert!(output["output"]
        .as_str()
        .unwrap()
        .contains(FILE_CONTENT.trim()));
    assert_eq!(
        std::fs::read_to_string(workspace.join("work-order.txt")).unwrap(),
        FILE_CONTENT
    );
}

struct Agent {
    child: Child,
    input: ChildStdin,
    output: Lines<BufReader<ChildStdout>>,
    file: PathBuf,
    reads: Vec<PathBuf>,
    seen: Vec<Value>,
}

impl Agent {
    fn launch(root: &Path, workspace: &Path, config: &Path, file: PathBuf) -> Self {
        let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_a3s-code-acp"))
            .current_dir(workspace)
            .arg("--cwd")
            .arg(workspace)
            .arg("--config")
            .arg(config)
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("A3S_HOME", root.join("product"))
            .env("A3S_STATE_HOME", root.join("state"))
            .env("A3S_CODE_LOG_DIR", root.join("logs"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        Self {
            input: child.stdin.take().unwrap(),
            output: BufReader::new(child.stdout.take().unwrap()).lines(),
            child,
            file: std::fs::canonicalize(file).unwrap(),
            reads: Vec::new(),
            seen: Vec::new(),
        }
    }

    async fn send(&mut self, message: Value) {
        self.input
            .write_all(format!("{message}\n").as_bytes())
            .await
            .unwrap();
        self.input.flush().await.unwrap();
    }

    async fn rpc(&mut self, request: Value) -> Value {
        let id = request["id"].clone();
        self.send(request).await;
        tokio::time::timeout(Duration::from_secs(30), async {
            loop {
                let line = self
                    .output
                    .next_line()
                    .await
                    .unwrap()
                    .expect("ACP stdout ended");
                let value: Value = serde_json::from_str(&line).unwrap();
                self.seen.push(value.clone());
                if value.get("method").is_none() && value["id"] == id {
                    return value;
                }
                if value.get("id").is_none() || value.get("method").is_none() {
                    continue;
                }
                let result = match value["method"].as_str().unwrap() {
                    "fs/read_text_file" => {
                        let path = PathBuf::from(value["params"]["path"].as_str().unwrap());
                        let canonical = std::fs::canonicalize(&path).unwrap();
                        assert_eq!(canonical, self.file);
                        self.reads.push(canonical);
                        json!({"content":std::fs::read_to_string(path).unwrap()})
                    }
                    "session/request_permission" => {
                        assert_eq!(value["params"]["toolCall"]["kind"], "read");
                        let option = value["params"]["options"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .find(|option| option["kind"] == "allow_once")
                            .unwrap();
                        json!({"outcome":{"outcome":"selected","optionId":option["optionId"]}})
                    }
                    other => panic!("unexpected ACP client request {other}: {value}"),
                };
                self.send(json!({"jsonrpc":"2.0","id":value["id"],"result":result}))
                    .await;
            }
        })
        .await
        .unwrap_or_else(|_| panic!("ACP response timed out; saw {:?}", self.seen))
    }
}

#[derive(Debug)]
struct Request {
    path: String,
    headers: HashMap<String, String>,
    body: Value,
}

impl Request {
    fn has_read(&self) -> bool {
        self.body["tools"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|tool| tool["name"] == "read")
    }
}

struct Gateway {
    address: std::net::SocketAddr,
    requests: Arc<Mutex<Vec<Request>>>,
    task: tokio::task::JoinHandle<()>,
}

impl Gateway {
    async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let received = Arc::clone(&requests);
        let task = tokio::spawn(async move {
            let mut main_calls = 0;
            loop {
                let (mut stream, _) = listener.accept().await.unwrap();
                tokio::time::timeout(Duration::from_secs(30), async {
                    let mut reader = BufReader::new(&mut stream);
                    let mut first = String::new();
                    reader.read_line(&mut first).await.unwrap();
                    let path = first.split_whitespace().nth(1).unwrap().to_owned();
                    let mut headers = HashMap::new();
                    loop {
                        let mut line = String::new();
                        assert!(reader.read_line(&mut line).await.unwrap() > 0);
                        if line == "\r\n" { break; }
                        let (key, value) = line.split_once(':').unwrap();
                        headers.insert(key.to_ascii_lowercase(), value.trim().to_owned());
                    }
                    let length: usize = headers["content-length"].parse().unwrap();
                    assert!(length <= 16 * 1024 * 1024);
                    let mut bytes = vec![0; length];
                    reader.read_exact(&mut bytes).await.unwrap();
                    let request = Request { path, headers, body: serde_json::from_slice(&bytes).unwrap() };
                    let mut events = Vec::new();
                    if request.has_read() {
                        main_calls += 1;
                        assert!(main_calls <= 2, "unexpected extra tool round: {request:?}");
                    }
                    if request.has_read() && main_calls == 1 {
                        let args = json!({"file_path":"work-order.txt"}).to_string();
                        let (first, rest) = args.split_at(args.len() / 2);
                        events.extend([
                            json!({"type":"response.output_item.added","item":{"type":"function_call","id":"item-read","call_id":"call-read","name":"read","arguments":""}}),
                            json!({"type":"response.function_call_arguments.delta","item_id":"item-read","delta":first}),
                            json!({"type":"response.function_call_arguments.delta","item_id":"item-read","delta":rest}),
                        ]);
                    } else {
                        events.push(json!({"type":"response.output_text.delta","delta":ANSWER}));
                    }
                    events.push(json!({"type":"response.completed","response":{"id":format!("response-{main_calls}"),"usage":{"input_tokens":10,"output_tokens":6,"total_tokens":16},"output":[]}}));
                    received.lock().unwrap().push(request);
                    let body: String = events.iter().map(|event| format!("data: {event}\n\n")).collect();
                    let headers = format!("HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
                    stream.write_all(headers.as_bytes()).await.unwrap();
                    stream.write_all(body.as_bytes()).await.unwrap();
                }).await.expect("bounded gateway request");
            }
        });
        Self {
            address,
            requests,
            task,
        }
    }
}

impl Drop for Gateway {
    fn drop(&mut self) {
        self.task.abort();
    }
}
