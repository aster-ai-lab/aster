use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::Notify;

const CHILD_SYSTEM: &str = "background_child_settlement_before_parent";

#[derive(Default)]
struct BackgroundChildClient {
    return_parent_early: bool,
    parent_calls: AtomicUsize,
    memory_calls: AtomicUsize,
    child_calls: AtomicUsize,
    child_started: Notify,
    parent_ready: Notify,
    release_child: Notify,
    finish_parent: Notify,
}

#[async_trait::async_trait]
impl crate::llm::LlmClient for BackgroundChildClient {
    async fn complete(
        &self,
        _messages: &[Message],
        system: Option<&str>,
        tools: &[crate::llm::ToolDefinition],
    ) -> anyhow::Result<LlmResponse> {
        let mut response = if tools.is_empty()
            && system.is_some_and(|text| {
                text.starts_with("You extract durable, reusable memory for a coding agent.\n")
            }) {
            self.memory_calls.fetch_add(1, Ordering::SeqCst);
            scripted_text_response(r#"{"items":[]}"#)
        } else if system.is_some_and(|system| system.contains(CHILD_SYSTEM)) {
            match self.child_calls.fetch_add(1, Ordering::SeqCst) {
                0 => {
                    self.child_started.notify_one();
                    self.release_child.notified().await;
                    scripted_tool_call_response(
                        "child-read",
                        "read",
                        serde_json::json!({"file_path":"alpha.txt"}),
                    )
                }
                1 => scripted_text_response("ALPHA"),
                _ => anyhow::bail!("unexpected child model call"),
            }
        } else {
            let call = self.parent_calls.fetch_add(1, Ordering::SeqCst);
            match call {
                0 => scripted_tool_call_response(
                    "start-child",
                    "task",
                    serde_json::json!({"tasks":[{
                        "agent":"background-reader", "description":"Read alpha independently",
                        "prompt":"Read alpha.txt and return ALPHA.", "background":true
                    }]}),
                ),
                1 => scripted_tool_call_response(
                    "parent-read",
                    "read",
                    serde_json::json!({"file_path":"beta.txt"}),
                ),
                2 => {
                    self.parent_ready.notify_one();
                    if !self.return_parent_early {
                        self.finish_parent.notified().await;
                    }
                    scripted_text_response("BETA")
                }
                3 if self.return_parent_early => scripted_text_response("FOLLOW_UP"),
                _ => anyhow::bail!("unexpected parent model call"),
            }
        };
        response.usage = TokenUsage {
            prompt_tokens: 10,
            completion_tokens: 1,
            total_tokens: 11,
            ..Default::default()
        };
        Ok(response)
    }

    async fn complete_streaming(
        &self,
        messages: &[Message],
        system: Option<&str>,
        tools: &[crate::llm::ToolDefinition],
        _cancel_token: tokio_util::sync::CancellationToken,
    ) -> anyhow::Result<tokio::sync::mpsc::Receiver<StreamEvent>> {
        let response = self.complete(messages, system, tools).await?;
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        tx.send(StreamEvent::Done(response)).await.unwrap();
        Ok(rx)
    }
}

#[tokio::test]
async fn background_cancellation_persists_terminal_without_new_usage() {
    use crate::store::{FileSessionStore, SessionStore};
    use crate::subagent_task_tracker::SubagentStatus;
    let workspace = tempfile::tempdir().unwrap();
    let storage = tempfile::tempdir().unwrap();
    std::fs::write(workspace.path().join("alpha.txt"), "ALPHA").unwrap();
    std::fs::write(workspace.path().join("beta.txt"), "BETA").unwrap();
    let store = Arc::new(FileSessionStore::new(storage.path()).await.unwrap());
    let client = Arc::new(BackgroundChildClient {
        return_parent_early: true,
        ..Default::default()
    });
    let mut config = test_config();
    config.memory = Some(crate::memory::MemoryConfig {
        llm_extraction: false,
        ..Default::default()
    });
    let agent = Agent::from_config(config).await.unwrap();
    let session = agent
        .build_session(
            workspace.path().to_str().unwrap().to_owned(),
            client.clone(),
            &SessionOptions::new()
                .with_session_id("background-terminal-durable")
                .with_session_store(store)
                .with_auto_save(true)
                .with_planning_mode(crate::PlanningMode::Disabled)
                .with_auto_delegation_enabled(false)
                .with_manual_delegation_enabled(true)
                .with_permission_policy(crate::permissions::PermissionPolicy::new().allow("*"))
                .with_worker_agent(
                    crate::subagent::WorkerAgentSpec::custom(
                        "background-reader",
                        "Independent reader",
                    )
                    .with_prompt(CHILD_SYSTEM)
                    .with_max_steps(3),
                ),
        )
        .unwrap();
    let result = session
        .send(
            "Start reader, read beta and return while the child runs.",
            None,
        )
        .await
        .unwrap();
    assert_eq!(result.text, "BETA");
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        client.child_started.notified(),
    )
    .await
    .unwrap();
    let task = session
        .pending_subagent_tasks()
        .await
        .into_iter()
        .next()
        .unwrap();
    let usage = session.recorded_usage_tokens();
    assert!(session.cancel_subagent_task(&task.task_id).await);
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while session.background_tasks.live().contains_key(&task.task_id) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("original background owner settles");
    let reopened = FileSessionStore::new(storage.path()).await.unwrap();
    let saved = reopened
        .load_snapshot(session.session_id())
        .await
        .unwrap()
        .unwrap();
    let after = session.recorded_usage_tokens();
    session.close().await;
    agent.close().await;
    assert_eq!(
        usage, after,
        "terminal persistence needs no invented model usage"
    );
    assert_eq!(saved.subagent_tasks.len(), 1);
    assert_eq!(saved.subagent_tasks[0].status, SubagentStatus::Cancelled);
    assert!(saved.subagent_tasks[0].finished_ms.is_some());
    assert_eq!(saved.session.total_usage.total_tokens as u64, after.2);
}

#[tokio::test]
async fn completed_background_child_settles_before_parent_terminal_and_saves_usage() {
    use crate::subagent_task_tracker::SubagentStatus;
    let workspace = tempfile::tempdir().unwrap();
    let root = workspace.path().join("background 空 格");
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("alpha.txt"), "ALPHA").unwrap();
    std::fs::write(root.join("beta.txt"), "BETA").unwrap();
    let store_dir = tempfile::tempdir().unwrap();
    let store = Arc::new(
        crate::store::FileSessionStore::new(store_dir.path())
            .await
            .unwrap(),
    );
    let client = Arc::new(BackgroundChildClient::default());
    let agent = Agent::from_config(test_config()).await.unwrap();
    let session = Arc::new(
        agent
            .build_session(
                root.to_str().unwrap().to_owned(),
                client.clone(),
                &SessionOptions::new()
                    .with_session_id("completed-background-child")
                    .with_session_store(store.clone())
                    .with_auto_save(true)
                    .with_planning_mode(crate::prompts::PlanningMode::Disabled)
                    .with_auto_delegation_enabled(false)
                    .with_manual_delegation_enabled(true)
                    .with_permission_policy(crate::permissions::PermissionPolicy::new().allow("*"))
                    .with_worker_agent(
                        crate::subagent::WorkerAgentSpec::custom(
                            "background-reader",
                            "Read an independent file",
                        )
                        .with_prompt(CHILD_SYSTEM)
                        .with_max_steps(3),
                    ),
            )
            .unwrap(),
    );
    let running = Arc::clone(&session);
    let join = tokio::spawn(async move {
        running
            .send("Start reader, read beta, then report.", None)
            .await
    });
    tokio::time::timeout(
        std::time::Duration::from_secs(10),
        client.parent_ready.notified(),
    )
    .await
    .unwrap();
    client.release_child.notify_one();
    let child = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            let tasks = session.subagent_tasks().await;
            if let Some(task) = tasks.into_iter().find(|task| {
                task.status == SubagentStatus::Completed
                    && crate::porcelain::peek_settled_workspace_child(&task.task_id).is_some()
            }) {
                break task;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("child and workspace owner must settle while parent is still blocked");
    assert_eq!(child.success, Some(true));
    use aster_effect::LogStore;
    let child_log = aster_effect::FileLog::open(
        crate::session_folder::child_effect_log_dir(&root, &child.child_session_id).unwrap(),
    )
    .unwrap();
    let child_facts = child_log
        .read(&crate::fact_control::thread_for_session(
            &child.child_session_id,
        ))
        .unwrap();
    assert!(child_facts.iter().any(|fact| fact.kind == "turn.complete"));
    assert_eq!(
        child_facts
            .iter()
            .filter(|fact| fact.kind == "tool.prepared"
                && fact.payload["prepared"]["tool_name"] == "read")
            .count(),
        1
    );
    assert_eq!(child.output.as_deref(), Some("ALPHA"));
    assert!(
        !join.is_finished(),
        "the parent must remain active until this observation"
    );
    client.finish_parent.notify_one();
    let result = tokio::time::timeout(std::time::Duration::from_secs(10), join)
        .await
        .unwrap()
        .unwrap();
    let facts = crate::fact_control::read_workspace_facts(
        &root,
        &crate::fact_control::thread_for_session(session.id()),
    )
    .unwrap();
    if let Err(error) = &result {
        eprintln!("COMPLETED_CHILD_PARENT_ERROR {error:#}; child_status={:?}; model_calls=parent:{},child:{}; parent_facts={:?}", child.status,
            client.parent_calls.load(Ordering::SeqCst),client.child_calls.load(Ordering::SeqCst),facts.iter().map(|fact| &fact.kind).collect::<Vec<_>>());
    }
    assert_eq!(
        result
            .expect("a genuinely completed background child must not leave execution debt")
            .text,
        "BETA"
    );
    assert!(facts.iter().any(|fact| fact.kind == "turn.complete"));
    let terminal_index = facts
        .iter()
        .position(|fact| fact.kind == "turn.complete")
        .unwrap();
    let prefix = &facts[..terminal_index];
    assert!(!crate::fact_control::has_unsettled_effects(prefix));
    let completion = prefix
        .iter()
        .find(|fact| fact.kind == "subagent.settled")
        .unwrap();
    let child_cut = aster_effect::cut_log(&child_facts);
    assert_eq!(
        completion.payload["child_log_cut"],
        serde_json::json!({"seq":child_cut.seq,"digest":child_cut.digest})
    );
    assert!(facts[terminal_index].payload["background_receipt_keys"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!(completion.key)));
    let missing = prefix
        .iter()
        .filter(|fact| fact.kind != "subagent.settled")
        .cloned()
        .collect::<Vec<_>>();
    assert!(crate::fact_control::has_unsettled_effects(&missing));
    for (field, replacement) in [
        (
            "/background_subagent/parent_key",
            serde_json::json!("another-parent"),
        ),
        (
            "/background_subagent/task_id",
            serde_json::json!("another-task"),
        ),
        (
            "/background_subagent/child_session_id",
            serde_json::json!("another-child"),
        ),
        ("/child_log_cut/seq", serde_json::json!(0)),
        ("/child_log_cut/digest", serde_json::json!("malformed")),
        (
            "/effects",
            serde_json::json!(aster_effect::ToolEffectObservationV1::incomplete(
                "observation unavailable"
            )),
        ),
        (
            "/effects/unresolved_external_operations",
            serde_json::json!(["unknown-external-operation"]),
        ),
    ] {
        let mut invalid = prefix.to_vec();
        let completion = invalid
            .iter_mut()
            .find(|fact| fact.kind == "subagent.settled")
            .unwrap();
        let (parent, name) = field.rsplit_once('/').unwrap();
        completion.payload.pointer_mut(parent).unwrap()[name] = replacement;
        assert!(
            crate::fact_control::has_unsettled_effects(&invalid),
            "{field} must not settle a child"
        );
    }
    session.close().await;
    let reported = 55 + 11 * client.memory_calls.load(Ordering::SeqCst) as u64;
    assert_eq!(session.recorded_usage_tokens().2, reported);
    let saved = store
        .load_snapshot("completed-background-child")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        u64::try_from(saved.session.total_usage.total_tokens).unwrap(),
        reported
    );
    assert_eq!(
        std::fs::read_to_string(root.join("alpha.txt")).unwrap(),
        "ALPHA"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("beta.txt")).unwrap(),
        "BETA"
    );
    agent.close().await;
}

#[tokio::test]
async fn background_child_survives_parent_answer_and_followup() {
    use crate::subagent_task_tracker::SubagentStatus;
    let workspace = tempfile::tempdir().unwrap();
    let root = workspace.path().join("background session 空 格");
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("alpha.txt"), "ALPHA").unwrap();
    std::fs::write(root.join("beta.txt"), "BETA").unwrap();
    let client = Arc::new(BackgroundChildClient {
        return_parent_early: true,
        ..Default::default()
    });
    let agent = Agent::from_config(test_config()).await.unwrap();
    let session = agent
        .build_session(
            root.to_str().unwrap().to_owned(),
            client.clone(),
            &SessionOptions::new()
                .with_session_id("background-across-parent")
                .with_planning_mode(crate::prompts::PlanningMode::Disabled)
                .with_auto_delegation_enabled(false)
                .with_manual_delegation_enabled(true)
                .with_permission_policy(crate::permissions::PermissionPolicy::new().allow("*"))
                .with_worker_agent(
                    crate::subagent::WorkerAgentSpec::custom(
                        "background-reader",
                        "Read an independent file",
                    )
                    .with_prompt(CHILD_SYSTEM)
                    .with_max_steps(3),
                ),
        )
        .unwrap();
    let first = tokio::time::timeout(
        std::time::Duration::from_secs(15),
        session.send(
            "Start reader, read beta and return; child continues independently.",
            None,
        ),
    )
    .await
    .expect("parent must not wait for the blocked child");
    if let Err(error) = &first {
        eprintln!(
            "CROSS_PARENT_RED {error:#}; parent_calls={}; child_calls={}; tasks={:?}",
            client.parent_calls.load(Ordering::SeqCst),
            client.child_calls.load(Ordering::SeqCst),
            session.subagent_tasks().await
        );
    }
    assert_eq!(
        first
            .expect("normal parent answer must survive a live admitted background child")
            .text,
        "BETA"
    );
    let pending_facts = crate::fact_control::read_workspace_facts(
        &root,
        &crate::fact_control::thread_for_session(session.id()),
    )
    .unwrap();
    assert!(pending_facts.iter().any(|fact| fact.kind == "turn.yielded"));
    assert!(!pending_facts
        .iter()
        .any(|fact| fact.kind == "turn.complete"));
    assert!(crate::fact_control::has_unsettled_effects(&pending_facts));
    let transcript = session
        .transcript_page(Default::default())
        .expect("yielded parent history remains readable");
    assert!(transcript
        .entries
        .iter()
        .any(|entry| matches!(&entry.payload,
        crate::store::TranscriptEntryPayload::Assistant { text } if text == "BETA")));
    assert!(!transcript.entries.iter().any(|entry| matches!(
        &entry.payload,
        crate::store::TranscriptEntryPayload::TurnComplete { .. }
    )));
    let tasks = session.pending_subagent_tasks().await;
    eprintln!(
        "CROSS_PARENT_RUNNING_OBSERVATION parent_calls={} child_calls={} tasks={:?}",
        client.parent_calls.load(Ordering::SeqCst),
        client.child_calls.load(Ordering::SeqCst),
        session.subagent_tasks().await
    );
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].status, SubagentStatus::Running);
    assert_eq!(tasks[0].parent_session_id, session.id());
    let task_id = tasks[0].task_id.clone();
    let followup = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        session.send(
            "A separate follow-up while the same child still runs.",
            None,
        ),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(followup.text, "FOLLOW_UP");
    assert_eq!(
        session.subagent_task(&task_id).await.unwrap().status,
        SubagentStatus::Running
    );
    client.release_child.notify_one();
    let child = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let child = session.subagent_task(&task_id).await.unwrap();
            if child.status != SubagentStatus::Running
                && crate::porcelain::peek_settled_workspace_child(&task_id).is_some()
            {
                break child;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(child.status, SubagentStatus::Completed);
    assert_eq!(child.output.as_deref(), Some("ALPHA"));
    assert_eq!(client.child_calls.load(Ordering::SeqCst), 2);
    session.close().await;
    assert_eq!(client.parent_calls.load(Ordering::SeqCst), 4);
    let reported = 66 + 11 * client.memory_calls.load(Ordering::SeqCst) as u64;
    assert_eq!(session.recorded_usage_tokens().2, reported);
    assert!(session.pending_subagent_tasks().await.is_empty());
    agent.close().await;
}

#[tokio::test]
async fn rejected_session_background_launch_has_no_running_row_or_canceller() {
    let workspace = tempfile::tempdir().unwrap();
    std::fs::write(workspace.path().join("alpha.txt"), "ALPHA").unwrap();
    std::fs::write(workspace.path().join("beta.txt"), "BETA").unwrap();
    let client = Arc::new(BackgroundChildClient {
        return_parent_early: true,
        ..Default::default()
    });
    let agent = Agent::from_config(test_config()).await.unwrap();
    let session = agent
        .build_session(
            workspace.path().to_str().unwrap().to_owned(),
            client.clone(),
            &SessionOptions::new()
                .with_session_id("background-admission-closed")
                .with_planning_mode(crate::prompts::PlanningMode::Disabled)
                .with_auto_delegation_enabled(false)
                .with_manual_delegation_enabled(true)
                .with_permission_policy(crate::permissions::PermissionPolicy::new().allow("*"))
                .with_worker_agent(
                    crate::subagent::WorkerAgentSpec::custom(
                        "background-reader",
                        "Read an independent file",
                    )
                    .with_prompt(CHILD_SYSTEM)
                    .with_max_steps(3),
                ),
        )
        .unwrap();
    session.background_tasks.close().await.unwrap();
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        session.send("Read beta; a background launch may be refused.", None),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(result.text, "BETA");
    let tasks = session.subagent_tasks().await;
    assert_eq!(tasks.len(), 1);
    assert_eq!(
        tasks[0].status,
        crate::subagent_task_tracker::SubagentStatus::Failed
    );
    assert!(session.pending_subagent_tasks().await.is_empty());
    assert!(!session.cancel_subagent_task(&tasks[0].task_id).await);
    assert!(!crate::porcelain::workspace_child_pending(
        &tasks[0].task_id
    ));
    assert_eq!(client.child_calls.load(Ordering::SeqCst), 0);
    session.close().await;
    agent.close().await;
}

#[tokio::test]
async fn live_background_does_not_forgive_another_prepared_invocation() {
    use aster_effect::LogStore;
    let workspace = tempfile::tempdir().unwrap();
    std::fs::write(workspace.path().join("alpha.txt"), "ALPHA").unwrap();
    std::fs::write(workspace.path().join("beta.txt"), "BETA").unwrap();
    let client = Arc::new(BackgroundChildClient::default());
    let agent = Agent::from_config(test_config()).await.unwrap();
    let session = Arc::new(
        agent
            .build_session(
                workspace.path().to_str().unwrap().to_owned(),
                client.clone(),
                &SessionOptions::new()
                    .with_session_id("background-other-debt")
                    .with_planning_mode(crate::prompts::PlanningMode::Disabled)
                    .with_auto_delegation_enabled(false)
                    .with_manual_delegation_enabled(true)
                    .with_permission_policy(crate::permissions::PermissionPolicy::new().allow("*"))
                    .with_worker_agent(
                        crate::subagent::WorkerAgentSpec::custom(
                            "background-reader",
                            "Read an independent file",
                        )
                        .with_prompt(CHILD_SYSTEM)
                        .with_max_steps(3),
                    ),
            )
            .unwrap(),
    );
    let running = session.clone();
    let join = tokio::spawn(async move {
        running
            .send("Start reader, read beta, then report.", None)
            .await
    });
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        client.parent_ready.notified(),
    )
    .await
    .unwrap();
    let thread = crate::fact_control::thread_for_session(session.id());
    let shared = session.run_admission.fact_log_slot();
    let log = shared.lock().unwrap().as_ref().unwrap().clone();
    let facts = log.read(&thread).unwrap();
    let original = facts
        .iter()
        .find(|fact| {
            fact.kind == "tool.prepared" && fact.payload["prepared"]["tool_name"] == "read"
        })
        .unwrap();
    let mut pending: aster_effect::ToolPreparedV1 =
        serde_json::from_value(original.payload["prepared"].clone()).unwrap();
    pending.identity.tool_call_id = "separate-unobserved-call".into();
    pending.validate().unwrap();
    let key = pending.identity.key().unwrap();
    log.append(
        &thread,
        &[aster_effect::NewFact {
            kind: "tool.prepared".into(),
            key: format!("tool-prepared:{key}"),
            payload: serde_json::json!({"prepared":pending}),
        }],
        None,
    )
    .unwrap();
    client.finish_parent.notify_one();
    let result = tokio::time::timeout(std::time::Duration::from_secs(5), join)
        .await
        .unwrap()
        .unwrap();
    assert!(
        result.is_err(),
        "a live background child cannot waive another unknown invocation"
    );
    let facts = log.read(&thread).unwrap();
    assert!(crate::fact_control::has_unsettled_effects(&facts));
    assert!(!facts
        .iter()
        .any(|fact| fact.kind == "turn.complete" || fact.kind == "turn.yielded"));
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while !session.background_tasks.live().is_empty() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("parent failure must cancel its original background group");
    assert!(session.pending_subagent_tasks().await.is_empty());
    session.close().await;
    agent.close().await;
}
