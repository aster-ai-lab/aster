use super::*;
use crate::AgentRegistry;

#[derive(Debug, Clone)]
struct ModelRequest {
    tool_count: usize,
    text: String,
    system: String,
}

struct ReadOnlyAnalysisClient {
    inner: ScriptedStreamingClient,
    intent: &'static str,
    pre_analysis_calls: Arc<std::sync::atomic::AtomicUsize>,
    requests: Arc<std::sync::Mutex<Vec<ModelRequest>>>,
}

impl ReadOnlyAnalysisClient {
    fn new(intent: &'static str, responses: Vec<LlmResponse>) -> Self {
        Self {
            inner: ScriptedStreamingClient::new(responses),
            intent,
            pre_analysis_calls: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            requests: Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }
}

#[async_trait::async_trait]
impl LlmClient for ReadOnlyAnalysisClient {
    async fn complete(
        &self,
        messages: &[Message],
        system: Option<&str>,
        _tools: &[crate::llm::ToolDefinition],
    ) -> anyhow::Result<LlmResponse> {
        if system.is_some_and(|value| value.contains(crate::prompts::PRE_ANALYSIS_SYSTEM)) {
            self.pre_analysis_calls
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let prompt = messages.last().map(Message::text).unwrap_or_default();
            return Ok(scripted_text_response(
                &serde_json::json!({
                    "intent": self.intent,
                    "requires_planning": false,
                    "goal": {"description": prompt, "success_criteria": []},
                    "execution_plan": {
                        "complexity": "Simple",
                        "steps": [],
                        "required_tools": []
                    },
                    "optimized_input": prompt
                })
                .to_string(),
            ));
        }
        self.inner.next_response()
    }

    async fn complete_streaming(
        &self,
        messages: &[Message],
        system: Option<&str>,
        tools: &[crate::llm::ToolDefinition],
        _cancel_token: tokio_util::sync::CancellationToken,
    ) -> anyhow::Result<mpsc::Receiver<StreamEvent>> {
        self.requests.lock().unwrap().push(ModelRequest {
            tool_count: tools.len(),
            system: system.unwrap_or_default().to_owned(),
            text: messages
                .iter()
                .map(Message::text)
                .collect::<Vec<_>>()
                .join("\n"),
        });
        let response = self.inner.next_response()?;
        let (tx, rx) = mpsc::channel(8);
        tokio::spawn(async move {
            let text = response.text();
            if !text.is_empty() {
                let _ = tx.send(StreamEvent::TextDelta(text)).await;
            }
            let _ = tx.send(StreamEvent::Done(response)).await;
        });
        Ok(rx)
    }
}

#[tokio::test]
async fn empty_catalog_recovers_a_leaked_call_without_inventing_a_tool_receipt() {
    let leaked = "<｜DSML｜invoke name=\"ls\"><｜DSML｜parameter name=\"path\" string=\"true\">.</｜DSML｜parameter></｜DSML｜invoke>";
    let final_text =
        format!("Here is the example:\n\n```xml\n{leaked}\n```\n\nInline: `{leaked}`.");
    let mut private_attempt = scripted_text_response("");
    private_attempt.message.reasoning_content = Some(leaked.to_owned());
    for (first, retry, fail) in [
        (scripted_text_response(&final_text), false, false),
        (scripted_text_response(leaked), true, false),
        (private_attempt, true, false),
        (
            scripted_tool_call_response("unavailable", "ls", serde_json::json!({})),
            true,
            true,
        ),
    ] {
        let root = tempfile::tempdir().unwrap();
        let replacement = if fail { leaked } else { &final_text };
        let client = Arc::new(ReadOnlyAnalysisClient::new(
            "GeneralPurpose",
            vec![first, scripted_text_response(replacement)],
        ));
        let agent = Agent::from_config(test_config()).await.unwrap();
        let session = agent
            .session_async(
                root.path().display().to_string(),
                Some(
                    SessionOptions::new()
                        .with_llm_client(client.clone())
                        .with_planning_mode(PlanningMode::Disabled)
                        .with_tool_presentation_profile(
                            crate::tools::ToolPresentationProfileV1::disabled(),
                        ),
                ),
            )
            .await
            .unwrap();
        let result = session
            .send("Explain the provided markup in normal text.", None)
            .await;
        if fail {
            assert!(result.unwrap_err().to_string().contains("invocation twice"));
        } else {
            let result = result.unwrap();
            assert_eq!(result.text, final_text);
            assert_eq!(result.tool_calls_count, 0);
        }
        {
            let requests = client.requests.lock().unwrap();
            assert_eq!(requests.len(), if retry { 2 } else { 1 });
            assert!(requests.iter().all(|request| request.tool_count == 0));
            assert!(requests[0]
                .system
                .contains("No tools are available for this request."));
            if retry {
                assert!(requests[1]
                    .text
                    .contains("previous model attempt proposed a tool"));
                assert!(!requests[1].text.contains(leaked));
            }
        }
        let facts =
            crate::fact_control::read_workspace_facts(root.path(), session.session_id()).unwrap();
        assert!(!facts.iter().any(|fact| fact.kind.starts_with("tool.")));
        session.close().await;
        agent.close().await;
    }
}

#[tokio::test]
async fn read_only_analysis_allows_one_gap_wave_then_settles_without_tools() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("main.rs"), "fn main() {}\n").unwrap();
    std::fs::write(root.path().join("lib.rs"), "pub fn module_entry() {}\n").unwrap();
    std::fs::write(root.path().join("README.md"), "architecture fixture\n").unwrap();
    std::fs::write(root.path().join("gap.rs"), "pub fn named_gap() {}\n").unwrap();

    let mut first_wave = scripted_tool_call_response(
        "read-main",
        "read",
        serde_json::json!({"file_path":"main.rs"}),
    );
    first_wave.message.content.push(ContentBlock::ToolUse {
        id: "read-lib".into(),
        name: "read".into(),
        input: serde_json::json!({"file_path":"lib.rs"}),
    });
    let client = Arc::new(ReadOnlyAnalysisClient::new(
        "Plan",
        vec![
            first_wave,
            scripted_tool_call_response(
                "read-readme",
                "read",
                serde_json::json!({"file_path":"README.md"}),
            ),
            scripted_tool_call_response(
                "read-gap",
                "read",
                serde_json::json!({"file_path":"gap.rs"}),
            ),
            scripted_text_response(
                "Architecture: main.rs enters the crate; lib.rs exposes the module; gap.rs closes the named gap.",
            ),
        ],
    ));
    let agent = Agent::from_config(test_config()).await.unwrap();
    let options = SessionOptions::new()
        .with_llm_client(client.clone())
        .with_planning_mode(PlanningMode::Disabled)
        .with_confirmation_policy(crate::hitl::ConfirmationPolicy::enabled())
        .with_permission_policy(crate::permissions::PermissionPolicy::new().allow("read"))
        .with_read_only_synthesis_rounds(2)
        .with_auto_compact(false);
    let session = agent
        .session_async(root.path().display().to_string(), Some(options))
        .await
        .unwrap();
    assert_eq!(session.config.read_only_synthesis_rounds, Some(2));
    assert_eq!(session.config.planning_mode, PlanningMode::Disabled);
    let result = session
        .send("Give a supported architecture overview", None)
        .await
        .unwrap();

    assert_eq!(
        result.text,
        "Architecture: main.rs enters the crate; lib.rs exposes the module; gap.rs closes the named gap."
    );
    assert_eq!(result.tool_calls_count, 4);
    assert_eq!(
        client
            .pre_analysis_calls
            .load(std::sync::atomic::Ordering::SeqCst),
        1
    );
    let requests = client.requests.lock().unwrap();
    assert_eq!(requests.len(), 4);
    assert!(requests[2].tool_count > 0);
    assert!(
        requests[2].text.contains("interactive exploration budget"),
        "{requests:?}"
    );
    assert_eq!(requests[3].tool_count, 0);
    assert!(
        requests[3].text.contains("named-gap exploration wave"),
        "{requests:?}"
    );
    drop(requests);
    let history = serde_json::to_string(&session.history()).unwrap();
    assert!(history.contains("fn main()"));
    assert!(history.contains("pub fn module_entry()"));
    assert!(history.contains("architecture fixture"));
    assert!(history.contains("pub fn named_gap()"));
    session.close().await;
    agent.close().await;
}

#[tokio::test]
async fn general_purpose_coding_intent_never_activates_staged_synthesis() {
    for planning_mode in [PlanningMode::Disabled, PlanningMode::Enabled] {
        let root = tempfile::tempdir().unwrap();
        for name in ["one.rs", "two.rs", "three.rs"] {
            std::fs::write(root.path().join(name), format!("// {name}\n")).unwrap();
        }
        let client = Arc::new(ReadOnlyAnalysisClient::new(
            "GeneralPurpose",
            vec![
                scripted_tool_call_response(
                    "read-one",
                    "read",
                    serde_json::json!({"file_path":"one.rs"}),
                ),
                scripted_tool_call_response(
                    "read-two",
                    "read",
                    serde_json::json!({"file_path":"two.rs"}),
                ),
                scripted_tool_call_response(
                    "read-three",
                    "read",
                    serde_json::json!({"file_path":"three.rs"}),
                ),
                scripted_text_response("Coding inspection complete."),
            ],
        ));
        let agent = Agent::from_config(test_config()).await.unwrap();
        let options = SessionOptions::new()
            .with_llm_client(client.clone())
            .with_planning_mode(planning_mode)
            .with_confirmation_policy(crate::hitl::ConfirmationPolicy::enabled())
            .with_permission_policy(crate::permissions::PermissionPolicy::new().allow("read"))
            .with_read_only_synthesis_rounds(2)
            .with_auto_compact(false);
        let session = agent
            .session_async(root.path().display().to_string(), Some(options))
            .await
            .unwrap();
        let result = session
            .send("Inspect these files before a code change", None)
            .await
            .unwrap();

        assert_eq!(result.text, "Coding inspection complete.");
        assert_eq!(result.tool_calls_count, 3);
        let requests = client.requests.lock().unwrap();
        assert_eq!(requests.len(), 4);
        assert!(requests.iter().all(|request| request.tool_count > 0));
        assert!(requests
            .iter()
            .all(|request| !request.text.contains("exploration budget")));
        drop(requests);
        session.close().await;
        agent.close().await;
    }
}

#[test]
fn staged_synthesis_checkpoint_restores_the_finalization_boundary() {
    let root = tempfile::tempdir().unwrap();
    let client = Arc::new(ReadOnlyAnalysisClient::new("Plan", Vec::new()));
    let executor = Arc::new(crate::tools::ToolExecutor::new(
        root.path().display().to_string(),
    ));
    let config = crate::agent::AgentConfig {
        read_only_synthesis_rounds: Some(2),
        planning_mode: PlanningMode::Disabled,
        ..Default::default()
    };
    let agent = crate::agent::AgentLoop::new(
        client.clone(),
        executor,
        crate::tools::ToolContext::new(root.path().to_path_buf()),
        config,
    );
    agent.fact_bind_read_only_synthesis(crate::prompts::AgentStyle::Plan);
    agent.fact_observe_read_only_synthesis_tools();
    agent.fact_observe_read_only_synthesis_tools();
    assert_eq!(
        agent.fact_read_only_synthesis_directive(),
        Some(crate::agent::FactReadOnlySynthesisDirective::Synthesize)
    );
    agent.fact_observe_read_only_synthesis_tools();

    let checkpoint = agent.fact_convergence_snapshot();
    let encoded = serde_json::to_string(&checkpoint).unwrap();
    let decoded: crate::loop_checkpoint::LoopConvergenceState =
        serde_json::from_str(&encoded).unwrap();
    let restored = crate::agent::AgentLoop::new(
        client,
        Arc::new(crate::tools::ToolExecutor::new(
            root.path().display().to_string(),
        )),
        crate::tools::ToolContext::new(root.path().to_path_buf()),
        crate::agent::AgentConfig {
            read_only_synthesis_rounds: Some(2),
            planning_mode: PlanningMode::Disabled,
            ..Default::default()
        },
    );
    restored.fact_restore_read_only_synthesis(&decoded);
    assert_eq!(
        restored.fact_read_only_synthesis_directive(),
        Some(crate::agent::FactReadOnlySynthesisDirective::Finalize)
    );
}

#[tokio::test]
async fn built_in_explore_runs_through_real_task_delegation() {
    let root = tempfile::tempdir().unwrap();
    let note = root.path().join("note.txt");
    std::fs::write(&note, "bounded child evidence\n").unwrap();
    let client = Arc::new(ScriptedStreamingClient::new(vec![
        scripted_tool_call_response(
            "child-read",
            "read",
            serde_json::json!({"file_path":"note.txt"}),
        ),
        scripted_text_response("Child finding: note.txt contains bounded child evidence."),
    ]));
    let agent = Agent::from_config(test_config()).await.unwrap();
    let options = SessionOptions::new()
        .with_llm_client(client.clone())
        .with_planning_mode(PlanningMode::Disabled)
        .with_confirmation_policy(crate::hitl::ConfirmationPolicy::enabled())
        .with_permission_policy(crate::permissions::PermissionPolicy::new().allow("read"));
    let session = agent
        .session_async(root.path().display().to_string(), Some(options))
        .await
        .unwrap();
    let result = session
        .tool(
            "task",
            serde_json::json!({
                "agent":"explore",
                "description":"Inspect one architecture fixture",
                "prompt":"Read note.txt and return its content as evidence."
            }),
        )
        .await
        .unwrap();

    assert_eq!(result.exit_code, 0, "{}", result.output);
    assert!(result.output.contains("Child finding"));
    assert!(client.responses.lock().unwrap().is_empty());
    assert_eq!(
        std::fs::read_to_string(note).unwrap(),
        "bounded child evidence\n"
    );
    let explore = AgentRegistry::new().get("explore").unwrap();
    assert_eq!(explore.max_steps, Some(8));
    assert_eq!(
        explore
            .permissions
            .check("write", &serde_json::json!({"file_path":"x","content":"y"})),
        crate::permissions::PermissionDecision::Deny
    );
    session.close().await;
    agent.close().await;
}

#[tokio::test]
async fn disabled_planning_defers_intent_classification_until_synthesis_budget() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("note.txt"), "one-wave evidence\n").unwrap();

    // 0-round and 1-round turns never trigger the auxiliary classify_intent call.
    let zero_round_client = Arc::new(ReadOnlyAnalysisClient::new(
        "Plan",
        vec![scripted_text_response("Direct answer without tools.")],
    ));
    let one_round_client = Arc::new(ReadOnlyAnalysisClient::new(
        "Plan",
        vec![
            scripted_tool_call_response(
                "read-note",
                "read",
                serde_json::json!({"file_path":"note.txt"}),
            ),
            scripted_text_response("Answered after one read wave."),
        ],
    ));

    for (client, prompt, expected_text, expected_tools) in [
        (
            zero_round_client.clone(),
            "Say hello",
            "Direct answer without tools.",
            0usize,
        ),
        (
            one_round_client.clone(),
            "Read note.txt and summarize",
            "Answered after one read wave.",
            1usize,
        ),
    ] {
        let agent = Agent::from_config(test_config()).await.unwrap();
        let options = SessionOptions::new()
            .with_llm_client(client.clone())
            .with_planning_mode(PlanningMode::Disabled)
            .with_confirmation_policy(crate::hitl::ConfirmationPolicy::enabled())
            .with_permission_policy(crate::permissions::PermissionPolicy::new().allow("read"))
            .with_read_only_synthesis_rounds(2)
            .with_auto_compact(false);
        let session = agent
            .session_async(root.path().display().to_string(), Some(options))
            .await
            .unwrap();
        let result = session.send(prompt, None).await.unwrap();
        assert_eq!(result.text, expected_text);
        assert_eq!(result.tool_calls_count, expected_tools);
        assert_eq!(
            client
                .pre_analysis_calls
                .load(std::sync::atomic::Ordering::SeqCst),
            0,
            "turns below synthesis budget must not pay an intent-classification call"
        );
        session.close().await;
        agent.close().await;
    }
}

#[tokio::test]
async fn settled_workspace_change_skips_lazy_classification_for_its_run_only() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("notes.txt"), "original\n").unwrap();
    std::fs::write(root.path().join("guide.txt"), "separate evidence\n").unwrap();
    let read_notes =
        |id| scripted_tool_call_response(id, "read", serde_json::json!({"file_path":"notes.txt"}));
    let client = Arc::new(ReadOnlyAnalysisClient::new(
        "Plan",
        vec![
            read_notes("read-before-edit"),
            scripted_tool_call_response(
                "write-notes",
                "write",
                serde_json::json!({"file_path":"notes.txt","content":"updated\n"}),
            ),
            scripted_text_response("The file is updated."),
            read_notes("read-after-edit"),
            scripted_tool_call_response(
                "read-guide",
                "read",
                serde_json::json!({"file_path":"guide.txt"}),
            ),
            scripted_text_response("The requested observation is complete."),
        ],
    ));
    let mut config = test_config();
    config.memory = Some(crate::memory::MemoryConfig {
        llm_extraction: false,
        ..Default::default()
    });
    let agent = Agent::from_config(config).await.unwrap();
    let session = agent
        .session_async(
            root.path().display().to_string(),
            Some(
                SessionOptions::new()
                    .with_llm_client(client.clone())
                    .with_planning_mode(PlanningMode::Disabled)
                    .with_permission_policy(
                        crate::permissions::PermissionPolicy::new()
                            .allow("read")
                            .allow("write(*)"),
                    )
                    .with_completion_attestor(Arc::new(
                        crate::completion_attestor::DiskContentAttestor::new(root.path()),
                    ))
                    .with_assistant_final_response(true)
                    .with_read_only_synthesis_rounds(2)
                    .with_auto_compact(false),
            ),
        )
        .await
        .unwrap();
    let mutation = session
        .send("Update notes.txt as requested.", None)
        .await
        .unwrap();
    assert_eq!(mutation.text, "The file is updated.");
    assert!(matches!(
        mutation.completion,
        crate::harness_loop::CompletionTerminal::Verified { .. }
    ));
    assert_eq!(
        std::fs::read(root.path().join("notes.txt")).unwrap(),
        b"updated\n"
    );
    let classification_count = || {
        client
            .pre_analysis_calls
            .load(std::sync::atomic::Ordering::SeqCst)
    };
    assert_eq!(classification_count(), 0);

    let observation = session
        .send("Inspect the workspace and report its state.", None)
        .await
        .unwrap();
    assert_eq!(observation.text, "The requested observation is complete.");
    assert_eq!(observation.tool_calls_count, 2);
    assert_eq!(classification_count(), 1);
    {
        let requests = client.requests.lock().unwrap();
        assert!(!requests[2].text.contains("interactive exploration budget"));
        assert!(requests[5].text.contains("interactive exploration budget"));
    }
    session.close().await;
    agent.close().await;
}
