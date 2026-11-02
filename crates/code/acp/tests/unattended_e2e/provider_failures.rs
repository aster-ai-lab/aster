//! Issue #295: real ACP, HTTP failures, workspace writes and verification.
use super::*;

const CHECK: &str = "test -f answer.txt && test \"$(cat answer.txt)\" = 42";

#[derive(Clone, Copy)]
enum Scenario {
    Exhausted,
    Recovered,
    GoalFailure,
}

pub(super) fn planning_reply(request: &serde_json::Value) -> Option<String> {
    let tool = request["tools"].as_array().is_some_and(|tools| {
        tools
            .iter()
            .any(|tool| tool["function"]["name"] == "emit_pre_analysis")
    });
    let planning = request["response_format"]["json_schema"]["name"] == "pre_analysis"
        || request["messages"].as_array().is_some_and(|messages| {
            messages.iter().any(|message| {
                message["role"] == "system"
                    && message["content"]
                        .as_str()
                        .is_some_and(|text| text.contains("You plan a durable Goal."))
            })
        });
    if !tool && !planning {
        return None;
    }
    // The simple plan uses the original bound Goal's criteria, not new acceptance facts.
    let analysis = serde_json::json!({
        "intent":"GeneralPurpose", "requires_planning":false,
        "goal":{"description":"Complete the original task", "success_criteria":["Every requirement of the task holds in the workspace."]},
        "execution_plan":{"complexity":"Simple", "stages":[], "required_tools":[]},
        "optimized_input":"Follow the original task."
    });
    let streaming = request["stream"].as_bool().unwrap_or(false);
    Some(if tool {
        reply_tool("emit_pre_analysis", analysis, 1, streaming)
    } else {
        reply_text(&analysis.to_string(), streaming)
    })
}

struct ProviderState {
    failures: usize,
    guidance_seen: bool,
    checks: usize,
}

fn provider(
    source: PathBuf,
    status: u16,
    recover_after: Option<usize>,
) -> (SocketAddr, Arc<Mutex<ProviderState>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let state = Arc::new(Mutex::new(ProviderState {
        failures: 0,
        guidance_seen: false,
        checks: 0,
    }));
    let observed = Arc::clone(&state);
    std::thread::spawn(move || {
        for connection in listener.incoming() {
            let Ok(mut stream) = connection else { break };
            stream
                .set_read_timeout(Some(Duration::from_secs(10)))
                .unwrap();
            let Some(raw) = read_http_body(&mut stream) else {
                continue;
            };
            let request: serde_json::Value = serde_json::from_str(&raw).unwrap();
            if let Some(body) = planning_reply(&request) {
                write_http(&mut stream, &body).unwrap();
                continue;
            }
            let streaming = request["stream"].as_bool().unwrap_or(false);
            let tools = request["tools"].as_array();
            if tools
                .is_none_or(|tools| !tools.iter().any(|tool| tool["function"]["name"] == "write"))
            {
                write_http(&mut stream, &reply_text("ok", streaming)).unwrap();
                continue;
            }
            let mut state = observed.lock().unwrap();
            state.guidance_seen |= raw.contains("Next step: run one `bash` command");
            if !source.exists() {
                let body = reply_tool(
                    "write",
                    serde_json::json!({"file_path":"answer.txt","content":"42\n"}),
                    1,
                    streaming,
                );
                write_http(&mut stream, &body).unwrap();
            } else if recover_after.is_none_or(|limit| state.failures < limit) {
                state.failures += 1;
                let body = r#"{"error":{"message":"fixture transient provider failure"}}"#;
                let response = format!(
                    "HTTP/1.1 {status} Fixture Error\r\nContent-Type: application/json\r\nRetry-After: 0.001\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                stream.write_all(response.as_bytes()).unwrap();
            } else {
                let body = if state.checks == 0 {
                    state.checks += 1;
                    reply_tool(
                        "bash",
                        serde_json::json!({
                            "command": CHECK,
                            "sandbox_permissions":"require_escalated",
                            "justification":"Verify the private fixture through approved host execution"
                        }),
                        state.checks as u64 + 1,
                        streaming,
                    )
                } else {
                    reply_text("answer.txt was written and checked.", streaming)
                };
                write_http(&mut stream, &body).unwrap();
            }
        }
    });
    (address, state)
}

async fn run(status: u16, scenario: Scenario) {
    let recover = matches!(scenario, Scenario::Recovered);
    let goal = matches!(scenario, Scenario::GoalFailure);
    tokio::task::LocalSet::new().run_until(async {
        let workspace = isolate("provider failure 工作区");
        let outside = isolate("provider-failure-state");
        let home = isolate("provider-failure-home");
        let tmp = isolate("provider-failure-tmp");
        // Eleven HTTP attempts exhaust one default provider retry cycle.
        let (address, observed) = provider(workspace.join("answer.txt"), status, recover.then_some(13));
        let config = write_acl(&outside, &format!("http://{address}/v1"));
        let result_file = outside.join("result.json");
        let result_arg = result_file.display().to_string();
        let agent = spawn_agent(&workspace, &config, &home, &tmp, &[
            "--unattended", if goal { "--goal-continuation" } else { "--no-goal-continuation" }, "--max-execution-ms", "45000",
            "--result-file", &result_arg,
        ]).await;
        let session = open(&agent, &workspace).await;
        let response = tokio::time::timeout(Duration::from_secs(60), say(
            &agent, &session, "Write answer.txt containing 42 followed by a newline. Verify its contents with bash before finishing.",
        )).await.expect("ACP run must settle within the test deadline");
        let record: serde_json::Value = serde_json::from_slice(&std::fs::read(&result_file).unwrap()).unwrap();
        eprintln!("PROVIDER_RESULT status={status} goal={goal} record={record} response={response:?}");
        let facts = private_fixture_facts(&tmp);
        assert!(facts.iter().any(|fact| fact["type"] == "tool.result"), "must inspect actual tool receipts");
        let verified = facts.iter().any(|fact| fact["type"] == "turn.complete" && fact["payload"]["terminal"] == "verified");
        let checked = facts.iter().any(|fact| {
            fact["type"] == "tool.result"
                && fact["payload"]["receipt"]["outcome"] == "succeeded"
                && fact["payload"]["receipt"]["exit_code"] == 0
                && fact["payload"]["output"].as_str()
                    .and_then(|output| serde_json::from_str::<serde_json::Value>(output).ok())
                    .is_some_and(|output| output["metadata"]["verification_shell_command"] == CHECK)
        });
        assert_eq!(std::fs::read(workspace.join("answer.txt")).unwrap(), b"42\n");
        let state = observed.lock().unwrap();
        eprintln!("ORACLE issue=295 status={status} recover={recover} failures={} checks={} record={record} response={response:?}", state.failures, state.checks);
        assert_eq!(response.unwrap().stop_reason, acp::StopReason::EndTurn);
        if recover {
            assert_eq!(record["outcome"], "succeeded", "{record}");
            assert_eq!(record["exitCode"], 0);
            assert_eq!(state.failures, 13);
            assert!(state.checks > 0);
            assert!(state.guidance_seen, "completion guidance must reach the recovered model");
            assert!(record["successfulToolCalls"].as_u64().unwrap() >= 2);
            assert!(verified, "the recovered run must pass the real completion gate");
            assert!(checked, "the exact checker must actually exit 0");
        } else {
            assert_eq!(record["outcome"], "failed", "{record}");
            assert_eq!(record["reason"], "retryable_external", "{record}");
            assert_eq!(record["exitCode"], 1);
            assert_eq!(record["providerStatus"], status);
            assert_eq!(state.checks, 0, "an unavailable model cannot have verified the write");
            assert!(!verified, "provider failure must not waive the completion gate");
            assert!(!checked);
            assert_eq!(state.failures, if goal { 11 } else { 33 });
            assert!(record["lastError"].as_str().unwrap().contains(&status.to_string()));
        }
    }).await;
}

#[tokio::test(flavor = "current_thread")]
async fn exhausted_provider_failure_has_a_retryable_external_result() {
    for status in [429, 503] {
        run(status, Scenario::Exhausted).await;
    }
}

#[tokio::test(flavor = "current_thread")]
async fn recovered_provider_can_receive_gate_guidance_and_verify_the_write() {
    for status in [429, 503] {
        run(status, Scenario::Recovered).await;
    }
}

#[tokio::test(flavor = "current_thread")]
async fn failed_goal_provider_is_retryable_without_replaying_the_exact_run() {
    for status in [429, 503] {
        run(status, Scenario::GoalFailure).await;
    }
}
