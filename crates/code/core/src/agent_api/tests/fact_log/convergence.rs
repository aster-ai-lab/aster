use super::*;
use crate::tools::{Tool, ToolCapabilities, ToolContext, ToolOutput};
use serde_json::{json, Value};

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
    assert!(facts.iter().any(|fact| fact.payload["call"]["text"]
        .as_str()
        .is_some_and(|text| text.contains("PUBLIC-CHECKPOINT-0"))));
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
