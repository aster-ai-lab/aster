use super::*;
use crate::fact::NewFact;
use crate::log::{LogStore, MemoryLog};
use std::sync::atomic::{AtomicUsize, Ordering};

struct ObservedModel {
    calls: AtomicUsize,
    usage: Vec<Option<u64>>,
}

impl Completion for ObservedModel {
    fn complete(
        &self,
        _: CompletionRequest,
        _: CancellationToken,
    ) -> BoxFuture<Result<ModelDecision, ActorError>> {
        let index = self.calls.fetch_add(1, Ordering::SeqCst);
        let decision = if index < 3 {
            ModelDecision::Tool {
                call: ToolCall {
                    id: format!("observation-{index}"),
                    name: "observe".into(),
                    args: json!({"page":index}),
                    needs_confirmation: false,
                    text: None,
                    reasoning: None,
                },
                also: vec![],
            }
        } else {
            ModelDecision::Text {
                text: "done".into(),
            }
        };
        Box::pin(async move { Ok(decision) })
    }

    fn reported_prompt_tokens(&self) -> Option<u64> {
        self.usage[self.calls.load(Ordering::SeqCst) - 1]
    }
}

struct LargeObservation;

impl ToolRunner for LargeObservation {
    fn run(&self, _: ToolCall, _: CancellationToken) -> BoxFuture<Result<Value, ActorError>> {
        Box::pin(async { Ok(json!("x".repeat(2048))) })
    }
}

struct Summarizer(Arc<AtomicUsize>);

impl Compactor for Summarizer {
    fn compact(&self, _: &[String]) -> BoxFuture<Result<String, ActorError>> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Ok("earlier observations".into()) })
    }
}

async fn pressure(usage: Vec<Option<u64>>) -> (usize, Vec<Fact>) {
    let compactions = Arc::new(AtomicUsize::new(0));
    let services = Arc::new(CodingServices {
        completion: Arc::new(ObservedModel {
            calls: AtomicUsize::new(0),
            usage,
        }),
        tools: Arc::new(LargeObservation),
        compactor: Arc::new(Summarizer(compactions.clone())),
    });
    let config = HarnessConfig::new(8, 1024, 64, 1, vec![], vec![])
        .unwrap()
        .with_token_compact(10_000, 1);
    let result = ingest_coding(
        &coding_actor(config),
        &MemoryLog::new(),
        services,
        "pressure",
        message_fact("request", "Observe three pages."),
        64,
    )
    .await
    .unwrap();
    assert!(result.view.schema_error.is_none());
    assert_eq!(result.view.assistant.as_deref(), Some("done"));
    (compactions.load(Ordering::SeqCst), result.log)
}

#[tokio::test]
async fn missing_provider_usage_keeps_character_compaction_available() {
    let (compactions, facts) = pressure(vec![None; 4]).await;
    eprintln!("missing usage: {compactions} compactions");
    assert!(
        compactions > 0,
        "missing usage must not disable every pressure signal"
    );
    assert!(facts
        .iter()
        .filter(|f| f.kind == "model.turn")
        .all(|f| f.payload.get("prompt_tokens").is_none()));
}

#[tokio::test]
async fn a_later_missing_usage_does_not_reuse_an_old_small_prompt() {
    let (compactions, _) = pressure(vec![Some(16), None, None, None]).await;
    eprintln!("later missing usage: {compactions} compactions");
    assert!(
        compactions > 0,
        "the first response cannot describe later grown prompts"
    );
}

#[tokio::test]
async fn fresh_reported_usage_keeps_priority_over_the_character_estimate() {
    let (compactions, _) = pressure(vec![Some(16); 4]).await;
    eprintln!("fresh known usage: {compactions} compactions");
    assert_eq!(compactions, 0);
}

#[test]
fn prunable_byte_pressure_compacts_without_a_message_prefix() {
    let config = HarnessConfig::new(8, usize::MAX, 64, 1, vec![], vec![])
        .unwrap()
        .with_token_compact(u64::MAX, 1)
        .with_compact_after_prunable_bytes(64);
    let state = SchedulerState {
        prunable_fact_bytes: 128,
        ..SchedulerState::new()
    };
    let compacted = after_input(state, &config);
    assert_eq!(compacted.phase, CodingPhase::Compact);
    assert!(compacted.messages.is_empty());

    let awaiting = after_input(
        SchedulerState {
            prunable_fact_bytes: 128,
            compacted_awaiting_model: true,
            phase: CodingPhase::Infer,
            ..SchedulerState::new()
        },
        &config,
    );
    assert_eq!(awaiting.phase, CodingPhase::Infer);
}

#[test]
fn large_post_model_tool_burst_triggers_proactive_compaction_with_reported_usage() {
    let config = HarnessConfig::new(8, 1024, 64, 1, vec![], vec![])
        .unwrap()
        .with_token_compact(1_000, 1);
    let state = SchedulerState {
        messages: vec!["prior turn".into(), "large tool output".into()],
        prompt_tokens: Some(900),
        tokens_since_model: 200,
        phase: CodingPhase::Infer,
        ..SchedulerState::new()
    };
    let next = after_input(state, &config);
    assert_eq!(next.phase, CodingPhase::Compact);
}

#[tokio::test]
async fn context_overflow_triggers_bounded_emergency_compaction_and_recovers() {
    struct OverflowThenSuccess {
        calls: AtomicUsize,
        fail_always: bool,
    }

    impl Completion for OverflowThenSuccess {
        fn complete(
            &self,
            _: CompletionRequest,
            _: CancellationToken,
        ) -> BoxFuture<Result<ModelDecision, ActorError>> {
            let n = self.calls.fetch_add(1, Ordering::SeqCst);
            let fail_always = self.fail_always;
            Box::pin(async move {
                if n == 0 || fail_always {
                    Err(ActorError::ContextOverflow(
                        "HTTP 400: maximum context length exceeded".into(),
                    ))
                } else {
                    Ok(ModelDecision::Text {
                        text: "recovered after compaction".into(),
                    })
                }
            })
        }
    }

    let compactions = Arc::new(AtomicUsize::new(0));
    let services = Arc::new(CodingServices {
        completion: Arc::new(OverflowThenSuccess {
            calls: AtomicUsize::new(0),
            fail_always: false,
        }),
        tools: Arc::new(LargeObservation),
        compactor: Arc::new(Summarizer(Arc::clone(&compactions))),
    });
    let config = HarnessConfig::new(8, usize::MAX, 64, 1, vec![], vec![])
        .unwrap()
        .with_token_compact(u64::MAX, 1);
    let actor = coding_actor(config.clone());
    let log = MemoryLog::new();
    // Seed a multi-message history so emergency compaction has entries to fold.
    let seeded = vec![
        NewFact {
            kind: "user.message".into(),
            key: "seed-u1".into(),
            payload: json!({ "text": "first task" }),
        },
        NewFact {
            kind: "model.turn".into(),
            key: "seed-a1".into(),
            payload: json!({
                "kind": "text",
                "text": "x".repeat(4096),
            }),
        },
        NewFact {
            kind: "user.message".into(),
            key: "seed-u2".into(),
            payload: json!({ "text": "continue after seed" }),
        },
    ];
    log.append("overflow-recover", &seeded, None).unwrap();
    let result = ingest_coding(
        &actor,
        &log,
        services,
        "overflow-recover",
        message_fact("seed-u3", "finish"),
        64,
    )
    .await
    .unwrap();
    assert_eq!(
        result.view.assistant.as_deref(),
        Some("recovered after compaction")
    );
    assert_eq!(compactions.load(Ordering::SeqCst), 1);
    assert!(
        result.log.iter().any(|f| {
            f.kind == "compaction.done" && f.payload["overflow_recovery"] == json!(true)
        }),
        "expected overflow_recovery compaction fact"
    );

    // Repeating ContextOverflow after the bounded emergency compaction must fail closed.
    let always_overflow = Arc::new(CodingServices {
        completion: Arc::new(OverflowThenSuccess {
            calls: AtomicUsize::new(0),
            fail_always: true,
        }),
        tools: Arc::new(LargeObservation),
        compactor: Arc::new(Summarizer(Arc::new(AtomicUsize::new(0)))),
    });
    let log2 = MemoryLog::new();
    log2.append("overflow-bounded", &seeded, None).unwrap();
    let Err(err) = ingest_coding(
        &coding_actor(config),
        &log2,
        always_overflow,
        "overflow-bounded",
        message_fact("seed-u3", "finish"),
        64,
    )
    .await
    else {
        panic!("expected ContextOverflow failure after single recovery attempt");
    };
    assert!(
        matches!(&err, Exit::Die(msg) if msg.contains("maximum context length exceeded")),
        "expected bounded non-retrying Die(ContextOverflow), got {err:?}"
    );
}

#[test]
fn inspect_coding_phase_distinguishes_idle_resumable_and_unresolved_tool() {
    let idle = inspect_coding_phase(&[
        Fact {
            seq: 1,
            kind: "user.message".into(),
            key: "u1".into(),
            cause: None,
            payload: json!({ "text": "hi" }),
        },
        Fact {
            seq: 2,
            kind: "model.turn".into(),
            key: "a1".into(),
            cause: None,
            payload: json!({
                "kind": "text",
                "text": "done",
            }),
        },
    ])
    .unwrap();
    assert_eq!(idle.phase, CodingPhase::Done);
    assert!(!idle.has_unresolved_preparation);

    let resumable = inspect_coding_phase(&[Fact {
        seq: 1,
        kind: "user.message".into(),
        key: "u1".into(),
        cause: None,
        payload: json!({ "text": "crash mid infer" }),
    }])
    .unwrap();
    assert_eq!(resumable.phase, CodingPhase::Infer);
    assert!(!resumable.has_unresolved_preparation);

    let args = json!({"file_path": "a.txt"});
    let digest = tool_args_digest(&args).unwrap();
    let prepared = ToolPreparedV1 {
        schema: TOOL_PREPARED_SCHEMA.into(),
        identity: ToolInvocationIdentityV1 {
            turn: 1,
            cycle: 0,
            tool_call_id: "call-1".into(),
            batch_index: 0,
            admission: None,
        },
        tool_name: "write".into(),
        original_args_digest: digest.clone(),
        effective_args_digest: digest,
        recovery: ToolRecoveryPolicy::NeverReplay,
        baseline: None,
    };
    let unresolved = inspect_coding_phase(&[
        Fact {
            seq: 1,
            kind: "user.message".into(),
            key: "u1".into(),
            cause: None,
            payload: json!({ "text": "run tool" }),
        },
        Fact {
            seq: 2,
            kind: "model.turn".into(),
            key: "m1".into(),
            cause: None,
            payload: json!({
                "kind": "tool",
                "tool_effect_protocol": 1,
                "call": {
                    "id": "call-1",
                    "name": "write",
                    "args": args,
                    "needs_confirmation": false,
                },
                "also": [],
            }),
        },
        Fact {
            seq: 3,
            kind: "tool.prepared".into(),
            key: "prep-1".into(),
            cause: Some("m1".into()),
            payload: json!({ "prepared": prepared }),
        },
    ])
    .unwrap();
    assert!(unresolved.has_unresolved_preparation);
}

#[tokio::test]
async fn replay_safe_preparation_replays_cleanly_on_resume_without_unresolved_debt() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct ResumeModel(AtomicUsize);
    impl Completion for ResumeModel {
        fn complete(
            &self,
            _request: CompletionRequest,
            _cancel: CancellationToken,
        ) -> BoxFuture<Result<ModelDecision, ActorError>> {
            let call_index = self.0.fetch_add(1, Ordering::SeqCst);
            Box::pin(async move {
                if call_index == 0 {
                    Ok(ModelDecision::Text {
                        text: "replayed cleanly".into(),
                    })
                } else {
                    Ok(ModelDecision::Text {
                        text: "unexpected extra call".into(),
                    })
                }
            })
        }
    }

    struct PassCompactor;
    impl Compactor for PassCompactor {
        fn compact(&self, _messages: &[String]) -> BoxFuture<Result<String, ActorError>> {
            Box::pin(async { Ok(String::new()) })
        }
    }

    struct ReplayTools {
        executions: Arc<AtomicUsize>,
    }
    impl ToolRunner for ReplayTools {
        fn run(
            &self,
            _call: ToolCall,
            _cancel: CancellationToken,
        ) -> BoxFuture<Result<Value, ActorError>> {
            unreachable!("prepared runner only")
        }

        fn prepare(
            self: Arc<Self>,
            request: ToolPreparationRequest,
            _cancel: CancellationToken,
        ) -> BoxFuture<Result<PreparedToolExecution, ActorError>> {
            Box::pin(async move {
                let mut identity = request.identity;
                identity.admission = Some(ToolAdmissionIdentityV1 {
                    run_id: "resumed-run".into(),
                    thread_id: "replay-thread".into(),
                    capability_binding_digest: format!("sha256:{}", "b".repeat(64)),
                });
                let digest = tool_args_digest(&request.call.args)?;
                let prepared = ToolPreparedV1 {
                    schema: TOOL_PREPARED_SCHEMA.into(),
                    identity,
                    tool_name: request.call.name,
                    original_args_digest: digest.clone(),
                    effective_args_digest: digest,
                    recovery: ToolRecoveryPolicy::ReplaySafe,
                    baseline: None,
                };
                let for_execute = prepared.clone();
                let executions = Arc::clone(&self.executions);
                let execute = Effect::from_async_observed(move |_services, _cancel| {
                    let executions = Arc::clone(&executions);
                    let prepared = for_execute.clone();
                    async move {
                        executions.fetch_add(1, Ordering::SeqCst);
                        Ok(ToolExecutionResultV1 {
                            schema: TOOL_RESULT_SCHEMA.into(),
                            identity: prepared.identity.clone(),
                            prepared_digest: prepared.digest().map_err(Exit::Fail)?,
                            outcome: ToolExecutionOutcome::Succeeded,
                            exit_code: Some(0),
                            output: json!("REPLAYED-OUTPUT"),
                            effects: ToolEffectObservationV1::complete(Vec::new()),
                        })
                    }
                });
                Ok(PreparedToolExecution { prepared, execute })
            })
        }
    }

    let args = json!({ "file_path": "note.txt" });
    let digest = tool_args_digest(&args).unwrap();
    let original_prepared = ToolPreparedV1 {
        schema: TOOL_PREPARED_SCHEMA.into(),
        identity: ToolInvocationIdentityV1 {
            turn: 1,
            cycle: 0,
            tool_call_id: "call-read-1".into(),
            batch_index: 0,
            admission: Some(ToolAdmissionIdentityV1 {
                run_id: "crashed-run".into(),
                thread_id: "replay-thread".into(),
                capability_binding_digest: format!("sha256:{}", "a".repeat(64)),
            }),
        },
        tool_name: "read".into(),
        original_args_digest: digest.clone(),
        effective_args_digest: digest,
        recovery: ToolRecoveryPolicy::ReplaySafe,
        baseline: None,
    };
    let seeded_facts = vec![
        Fact {
            seq: 1,
            kind: "user.message".into(),
            key: "u1".into(),
            cause: None,
            payload: json!({ "text": "read note" }),
        },
        Fact {
            seq: 2,
            kind: "model.turn".into(),
            key: "m1".into(),
            cause: None,
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
        Fact {
            seq: 3,
            kind: "tool.prepared".into(),
            key: "prep-1".into(),
            cause: Some("m1".into()),
            payload: json!({ "prepared": original_prepared }),
        },
    ];
    let probe = inspect_coding_phase(&seeded_facts).unwrap();
    assert_eq!(probe.phase, CodingPhase::Tool);
    assert!(
        !probe.has_unresolved_preparation,
        "ReplaySafe preparation must remain resumable without unresolved debt"
    );

    let log = crate::log::MemoryLog::new();
    let new_facts: Vec<NewFact> = seeded_facts
        .into_iter()
        .map(|fact| NewFact {
            kind: fact.kind,
            key: fact.key,
            payload: fact.payload,
        })
        .collect();
    log.append("replay-thread", &new_facts, None).unwrap();

    let executions = Arc::new(AtomicUsize::new(0));
    let services = Arc::new(CodingServices {
        completion: Arc::new(ResumeModel(AtomicUsize::new(0))),
        tools: Arc::new(ReplayTools {
            executions: Arc::clone(&executions),
        }),
        compactor: Arc::new(PassCompactor),
    });
    let actor = coding_actor(
        HarnessConfig::new(100_000, usize::MAX, 16, 1, Vec::new(), Vec::new()).unwrap(),
    );
    let settled = resume_coding_with_cancel(
        &actor,
        &log,
        services,
        "replay-thread",
        16,
        &CancellationToken::new(),
    )
    .await
    .expect("ReplaySafe interrupted tool should resume and settle cleanly");
    assert_eq!(settled.view.assistant.as_deref(), Some("replayed cleanly"));
    assert_eq!(executions.load(Ordering::SeqCst), 1);

    let final_facts = log.read("replay-thread").unwrap();
    assert_eq!(
        final_facts
            .iter()
            .filter(|f| f.kind == "tool.prepared")
            .count(),
        1,
        "replay must reuse durable tool.prepared rather than appending a duplicate"
    );
    let results: Vec<_> = final_facts
        .iter()
        .filter(|f| f.kind == "tool.result")
        .collect();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].payload["ok"], json!(true));
    assert_eq!(
        results[0].payload["receipt"]["output"],
        json!("REPLAYED-OUTPUT")
    );
}
