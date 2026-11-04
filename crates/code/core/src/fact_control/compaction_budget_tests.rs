#![cfg(test)]

use super::*;
use crate::compaction::CompactionBudget;
use crate::llm::{StreamEvent, TokenUsage};

struct BudgetSummaryClient {
    calls: AtomicUsize,
    text: String,
    stop: &'static str,
}

#[tokio::test]
async fn fixed_context_does_not_force_a_fitting_public_handoff_into_the_minimum_allowance() {
    let root = tempfile::tempdir().unwrap();
    let state = (0..48)
        .map(|index| {
            format!("Observed opaque value 値🙂-{index:02}; validation remains pending.\n")
        })
        .collect::<String>();
    let client = Arc::new(BudgetSummaryClient {
        calls: AtomicUsize::new(0),
        text: state.clone(),
        stop: "end_turn",
    });
    let (events, mut received) = tokio::sync::mpsc::channel(4);
    let compactor = SessionCompact {
        client: client.clone(),
        history: (Arc::new(FileLog::open(root.path()).unwrap()), THREAD.into()),
        source: None,
        artifacts: None,
        events: Some(events),
        budget: CompactionBudget::for_auto_compaction(24_576, 0.85, 0),
        fixed_prompt_tokens: Arc::new(AtomicU64::new(14_000)),
        prompt_tokens: Arc::new(AtomicU64::new(23_000)),
    };
    let prefix = vec![format!(
        "assistant\n{state}\n{}",
        "Original supporting observation.\n".repeat(4_000)
    )];
    let kept = vec!["tool\nLatest original observation.".to_string()];
    let summary = compactor.compact_with_tail(&prefix, &kept).await.unwrap();
    assert_eq!(client.calls.load(Ordering::SeqCst), 1);
    assert!(
        summary.contains(state.trim_end()),
        "fixed context displaced fitting public state"
    );
    assert!(
        matches!(received.recv().await, Some(crate::agent::AgentEvent::AutoCompact { phase, .. }) if phase == "started")
    );
    let Some(crate::agent::AgentEvent::AutoCompact { tokens_after, .. }) = received.recv().await
    else {
        panic!("completion estimate missing");
    };
    assert!(tokens_after > 14_000);
    assert!(
        tokens_after < 20_000,
        "the original trigger still has headroom"
    );
    assert_eq!(kept, ["tool\nLatest original observation."]);
}

#[tokio::test]
async fn retained_receipt_metadata_does_not_displace_public_summary_context() {
    use super::tool_history_tests::{append, call, error_result, result};
    let mut reserved = Vec::new();
    for (is_error, images) in [
        (false, Vec::new()),
        (true, Vec::new()),
        (
            false,
            vec![crate::llm::Attachment::new(vec![7; 8_192], "image/png")],
        ),
    ] {
        let body = format!(
            "{}Final checkpoint Ω-4c29.",
            "Established checkpoint 値🙂-9b27.\n".repeat(200)
        );
        let mut summaries = Vec::new();
        let mut estimates = Vec::new();
        for metadata in [
            None,
            Some(serde_json::json!({"receipt": "INTERNAL-RECEIPT-ONLY".repeat(4_000)})),
        ] {
            let root = tempfile::tempdir().unwrap();
            let log = Arc::new(FileLog::open(log_dir(root.path())).unwrap());
            let decision = call("retained");
            append(&log, "call", "model.turn", decision.clone());
            let wrapped = super::receipts::runner_result_value(
                "Original observation 値🙂-6f19.",
                is_error,
                &images,
                if is_error { 7 } else { 0 },
                metadata.as_ref(),
                None,
            );
            let wrapped = wrapped.as_str().unwrap();
            if is_error {
                error_result(&log, "result", "retained", wrapped);
            } else {
                result(&log, "result", "retained", wrapped);
            }
            let before = log.read(THREAD).unwrap();
            let client = Arc::new(BudgetSummaryClient {
                calls: AtomicUsize::new(0),
                text: body.clone(),
                stop: "end_turn",
            });
            let (events, mut received) = tokio::sync::mpsc::channel(4);
            let compactor = SessionCompact {
                artifacts: None,
                client: client.clone(),
                history: (log.clone(), THREAD.into()),
                source: None,
                events: Some(events),
                budget: CompactionBudget::for_auto_compaction(32_768, 0.8, 0),
                fixed_prompt_tokens: Arc::new(AtomicU64::new(1_000)),
                prompt_tokens: Arc::new(AtomicU64::new(32_000)),
            };
            let kept = vec![
                format!("assistant\ntool read\n{}", decision["call"]["args"]),
                format!(
                    "{}\n{wrapped}",
                    if is_error { "tool-error" } else { "tool" }
                ),
            ];
            let original_kept = kept.clone();
            let summary = compactor
                .compact_with_tail(
                    &[format!(
                        "assistant\n{}",
                        "Older public observation.\n".repeat(4_000)
                    )],
                    &kept,
                )
                .await
                .unwrap();
            assert_eq!(client.calls.load(Ordering::SeqCst), 1);
            assert_eq!(kept, original_kept);
            assert_eq!(log.read(THREAD).unwrap(), before);
            assert!(!summary.contains("INTERNAL-RECEIPT-ONLY"));
            assert!(!summary.contains("original provider continuation"));
            summaries.push(summary);
            received.recv().await.unwrap();
            let Some(crate::agent::AgentEvent::AutoCompact { tokens_after, .. }) =
                received.recv().await
            else {
                panic!("completion estimate missing");
            };
            estimates.push(tokens_after);
        }
        assert!(summaries[0].contains(&body));
        assert_eq!(
            summaries[0], summaries[1],
            "receipt metadata changed the handoff"
        );
        assert_eq!(
            estimates[0], estimates[1],
            "receipt metadata reached the token estimate"
        );
        reserved.push(estimates[0]);
    }
    assert!(
        reserved[2] > reserved[0],
        "retained images still occupy context"
    );
}

#[tokio::test]
async fn archived_fallback_keeps_observations_instead_of_receipt_metadata() {
    use super::tool_history_tests::{append, call, result};
    let root = tempfile::tempdir().unwrap();
    let log = Arc::new(FileLog::open(log_dir(root.path())).unwrap());
    let executor = ToolExecutor::new(root.path().display().to_string());
    let client = Arc::new(BudgetSummaryClient {
        calls: AtomicUsize::new(0),
        text: "unused".into(),
        stop: "end_turn",
    });
    let output = "Observed value 値🙂-61af.\nNext observation Ω-72bb.";
    let metadata = serde_json::json!({"receipt": "INTERNAL-RECEIPT-ONLY".repeat(600)});
    let wrapped = super::receipts::runner_result_value(
        output,
        false,
        &[],
        0,
        Some(&metadata),
        Some("internal-digest"),
    );
    let wrapped = wrapped.as_str().unwrap();
    let mut decision = call("observed");
    decision["call"]["text"] = serde_json::json!("");
    append(&log, "model", "model.turn", decision.clone());
    result(&log, "result", "observed", wrapped);
    let before = serde_json::to_value(log.read(THREAD).unwrap()).unwrap();
    let compactor = SessionCompact {
        artifacts: Some(executor.artifact_store()),
        client: client.clone(),
        history: (log.clone(), THREAD.into()),
        source: None,
        events: None,
        budget: CompactionBudget::for_auto_compaction(4_096, 0.8, 0),
        fixed_prompt_tokens: Arc::new(AtomicU64::new(4_000)),
        prompt_tokens: Arc::new(AtomicU64::new(6_000)),
    };
    let kept = vec!["user\nContinue the original task.".into()];
    let original_kept = kept.clone();
    let summary = compactor
        .compact_with_tail(
            &[
                format!("assistant\ntool read\n{}", decision["call"]["args"]),
                format!("tool\n{wrapped}"),
            ],
            &kept,
        )
        .await
        .unwrap();
    assert!(summary.contains(output));
    assert!(!summary.contains("INTERNAL-RECEIPT-ONLY"));
    assert!(!summary.contains(FACT_TOOL_RESULT_KIND));
    assert!(!summary.contains("original provider continuation"));
    assert_eq!(client.calls.load(Ordering::SeqCst), 0);
    assert_eq!(kept, original_kept);
    assert_eq!(
        serde_json::to_value(log.read(THREAD).unwrap()).unwrap(),
        before
    );
}

#[tokio::test]
async fn archived_handoff_attempts_summary_before_falling_back_with_public_checkpoint() {
    use super::tool_history_tests::{append, call, result};
    let root = tempfile::tempdir().unwrap();
    let log = Arc::new(FileLog::open(log_dir(root.path())).unwrap());
    let executor = ToolExecutor::new(root.path().display().to_string());
    let store = executor.artifact_store();
    let mut older = call("older");
    older["call"]["text"] = serde_json::json!("");
    append(&log, "older-call", "model.turn", older.clone());
    let output = format!(
        "Original opaque observation Ω-6c92.\n{}",
        "raw data\n".repeat(1_000)
    );
    result(&log, "older-result", "older", &output);
    let mut next = call("next");
    let checkpoint = "Established public checkpoint 値🙂-8f34.";
    next["call"]["text"] = serde_json::json!(checkpoint);
    append(&log, "next-call", "model.turn", next.clone());
    result(&log, "next-result", "next", "Retained observation.");
    let before = serde_json::to_value(log.read(THREAD).unwrap()).unwrap();
    let client = Arc::new(BudgetSummaryClient {
        calls: AtomicUsize::new(0),
        text: "unused failed summary".into(),
        stop: "failure",
    });
    let compactor = SessionCompact {
        artifacts: Some(store.clone()),
        client: client.clone(),
        history: (log.clone(), THREAD.into()),
        source: None,
        events: None,
        budget: CompactionBudget::for_auto_compaction(4_096, 0.8, 0),
        fixed_prompt_tokens: Arc::new(AtomicU64::new(4_000)),
        prompt_tokens: Arc::new(AtomicU64::new(6_000)),
    };
    let kept = vec![
        format!("assistant\ntool read\n{}", next["call"]["args"]),
        "tool\nRetained observation.".into(),
    ];
    let original_kept = kept.clone();
    let summary = compactor
        .compact_with_tail(
            &[
                format!("assistant\ntool read\n{}", older["call"]["args"]),
                format!("tool\n{output}"),
            ],
            &kept,
        )
        .await
        .unwrap();
    assert_eq!(client.calls.load(Ordering::SeqCst), 1);
    assert!(summary.contains(checkpoint));
    assert!(!summary.contains("original provider continuation"));
    let artifacts = store.artifacts();
    assert_eq!(artifacts.len(), 1);
    assert!(summary.contains(&artifacts[0].artifact_uri));
    assert!(artifacts[0].content.contains(&output));
    assert!(artifacts[0].content.contains("Retained observation."));
    assert!(!artifacts[0]
        .content
        .contains("original provider continuation"));
    assert_eq!(kept, original_kept);
    assert_eq!(
        serde_json::to_value(log.read(THREAD).unwrap()).unwrap(),
        before
    );
}

#[tokio::test]
async fn latest_compaction_source_directly_recovers_older_original_observations() {
    use super::tool_history_tests::{append, call, result};
    let root = tempfile::tempdir().unwrap();
    let log = Arc::new(FileLog::open(log_dir(root.path())).unwrap());
    let executor = ToolExecutor::new_with_artifact_limits(
        root.path().display().to_string(),
        crate::tools::ArtifactStoreLimits {
            max_artifacts: 1,
            max_bytes: 64 * 1024,
        },
    );
    let store = executor.artifact_store();
    let compactor = SessionCompact {
        artifacts: Some(store.clone()),
        history: (log.clone(), THREAD.into()),
        source: None,
        client: Arc::new(BudgetSummaryClient {
            calls: AtomicUsize::new(0),
            text: "A deliberately lossy account.".into(),
            stop: "end_turn",
        }),
        events: None,
        budget: CompactionBudget::for_auto_compaction(2048, 0.8, 0),
        fixed_prompt_tokens: Arc::new(AtomicU64::new(0)),
        prompt_tokens: Arc::new(AtomicU64::new(4000)),
    };
    let mut summary = String::new();
    let mut first_uri = String::new();
    for (index, token) in ["Observation 値🙂-5a1e93", "Observation Ω-f4b237"]
        .into_iter()
        .enumerate()
    {
        let mut decision = call("reused-call-id");
        decision["call"]["reasoning"] = serde_json::json!("SYNTHETIC-PRIVATE-CONTINUATION");
        append(
            &log,
            &format!("model-{index}"),
            "model.turn",
            decision.clone(),
        );
        let output = format!("{token}\n{}", "public data\n".repeat(700));
        result(&log, &format!("result-{index}"), "reused-call-id", &output);
        let before = log.read(THREAD).unwrap();
        let lines = vec![
            format!("user\n{summary}"),
            format!("assistant\ntool read\n{}", decision["call"]["args"]),
            format!("tool\n{output}"),
        ];
        summary = compactor.compact(&lines).await.unwrap();
        assert!(!summary.contains(token));
        assert_eq!(
            serde_json::to_value(log.read(THREAD).unwrap()).unwrap(),
            serde_json::to_value(before).unwrap()
        );
        if index == 0 {
            first_uri = store.artifacts()[0].artifact_uri.clone();
            append(
                &log,
                "compact-1",
                "compaction.done",
                serde_json::json!({"summary":summary,"kept":[]}),
            );
        }
    }
    let archived = store.artifacts();
    assert_eq!(archived.len(), 1);
    let latest = &archived[0];
    assert!(summary.contains(&latest.artifact_uri));
    assert!(store.get(&first_uri).is_none());
    assert!(
        latest.content.contains("Observation 値🙂-5a1e93"),
        "latest source must recover the original directly, without the evicted source"
    );
    assert!(latest.content.contains("Observation Ω-f4b237"));
    assert_eq!(latest.content.matches("Observation 値🙂-5a1e93").count(), 1);
    assert!(!latest.content.contains(&first_uri));
    assert!(!latest.content.contains("SYNTHETIC-PRIVATE-CONTINUATION"));
    let found = executor
        .registry()
        .execute(
            "search",
            &serde_json::json!({"mode":"grep","path":latest.artifact_uri,"query":"Observation"}),
        )
        .await
        .unwrap();
    assert_eq!(found.exit_code, 0);
    assert!(found.output.contains("Observation 値🙂-5a1e93"));
    assert!(found.output.contains("Observation Ω-f4b237"));
}

#[tokio::test]
async fn fact_compaction_archives_public_results_without_private_call_metadata() {
    use super::tool_history_tests::{append, call, result};
    let root = tempfile::tempdir().unwrap();
    let log = Arc::new(FileLog::open(log_dir(root.path())).unwrap());
    let executor = ToolExecutor::new(root.path().display().to_string());
    let store = executor.artifact_store();
    let private = "SYNTHETIC-PRIVATE-CONTINUATION";
    let token = "Opaque observation Ω-9f773c";
    let tail = "END-PUBLIC-OBSERVATION Ω-81af0";
    let output = format!("{token}\n{}{tail}", "public data\n".repeat(1000));
    let wrapped = super::receipts::runner_result_value(&output, false, &[], 0, None, None);
    let wrapped = wrapped.as_str().unwrap();
    let mut decision = call("original");
    decision["call"]["reasoning"] = serde_json::json!(private);
    append(&log, "model", "model.turn", decision.clone());
    result(&log, "result", "original", wrapped);
    let before = log.read(THREAD).unwrap();
    let compactor = SessionCompact {
        artifacts: Some(store.clone()),
        history: (log.clone(), THREAD.into()),
        source: None,
        client: Arc::new(BudgetSummaryClient {
            calls: AtomicUsize::new(0),
            text: "general summary".into(),
            stop: "end_turn",
        }),
        events: None,
        budget: CompactionBudget::for_auto_compaction(2048, 0.8, 0),
        fixed_prompt_tokens: Arc::new(AtomicU64::new(0)),
        prompt_tokens: Arc::new(AtomicU64::new(4000)),
    };
    let lines = vec![
        "user\nRetain original observations.".into(),
        format!("assistant\ntool read\n{}", decision["call"]["args"]),
        format!("tool\n{wrapped}"),
    ];
    let summary = compactor.compact(&lines).await.unwrap();
    assert!(!summary.contains(token));
    let artifacts = store.artifacts();
    assert_eq!(artifacts.len(), 1);
    assert!(summary.contains(&artifacts[0].artifact_uri));
    assert!(artifacts[0].content.contains(token));
    assert!(!artifacts[0].content.contains(private));
    let read = executor
        .registry()
        .execute(
            "read",
            &serde_json::json!({"file_path":artifacts[0].artifact_uri}),
        )
        .await
        .unwrap();
    assert_eq!(read.exit_code, 0);
    assert!(read.output.contains(token));
    let found = executor
        .registry()
        .execute(
            "search",
            &serde_json::json!({
                "mode":"grep", "path":artifacts[0].artifact_uri, "query":tail,
            }),
        )
        .await
        .unwrap();
    assert_eq!(found.exit_code, 0);
    assert!(found.output.contains(tail));
    assert_eq!(
        serde_json::to_value(log.read(THREAD).unwrap()).unwrap(),
        serde_json::to_value(before).unwrap()
    );
}

#[async_trait::async_trait]
impl LlmClient for BudgetSummaryClient {
    async fn complete(
        &self,
        _: &[Message],
        _: Option<&str>,
        _: &[ToolDefinition],
    ) -> Result<LlmResponse> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.stop == "failure" {
            anyhow::bail!("synthetic transport failure");
        }
        Ok(LlmResponse {
            message: Message::assistant(&self.text),
            usage: TokenUsage::default(),
            stop_reason: Some(self.stop.into()),
            token_logprobs: Vec::new(),
            meta: None,
        })
    }

    async fn complete_streaming(
        &self,
        _: &[Message],
        _: Option<&str>,
        _: &[ToolDefinition],
        _: tokio_util::sync::CancellationToken,
    ) -> Result<tokio::sync::mpsc::Receiver<StreamEvent>> {
        anyhow::bail!("summary fixture is non-streaming");
    }
}

#[tokio::test]
async fn automatic_handoffs_bound_success_failure_and_truncation_across_windows() {
    for window in [1_024, 4_096, 16_384, 65_536] {
        let budget = CompactionBudget::for_auto_compaction(window, 0.8, 0);
        for stop in ["end_turn", "failure", "max_tokens"] {
            let client = BudgetSummaryClient {
                calls: AtomicUsize::new(0),
                text: "起点🙂".repeat(20_000),
                stop,
            };
            let lines = vec![format!("tool\n{}", "输入终点🙂".repeat(20_000))];
            let summary = compact_handoff(&client, &lines, budget, None).await;
            assert!(
                summary.len() <= budget.summary_token_limit() * 4 + "## Summary\n".len(),
                "window={window}, stop={stop}, bytes={}",
                summary.len()
            );
            assert_eq!(client.calls.load(Ordering::SeqCst), 1);
            if stop != "end_turn" {
                assert!(
                    summary.len() <= bounded_summary(&lines).len() + "## Summary\n".len(),
                    "fallback expanded the existing handoff: window={window}, bytes={}",
                    summary.len()
                );
            }
            assert_eq!(lines.len(), 1);
            assert!(lines[0].ends_with("输入终点🙂"));
        }
    }
}

#[tokio::test]
async fn fitting_handoff_does_not_spend_a_model_call_or_rewrite_public_values() {
    let client = BudgetSummaryClient {
        calls: AtomicUsize::new(0),
        text: "the model replaced the checkpoint".into(),
        stop: "end_turn",
    };
    let checkpoint = "assistant\nOpaque checkpoint: 鳶🙂 = 4e8b6fc1; pending = [17, 29].";
    let summary = compact_handoff(
        &client,
        &[checkpoint.into()],
        CompactionBudget::for_auto_compaction(65_536, 0.8, 0),
        None,
    )
    .await;
    assert_eq!(client.calls.load(Ordering::SeqCst), 0);
    assert!(summary.contains(checkpoint));
}

#[tokio::test]
async fn lossless_handoff_does_not_generate_for_host_sealing_overhead() {
    let budget = CompactionBudget::for_auto_compaction(16_384, 0.85, 16_384);
    let allowance = budget.summary_token_limit() * 4;
    let goal = "Preserve original identifiers Ω/値🙂 and acceptance criteria. "
        .repeat(20)
        .trim_end()
        .to_string();
    let cases = [
        vec![
            format!("user\n{goal}"),
            "assistant\nObserved opaque value ξ=9b; validation remains pending.".into(),
        ],
        vec![format!(
            "assistant\n{}",
            "x".repeat(allowance - "assistant\n".len())
        )],
    ];
    let mut calls = Vec::new();
    for messages in cases {
        let original = messages.join("\n");
        assert!(original.len() <= allowance);
        let client = BudgetSummaryClient {
            calls: AtomicUsize::new(0),
            text: "the fitting transcript must never reach this model".into(),
            stop: "failure",
        };
        let handoff = compact_handoff(&client, &messages, budget, None).await;
        assert!(handoff.contains(&original), "lossless input changed");
        assert!(
            handoff.len() > allowance,
            "fixture must exercise host sealing overhead"
        );
        calls.push(client.calls.load(Ordering::SeqCst));
    }
    assert_eq!(
        calls,
        [0, 0],
        "host-restored sections must not force model generation"
    );
}

#[tokio::test]
async fn fallback_strips_repeated_goal_before_capping_observed_state() {
    let budget = CompactionBudget::for_auto_compaction(16_384, 0.85, 16_384);
    let goal = "Preserve original obligations and opaque identifiers Ω/値🙂. ".repeat(80);
    let carried = crate::compaction_carry::obligations_from_messages(&[Message::user(&goal)]);
    assert!(goal.len() > budget.summary_token_limit() * 4);
    let states = [
        "## Current State\nObserved checkpoint Ω-14c9; final verification remains pending."
            .to_string(),
        format!(
            "## Current State\n{}",
            "Observed checkpoint Ω/値🙂; final verification remains pending.\n"
                .repeat(24)
                .trim_end()
        ),
    ];
    assert!(states[1].len() > budget.summary_token_limit() * 2);
    for state in states {
        assert!(state.len() < budget.summary_token_limit() * 4);
        let previous = crate::compaction_carry::seal_summary(&state, &carried);
        let lines = vec![format!(
            "user\n{}{}",
            crate::prompts::CONTEXT_SUMMARY_PREFIX,
            previous
        )];
        for stop in ["failure", "max_tokens"] {
            let client = BudgetSummaryClient {
                calls: AtomicUsize::new(0),
                text: "unusable truncated model output".into(),
                stop,
            };
            let summary = compact_handoff(&client, &lines, budget, None).await;
            assert_eq!(client.calls.load(Ordering::SeqCst), 1);
            assert!(
                summary.contains(goal.trim()),
                "original obligations changed"
            );
            assert!(
                summary.contains(&state),
                "a repeated goal displaced fitting observed state on {stop}"
            );
        }
    }
}

#[tokio::test]
async fn automatic_budget_reserves_fixed_prompt_and_retained_tail_and_reports_total() {
    let root = tempfile::tempdir().unwrap();
    let client = Arc::new(BudgetSummaryClient {
        calls: AtomicUsize::new(0),
        text: "opaque-summary🙂".repeat(1_000),
        stop: "end_turn",
    });
    let (events, mut received) = tokio::sync::mpsc::channel(4);
    let compactor = SessionCompact {
        artifacts: None,
        client: client.clone(),
        history: (Arc::new(FileLog::open(root.path()).unwrap()), THREAD.into()),
        source: None,
        events: Some(events),
        budget: CompactionBudget::for_auto_compaction(4_096, 0.8, 0),
        fixed_prompt_tokens: Arc::new(AtomicU64::new(1_000)),
        prompt_tokens: Arc::new(AtomicU64::new(4_000)),
    };
    let kept = vec![format!("tool\n{}", "retained🙂".repeat(200))];
    let original_kept = kept.clone();
    let retained_tokens = crate::llm::estimate_message_tokens(&[message_from_folded_tool(
        "",
        &"retained🙂".repeat(200),
        false,
    )]);
    let expected = compactor
        .budget
        .with_fixed_prompt_tokens(1_000 + retained_tokens);
    let summary = compactor
        .compact_with_tail(
            &[format!("tool\n{}", "old observation".repeat(1_000))],
            &kept,
        )
        .await
        .unwrap();
    assert_eq!(client.calls.load(Ordering::SeqCst), 1);
    assert!(summary.len() <= expected.summary_token_limit() * 4 + "## Summary\n".len());
    assert_eq!(kept, original_kept);
    assert!(
        matches!(received.recv().await, Some(crate::agent::AgentEvent::AutoCompact { phase, .. }) if phase == "started")
    );
    let Some(crate::agent::AgentEvent::AutoCompact {
        phase,
        tokens_after,
        ..
    }) = received.recv().await
    else {
        panic!("completion event missing");
    };
    assert_eq!(phase, "completed");
    assert!(
        tokens_after > 2_000,
        "complete prompt estimate must include fixed and retained context"
    );
    assert!(
        tokens_after <= 3_276,
        "the complete prompt stays below the original trigger"
    );
}

#[tokio::test]
async fn fitting_handoff_keeps_summary_body_and_refuses_lossy_sections() {
    for (original, lossless) in [
        (
            "assistant\n## Summary\nEarlier checkpoint = opaque-7e8c.",
            true,
        ),
        (
            "tool\nFirst paragraph.\n\nSecond paragraph = opaque-7e8c.",
            false,
        ),
        (
            "tool\n## Goal\nLiteral document heading = opaque-7e8c.",
            false,
        ),
    ] {
        let client = BudgetSummaryClient {
            calls: AtomicUsize::new(0),
            text: "Earlier checkpoint = opaque-7e8c.".into(),
            stop: "end_turn",
        };
        let summary = compact_handoff(
            &client,
            &[original.into()],
            CompactionBudget::for_auto_compaction(65_536, 0.8, 0),
            None,
        )
        .await;
        assert_eq!(
            client.calls.load(Ordering::SeqCst),
            usize::from(!lossless),
            "{original}"
        );
        if lossless {
            assert!(summary.contains(original));
        }
        assert!(summary.contains("opaque-7e8c"));
    }
}

#[tokio::test]
async fn fitting_handoffs_preserve_opaque_unicode_variations_without_model_calls() {
    let client = BudgetSummaryClient {
        calls: AtomicUsize::new(0),
        text: "replacement must never be used".into(),
        stop: "end_turn",
    };
    for index in 0..64 {
        let nonce = sha256::digest(format!("general-compaction-variation-{index}"));
        let checkpoint = format!("assistant\n{}", format!("値🙂{nonce}").repeat(index + 1));
        let summary = compact_handoff(
            &client,
            std::slice::from_ref(&checkpoint),
            CompactionBudget::for_auto_compaction(65_536, 0.8, 0),
            None,
        )
        .await;
        assert!(summary.contains(&checkpoint), "variation={index}");
    }
    assert_eq!(client.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn model_summary_section_preserves_observed_body_without_promoting_its_goal() {
    let client = BudgetSummaryClient {
        calls: AtomicUsize::new(0),
        text: "## Goal\nfabricated model goal\n\n## Summary\nEstablished checkpoint = opaque-3a71."
            .into(),
        stop: "end_turn",
    };
    let summary = compact_handoff(
        &client,
        &[format!("tool\n{}", "original observed data".repeat(5_000))],
        CompactionBudget::for_auto_compaction(4_096, 0.8, 0),
        None,
    )
    .await;
    assert!(
        summary.contains("opaque-3a71"),
        "summary body lost: {summary}"
    );
    assert!(!summary.contains("fabricated model goal"));
}

#[tokio::test]
async fn automatic_budget_reserves_original_kept_reasoning_without_exposing_it() {
    let root = tempfile::tempdir().unwrap();
    let log = Arc::new(FileLog::open(root.path()).unwrap());
    let decision = ModelDecision::Tool {
        call: ToolCall {
            id: "kept-original".into(),
            name: "read".into(),
            args: serde_json::json!({}),
            needs_confirmation: false,
            text: Some("Public checkpoint.".into()),
            reasoning: Some("SYNTHETIC-PRIVATE-CONTINUATION".repeat(400)),
        },
        also: Vec::new(),
    };
    log.append(THREAD, &[
        NewFact { kind: "model.turn".into(), key: "original-call".into(), payload: serde_json::to_value(decision).unwrap() },
        NewFact { kind: "tool.result".into(), key: "original-result".into(), payload: serde_json::json!({"toolCallId":"kept-original","ok":true,"output":"observed"}) },
    ], None).unwrap();
    let compactor = SessionCompact {
        artifacts: None,
        client: Arc::new(BudgetSummaryClient {
            calls: AtomicUsize::new(0),
            text: "summary body🙂".repeat(2_000),
            stop: "end_turn",
        }),
        history: (log, THREAD.into()),
        source: None,
        events: None,
        budget: CompactionBudget::for_auto_compaction(4_096, 0.8, 0),
        fixed_prompt_tokens: Arc::new(AtomicU64::new(1_000)),
        prompt_tokens: Arc::new(AtomicU64::new(4_000)),
    };
    let summary = compactor
        .compact_with_tail(
            &[format!("tool\n{}", "older observed data".repeat(2_000))],
            &["assistant\ntool read\n{}".into(), "tool\nobserved".into()],
        )
        .await
        .unwrap();
    assert!(
        summary.len() <= 512 * 4 + "## Summary\n".len(),
        "kept continuation was not reserved: {} bytes",
        summary.len()
    );
    assert!(!summary.contains("SYNTHETIC-PRIVATE-CONTINUATION"));
}
