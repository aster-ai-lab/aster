use super::*;
use crate::tools::{Tool, ToolCapabilities, ToolContext, ToolOutput};
use serde_json::{json, Value};

#[tokio::test]
async fn denied_bash_gives_one_bounded_opportunity_to_change_approach() {
    for alternate in [true, false] {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("source.txt"), "observed source").unwrap();
        let mut config = test_config();
        config.memory = None;
        let agent = crate::Agent::from_config(config).await.unwrap();
        let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let session = session_for(
            &agent,
            root.path(),
            "denied-bash-recovery",
            Arc::new(ScriptedClient {
                seen: Arc::clone(&seen),
                responses: std::sync::Mutex::new(vec![
                    tool_response("first", "bash", json!({"command":"touch first"})),
                    if alternate {
                        tool_response("read", "read", json!({"file_path":"source.txt"}))
                    } else {
                        tool_response("second", "bash", json!({"command":"touch second"}))
                    },
                    text_response("The source is observed."),
                ]),
            }),
            &crate::SessionOptions::new()
                .with_permission_policy(
                    crate::permissions::PermissionPolicy::new()
                        .deny("bash(*)")
                        .allow("read(*)"),
                )
                .with_max_tool_rounds(0)
                .with_auto_compact(false)
                .with_auto_delegation_enabled(false)
                .with_continuation(false),
        );
        let result = session.send("Observe the existing source.", None).await;
        let facts = read_workspace_facts(root.path(), session.session_id()).unwrap();
        session.close().await;
        agent.close().await;

        assert!(!root.path().join("first").exists());
        assert!(!root.path().join("second").exists());
        let requests = seen.lock().unwrap();
        let recovery_requests = requests
            .iter()
            .filter(|request| {
                request.messages.iter().any(|message| {
                    message.role == "user" && message.text().contains("[host tool recovery]")
                })
            })
            .count();
        assert_eq!(recovery_requests, 1, "exactly one recovery instruction");
        if alternate {
            assert!(result.is_ok(), "{result:?}");
            assert!(facts.iter().any(|fact| {
                fact.kind == "tool.result"
                    && fact.payload["receipt"]["outcome"] == "succeeded"
                    && fact.payload["receipt"]["output"]
                        .as_str()
                        .is_some_and(|output| output.contains("observed source"))
            }));
        } else {
            assert!(result
                .unwrap_err()
                .to_string()
                .contains("run.tool_refusal_cycle"));
            assert_eq!(
                requests
                    .iter()
                    .filter(|request| !request.tools.is_empty())
                    .count(),
                2
            );
        }
    }
}

struct CompactingObservations {
    requests: Arc<AtomicUsize>,
    compactions: Arc<AtomicUsize>,
}

#[async_trait]
impl LlmClient for CompactingObservations {
    async fn complete(
        &self,
        _: &[Message],
        system: Option<&str>,
        _: &[ToolDefinition],
    ) -> anyhow::Result<LlmResponse> {
        if system == Some(crate::compaction::compaction_system_prompt()) {
            self.compactions.fetch_add(1, Ordering::SeqCst);
            return Ok(text_response(
                "The unchanged external state has already been observed.",
            ));
        }
        let index = self.requests.fetch_add(1, Ordering::SeqCst);
        let mut response = if index < 7 {
            tool_response(&format!("observe-{index}"), "convergence_stable", json!({}))
        } else {
            text_response("done")
        };
        response.usage = TokenUsage {
            prompt_tokens: 14_000,
            completion_tokens: 1,
            total_tokens: 14_001,
            ..Default::default()
        };
        Ok(response)
    }
    async fn complete_streaming(
        &self,
        _: &[Message],
        _: Option<&str>,
        _: &[ToolDefinition],
        _: CancellationToken,
    ) -> anyhow::Result<mpsc::Receiver<StreamEvent>> {
        anyhow::bail!("controlled non-streaming fixture")
    }
}

struct StableObservation(Arc<AtomicUsize>);

#[async_trait]
impl Tool for StableObservation {
    fn name(&self) -> &str {
        "convergence_stable"
    }
    fn description(&self) -> &str {
        "Read the unchanged external observation."
    }
    fn parameters(&self) -> Value {
        json!({"type":"object","properties":{},"additionalProperties":false})
    }
    fn capabilities(&self, _: &Value) -> ToolCapabilities {
        ToolCapabilities::read_only_paginated(1)
    }
    async fn execute(&self, _: &Value, _: &ToolContext) -> anyhow::Result<ToolOutput> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(ToolOutput::success(
            json!({"state":"unchanged","padding":"x".repeat(20_000)}).to_string(),
        ))
    }
}

struct FreshObservation(Arc<AtomicUsize>);

#[async_trait]
impl Tool for FreshObservation {
    fn name(&self) -> &str {
        "convergence_observe"
    }
    fn description(&self) -> &str {
        "Read a changing external observation."
    }
    fn parameters(&self) -> Value {
        json!({"type":"object","properties":{},"additionalProperties":false})
    }
    fn capabilities(&self, _: &Value) -> ToolCapabilities {
        ToolCapabilities::read_only_paginated(1)
    }
    async fn execute(&self, _: &Value, _: &ToolContext) -> anyhow::Result<ToolOutput> {
        let index = self.0.fetch_add(1, Ordering::SeqCst);
        Ok(ToolOutput::success(format!("External state {index}")))
    }
}

#[tokio::test]
async fn identical_calls_with_new_external_observations_complete_through_the_session() {
    let root = tempfile::tempdir().unwrap();
    let agent = crate::Agent::from_config(test_config()).await.unwrap();
    let mut responses: Vec<_> = (0..7)
        .map(|index| {
            tool_response(
                &format!("observation-{index}"),
                "convergence_observe",
                json!({}),
            )
        })
        .collect();
    responses.extend([
        text_response("All seven observations are recorded."),
        text_response("[]"),
    ]);
    let session = session_for(
        &agent,
        root.path(),
        "fresh-observations",
        Arc::new(ScriptedClient {
            seen: Default::default(),
            responses: std::sync::Mutex::new(responses),
        }),
        &crate::SessionOptions::new()
            .with_permission_policy(
                crate::permissions::PermissionPolicy::new().allow("convergence_observe(*)"),
            )
            .with_max_tool_rounds(0)
            .with_duplicate_tool_call_threshold(3)
            .with_auto_compact(false)
            .with_auto_delegation_enabled(false)
            .with_continuation(false),
    );
    let executions = Arc::new(AtomicUsize::new(0));
    session
        .register_dynamic_tool(Arc::new(FreshObservation(executions.clone())))
        .unwrap();
    let result = session
        .send("Record seven changing external observations.", None)
        .await;
    let facts = read_workspace_facts(root.path(), "fresh-observations").unwrap();
    session.close().await;
    agent.close().await;
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(executions.load(Ordering::SeqCst), 7);
    assert_eq!(
        facts
            .iter()
            .filter(|fact| fact.kind == "tool.result"
                && fact.payload["receipt"]["outcome"] == "succeeded")
            .count(),
        7
    );
    assert!(!facts
        .iter()
        .any(|fact| fact.kind == "tool.result" && fact.payload["receipt"]["outcome"] == "denied"));
}

#[tokio::test]
async fn repeated_compaction_cannot_refresh_the_original_session_duplicate_allowance() {
    let root = tempfile::tempdir().unwrap();
    let mut config = test_config();
    config.memory = Some(crate::memory::MemoryConfig {
        llm_extraction: false,
        prune_policy: None,
        ..Default::default()
    });
    let agent = crate::Agent::from_config(config).await.unwrap();
    let requests = Arc::new(AtomicUsize::new(0));
    let compactions = Arc::new(AtomicUsize::new(0));
    let executions = Arc::new(AtomicUsize::new(0));
    let session = agent
        .build_session(
            root.path().to_str().unwrap().to_string(),
            Arc::new(CompactingObservations {
                requests: requests.clone(),
                compactions: compactions.clone(),
            }),
            &crate::SessionOptions::new()
                .with_session_id("compacted-observations")
                .with_permission_policy(
                    crate::permissions::PermissionPolicy::new().allow("convergence_stable(*)"),
                )
                .with_max_tool_rounds(0)
                .with_duplicate_tool_call_threshold(3)
                .with_max_context_tokens(16_384)
                .with_auto_compact(true)
                .with_planning(false)
                .with_auto_delegation_enabled(false)
                .with_continuation(false),
        )
        .unwrap();
    session
        .register_dynamic_tool(Arc::new(StableObservation(executions.clone())))
        .unwrap();
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(15),
        session.send("Observe the external state without repeating work.", None),
    )
    .await
    .unwrap();
    let facts = read_workspace_facts(root.path(), "compacted-observations").unwrap();
    session.close().await;
    agent.close().await;
    assert!(
        result.is_err(),
        "a refusal cycle cannot finish successfully: {result:?}"
    );
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("run.tool_refusal_cycle:"));
    assert_eq!(compactions.load(Ordering::SeqCst), 3);
    assert_eq!(executions.load(Ordering::SeqCst), 3);
    assert_eq!(requests.load(Ordering::SeqCst), 5);
    let compacted: Vec<_> = facts
        .iter()
        .filter(|fact| fact.kind == "compaction.done")
        .collect();
    assert!(
        compacted.len() >= 2,
        "actual repeated compaction must be exercised"
    );
    assert!(compacted.iter().any(
        |fact| fact.payload["kept"].as_array().is_some_and(|kept| kept
            .iter()
            .filter_map(Value::as_str)
            .filter(|line| line.starts_with("assistant\ntool "))
            .count()
            == 1)
    ));
}

#[tokio::test]
async fn truncated_mutating_session_keeps_the_stop_without_verifying_or_retrying() {
    for stop in ["length", "max_tokens"] {
        let root = tempfile::tempdir().unwrap();
        let agent = crate::Agent::from_config(test_config()).await.unwrap();
        let mut truncated = text_response("partial answer");
        truncated.stop_reason = Some(stop.into());
        let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let session = session_for(
            &agent,
            root.path(),
            "truncated-mutation",
            Arc::new(ScriptedClient {
                seen: seen.clone(),
                responses: std::sync::Mutex::new(vec![
                    tool_response(
                        "actual-write",
                        "write",
                        json!({"file_path":"partial 甲.txt","content":"partial work"}),
                    ),
                    truncated,
                    text_response("[]"),
                ]),
            }),
            &crate::SessionOptions::new()
                .with_permission_policy(
                    crate::permissions::PermissionPolicy::new().allow("write(*)"),
                )
                .with_auto_delegation_enabled(false)
                .with_auto_compact(false)
                .with_continuation(false),
        );
        let error = session
            .send(
                "Write the requested artifact and finish only with current verification.",
                None,
            )
            .await
            .expect_err("a truncated mutation is not verified");
        let crate::CodeError::Session(message) = error else {
            panic!("preserve the session error contract: {error:?}");
        };
        assert!(message.contains("response_truncated"));
        assert!(message.starts_with("completion gate:"));
        assert_eq!(
            std::fs::read(root.path().join("partial 甲.txt")).unwrap(),
            b"partial work"
        );
        assert_eq!(
            seen.lock()
                .unwrap()
                .iter()
                .filter(|request| request.tools.iter().any(|tool| tool == "write"))
                .count(),
            2
        );
        let facts =
            crate::fact_control::read_workspace_facts(root.path(), session.session_id()).unwrap();
        assert!(!facts.iter().any(|fact| fact.kind == "turn.complete"));
        session.close().await;
        agent.close().await;
    }
}

struct PublicCheckpointClient {
    requests: AtomicUsize,
    summaries: std::sync::Mutex<Vec<String>>,
}

#[async_trait]
impl LlmClient for PublicCheckpointClient {
    async fn complete(
        &self,
        messages: &[Message],
        system: Option<&str>,
        _: &[ToolDefinition],
    ) -> anyhow::Result<LlmResponse> {
        if system == Some(crate::compaction::compaction_system_prompt()) {
            self.summaries.lock().unwrap().push(
                messages
                    .iter()
                    .map(Message::text)
                    .collect::<Vec<_>>()
                    .join("\n"),
            );
            return Ok(text_response("Preserve the original task and continue."));
        }
        let index = self.requests.fetch_add(1, Ordering::SeqCst);
        let mut response = if index < 3 {
            let mut response = tool_response(
                &format!("checkpoint-read-{index}"),
                "convergence_stable",
                json!({}),
            );
            response.message.content.insert(
                0,
                crate::llm::ContentBlock::Text {
                    text: format!(
                        "PUBLIC-CHECKPOINT-{index}: established result = {}\n{}",
                        17 + index,
                        "Original observed checkpoint context. ".repeat(1_000)
                    ),
                },
            );
            response.message.reasoning_content = Some("SYNTHETIC-PRIVATE-REASONING".into());
            response
        } else {
            text_response("All three observations are complete.")
        };
        response.usage = TokenUsage {
            prompt_tokens: 14_000,
            completion_tokens: 1,
            total_tokens: 14_001,
            ..Default::default()
        };
        Ok(response)
    }
    async fn complete_streaming(
        &self,
        _: &[Message],
        _: Option<&str>,
        _: &[ToolDefinition],
        _: CancellationToken,
    ) -> anyhow::Result<mpsc::Receiver<StreamEvent>> {
        anyhow::bail!("controlled non-streaming fixture")
    }
}

#[tokio::test]
async fn automatic_compaction_receives_original_public_tool_turn_text_without_private_reasoning() {
    let root = tempfile::tempdir().unwrap();
    let mut config = test_config();
    config.memory = Some(crate::memory::MemoryConfig {
        llm_extraction: false,
        prune_policy: None,
        ..Default::default()
    });
    let agent = crate::Agent::from_config(config).await.unwrap();
    let client = Arc::new(PublicCheckpointClient {
        requests: AtomicUsize::new(0),
        summaries: Default::default(),
    });
    let session = agent
        .build_session(
            root.path().to_str().unwrap().to_string(),
            client.clone(),
            &crate::SessionOptions::new()
                .with_session_id("public-checkpoint")
                .with_artifact_store_limits(crate::tools::ArtifactStoreLimits {
                    max_artifacts: 0,
                    ..Default::default()
                })
                .with_permission_policy(
                    crate::permissions::PermissionPolicy::new().allow("convergence_stable(*)"),
                )
                .with_max_tool_rounds(0)
                .with_max_context_tokens(16_384)
                .with_auto_compact(true)
                .with_planning(false)
                .with_auto_delegation_enabled(false)
                .with_continuation(false),
        )
        .unwrap();
    let executions = Arc::new(AtomicUsize::new(0));
    session
        .register_dynamic_tool(Arc::new(StableObservation(executions.clone())))
        .unwrap();
    let result = session
        .send(
            "Read the external state three times and retain the established results.",
            None,
        )
        .await;
    let facts = read_workspace_facts(root.path(), session.session_id()).unwrap();
    session.close().await;
    agent.close().await;
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(executions.load(Ordering::SeqCst), 3);
    assert!(facts.iter().any(|fact| {
        fact.payload["call"]["text"]
            .as_str()
            .is_some_and(|text| text.contains("PUBLIC-CHECKPOINT-0"))
    }));
    let summaries = client.summaries.lock().unwrap();
    assert!(
        summaries.len() >= 2,
        "actual repeated model compaction is required; got {}",
        summaries.len()
    );
    assert!(
        summaries
            .iter()
            .any(|text| text.contains("PUBLIC-CHECKPOINT-0")),
        "the original public result must reach the compactor"
    );
    assert!(summaries
        .iter()
        .any(|text| text.contains("PUBLIC-CHECKPOINT-1")));
    assert!(summaries
        .iter()
        .all(|text| !text.contains("SYNTHETIC-PRIVATE-REASONING")));
}

fn batch_tool_response(calls: &[(&str, &str, Value)]) -> LlmResponse {
    LlmResponse {
        message: Message {
            role: "assistant".into(),
            content: calls
                .iter()
                .map(|(id, name, input)| crate::llm::ContentBlock::ToolUse {
                    id: (*id).into(),
                    name: (*name).into(),
                    input: input.clone(),
                })
                .collect(),
            reasoning_content: None,
            transcript_text: None,
            transcript_visibility: crate::llm::TranscriptVisibility::Product,
        },
        usage: TokenUsage::default(),
        stop_reason: None,
        token_logprobs: Vec::new(),
        meta: None,
    }
}

#[tokio::test]
async fn productive_multi_file_reads_reset_duplicate_read_budget_while_oscillation_still_stops() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("center.txt"), "CENTER-CONTENT\n").unwrap();
    for i in 1..=4 {
        std::fs::write(
            root.path().join(format!("other-{i}.txt")),
            format!("OTHER-CONTENT-{i}\n"),
        )
        .unwrap();
    }

    let mut config = test_config();
    config.memory = None;
    let agent = crate::Agent::from_config(config).await.unwrap();
    let productive_session = session_for(
        &agent,
        root.path(),
        "productive-multi-reads",
        Arc::new(ScriptedClient {
            seen: Default::default(),
            responses: std::sync::Mutex::new(vec![
                tool_response("c1", "read", json!({"file_path":"center.txt"})),
                tool_response("o1", "read", json!({"file_path":"other-1.txt"})),
                tool_response("c2", "read", json!({"file_path":"center.txt"})),
                tool_response("o2", "read", json!({"file_path":"other-2.txt"})),
                tool_response("c3", "read", json!({"file_path":"center.txt"})),
                tool_response("o3", "read", json!({"file_path":"other-3.txt"})),
                tool_response("c4", "read", json!({"file_path":"center.txt"})),
                tool_response("o4", "read", json!({"file_path":"other-4.txt"})),
                tool_response("c5", "read", json!({"file_path":"center.txt"})),
                text_response("Productive reads finished."),
            ]),
        }),
        &crate::SessionOptions::new()
            .with_permission_policy(crate::permissions::PermissionPolicy::new())
            .with_permission_checker(Arc::new(
                crate::permissions::InteractiveToolGuardrail::default().with_workspace(root.path()),
            ))
            .with_max_tool_rounds(0)
            .with_duplicate_tool_call_threshold(3)
            .with_auto_compact(false)
            .with_auto_delegation_enabled(false)
            .with_continuation(false),
    );
    let result = productive_session
        .send("Read center.txt across multiple other files.", None)
        .await;
    let facts = read_workspace_facts(root.path(), productive_session.session_id()).unwrap();
    productive_session.close().await;
    assert!(result.is_ok(), "{result:?}");
    assert!(!facts
        .iter()
        .any(|fact| fact.kind == "tool.result" && fact.payload["receipt"]["outcome"] == "denied"));

    let mut oscillation_responses = vec![
        tool_response("osc-c0", "read", json!({"file_path":"center.txt"})),
        tool_response("osc-o0", "read", json!({"file_path":"other-1.txt"})),
    ];
    for idx in 1..=6 {
        oscillation_responses.push(tool_response(
            &format!("osc-c{idx}"),
            "read",
            json!({"file_path":"center.txt"}),
        ));
        oscillation_responses.push(tool_response(
            &format!("osc-o{idx}"),
            "read",
            json!({"file_path":"other-1.txt"}),
        ));
    }
    oscillation_responses.push(text_response("unreachable"));
    let oscillation_session = session_for(
        &agent,
        root.path(),
        "oscillating-reads",
        Arc::new(ScriptedClient {
            seen: Default::default(),
            responses: std::sync::Mutex::new(oscillation_responses),
        }),
        &crate::SessionOptions::new()
            .with_permission_policy(crate::permissions::PermissionPolicy::new())
            .with_permission_checker(Arc::new(
                crate::permissions::InteractiveToolGuardrail::default().with_workspace(root.path()),
            ))
            .with_max_tool_rounds(0)
            .with_duplicate_tool_call_threshold(3)
            .with_auto_compact(false)
            .with_auto_delegation_enabled(false)
            .with_continuation(false),
    );
    let err = oscillation_session
        .send("Keep oscillating between two already-read files.", None)
        .await
        .expect_err("oscillation without fresh information must stop");
    oscillation_session.close().await;
    agent.close().await;
    assert!(err.to_string().contains("run.tool_refusal_cycle:"), "{err}");
}

#[tokio::test]
async fn same_batch_duplicate_reads_and_denied_reads_allow_bounded_recovery() {
    let root = tempfile::tempdir().unwrap();
    let workspace = root.path().join("ws");
    let outside = root.path().join("outside");
    std::fs::create_dir_all(&workspace).unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(workspace.join("a.txt"), "INSIDE-A").unwrap();
    let outside_file = outside.join("secret.txt");
    std::fs::write(&outside_file, "OUTSIDE-SECRET").unwrap();

    let mut config = test_config();
    config.memory = None;
    let agent = crate::Agent::from_config(config).await.unwrap();

    let twin_session = session_for(
        &agent,
        &workspace,
        "same-batch-twin-reads",
        Arc::new(ScriptedClient {
            seen: Default::default(),
            responses: std::sync::Mutex::new(vec![
                batch_tool_response(&[
                    ("t1", "read", json!({"file_path":"a.txt"})),
                    ("t2", "read", json!({"file_path":"a.txt"})),
                    ("t3", "read", json!({"file_path":"a.txt"})),
                ]),
                text_response("Recovered after same-batch duplicate nudge."),
            ]),
        }),
        &crate::SessionOptions::new()
            .with_permission_policy(crate::permissions::PermissionPolicy::new())
            .with_permission_checker(Arc::new(
                crate::permissions::InteractiveToolGuardrail::default().with_workspace(&workspace),
            ))
            .with_max_tool_rounds(0)
            .with_duplicate_tool_call_threshold(3)
            .with_auto_compact(false)
            .with_auto_delegation_enabled(false)
            .with_continuation(false),
    );
    let twin_res = twin_session.send("Read a.txt.", None).await;
    twin_session.close().await;
    assert!(twin_res.is_ok(), "{twin_res:?}");

    for stubborn in [false, true] {
        let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let out_arg = outside_file.display().to_string();
        let session = session_for(
            &agent,
            &workspace,
            if stubborn {
                "same-batch-denied-stubborn"
            } else {
                "same-batch-denied-recover"
            },
            Arc::new(ScriptedClient {
                seen: Arc::clone(&seen),
                responses: std::sync::Mutex::new(vec![
                    batch_tool_response(&[
                        ("d1", "read", json!({"file_path": out_arg})),
                        ("d2", "read", json!({"file_path": out_arg})),
                    ]),
                    if stubborn {
                        tool_response("d3", "read", json!({"file_path": out_arg}))
                    } else {
                        tool_response("ok1", "read", json!({"file_path": "a.txt"}))
                    },
                    text_response("Recovered to INSIDE-A."),
                ]),
            }),
            &crate::SessionOptions::new()
                .with_permission_policy(crate::permissions::PermissionPolicy::new())
                .with_permission_checker(Arc::new(
                    crate::permissions::InteractiveToolGuardrail::default()
                        .with_workspace(&workspace),
                ))
                .with_max_tool_rounds(0)
                .with_auto_compact(false)
                .with_auto_delegation_enabled(false)
                .with_continuation(false),
        );
        let result = session.send("Inspect target file.", None).await;
        session.close().await;
        let requests = seen.lock().unwrap();
        assert!(requests.iter().any(|req| {
            req.messages
                .iter()
                .any(|m| m.role == "user" && m.text().contains("[host tool recovery]"))
        }));
        if stubborn {
            let err = result.expect_err("second round repeating refused read must stop");
            assert!(err.to_string().contains("run.tool_refusal_cycle:"), "{err}");
            assert_eq!(
                requests.iter().filter(|req| !req.tools.is_empty()).count(),
                2
            );
        } else {
            assert!(result.is_ok(), "{result:?}");
        }
    }
    agent.close().await;
}

#[tokio::test]
async fn malformed_batch_read_reports_invalid_arguments_and_recovers_without_refusal_cycle() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("a.txt"), "ALPHA-CONTENT").unwrap();
    let mut config = test_config();
    config.memory = None;
    let agent = crate::Agent::from_config(config).await.unwrap();
    let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
    let session = session_for(
        &agent,
        root.path(),
        "malformed-batch-read",
        Arc::new(ScriptedClient {
            seen: Arc::clone(&seen),
            responses: std::sync::Mutex::new(vec![
                tool_response("bad1", "read", json!({"files": [{"file_path": "a.txt"}]})),
                tool_response("good1", "read", json!({"files": [{"path": "a.txt"}]})),
                text_response("Read ALPHA-CONTENT cleanly."),
            ]),
        }),
        &crate::SessionOptions::new()
            .with_permission_policy(crate::permissions::PermissionPolicy::new())
            .with_permission_checker(Arc::new(
                crate::permissions::InteractiveToolGuardrail::default().with_workspace(root.path()),
            ))
            .with_max_tool_rounds(0)
            .with_auto_compact(false)
            .with_auto_delegation_enabled(false)
            .with_continuation(false),
    );
    let result = session.send("Read a.txt in batch mode.", None).await;
    let facts = read_workspace_facts(root.path(), session.session_id()).unwrap();
    session.close().await;
    agent.close().await;
    assert!(result.is_ok(), "{result:?}");
    let receipts: Vec<_> = facts
        .iter()
        .filter(|fact| fact.kind == "tool.result")
        .map(|fact| &fact.payload["receipt"])
        .collect();
    assert_eq!(receipts.len(), 2);
    assert_eq!(receipts[0]["outcome"], "failed");
    assert!(receipts[0]["output"]
        .as_str()
        .unwrap()
        .contains("Invalid arguments for tool 'read'"));
    assert_eq!(receipts[1]["outcome"], "succeeded");
    assert!(receipts[1]["output"]
        .as_str()
        .unwrap()
        .contains("ALPHA-CONTENT"));
    assert!(!seen.lock().unwrap().iter().any(|req| {
        req.messages
            .iter()
            .any(|m| m.role == "user" && m.text().contains("[host tool recovery]"))
    }));
}

#[cfg(unix)]
#[tokio::test]
async fn missing_file_under_in_workspace_symlink_dir_fails_as_not_found_without_denial() {
    use std::os::unix::fs::symlink;

    let root = tempfile::tempdir().unwrap();
    let real = root.path().join("real");
    std::fs::create_dir(&real).unwrap();
    std::fs::write(real.join("present.txt"), "SYMLINK-PRESENT").unwrap();
    symlink(&real, root.path().join("alias")).unwrap();

    let mut config = test_config();
    config.memory = None;
    let agent = crate::Agent::from_config(config).await.unwrap();
    let session = session_for(
        &agent,
        root.path(),
        "symlink-missing-read",
        Arc::new(ScriptedClient {
            seen: Default::default(),
            responses: std::sync::Mutex::new(vec![
                tool_response("m1", "read", json!({"file_path": "alias/missing1.txt"})),
                tool_response("m2", "read", json!({"file_path": "alias/missing2.txt"})),
                tool_response("ok", "read", json!({"file_path": "alias/present.txt"})),
                text_response("Found SYMLINK-PRESENT."),
            ]),
        }),
        &crate::SessionOptions::new()
            .with_permission_policy(crate::permissions::PermissionPolicy::new())
            .with_permission_checker(Arc::new(
                crate::permissions::InteractiveToolGuardrail::default().with_workspace(root.path()),
            ))
            .with_max_tool_rounds(0)
            .with_auto_compact(false)
            .with_auto_delegation_enabled(false)
            .with_continuation(false),
    );
    let result = session.send("Inspect alias directory.", None).await;
    let facts = read_workspace_facts(root.path(), session.session_id()).unwrap();
    session.close().await;
    agent.close().await;
    assert!(result.is_ok(), "{result:?}");
    let receipts: Vec<_> = facts
        .iter()
        .filter(|fact| fact.kind == "tool.result")
        .map(|fact| &fact.payload["receipt"])
        .collect();
    assert_eq!(receipts.len(), 3);
    for failed in &receipts[..2] {
        assert_eq!(failed["outcome"], "failed");
        assert!(failed["output"]
            .as_str()
            .unwrap()
            .contains("no file found at this exact workspace path"));
    }
    assert_eq!(receipts[2]["outcome"], "succeeded");
    assert!(receipts[2]["output"]
        .as_str()
        .unwrap()
        .contains("SYMLINK-PRESENT"));
    assert!(!facts
        .iter()
        .any(|fact| fact.kind == "tool.result" && fact.payload["receipt"]["outcome"] == "denied"));
}

#[cfg(unix)]
#[tokio::test]
async fn bash_catastrophic_hint_and_in_workspace_symlink_recover_without_refusal_cycle() {
    use std::os::unix::fs::symlink;

    let root = tempfile::tempdir().unwrap();
    let deps_bin = root.path().join("build/wasi-deps/bin");
    std::fs::create_dir_all(&deps_bin).unwrap();
    std::fs::write(
        deps_bin.join("tool-real"),
        "#!/bin/sh\necho wasi-symlink-ok\n",
    )
    .unwrap();
    symlink("tool-real", deps_bin.join("tool")).unwrap();
    symlink("wasi-deps", root.path().join("build/wasi")).unwrap();

    let mut config = test_config();
    config.memory = None;
    let agent = crate::Agent::from_config(config).await.unwrap();
    let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
    let long_rm = format!(
        "echo {} && rm -rf \"$work\"",
        "wasi-padding-step-".repeat(15)
    );
    let session = session_for(
        &agent,
        root.path(),
        "bash-symlink-and-hint-recovery",
        Arc::new(ScriptedClient {
            seen: Arc::clone(&seen),
            responses: std::sync::Mutex::new(vec![
                tool_response("bad-rm", "bash", json!({"command": long_rm})),
                tool_response(
                    "good-symlink",
                    "bash",
                    json!({"command": "sh build/wasi/bin/tool"}),
                ),
                text_response("Verified wasi-symlink-ok."),
            ]),
        }),
        &crate::SessionOptions::new()
            .with_permission_policy(crate::permissions::PermissionPolicy::new().allow("bash"))
            .with_permission_checker(Arc::new(
                crate::permissions::InteractiveToolGuardrail::for_mode("force")
                    .unwrap()
                    .with_workspace(root.path()),
            ))
            .with_max_tool_rounds(0)
            .with_auto_compact(false)
            .with_auto_delegation_enabled(false)
            .with_continuation(false),
    );
    let result = session.send("Run the workspace WASI tool.", None).await;
    let facts = read_workspace_facts(root.path(), session.session_id()).unwrap();
    session.close().await;
    agent.close().await;
    assert!(result.is_ok(), "{result:?}");

    let receipts: Vec<_> = facts
        .iter()
        .filter(|fact| fact.kind == "tool.result")
        .map(|fact| &fact.payload["receipt"])
        .collect();
    assert_eq!(receipts.len(), 2);
    assert_eq!(receipts[0]["outcome"], "denied");
    let denial_output = receipts[0]["output"].as_str().unwrap();
    assert!(
        denial_output.contains("catastrophic_operation"),
        "{denial_output}"
    );
    assert!(denial_output.contains("rm -rf"), "{denial_output}");
    assert!(
        !denial_output.contains("refused command shape:"),
        "{denial_output}"
    );

    assert_eq!(receipts[1]["outcome"], "succeeded");
    assert!(receipts[1]["output"]
        .as_str()
        .unwrap()
        .contains("wasi-symlink-ok"));

    let requests = seen.lock().unwrap();
    assert!(requests.iter().any(|req| {
        req.messages
            .iter()
            .any(|m| m.role == "user" && m.text().contains("[host tool recovery]"))
    }));
}

#[cfg(unix)]
#[tokio::test]
async fn yolo_background_start_and_tail_complete_without_refusal_cycle() {
    let root = tempfile::tempdir().unwrap();
    let mut config = test_config();
    config.memory = None;
    let agent = crate::Agent::from_config(config).await.unwrap();
    let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
    let session = session_for(
        &agent,
        root.path(),
        "yolo-background-complete",
        Arc::new(ScriptedClient {
            seen: Arc::clone(&seen),
            responses: std::sync::Mutex::new(vec![
                tool_response(
                    "bg-start",
                    "background",
                    json!({
                        "action": "start",
                        "name": "job",
                        "command": "sleep 0.15 && echo yolo-bg-done"
                    }),
                ),
                tool_response(
                    "bg-tail",
                    "background",
                    json!({
                        "action": "tail",
                        "name": "job",
                        "timeout_ms": 5000
                    }),
                ),
                text_response("Verified yolo-bg-done."),
            ]),
        }),
        &crate::SessionOptions::new()
            .with_permission_policy(crate::permissions::policy_for_posture("yolo"))
            .with_permission_checker(Arc::new(
                crate::permissions::InteractiveToolGuardrail::for_mode("force")
                    .unwrap()
                    .with_workspace(root.path()),
            ))
            .with_max_tool_rounds(0)
            .with_auto_compact(false)
            .with_auto_delegation_enabled(false)
            .with_continuation(false),
    );
    let result = session
        .send("Run the background job under yolo.", None)
        .await;
    let facts = read_workspace_facts(root.path(), session.session_id()).unwrap();
    session.close().await;
    agent.close().await;
    assert!(result.is_ok(), "{result:?}");

    let receipts: Vec<_> = facts
        .iter()
        .filter(|fact| fact.kind == "tool.result")
        .map(|fact| &fact.payload["receipt"])
        .collect();
    assert_eq!(receipts.len(), 2);
    assert_eq!(receipts[0]["outcome"], "succeeded", "{:?}", receipts[0]);
    assert_eq!(receipts[1]["outcome"], "succeeded", "{:?}", receipts[1]);
    assert!(receipts[1]["output"]
        .as_str()
        .unwrap()
        .contains("yolo-bg-done"));
    assert!(!facts
        .iter()
        .any(|fact| fact.kind == "tool.result" && fact.payload["receipt"]["outcome"] == "denied"));
}

#[cfg(unix)]
#[tokio::test]
async fn same_model_turn_split_confirm_denials_and_plan_mode_recovery_complete_without_refusal_cycle(
) {
    use std::os::unix::fs::symlink;

    let root = tempfile::tempdir().unwrap();
    let real = root.path().join("real");
    std::fs::create_dir(&real).unwrap();
    std::fs::write(real.join("inside.txt"), "INSIDE-FILE\n").unwrap();
    symlink(&real, root.path().join("alias")).unwrap();

    let mut config = test_config();
    config.memory = None;
    let agent = crate::Agent::from_config(config).await.unwrap();

    // Part A: One model turn emits two mutating bash calls, both denied as
    // `plan_mode_read_only`. The model must still receive one recovery turn and
    // succeed via `ls("alias")`.
    let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
    let session = session_for(
        &agent,
        root.path(),
        "split-confirm-plan-recover",
        Arc::new(ScriptedClient {
            seen: Arc::clone(&seen),
            responses: std::sync::Mutex::new(vec![
                batch_tool_response(&[
                    ("b1", "bash", json!({"command": "touch one.txt"})),
                    ("b2", "bash", json!({"command": "touch two.txt"})),
                ]),
                tool_response("ls-1", "ls", json!({"path": "alias"})),
                text_response("Inspected alias/inside.txt in Plan mode."),
            ]),
        }),
        &crate::SessionOptions::new()
            .with_permission_policy(crate::permissions::PermissionPolicy::new().allow("*"))
            .with_permission_checker(Arc::new(
                crate::permissions::InteractiveToolGuardrail::for_mode("plan")
                    .unwrap()
                    .with_workspace(root.path()),
            ))
            .with_max_tool_rounds(0)
            .with_auto_compact(false)
            .with_auto_delegation_enabled(false)
            .with_continuation(false),
    );
    let result = session.send("Inspect alias directory.", None).await;
    let facts = read_workspace_facts(root.path(), session.session_id()).unwrap();
    session.close().await;
    assert!(result.is_ok(), "{result:?}");

    let receipts: Vec<_> = facts
        .iter()
        .filter(|fact| fact.kind == "tool.result")
        .map(|fact| &fact.payload["receipt"])
        .collect();
    assert_eq!(receipts.len(), 3);
    for denied in &receipts[..2] {
        assert_eq!(denied["outcome"], "denied");
        let output = denied["output"].as_str().unwrap();
        assert!(output.contains("plan_mode_read_only"), "{output}");
        assert!(output.contains("read-only Plan mode"), "{output}");
    }
    assert_eq!(receipts[2]["outcome"], "succeeded");
    assert!(receipts[2]["output"]
        .as_str()
        .unwrap()
        .contains("inside.txt"));
    assert!(seen.lock().unwrap().iter().any(|req| {
        req.messages
            .iter()
            .any(|m| m.role == "user" && m.text().contains("[host tool recovery]"))
    }));

    // Part B: Repeating `bash` on the second model turn must still stop.
    let stubborn_session = session_for(
        &agent,
        root.path(),
        "split-confirm-plan-stubborn",
        Arc::new(ScriptedClient {
            seen: Default::default(),
            responses: std::sync::Mutex::new(vec![
                batch_tool_response(&[
                    ("s1", "bash", json!({"command": "touch one.txt"})),
                    ("s2", "bash", json!({"command": "touch two.txt"})),
                ]),
                tool_response("s3", "bash", json!({"command": "touch three.txt"})),
                text_response("unreachable"),
            ]),
        }),
        &crate::SessionOptions::new()
            .with_permission_policy(crate::permissions::PermissionPolicy::new().allow("*"))
            .with_permission_checker(Arc::new(
                crate::permissions::InteractiveToolGuardrail::for_mode("plan")
                    .unwrap()
                    .with_workspace(root.path()),
            ))
            .with_max_tool_rounds(0)
            .with_auto_compact(false)
            .with_auto_delegation_enabled(false)
            .with_continuation(false),
    );
    let err = stubborn_session
        .send("Keep retrying bash in plan mode.", None)
        .await
        .expect_err("second model turn repeating bash must stop");
    stubborn_session.close().await;
    agent.close().await;
    assert!(err.to_string().contains("run.tool_refusal_cycle:"), "{err}");
}

#[tokio::test]
async fn context_overflow_auto_compacts_and_completes_turn_without_failure() {
    struct OverflowRecoveryClient {
        calls: std::sync::atomic::AtomicUsize,
    }

    #[async_trait]
    impl LlmClient for OverflowRecoveryClient {
        async fn complete(
            &self,
            _messages: &[Message],
            system: Option<&str>,
            tools: &[ToolDefinition],
        ) -> anyhow::Result<LlmResponse> {
            if tools.is_empty()
                && system.is_some_and(|s| s.to_ascii_lowercase().contains("compact"))
            {
                return Ok(text_response("Emergency compacted summary of big.txt"));
            }
            let step = self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            match step {
                0 => Ok(tool_response(
                    "t1",
                    "read",
                    json!({ "file_path": "big.txt" }),
                )),
                1 => Err(anyhow::Error::new(
                    crate::llm::NonRetryableLlmError::from_status(
                        "openai",
                        400,
                        r#"{"error":{"code":"context_length_exceeded","message":"maximum context length is 8192"}}"#,
                    ),
                )),
                _ => Ok(text_response(
                    "Recovered cleanly after overflow compaction.",
                )),
            }
        }

        async fn complete_streaming(
            &self,
            messages: &[Message],
            system: Option<&str>,
            tools: &[ToolDefinition],
            _cancel_token: tokio_util::sync::CancellationToken,
        ) -> anyhow::Result<tokio::sync::mpsc::Receiver<crate::llm::StreamEvent>> {
            let response = self.complete(messages, system, tools).await?;
            let (tx, rx) = tokio::sync::mpsc::channel(8);
            tokio::spawn(async move {
                let text = response.text();
                if !text.is_empty() {
                    let _ = tx.send(crate::llm::StreamEvent::TextDelta(text)).await;
                }
                let _ = tx.send(crate::llm::StreamEvent::Done(response)).await;
            });
            Ok(rx)
        }
    }

    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("big.txt"), "B".repeat(4096)).unwrap();
    let mut config = test_config();
    config.memory = None;
    let agent = crate::Agent::from_config(config).await.unwrap();
    let client = Arc::new(OverflowRecoveryClient {
        calls: std::sync::atomic::AtomicUsize::new(0),
    });
    let session = agent
        .build_session(
            root.path().to_string_lossy().to_string(),
            client,
            &crate::SessionOptions::new()
                .with_session_id("overflow-auto-compact")
                .with_permission_policy(crate::permissions::PermissionPolicy::new().allow("*"))
                .with_max_tool_rounds(0)
                .with_auto_compact(true)
                .with_auto_delegation_enabled(false)
                .with_planning_mode(crate::PlanningMode::Disabled)
                .with_continuation(false),
        )
        .unwrap();

    let result = session
        .send("Read big.txt and recover from overflow.", None)
        .await
        .expect("reactive overflow compaction must recover the turn");
    assert!(
        result
            .text
            .contains("Recovered cleanly after overflow compaction."),
        "{result:?}"
    );
    let facts = read_workspace_facts(root.path(), session.session_id()).unwrap();
    session.close().await;
    agent.close().await;

    assert!(
        facts.iter().any(|fact| {
            fact.kind == "compaction.done" && fact.payload["overflow_recovery"] == json!(true)
        }),
        "expected overflow_recovery compaction fact in log: {facts:?}"
    );
}

#[tokio::test]
async fn mid_stream_failures_retry_beyond_two_attempts_and_complete() {
    struct MidStreamRetryClient {
        attempts: std::sync::atomic::AtomicUsize,
    }

    #[async_trait]
    impl LlmClient for MidStreamRetryClient {
        async fn complete(
            &self,
            _messages: &[Message],
            _system: Option<&str>,
            _tools: &[ToolDefinition],
        ) -> anyhow::Result<LlmResponse> {
            anyhow::bail!("streaming transport required")
        }

        async fn complete_streaming(
            &self,
            _messages: &[Message],
            _system: Option<&str>,
            _tools: &[ToolDefinition],
            _cancel_token: tokio_util::sync::CancellationToken,
        ) -> anyhow::Result<tokio::sync::mpsc::Receiver<crate::llm::StreamEvent>> {
            let attempt = self
                .attempts
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let (tx, rx) = tokio::sync::mpsc::channel(8);
            tokio::spawn(async move {
                if attempt < 3 {
                    let _ = tx
                        .send(crate::llm::StreamEvent::TextDelta("partial-drop ".into()))
                        .await;
                    drop(tx);
                } else {
                    let text = "Recovered on fourth stream attempt.".to_string();
                    let _ = tx
                        .send(crate::llm::StreamEvent::TextDelta(text.clone()))
                        .await;
                    let _ = tx
                        .send(crate::llm::StreamEvent::Done(text_response(&text)))
                        .await;
                }
            });
            Ok(rx)
        }
    }

    let root = tempfile::tempdir().unwrap();
    let mut config = test_config();
    config.memory = None;
    let agent = crate::Agent::from_config(config).await.unwrap();
    let client = Arc::new(MidStreamRetryClient {
        attempts: std::sync::atomic::AtomicUsize::new(0),
    });
    let session = agent
        .build_session(
            root.path().to_string_lossy().to_string(),
            Arc::clone(&client) as Arc<dyn LlmClient>,
            &crate::SessionOptions::new()
                .with_session_id("mid-stream-retry")
                .with_max_tool_rounds(0)
                .with_auto_compact(false)
                .with_auto_delegation_enabled(false)
                .with_continuation(false),
        )
        .unwrap();

    let result = session
        .send("Survive three mid-stream drops.", None)
        .await
        .expect("four total attempts must succeed under DEFAULT_MODEL_ATTEMPTS=5");
    session.close().await;
    agent.close().await;

    assert_eq!(client.attempts.load(std::sync::atomic::Ordering::SeqCst), 4);
    assert!(
        result.text.contains("Recovered on fourth stream attempt."),
        "{result:?}"
    );
}

#[tokio::test]
async fn interrupted_turn_resumes_from_durable_fact_log_without_duplicating_user_message_and_refuses_unresolved_tool(
) {
    use aster_effect::{
        tool_args_digest, FileLog, LogStore, NewFact, ToolInvocationIdentityV1, ToolPreparedV1,
        ToolRecoveryPolicy, TOOL_PREPARED_SCHEMA,
    };

    let root = tempfile::tempdir().unwrap();
    let mut config = test_config();
    config.memory = None;
    let agent = crate::Agent::from_config(config).await.unwrap();

    // Part A: Crash right after persisting user.message -> Resumable, completes with no duplicate user.message.
    let resume_id = "resume-ok";
    let log_ok = FileLog::open(crate::session_folder::effect_log_dir(
        root.path(),
        resume_id,
    ))
    .unwrap();
    log_ok
        .append(
            resume_id,
            &[NewFact {
                kind: "user.message".into(),
                key: "u-1".into(),
                payload: json!({ "text": "Finish the interrupted work." }),
            }],
            None,
        )
        .unwrap();

    let session_ok = session_for(
        &agent,
        root.path(),
        resume_id,
        Arc::new(ScriptedClient {
            seen: Arc::default(),
            responses: std::sync::Mutex::new(vec![text_response(
                "Resumed cleanly from durable log.",
            )]),
        }),
        &crate::SessionOptions::new()
            .with_max_tool_rounds(0)
            .with_auto_compact(false)
            .with_auto_delegation_enabled(false)
            .with_continuation(false),
    );
    assert_eq!(
        session_ok.interrupted_turn_state().unwrap(),
        crate::fact_control::InterruptedTurnState::Resumable
    );

    let mut stream = session_ok
        .stream_resume_interrupted_turn()
        .await
        .expect("resumable turn must launch");
    let mut text = String::new();
    while let Some(event) = stream.events.recv().await {
        match event {
            crate::AgentEvent::TextDelta { text: delta } => text.push_str(&delta),
            crate::AgentEvent::ToolEffectUnresolved { error } => {
                panic!("unexpected ToolEffectUnresolved: {error:?}")
            }
            crate::AgentEvent::Error { message } => panic!("unexpected resume error: {message}"),
            _ => {}
        }
    }
    stream.worker.await.unwrap();
    assert!(
        text.contains("Resumed cleanly from durable log."),
        "expected resumed output text, got {text:?}"
    );
    assert_eq!(
        session_ok.interrupted_turn_state().unwrap(),
        crate::fact_control::InterruptedTurnState::Idle
    );
    let facts_ok = read_workspace_facts(root.path(), resume_id).unwrap();
    assert_eq!(
        facts_ok
            .iter()
            .filter(|fact| fact.kind == "user.message")
            .count(),
        1,
        "resume must not duplicate user.message: {facts_ok:?}"
    );
    session_ok.close().await;

    // Part B: Crash after prepared non-replay-safe tool -> UnresolvedTool, fails closed.
    let unresolved_id = "resume-unresolved";
    let log_unresolved = FileLog::open(crate::session_folder::effect_log_dir(
        root.path(),
        unresolved_id,
    ))
    .unwrap();
    let args = json!({ "file_path": "a.txt", "content": "hi" });
    let digest = tool_args_digest(&args).unwrap();
    let prepared = ToolPreparedV1 {
        schema: TOOL_PREPARED_SCHEMA.into(),
        identity: ToolInvocationIdentityV1 {
            turn: 1,
            cycle: 0,
            tool_call_id: "call-u1".into(),
            batch_index: 0,
            admission: None,
        },
        tool_name: "write".into(),
        original_args_digest: digest.clone(),
        effective_args_digest: digest,
        recovery: ToolRecoveryPolicy::NeverReplay,
        baseline: None,
    };
    log_unresolved
        .append(
            unresolved_id,
            &[
                NewFact {
                    kind: "user.message".into(),
                    key: "u-1".into(),
                    payload: json!({ "text": "Write file safely." }),
                },
                NewFact {
                    kind: "model.turn".into(),
                    key: "m-1".into(),
                    payload: json!({
                        "kind": "tool",
                        "tool_effect_protocol": 1,
                        "call": {
                            "id": "call-u1",
                            "name": "write",
                            "args": args,
                            "needs_confirmation": false,
                        },
                        "also": [],
                    }),
                },
                NewFact {
                    kind: "tool.prepared".into(),
                    key: "prep-1".into(),
                    payload: json!({ "prepared": prepared }),
                },
            ],
            None,
        )
        .unwrap();

    let session_unresolved = session_for(
        &agent,
        root.path(),
        unresolved_id,
        Arc::new(ScriptedClient {
            seen: Arc::default(),
            responses: std::sync::Mutex::new(vec![text_response("should never run")]),
        }),
        &crate::SessionOptions::new()
            .with_max_tool_rounds(0)
            .with_auto_compact(false)
            .with_auto_delegation_enabled(false)
            .with_continuation(false),
    );
    assert_eq!(
        session_unresolved.interrupted_turn_state().unwrap(),
        crate::fact_control::InterruptedTurnState::UnresolvedTool
    );
    let err = match session_unresolved.stream_resume_interrupted_turn().await {
        Ok(_) => panic!("unresolved tool boundary must refuse automatic resume"),
        Err(err) => err,
    };
    assert!(
        err.to_string()
            .contains("no safely resumable interrupted turn"),
        "{err}"
    );
    session_unresolved.close().await;
    agent.close().await;
}

#[tokio::test]
async fn interrupted_read_only_tool_replays_safely_on_resume_without_duplicating_preparation() {
    use aster_effect::{
        tool_args_digest, FileLog, LogStore, NewFact, ToolAdmissionIdentityV1,
        ToolInvocationIdentityV1, ToolPreparedV1, ToolRecoveryPolicy, TOOL_PREPARED_SCHEMA,
    };

    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("sample.txt"), "REPLAY_SAFE_CONTENT").unwrap();
    let mut config = test_config();
    config.memory = None;
    let agent = crate::Agent::from_config(config).await.unwrap();

    let session_id = "resume-replay-safe";
    let log = FileLog::open(crate::session_folder::effect_log_dir(
        root.path(),
        session_id,
    ))
    .unwrap();
    let args = json!({ "file_path": "sample.txt" });
    let digest = tool_args_digest(&args).unwrap();
    let prepared = ToolPreparedV1 {
        schema: TOOL_PREPARED_SCHEMA.into(),
        identity: ToolInvocationIdentityV1 {
            turn: 1,
            cycle: 0,
            tool_call_id: "call-read-1".into(),
            batch_index: 0,
            admission: Some(ToolAdmissionIdentityV1 {
                run_id: "crashed-run".into(),
                thread_id: session_id.into(),
                capability_binding_digest: format!("sha256:{}", "a".repeat(64)),
            }),
        },
        tool_name: "read".into(),
        original_args_digest: digest.clone(),
        effective_args_digest: digest,
        recovery: ToolRecoveryPolicy::ReplaySafe,
        baseline: None,
    };
    log.append(
        session_id,
        &[
            NewFact {
                kind: "user.message".into(),
                key: "u1".into(),
                payload: json!({ "text": "read sample.txt" }),
            },
            NewFact {
                kind: "model.turn".into(),
                key: "model:1:0".into(),
                payload: json!({
                    "kind": "tool",
                    "tool_effect_protocol": 1,
                    "call": {
                        "id": "call-read-1",
                        "name": "read",
                        "args": args,
                        "needs_confirmation": false,
                    },
                    "also": [],
                }),
            },
            NewFact {
                kind: "tool.prepared".into(),
                key: "prep-1".into(),
                payload: json!({ "prepared": prepared }),
            },
        ],
        None,
    )
    .unwrap();

    let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
    let session = session_for(
        &agent,
        root.path(),
        session_id,
        Arc::new(ScriptedClient {
            seen: Arc::clone(&seen),
            responses: std::sync::Mutex::new(vec![text_response(
                "Observed sample.txt after replay.",
            )]),
        }),
        &crate::SessionOptions::new()
            .with_permission_policy(crate::permissions::PermissionPolicy::new().allow("read(*)"))
            .with_max_tool_rounds(0)
            .with_auto_compact(false)
            .with_auto_delegation_enabled(false)
            .with_continuation(false),
    );
    assert_eq!(
        session.interrupted_turn_state().unwrap(),
        crate::fact_control::InterruptedTurnState::Resumable
    );

    let mut stream = session
        .stream_resume_interrupted_turn()
        .await
        .expect("ReplaySafe interrupted tool must be auto-resumable");
    let mut text = String::new();
    while let Some(event) = stream.events.recv().await {
        match event {
            crate::AgentEvent::TextDelta { text: delta } => text.push_str(&delta),
            crate::AgentEvent::ToolEffectUnresolved { error } => {
                panic!("unexpected ToolEffectUnresolved: {error:?}")
            }
            crate::AgentEvent::Error { message } => panic!("unexpected resume error: {message}"),
            _ => {}
        }
    }
    stream.worker.await.unwrap();
    assert!(
        text.contains("Observed sample.txt after replay."),
        "expected final answer, got {text:?}"
    );

    let requests = seen.lock().unwrap();
    assert_eq!(
        requests.len(),
        1,
        "model should be called once after tool replay"
    );
    let saw_file_content = requests[0].messages.iter().any(|message| {
        message
            .content
            .iter()
            .any(|block| format!("{block:?}").contains("REPLAY_SAFE_CONTENT"))
    });
    assert!(
        saw_file_content,
        "replayed read tool output must reach the resumed model call: {:?}",
        requests[0].messages
    );
    drop(requests);

    let facts = read_workspace_facts(root.path(), session.session_id()).unwrap();
    assert_eq!(
        facts.iter().filter(|f| f.kind == "user.message").count(),
        1,
        "resume must not duplicate user.message"
    );
    assert_eq!(
        facts.iter().filter(|f| f.kind == "tool.prepared").count(),
        1,
        "ReplaySafe replay must not duplicate tool.prepared"
    );
    assert!(
        facts
            .iter()
            .any(|f| f.kind == "tool.result" && f.payload["ok"] == json!(true)),
        "expected successful tool.result after replay"
    );

    session.close().await;
    agent.close().await;
}
