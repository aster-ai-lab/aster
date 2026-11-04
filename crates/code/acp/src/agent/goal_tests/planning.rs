use super::*;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

pub(super) const SIMPLE_PLAN: &str = r#"{"goal":"Explicit task","complexity":"Simple","steps":[{"id":"task","description":"Complete the explicit task","success_criteria":"Every original requirement holds in the workspace"}]}"#;

pub(super) async fn launch(
    store: std::path::PathBuf,
    plan: &str,
) -> (
    crate::config::LaunchConfig,
    tokio::task::JoinHandle<Vec<serde_json::Value>>,
) {
    launch_replies(store, vec![analysis(plan)], "200 OK").await
}

fn analysis(plan: &str) -> String {
    let mut plan: serde_json::Value = serde_json::from_str(plan).unwrap();
    let description = plan.as_object_mut().unwrap().remove("goal").unwrap();
    let stages = plan.as_object_mut().unwrap().remove("steps").unwrap();
    plan["stages"] = stages;
    serde_json::json!({
        "intent":"plan", "requires_planning":true,
        "goal":{"description":description,"success_criteria":["Original requirements hold"]},
        "execution_plan":plan, "optimized_input":"Implement the authorized task"
    })
    .to_string()
}

async fn launch_replies(
    store: std::path::PathBuf,
    replies: Vec<String>,
    status: &'static str,
) -> (
    crate::config::LaunchConfig,
    tokio::task::JoinHandle<Vec<serde_json::Value>>,
) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut launch = test_launch(Some(store));
    launch.config.providers[0].base_url =
        Some(format!("http://{}/v1", listener.local_addr().unwrap()));
    let provider = tokio::spawn(async move {
        let mut requests = Vec::new();
        for reply in replies {
            let (socket, _) =
                tokio::time::timeout(std::time::Duration::from_secs(10), listener.accept())
                    .await
                    .unwrap()
                    .unwrap();
            let mut socket = BufReader::new(socket);
            let mut line = String::new();
            socket.read_line(&mut line).await.unwrap();
            assert!(line.starts_with("POST /v1/messages "), "{line}");
            let mut length = 0;
            loop {
                line.clear();
                assert!(socket.read_line(&mut line).await.unwrap() > 0);
                if line == "\r\n" {
                    break;
                }
                if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = value.trim().parse::<usize>().unwrap();
                }
            }
            assert!((1..=1024 * 1024).contains(&length));
            let mut bytes = vec![0; length];
            socket.read_exact(&mut bytes).await.unwrap();
            let request = serde_json::from_slice(&bytes).unwrap();
            let body = serde_json::json!({"content":[{"type":"text","text":reply}],"stop_reason":"end_turn","usage":{"input_tokens":11,"output_tokens":7}}).to_string();
            let response = format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
            socket
                .get_mut()
                .write_all(response.as_bytes())
                .await
                .unwrap();
            socket.get_mut().shutdown().await.unwrap();
            requests.push(request);
        }
        requests
    });
    (launch, provider)
}

#[tokio::test]
async fn model_stages_are_accepted_with_dependencies_and_pending_status() {
    let root = tempfile::tempdir().unwrap();
    let plan = r#"{"goal":"Repair and verify","complexity":"Medium","steps":[{"id":"repair","description":"Repair callers","success_criteria":"Current callers compile"},{"id":"verify","description":"Run original tests","dependencies":["repair"],"success_criteria":"Original tests pass"}]}"#;
    let (mut launch, provider) = launch(root.path().join("store"), plan).await;
    launch.workspace = root.path().into();
    let host = AsterCodeAgent::new(launch);
    let id = host
        .new_session(acp::NewSessionRequest::new(root.path()))
        .await
        .unwrap()
        .session_id;
    let session = host.sessions.lock().await[id.0.as_ref()].session.clone();
    crate::goal::bind_task_goal(&session, "Implement the authorized repair plan")
        .await
        .unwrap();
    let requests = provider.await.unwrap();
    let request = &requests[0];
    assert!(request["messages"]
        .to_string()
        .contains("Implement the authorized repair plan"));
    let definition = session.goal_definition().unwrap().unwrap();
    assert!(definition.plan_accepted);
    let plan = definition.plan.unwrap();
    assert_eq!(plan.steps.len(), 2);
    assert_eq!(plan.steps[1].dependencies, ["repair"]);
    assert!(plan
        .steps
        .iter()
        .all(|step| step.status == aster_core::planning::TaskStatus::Pending));
    let sequence = definition.goal_sequence;
    crate::goal::bind_task_goal(&session, "Implement the authorized repair plan")
        .await
        .unwrap();
    assert_eq!(
        session.goal_definition().unwrap().unwrap().goal_sequence,
        sequence,
        "an accepted plan is reused without another provider call or proposal"
    );
    assert_eq!(
        session
            .preview_goal_tick(&crate::goal::bound_tick_command(&session).unwrap().unwrap())
            .await
            .unwrap(),
        aster_core::goal_control::GoalTickDecisionV1::Advance
    );
    session.close().await;
}

#[tokio::test]
async fn a_long_scope_is_compacted_without_dropping_its_original_stage_obligations() {
    let root = tempfile::tempdir().unwrap();
    let (mut launch, provider) = launch_replies(
        root.path().join("store"),
        vec![
            "## Current State\nThe model claims everything is done.".into(),
            analysis(SIMPLE_PLAN),
        ],
        "200 OK",
    )
    .await;
    launch.workspace = root.path().into();
    let host = AsterCodeAgent::new(launch);
    let id = host
        .new_session(acp::NewSessionRequest::new(root.path()))
        .await
        .unwrap()
        .session_id;
    let session = host.sessions.lock().await[id.0.as_ref()].session.clone();
    let directory = aster_core::session_folder::effect_log_dir(root.path(), session.session_id());
    std::fs::create_dir_all(&directory).unwrap();
    let thread = aster_core::fact_control::thread_for_session(session.session_id());
    let before = aster_core::fact_control::read_workspace_facts(root.path(), &thread).unwrap();
    let sequence = before.last().map_or(0, |fact| fact.seq);
    let mut source = String::new();
    let approved_plan = format!(
        "Required steps:\n{}",
        (0..34)
            .map(|index| format!("- [ ] Keep stage {index} open."))
            .collect::<Vec<_>>()
            .join("\n")
    );
    for index in 0..34 {
        let text = if index == 0 {
            approved_plan.as_str()
        } else {
            "Keep working within the original authorized plan."
        };
        source.push_str(&serde_json::json!({"seq":sequence+index+1,"type":"user.message","key":format!("context-{index}"),"cause":null,"payload":{"text":text}}).to_string());
        source.push('\n');
    }
    // Retained-history setup preserves the original saved source prefix.
    let mut log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(directory.join(format!("{thread}.jsonl")))
        .unwrap();
    std::io::Write::write_all(&mut log, source.as_bytes()).unwrap();
    crate::goal::bind_task_goal(&session, "Complete the authorized stages")
        .await
        .unwrap();
    let requests = provider.await.unwrap();
    assert_eq!(
        requests.len(),
        2,
        "one compaction and one governed plan request"
    );
    let planning_input = requests[1]["messages"].to_string();
    assert!(planning_input.contains("Keep stage 0 open."));
    assert!(planning_input.contains("Keep stage 33 open."));
    let definition = session.goal_definition().unwrap().unwrap();
    let facts = aster_core::fact_control::read_workspace_facts(root.path(), &thread).unwrap();
    let compacted = facts
        .iter()
        .find(|fact| fact.kind == "compaction.done")
        .unwrap();
    assert_eq!(definition.binding.scope_fact_sequences, [compacted.seq]);
    assert!(definition
        .plan
        .unwrap()
        .steps
        .iter()
        .all(|step| step.status == aster_core::planning::TaskStatus::Pending));
    session.close().await;
}

#[tokio::test]
async fn a_planning_provider_rejection_retains_its_cause_as_an_internal_failure() {
    let root = tempfile::tempdir().unwrap();
    let (mut launch, provider) = launch_replies(
        root.path().join("store"),
        vec!["synthetic provider rejection".into()],
        "401 Unauthorized",
    )
    .await;
    launch.workspace = root.path().into();
    let host = AsterCodeAgent::new(launch);
    let id = host
        .new_session(acp::NewSessionRequest::new(root.path()))
        .await
        .unwrap()
        .session_id;
    let error = host
        .prompt(acp::PromptRequest::new(
            id.clone(),
            vec![acp::ContentBlock::from("/goal Verify provider failures")],
        ))
        .await
        .unwrap_err();
    assert_eq!(error.code, acp::Error::internal_error().code);
    assert!(error.data.unwrap().to_string().contains("401"));
    assert_eq!(provider.await.unwrap().len(), 1);
    let session = host.sessions.lock().await[id.0.as_ref()].session.clone();
    assert!(session.goal_definition().unwrap().unwrap().plan.is_none());
    session.close().await;
}

#[tokio::test]
async fn cancelling_goal_planning_does_not_accept_a_late_plan() {
    let root = tempfile::tempdir().unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut launch = test_launch(Some(root.path().join("store")));
    launch.workspace = root.path().into();
    launch.config.providers[0].base_url =
        Some(format!("http://{}/v1", listener.local_addr().unwrap()));
    let host = AsterCodeAgent::new(launch);
    let id = host
        .new_session(acp::NewSessionRequest::new(root.path()))
        .await
        .unwrap()
        .session_id;
    let session = host.sessions.lock().await[id.0.as_ref()].session.clone();
    let planning = prompt(&host, &id, "/goal Complete the explicit stages");
    let cancelling = async {
        let (_socket, _) = listener.accept().await.unwrap();
        host.cancel(acp::CancelNotification::new(id.clone()))
            .await
            .unwrap();
    };
    let (response, ()) = tokio::time::timeout(std::time::Duration::from_secs(2), async {
        tokio::join!(planning, cancelling)
    })
    .await
    .unwrap();
    assert_eq!(response.unwrap().stop_reason, acp::StopReason::Cancelled);
    assert!(session.goal_definition().unwrap().unwrap().plan.is_none());
    session.close().await;
}

#[tokio::test]
async fn the_unattended_deadline_also_bounds_plan_generation() {
    let root = tempfile::tempdir().unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut launch = test_launch(Some(root.path().join("store")));
    launch.workspace = root.path().into();
    launch.config.providers[0].base_url =
        Some(format!("http://{}/v1", listener.local_addr().unwrap()));
    let result_path = root.path().join("result.json");
    let unattended = crate::unattended::Unattended::new(3_000, Some(result_path.clone())).unwrap();
    let host = AsterCodeAgent::new(launch).with_unattended(unattended.clone());
    let id = host
        .new_session(acp::NewSessionRequest::new(root.path()))
        .await
        .unwrap()
        .session_id;
    let (session, queue) = {
        let sessions = host.sessions.lock().await;
        let live = &sessions[id.0.as_ref()];
        (live.session.clone(), live.turn_queue.clone())
    };
    let permit = queue.acquire_owned().await.unwrap();
    let mut running = Box::pin(host.unattended_prompt(
        &id,
        &session,
        "Complete the explicit stages",
        0,
        permit,
        &unattended,
    ));
    let (_socket, _) = tokio::select! {
        accepted = listener.accept() => accepted.unwrap(),
        result = &mut running => panic!("the provider must be entered before the deadline: {result:?}"),
    };
    tokio::time::timeout(std::time::Duration::from_secs(5), running)
        .await
        .unwrap()
        .unwrap();
    let result: serde_json::Value =
        serde_json::from_slice(&std::fs::read(result_path).unwrap()).unwrap();
    assert_eq!(result["reason"], "deadline_exceeded");
    assert_eq!(result["outcome"], "timed_out");
    assert_eq!(result["toolCalls"], 0);
    assert!(session.goal_definition().unwrap().unwrap().plan.is_none());
    session.close().await;
}
