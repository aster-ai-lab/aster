#![cfg(test)]

use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

pub const MODEL: &str = "fixture-model";
pub const KEY: &str = "local-scripted-fixture-key";

pub const REPAIRS: [(&str, &str); 4] = [
    ("input.rs", "pub fn quantity(text:&str)->Result<u32,&'static str>{text.trim().parse::<u32>().ok().filter(|n|(1..=1_000_000).contains(n)).ok_or(\"invalid quantity\")}\n"),
    ("calc.rs", "pub fn total(q:u32,p:u32,r:u32)->Result<u64,&'static str>{if !(1..=1_000_000).contains(&q)||!(1..=1_000_000).contains(&p)||r>10000{return Err(\"invalid amount\");}let n=u64::from(q)*u64::from(p);Ok(n+(n*u64::from(r)+5000)/10000)}\n"),
    ("display.rs", "pub fn money(c:u64)->String{format!(\"{}.{:02}\",c/100,c%100)}\n"),
    ("CHANGELOG.md", "Repair quantity bounds, integer tax rounding, and cent formatting.\n"),
];

#[derive(Default, Clone, serde::Serialize)]
pub struct Observations {
    pub requests: usize,
    pub main_steps: usize,
    pub auxiliary_requests: usize,
    pub repair_masked_check: bool,
    pub saw_exact_contract_advice: bool,
    pub saw_initial_exit_code: bool,
    pub initial_exit_code: Option<i32>,
}

pub struct Scripted {
    pub endpoint: String,
    observations: Arc<Mutex<Observations>>,
    cancel: CancellationToken,
    worker: Option<tokio::task::JoinHandle<()>>,
}

fn tool(id: &str, name: &str, args: Value) -> Value {
    json!({"id":id,"type":"function","function":{"name":name,"arguments":args.to_string()}})
}

fn original_check_exit_code(request: &Value) -> Option<i32> {
    let messages = request["messages"].as_array()?;
    for (index, message) in messages.iter().enumerate() {
        if message["role"] != "assistant" {
            continue;
        }
        for call in message["tool_calls"].as_array().into_iter().flatten() {
            if call["function"]["name"] != "bash" {
                continue;
            }
            let args: Value = serde_json::from_str(call["function"]["arguments"].as_str()?).ok()?;
            if args["command"] != super::fixture::LOCAL_CHECK {
                continue;
            }
            let id = call["id"].as_str()?;
            for result in messages[index + 1..]
                .iter()
                .take_while(|item| item["role"] == "tool")
            {
                if result["tool_call_id"] == id {
                    return result["content"]
                        .as_str()?
                        .split_once('\n')?
                        .0
                        .strip_prefix("Tool exit code: ")?
                        .parse()
                        .ok();
                }
            }
        }
    }
    None
}

fn decide(request: &Value, observations: &mut Observations) -> Result<Value, String> {
    observations.requests += 1;
    if request["model"] != MODEL || observations.requests > 24 {
        return Err("scripted_request_bound_or_model".into());
    }
    let definitions: Vec<&str> = request["tools"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|tool| tool["function"]["name"].as_str())
        .collect();
    if !definitions.contains(&"write") {
        observations.auxiliary_requests += 1;
        let analysis = json!({"intent":"general","requires_planning":false,"goal":{"description":"Repair the supplied Rust fixture","success_criteria":["All required tests pass"]},"execution_plan":{"complexity":"Simple","steps":[],"required_tools":[]},"optimized_input":"Read README.md and complete the requested repair, preserving protected files."});
        if definitions.contains(&"emit_pre_analysis") {
            return Ok(
                json!({"role":"assistant","tool_calls":[tool("pre-analysis", "emit_pre_analysis", analysis)]}),
            );
        }
        if request["response_format"]["json_schema"]["name"] == "pre_analysis" {
            return Ok(json!({"role":"assistant","content":analysis.to_string()}));
        }
        return Ok(json!({"role":"assistant","content":"{\"items\":[]}"}));
    }
    let step = observations.main_steps;
    observations.main_steps += 1;
    let calls = match step {
        0 => vec![tool(
            "read-contract",
            "read",
            json!({"file_path":"README.md"}),
        )],
        1 => vec![tool(
            "initial-check",
            "bash",
            json!({"command":super::fixture::LOCAL_CHECK,"timeout":30000}),
        )],
        2 => {
            observations.initial_exit_code = original_check_exit_code(request);
            observations.saw_initial_exit_code =
                observations.initial_exit_code.is_some_and(|code| code != 0);
            if !observations.saw_initial_exit_code {
                return Err("scripted_initial_exit_code_missing".into());
            }
            REPAIRS
                .iter()
                .enumerate()
                .map(|(i, (path, content))| {
                    tool(
                        &format!("repair-{i}"),
                        "write",
                        json!({"file_path":path,"content":content}),
                    )
                })
                .collect()
        }
        3 if observations.repair_masked_check => vec![tool(
            "masked-check",
            "bash",
            json!({"command":format!("{}; echo exit=$?",super::fixture::LOCAL_CHECK),"timeout":30000}),
        )],
        3 => vec![tool(
            "final-check",
            "bash",
            json!({"command":super::fixture::LOCAL_CHECK,"timeout":30000}),
        )],
        4 if observations.repair_masked_check => {
            observations.saw_exact_contract_advice = request["messages"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|message| {
                    message["role"] == "system"
                        && message["content"].as_str().is_some_and(|text| {
                            text.contains("Admitted exact commands covering current changes")
                                && text.contains(super::fixture::LOCAL_CHECK)
                        })
                });
            if !observations.saw_exact_contract_advice {
                return Err("scripted_exact_contract_guidance_missing".into());
            }
            vec![tool(
                "repaired-exact-check",
                "bash",
                json!({"command":super::fixture::LOCAL_CHECK,"timeout":30000}),
            )]
        }
        4 | 5 if step == 4 || observations.repair_masked_check => {
            return Ok(
                json!({"role":"assistant","content":"Repaired input.rs, calc.rs and display.rs, added CHANGELOG.md, and ran the exact local check successfully. Protected files were preserved."}),
            )
        }
        _ => return Err("scripted_unexpected_continuation".into()),
    };
    for call in &calls {
        if !definitions.contains(&call["function"]["name"].as_str().unwrap()) {
            return Err("scripted_tool_not_advertised".into());
        }
    }
    Ok(json!({"role":"assistant","tool_calls":calls,"content":null}))
}

fn response(message: Value, streaming: bool) -> String {
    let reason = if message["tool_calls"].is_array() {
        "tool_calls"
    } else {
        "stop"
    };
    if !streaming {
        return json!({"choices":[{"index":0,"message":message,"finish_reason":reason}],"usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}}).to_string();
    }
    let mut delta = message;
    delta.as_object_mut().unwrap().remove("role");
    if let Some(calls) = delta["tool_calls"].as_array_mut() {
        for (index, call) in calls.iter_mut().enumerate() {
            call["index"] = index.into();
        }
    }
    format!(
        "data: {}\n\ndata: {}\n\ndata: {}\n\ndata: [DONE]\n\n",
        json!({"choices":[{"index":0,"delta":delta}]}),
        json!({"choices":[{"index":0,"delta":{},"finish_reason":reason}]}),
        json!({"choices":[],"usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}})
    )
}

#[test]
fn scripted_json_and_sse_publish_the_same_explicit_usage() {
    let message = json!({"role":"assistant","content":"fixture"});
    let non_stream: Value = serde_json::from_str(&response(message.clone(), false)).unwrap();
    let streamed = response(message, true);
    let observed: Vec<Value> = streamed
        .lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter_map(|value| value.get("usage").cloned())
        .collect();
    assert_eq!(observed, vec![non_stream["usage"].clone()]);
    assert!(streamed.ends_with("data: [DONE]\n\n"));
}

#[test]
fn initial_check_uses_the_original_pair_with_an_opaque_host_id() {
    for id in ["initial-check", "host-issued-opaque-id"] {
        let request = json!({"messages":[
            {"role":"assistant","tool_calls":[tool(id,"bash",json!({"command":super::fixture::LOCAL_CHECK}))]},
            {"role":"tool","tool_call_id":id,"content":"Tool exit code: 7\noriginal failure"}
        ]});
        assert_eq!(original_check_exit_code(&request), Some(7));
    }
}

#[test]
fn initial_check_refuses_unbound_foreign_or_masked_result_frames() {
    let original = json!({"messages":[
        {"role":"assistant","tool_calls":[tool("opaque","bash",json!({"command":super::fixture::LOCAL_CHECK}))]},
        {"role":"tool","tool_call_id":"opaque","content":"Tool exit code: 1\noriginal failure"}
    ]});
    let mut wrong_id = original.clone();
    wrong_id["messages"][1]["tool_call_id"] = "initial-check".into();
    let mut wrong_command = original.clone();
    wrong_command["messages"][0]["tool_calls"][0]["function"]["arguments"] =
        json!({"command":format!("{}; echo exit=$?",super::fixture::LOCAL_CHECK)})
            .to_string()
            .into();
    let mut wrong_role = original.clone();
    wrong_role["messages"][1]["role"] = "user".into();
    let mut no_status = original.clone();
    no_status["messages"][1]["content"] = "stdout without an original exit code".into();
    let mut reversed = original.clone();
    reversed["messages"].as_array_mut().unwrap().reverse();
    for frame in [wrong_id, wrong_command, wrong_role, no_status, reversed] {
        assert_eq!(original_check_exit_code(&frame), None);
    }
}

impl Scripted {
    pub async fn start(repair_masked_check: bool) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!(
            "http://{}/v1/chat/completions",
            listener.local_addr().unwrap()
        );
        let cancel = CancellationToken::new();
        let stopping = cancel.clone();
        let observations = Arc::new(Mutex::new(Observations {
            repair_masked_check,
            ..Observations::default()
        }));
        let shared = observations.clone();
        let worker = tokio::spawn(async move {
            loop {
                let (mut socket, _) = tokio::select! { _ = stopping.cancelled() => break, accepted = listener.accept() => accepted.unwrap() };
                let exchange = async {
                    let request = super::relay::read_request(&mut socket, &format!("Bearer {KEY}"))
                        .await
                        .map_err(|_| "scripted_request_refused".to_owned())?;
                    let message = decide(&request, &mut shared.lock().unwrap())?;
                    let streaming = request["stream"] == true;
                    let body = response(message, streaming);
                    let kind = if streaming {
                        "text/event-stream"
                    } else {
                        "application/json"
                    };
                    let header = format!("HTTP/1.1 200 OK\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
                    socket
                        .write_all(header.as_bytes())
                        .await
                        .map_err(|_| "scripted_write")?;
                    socket
                        .write_all(body.as_bytes())
                        .await
                        .map_err(|_| "scripted_write")?;
                    Ok::<(), String>(())
                };
                tokio::select! { _ = stopping.cancelled() => break, result = tokio::time::timeout(std::time::Duration::from_secs(30), exchange) => result.expect("bounded scripted exchange").expect("scripted exchange") }
            }
        });
        Self {
            endpoint,
            observations,
            cancel,
            worker: Some(worker),
        }
    }

    pub async fn close(mut self) -> Observations {
        self.cancel.cancel();
        tokio::time::timeout(
            std::time::Duration::from_secs(5),
            self.worker.take().unwrap(),
        )
        .await
        .unwrap()
        .unwrap();
        self.observations.lock().unwrap().clone()
    }
}

impl Drop for Scripted {
    fn drop(&mut self) {
        self.cancel.cancel();
        if let Some(worker) = self.worker.take() {
            worker.abort();
        }
    }
}

#[test]
fn scripted_repairs_require_the_actual_initial_command_exit_status() {
    for status in [None, Some(0), Some(1), Some(101), Some(137)] {
        let mut observations = Observations {
            main_steps: 2,
            ..Default::default()
        };
        let mut messages = vec![
            json!({"role":"assistant","tool_calls":[tool("initial-check","bash",json!({"command":super::fixture::LOCAL_CHECK}))]}),
        ];
        messages.extend(status.into_iter().map(|code|json!({"role":"tool","tool_call_id":"initial-check","content":format!("Tool exit code: {code}\ncontrolled output")})));
        let request =
            json!({"model":MODEL,"messages":messages,"tools":[{"function":{"name":"write"}}]});
        let response = decide(&request, &mut observations);
        let failed = status.is_some_and(|code| code != 0);
        assert_eq!(response.is_ok(), failed);
        assert_eq!(observations.saw_initial_exit_code, failed);
        assert_eq!(observations.initial_exit_code, status);
    }
}

#[test]
fn scripted_schedule_batches_repairs_and_refuses_missing_tools_and_extra_rounds() {
    let mut observations = Observations::default();
    let request = json!({"model":MODEL,"messages":[
        {"role":"assistant","tool_calls":[tool("initial-check","bash",json!({"command":super::fixture::LOCAL_CHECK}))]},
        {"role":"tool","tool_call_id":"initial-check","content":"Tool exit code: 101\ncontrolled test failure"}],"tools":[{"function":{"name":"read"}},{"function":{"name":"write"}},{"function":{"name":"bash"}}]});
    assert_eq!(
        decide(&request, &mut observations).unwrap()["tool_calls"][0]["function"]["name"],
        "read"
    );
    assert_eq!(
        decide(&request, &mut observations).unwrap()["tool_calls"][0]["id"],
        "initial-check"
    );
    assert_eq!(
        decide(&request, &mut observations).unwrap()["tool_calls"]
            .as_array()
            .unwrap()
            .len(),
        4
    );
    assert_eq!(
        decide(&request, &mut observations).unwrap()["tool_calls"][0]["id"],
        "final-check"
    );
    assert!(decide(&request, &mut observations).unwrap()["content"].is_string());
    assert!(decide(&request, &mut observations).is_err());
    let absent = json!({"model":MODEL,"tools":[{"function":{"name":"write"}}]});
    assert!(decide(&absent, &mut Observations::default()).is_err());
    let bad = json!({"model":"foreign-model"});
    assert!(decide(&bad, &mut Observations::default()).is_err());
}

#[test]
fn masked_check_repair_requires_real_advice_before_the_exact_check() {
    let mut state = Observations {
        repair_masked_check: true,
        main_steps: 3,
        ..Observations::default()
    };
    let mut request = json!({"model":MODEL,"messages":[{"role":"system","content":"initial host manifest only"}],"tools":[{"function":{"name":"write"}},{"function":{"name":"bash"}}]});
    let masked = decide(&request, &mut state).unwrap();
    assert!(masked["tool_calls"][0]["function"]["arguments"]
        .as_str()
        .unwrap()
        .contains("echo"));
    assert!(decide(&request, &mut state).is_err());
    state.main_steps = 4;
    request["messages"][0]["content"] = format!(
        "Admitted exact commands covering current changes: {}",
        super::fixture::LOCAL_CHECK
    )
    .into();
    let exact = decide(&request, &mut state).unwrap();
    let args: Value = serde_json::from_str(
        exact["tool_calls"][0]["function"]["arguments"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(args["command"], super::fixture::LOCAL_CHECK);
    assert!(state.saw_exact_contract_advice);
}
