use super::*;
use crate::log::MemoryLog;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

struct ScriptedModel(Mutex<VecDeque<ModelDecision>>);

impl Completion for ScriptedModel {
    fn complete(
        &self,
        _: CompletionRequest,
        _: CancellationToken,
    ) -> BoxFuture<Result<ModelDecision, ActorError>> {
        let decision = self.0.lock().unwrap().pop_front().unwrap();
        Box::pin(async move { Ok(decision) })
    }
}

struct CountedCompactor(Arc<AtomicUsize>);

impl Compactor for CountedCompactor {
    fn compact(&self, _: &[String]) -> BoxFuture<Result<String, ActorError>> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Ok("Earlier observations are summarized.".into()) })
    }
}

struct ObservedTools {
    preparations: Arc<AtomicUsize>,
    executions: Arc<AtomicUsize>,
    fresh: bool,
}

impl ToolRunner for ObservedTools {
    fn run(&self, _: ToolCall, _: CancellationToken) -> BoxFuture<Result<Value, ActorError>> {
        unreachable!("the typed fixture executes through prepare")
    }

    fn prepare(
        self: Arc<Self>,
        request: ToolPreparationRequest,
        _: CancellationToken,
    ) -> BoxFuture<Result<PreparedToolExecution, ActorError>> {
        self.preparations.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move {
            let args = tool_args_digest(&request.call.args)?;
            let prepared = ToolPreparedV1 {
                schema: TOOL_PREPARED_SCHEMA.into(),
                identity: request.identity,
                tool_name: request.call.name,
                original_args_digest: args.clone(),
                effective_args_digest: args,
                recovery: ToolRecoveryPolicy::NeverReplay,
                baseline: None,
            };
            let bound = prepared.clone();
            let execute = Effect::from_async_observed(move |_, _| {
                let bound = bound.clone();
                let tools = self.clone();
                async move {
                    let index = tools.executions.fetch_add(1, Ordering::SeqCst);
                    Ok(ToolExecutionResultV1 {
                        schema: TOOL_RESULT_SCHEMA.into(),
                        identity: bound.identity.clone(),
                        prepared_digest: bound.digest().map_err(Exit::Fail)?,
                        outcome: ToolExecutionOutcome::Succeeded,
                        exit_code: Some(0),
                        output: if tools.fresh {
                            json!(index)
                        } else {
                            json!("unchanged")
                        },
                        effects: ToolEffectObservationV1::complete(vec![]),
                    })
                }
            });
            Ok(PreparedToolExecution { prepared, execute })
        })
    }
}

async fn observations(fresh: bool, compact: bool) -> (usize, usize, usize, Vec<Fact>) {
    observations_count(fresh, compact, 7).await
}

async fn observations_count(
    fresh: bool,
    compact: bool,
    count: u32,
) -> (usize, usize, usize, Vec<Fact>) {
    let preparations = Arc::new(AtomicUsize::new(0));
    let executions = Arc::new(AtomicUsize::new(0));
    let compactions = Arc::new(AtomicUsize::new(0));
    let mut script: VecDeque<_> = (0..count)
        .map(|index| ModelDecision::Tool {
            call: ToolCall {
                id: format!("observation-{index}"),
                name: "read_state".into(),
                args: json!({}),
                needs_confirmation: false,
                text: None,
                reasoning: None,
            },
            also: vec![],
        })
        .collect();
    script.push_back(ModelDecision::Text {
        text: "done".into(),
    });
    let config = HarnessConfig::new(
        20.max(count + 1),
        if compact { 1 } else { usize::MAX },
        200.max(count * 8 + 20),
        1,
        vec![],
        vec![],
    )
    .unwrap()
    .with_keep_tokens(1)
    .with_duplicate_threshold(3);
    let services = Arc::new(CodingServices {
        completion: Arc::new(ScriptedModel(Mutex::new(script))),
        tools: Arc::new(ObservedTools {
            preparations: preparations.clone(),
            executions: executions.clone(),
            fresh,
        }),
        compactor: Arc::new(CountedCompactor(compactions.clone())),
    });
    let settled = ingest_coding(
        &coding_actor(config),
        &MemoryLog::new(),
        services,
        "convergence",
        message_fact("start", "Observe the state seven times."),
        200.max(count * 8 + 20),
    )
    .await
    .unwrap();
    assert!(settled.view.schema_error.is_none(), "{:?}", settled.view);
    assert_eq!(settled.view.assistant.as_deref(), Some("done"));
    (
        preparations.load(Ordering::SeqCst),
        executions.load(Ordering::SeqCst),
        compactions.load(Ordering::SeqCst),
        settled.log,
    )
}

#[tokio::test]
async fn duplicate_refusal_does_not_enter_adapter_preparation() {
    let (preparations, executions, _, _) = observations(false, false).await;
    assert_eq!(executions, 3);
    assert_eq!(
        preparations, 3,
        "a refused duplicate must not run input preparation"
    );
}

#[tokio::test]
async fn compaction_does_not_refresh_the_duplicate_allowance() {
    let (_, executions, compactions, facts) = observations(false, true).await;
    assert!(compactions > 0);
    assert!(facts.iter().any(|fact| fact.kind == "compaction.done"));
    assert_eq!(
        executions, 3,
        "retaining one tool pair must not erase earlier repeats"
    );
}

#[tokio::test]
async fn fresh_observations_allow_identical_arguments_without_workspace_mutations() {
    let (_, executions, _, _) = observations(true, false).await;
    assert_eq!(
        executions, 7,
        "each result added a new original observation"
    );
}

#[tokio::test]
async fn cancellation_after_preparation_does_not_prove_zero_effects() {
    let tools = Arc::new(ObservedTools {
        preparations: Arc::new(AtomicUsize::new(0)),
        executions: Arc::new(AtomicUsize::new(0)),
        fresh: false,
    });
    let prepared = tools
        .clone()
        .prepare(
            ToolPreparationRequest {
                identity: ToolInvocationIdentityV1 {
                    turn: 1,
                    cycle: 0,
                    tool_call_id: "cancelled-preparation".into(),
                    batch_index: 0,
                    admission: None,
                },
                call: ToolCall {
                    id: "cancelled-preparation".into(),
                    name: "observe".into(),
                    args: json!({}),
                    needs_confirmation: false,
                    text: None,
                    reasoning: None,
                },
            },
            CancellationToken::new(),
        )
        .await
        .unwrap();
    let cancel = CancellationToken::new();
    cancel.cancel();
    let services = Arc::new(CodingServices {
        completion: Arc::new(ScriptedModel(Mutex::new(VecDeque::new()))),
        tools: tools.clone(),
        compactor: Arc::new(CountedCompactor(Arc::new(AtomicUsize::new(0)))),
    });
    let result = run_prepared_one(&prepared, services, cancel).await.unwrap();
    assert_eq!(tools.executions.load(Ordering::SeqCst), 0);
    assert_eq!(result.outcome, ToolExecutionOutcome::Cancelled);
    assert_eq!(
        result.effects.completeness,
        ToolObservationCompleteness::Incomplete
    );
}

#[test]
fn legacy_results_keep_duplicate_counts_across_compaction_and_replay() {
    let config = HarnessConfig::new(20, usize::MAX, 200, 1, vec![], vec![])
        .unwrap()
        .with_duplicate_threshold(3);
    let mut events = vec![("user.message", json!({"text":"observe"}))];
    for index in 0..3 {
        events.push((
            "model.turn",
            serde_json::to_value(ModelDecision::Tool {
                call: ToolCall {
                    id: format!("old-{index}"),
                    name: "read_state".into(),
                    args: json!({}),
                    needs_confirmation: false,
                    text: None,
                    reasoning: None,
                },
                also: vec![],
            })
            .unwrap(),
        ));
        events.push((
            "tool.result",
            json!({"toolCallId":format!("old-{index}"),"ok":true,"output":"unchanged"}),
        ));
        events.push((
            "compaction.done",
            json!({"summary":"old observations","kept":[]}),
        ));
    }
    let facts: Vec<_> = events
        .into_iter()
        .enumerate()
        .map(|(index, (kind, payload))| Fact {
            seq: index as u64 + 1,
            kind: kind.into(),
            key: format!("old-{index}"),
            cause: None,
            payload,
        })
        .collect();
    let mut uninterrupted = SchedulerState::new();
    for fact in &facts {
        uninterrupted = step(uninterrupted, fact, &config);
    }
    let replayed = facts.iter().fold(SchedulerState::new(), |state, fact| {
        step(state, fact, &config)
    });
    assert!(
        replayed.schema_error.is_none(),
        "{:?}",
        replayed.schema_error
    );
    assert_eq!(uninterrupted.duplicate_calls, replayed.duplicate_calls);
    assert_eq!(
        replayed
            .duplicate_calls
            .get(&("read_state".into(), tool_args_digest(&json!({})).unwrap())),
        Some(&3)
    );
    assert!(
        replayed.seen_information.is_empty(),
        "old facts cannot invent information credit"
    );
}

#[tokio::test]
async fn typed_observation_credit_replays_and_missing_credit_stays_unknown() {
    let (_, _, _, facts) = observations(true, true).await;
    let config = HarnessConfig::new(20, 1, 200, 1, vec![], vec![])
        .unwrap()
        .with_keep_tokens(1)
        .with_duplicate_threshold(3);
    let replay = |facts: &[Fact]| {
        facts.iter().fold(SchedulerState::new(), |state, fact| {
            step(state, fact, &config)
        })
    };
    let current = replay(&facts);
    let action = ("read_state".into(), tool_args_digest(&json!({})).unwrap());
    assert!(current.schema_error.is_none());
    assert_eq!(current.duplicate_calls.get(&action), Some(&1));
    assert_eq!(current.seen_information.len(), 7);
    assert!(!current
        .messages
        .iter()
        .any(|message| message.contains("sha256:")));
    let mut old_facts = facts.clone();
    for fact in &mut old_facts {
        if let Some(payload) = fact.payload.as_object_mut() {
            payload.remove("informationIdentity");
        }
    }
    let old = replay(&old_facts);
    assert!(old.schema_error.is_none());
    assert_eq!(old.duplicate_calls.get(&action), Some(&7));
    assert!(old.seen_information.is_empty());
    old_facts
        .iter_mut()
        .find(|fact| fact.kind == "tool.result")
        .unwrap()
        .payload["informationIdentity"] = json!("unvalidated observation");
    assert!(replay(&old_facts).schema_error.is_some());
}
