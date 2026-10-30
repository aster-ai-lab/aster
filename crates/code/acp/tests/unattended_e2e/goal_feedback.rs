//! Issue #20 through the real ACP binary, automatic ticks and workspace tools.
use super::*;

pub(super) fn criteria() -> Vec<String> {
    vec![
        "Required deliverable mutation.report.json does not exist".into(),
        "报告必须包含 verified 字段，并保留原始证据".into(),
    ]
}

pub(super) fn received(request: &serde_json::Value) -> bool {
    request["messages"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|message| {
            if message["role"] != "user" {
                return false;
            }
            let Some(text) = message["content"].as_str() else {
                return false;
            };
            text.lines()
                .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
                .any(|data| {
                    data["remaining_criteria"] == serde_json::json!(criteria())
                        && data["goal_id"].is_string()
                        && data["plan_revision"].is_u64()
                        && data["step_id"] == "task"
                })
        })
}

#[tokio::test(flavor = "current_thread")]
async fn rejected_goal_delivers_all_feedback_then_accepts_new_verified_file() {
    tokio::task::LocalSet::new().run_until(async {
        let workspace = isolate("feedback-ws");
        let outside = isolate("feedback-state");
        let home = isolate("feedback-home");
        let tmp = isolate("feedback-tmp");
        let (addr, script) = spawn_mock();
        script.lock().unwrap().feedback_required = true;
        let config = write_acl(&outside, &format!("http://{addr}/v1"));
        let result_file = outside.join("result.json");
        let result_arg = result_file.display().to_string();
        let agent = spawn_agent(&workspace, &config, &home, &tmp,
            &["--unattended", "--goal-continuation", "--max-execution-ms", "45000", "--result-file", &result_arg]).await;
        let session = open(&agent, &workspace).await;
        let response = say(&agent, &session, "UNATTENDED-TASK write and verify answer.txt and mutation.report.json").await.unwrap_or_else(|error| {
            let state = script.lock().unwrap();
            eprintln!("rejected={} seen={} delivered={} verified={} requests={}", state.rejected_judgments, state.feedback_seen, state.delivered, state.verified, state.bodies.len());
            for raw in state.bodies.iter().rev().take(2) {
                let value: serde_json::Value = serde_json::from_str(raw).unwrap();
                let last = value["messages"].as_array().unwrap().last().unwrap().to_string();
                eprintln!("last_message={}", last.chars().take(600).collect::<String>());
            }
            panic!("ACP feedback run: {error:?}")
        });
        assert_eq!(response.stop_reason, acp::StopReason::EndTurn);
        let record: serde_json::Value = serde_json::from_slice(&std::fs::read(&result_file).unwrap()).unwrap();
        assert_eq!(record["outcome"], "succeeded", "{record}");
        assert_eq!(std::fs::read(workspace.join("mutation.report.json")).unwrap(), b"{\"verified\":true}\n");
        let script = script.lock().unwrap();
        assert!(script.rejected_judgments > 0, "must first reject the missing deliverable");
        assert!(script.feedback_seen, "the execution model must receive the bound criteria in order");
        assert!(script.verified);
        assert!(script.verification_attempts >= 2, "new verification must follow the rejected judgment");
        eprintln!("ORACLE issue=20 result=pass rejected_judgments={} verifications={} deliverable=present", script.rejected_judgments, script.verification_attempts);
    }).await;
}
