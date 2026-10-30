#![cfg(test)]

use super::*;

fn child(
    root: &std::path::Path,
    client: Arc<GoalClient>,
    physical: Arc<AtomicUsize>,
    cancel: tokio_util::sync::CancellationToken,
) -> crate::agent_api::ChildRun {
    let executor = Arc::new(crate::tools::ToolExecutor::new(root.display().to_string()));
    executor.register_dynamic_tool_if_absent(Arc::new(RepeatedSource(physical)));
    let permission = crate::permissions::PermissionPolicy::new().allow("verify_source(*)");
    crate::agent_api::ChildRun {
        agent: crate::agent::AgentLoop::new(
            client,
            executor.clone(),
            ToolContext::new(root.to_path_buf()),
            crate::agent::AgentConfig {
                tools: executor.definitions(),
                max_tool_rounds: 0,
                duplicate_tool_call_threshold: 3,
                auto_compact: false,
                continuation_enabled: false,
                planning_mode: crate::PlanningMode::Disabled,
                permission_policy: Some(permission.clone()),
                permission_checker: Some(Arc::new(permission)),
                ..Default::default()
            },
        ),
        child_id: "owned-refusal-child".into(),
        budget_session: None,
        events: None,
        cancel,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_owned_child_log_cannot_keep_billing_after_repeated_refusals() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("source.txt"), "unchanged source").unwrap();
    let client = Arc::new(GoalClient::default());
    let weak = Arc::downgrade(&client);
    *client.before_response.lock().unwrap() = Some(Box::new(move |iteration| {
        weak.upgrade()
            .unwrap()
            .scripted
            .lock()
            .unwrap()
            .push_back(repeat(iteration));
    }));
    let physical = Arc::new(AtomicUsize::new(0));
    let cancel = tokio_util::sync::CancellationToken::new();
    let run = child(
        root.path(),
        client.clone(),
        physical.clone(),
        cancel.clone(),
    );
    let mut worker = tokio::spawn(async move { run.run("Read the original source.").await });
    let stopped = tokio::time::timeout(Duration::from_secs(1), &mut worker).await;
    let calls = client.calls.load(Ordering::SeqCst);
    let executions = physical.load(Ordering::SeqCst);
    eprintln!(
        "CHILD_REFUSAL_OBSERVATION model_calls={calls} physical_calls={executions} stopped={}",
        stopped.is_ok()
    );
    *client.before_response.lock().unwrap() = None;
    if stopped.is_err() {
        cancel.cancel();
        tokio::time::timeout(Duration::from_secs(5), worker)
            .await
            .unwrap()
            .unwrap()
            .ok();
    }
    assert!(
        stopped.is_ok(),
        "the owned child kept generating after original denials"
    );
    let result = stopped.unwrap().unwrap();
    assert!(format!("{:#}", result.unwrap_err()).contains("run.tool_refusal_cycle:"));
    assert_eq!(executions, 3);
    assert_eq!(
        calls, 5,
        "the child needs the same early stop as its parent"
    );
}

#[tokio::test]
async fn an_owned_child_can_complete_three_required_identical_checks() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("source.txt"), "unchanged source").unwrap();
    let client = Arc::new(GoalClient::default());
    client.scripted.lock().unwrap().extend((1..=3).map(repeat));
    client.scripted.lock().unwrap().push_back(productive::text(
        "The required source observations are recorded.",
    ));
    let physical = Arc::new(AtomicUsize::new(0));
    let result = tokio::time::timeout(
        Duration::from_secs(5),
        child(
            root.path(),
            client.clone(),
            physical.clone(),
            tokio_util::sync::CancellationToken::new(),
        )
        .run("Read the source exactly three times, then report the result."),
    )
    .await
    .unwrap();
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(physical.load(Ordering::SeqCst), 3);
    assert_eq!(client.calls.load(Ordering::SeqCst), 4);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_delegated_task_stops_the_child_and_returns_its_original_failure() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("source.txt"), "unchanged source").unwrap();
    let (agent, session, client) = fixture_with(
        root.path(),
        true,
        options("goal-run")
            .with_max_tool_rounds(0)
            .with_auto_compact(false)
            .with_manual_delegation_enabled(true)
            .with_auto_delegation_enabled(false)
            .with_worker_agent(crate::subagent::WorkerAgentSpec::read_only(
                "worker",
                "Read only",
            ))
            .with_permission_policy(
                crate::permissions::PermissionPolicy::new().allow_all(&["task", "read"]),
            ),
        Some(policy(true, 1, 4, now_ms() + 60_000)),
    )
    .await;
    let mut responses = vec![productive::text("")];
    responses[0].message.content = vec![crate::llm::ContentBlock::ToolUse {
        id: "delegate-original".into(),
        name: "task".into(),
        input: serde_json::json!({"agent":"worker","description":"read","prompt":"Read source.txt."}),
    }];
    for iteration in 1..=5 {
        let mut response = repeat(iteration);
        response.message.content = vec![crate::llm::ContentBlock::ToolUse {
            id: format!("child-read-{iteration}"),
            name: "read".into(),
            input: serde_json::json!({"file_path":"source.txt"}),
        }];
        responses.push(response);
    }
    responses.push(productive::text(
        "The delegated check failed; the goal remains incomplete.",
    ));
    client.scripted.lock().unwrap().extend(responses);
    let GoalTickAdmission::Admitted(AgentRunSpawn::Started { worker, .. }) = session
        .tick_goal(&request(&session, "wake-child-refusal"))
        .await
        .unwrap()
    else {
        panic!("the parent must start")
    };
    tokio::time::timeout(Duration::from_secs(5), worker)
        .await
        .unwrap()
        .unwrap();
    let calls = client.calls.load(Ordering::SeqCst);
    let facts = settlement::facts(root.path());
    session.close().await;
    agent.close().await;
    eprintln!("DELEGATED_REFUSAL_OBSERVATION model_calls={calls}");
    assert_eq!(calls, 7, "parent, five child attempts, then parent");
    assert!(
        facts.iter().any(|fact| fact.kind == "tool.result"
            && fact.payload.to_string().contains("run.tool_refusal_cycle:")),
        "the child's original failure must reach the parent"
    );
    assert_eq!(
        facts
            .iter()
            .filter(|fact| fact.kind == "a3s.goal.slice.settled")
            .count(),
        1
    );
    assert!(!facts.iter().any(|fact| fact.kind == "a3s.goal.completed"));
}
