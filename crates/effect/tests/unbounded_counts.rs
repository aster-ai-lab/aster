#![cfg(test)]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use a3s_effect::{
    append_input, coding_actor, coding_scheduler, ingest_coding_with_optional_limit, message_fact,
    resume_coding_with_cancel, resume_coding_with_optional_limit, Actor, ActorError,
    CodingServices, CodingView, Compactor, Completion, CompletionRequest, Exit, HarnessConfig,
    HarnessGraph, LogStore, MemoryLog, MetaHarnessSpec, ModelDecision, ToolCall, ToolRunner,
    ToolSpec,
};
use tokio_util::sync::CancellationToken;

struct FiniteModel {
    tools: usize,
    calls: Arc<AtomicUsize>,
    offered: Arc<Mutex<Vec<usize>>>,
}

impl Completion for FiniteModel {
    fn complete(
        &self,
        request: CompletionRequest,
        _cancel: CancellationToken,
    ) -> a3s_effect::coding::BoxFuture<Result<ModelDecision, ActorError>> {
        self.offered.lock().unwrap().push(request.tools.len());
        let index = self.calls.fetch_add(1, Ordering::SeqCst);
        let decision = if index < self.tools {
            ModelDecision::Tool {
                call: ToolCall {
                    id: format!("read-{index}"),
                    name: "read".into(),
                    args: serde_json::json!({"index": index}),
                    needs_confirmation: false,
                    text: None,
                    reasoning: None,
                },
                also: Vec::new(),
            }
        } else {
            ModelDecision::Text {
                text: "finite controlled provider finished".into(),
            }
        };
        Box::pin(async move { Ok(decision) })
    }
}

struct CountingTools {
    calls: Arc<AtomicUsize>,
    cancel_after: Option<usize>,
}

impl ToolRunner for CountingTools {
    fn run(
        &self,
        _call: ToolCall,
        cancel: CancellationToken,
    ) -> a3s_effect::coding::BoxFuture<Result<serde_json::Value, ActorError>> {
        let count = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        if self.cancel_after == Some(count) {
            cancel.cancel();
        }
        Box::pin(async { Ok(serde_json::json!("controlled result")) })
    }
}

struct NoCompact;

impl Compactor for NoCompact {
    fn compact(
        &self,
        _messages: &[String],
    ) -> a3s_effect::coding::BoxFuture<Result<String, ActorError>> {
        Box::pin(async { Ok("unused".into()) })
    }
}

fn config() -> HarnessConfig {
    HarnessConfig::new(
        50,
        usize::MAX,
        200,
        1,
        Vec::new(),
        vec![ToolSpec {
            name: "read".into(),
            description: "A controlled read".into(),
        }],
    )
    .unwrap()
    .with_tool_round_cap(50)
}

type Fixture = (
    Arc<CodingServices>,
    Arc<AtomicUsize>,
    Arc<AtomicUsize>,
    Arc<Mutex<Vec<usize>>>,
);

fn services(tools: usize, cancel_after: Option<usize>) -> Fixture {
    let model_calls = Arc::new(AtomicUsize::new(0));
    let tool_calls = Arc::new(AtomicUsize::new(0));
    let offered = Arc::new(Mutex::new(Vec::new()));
    (
        Arc::new(CodingServices {
            completion: Arc::new(FiniteModel {
                tools,
                calls: model_calls.clone(),
                offered: offered.clone(),
            }),
            tools: Arc::new(CountingTools {
                calls: tool_calls.clone(),
                cancel_after,
            }),
            compactor: Arc::new(NoCompact),
        }),
        model_calls,
        tool_calls,
        offered,
    )
}

async fn run(
    actor: &Actor<CodingServices, CodingView>,
    services: Arc<CodingServices>,
    log: &MemoryLog,
    limit: Option<u32>,
    cancel: &CancellationToken,
) -> Result<a3s_effect::Settlement<CodingView>, Exit<ActorError>> {
    ingest_coding_with_optional_limit(
        actor,
        log,
        services,
        "unbounded-counts",
        message_fact("start", "Run the finite controlled script"),
        limit,
        cancel,
    )
    .await
}

#[test]
fn optional_count_limits_preserve_zero_allowance_and_reject_zero_steps() {
    let zero = HarnessConfig::new(0, 100, 1, 1, Vec::new(), Vec::new()).unwrap();
    assert_eq!(zero.tool_budget_limit(), Some(0));
    assert_eq!(zero.budget(), 0);
    assert_eq!(zero.step_limit_option(), Some(1));
    assert!(HarnessConfig::new(1, 100, 0, 1, Vec::new(), Vec::new()).is_err());
    assert!(config().with_step_limit(Some(0)).is_err());

    let none = config().without_count_limits();
    assert_eq!(none.tool_budget_limit(), None);
    assert_eq!(none.step_limit_option(), None);
    assert_eq!(none.tool_round_cap(), None);
    assert_eq!((none.budget(), none.step_limit()), (0, 0));
    assert_eq!(
        none.clone()
            .with_tool_budget_limit(Some(3))
            .tool_budget_limit(),
        Some(3)
    );
    assert_eq!(
        none.with_step_limit(Some(7)).unwrap().step_limit_option(),
        Some(7)
    );
}

#[tokio::test]
async fn no_count_limits_pass_fifty_tools_and_two_hundred_transitions() {
    let config = config().without_count_limits();
    let (services, model_calls, tool_calls, offered) = services(128, None);
    let log = MemoryLog::new();
    let settled = run(
        &coding_actor(config.clone()),
        services,
        &log,
        config.step_limit_option(),
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(tool_calls.load(Ordering::SeqCst), 128);
    assert_eq!(model_calls.load(Ordering::SeqCst), 129);
    assert!(settled.steps > 200);
    assert_eq!(settled.view.tool_budget, None);
    assert_eq!(
        settled.view.assistant.as_deref(),
        Some("finite controlled provider finished")
    );
    assert!(offered.lock().unwrap().iter().all(|count| *count == 1));
    assert!(settled.log.iter().all(|fact| fact.kind != "budget.denied"));
}

#[tokio::test]
async fn no_count_limits_still_settle_cancellation_after_sixty_four_tools() {
    let (services, model_calls, tool_calls, _) = services(128, Some(64));
    let log = MemoryLog::new();
    let cancel = CancellationToken::new();
    let settled = run(
        &coding_actor(config().without_count_limits()),
        services,
        &log,
        None,
        &cancel,
    )
    .await
    .unwrap();
    assert!(cancel.is_cancelled());
    assert_eq!(tool_calls.load(Ordering::SeqCst), 64);
    assert_eq!(model_calls.load(Ordering::SeqCst), 64);
    assert_eq!(
        settled.view.assistant.as_deref(),
        Some("(Response interrupted by the user.)")
    );
    let last_tool = settled
        .log
        .iter()
        .rev()
        .find(|fact| fact.kind == "tool.result")
        .unwrap();
    assert_eq!(last_tool.payload["receipt"]["outcome"], "cancelled");
    assert!(settled.log.iter().all(|fact| fact.kind != "budget.denied"));
}

#[tokio::test]
async fn explicit_tool_allowances_keep_zero_and_positive_denials_with_unbounded_steps() {
    for budget in [0, 2] {
        let config = config()
            .without_count_limits()
            .with_tool_budget_limit(Some(budget));
        let (services, _, calls, _) = services(8, None);
        let log = MemoryLog::new();
        let settled = run(
            &coding_actor(config),
            services,
            &log,
            None,
            &CancellationToken::new(),
        )
        .await
        .unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), budget as usize);
        assert_eq!(settled.view.tool_budget, Some(budget));
        assert_eq!(
            settled
                .log
                .iter()
                .filter(|fact| fact.kind == "budget.denied")
                .count(),
            1
        );
    }
}

#[tokio::test]
async fn explicit_step_limit_still_blocks_preparation_without_a_tool_allowance() {
    let config = config()
        .without_count_limits()
        .with_step_limit(Some(3))
        .unwrap();
    let (services, _, calls, _) = services(8, None);
    let log = MemoryLog::new();
    let result = run(
        &coding_actor(config.clone()),
        services,
        &log,
        config.step_limit_option(),
        &CancellationToken::new(),
    )
    .await;
    assert!(matches!(
        result,
        Err(Exit::Fail(ActorError::StepLimit { limit: 3 }))
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        log.read("unbounded-counts")
            .unwrap()
            .iter()
            .filter(|fact| fact.kind == "tool.prepared")
            .count(),
        1
    );
}

#[tokio::test]
async fn old_and_optional_zero_step_entry_points_remain_nonexecuting_peeks() {
    let actor = coding_actor(config().without_count_limits());
    let (services, model_calls, calls, _) = services(1, None);
    let log = MemoryLog::new();
    let cancel = CancellationToken::new();
    append_input(&log, "unbounded-counts", message_fact("start", "peek")).unwrap();
    let old = resume_coding_with_cancel(
        &actor,
        &log,
        services.clone(),
        "unbounded-counts",
        0,
        &cancel,
    )
    .await;
    let optional = resume_coding_with_optional_limit(
        &actor,
        &log,
        services,
        "unbounded-counts",
        Some(0),
        &cancel,
    )
    .await;
    for result in [old, optional] {
        assert!(matches!(
            result,
            Err(Exit::Fail(ActorError::StepLimit { limit: 0 }))
        ));
    }
    assert_eq!(model_calls.load(Ordering::SeqCst), 0);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(log.read("unbounded-counts").unwrap().len(), 1);
}

#[tokio::test]
async fn numeric_meta_recipes_keep_their_bound_and_optional_graphs_keep_none() {
    for optional in [false, true] {
        let config = if optional {
            config().without_count_limits()
        } else {
            config()
        };
        let spec = MetaHarnessSpec {
            budget: config.budget(),
            step_limit: 200,
            tools: config.tools().to_vec(),
            tool_round_cap: config.tool_round_cap(),
            ..MetaHarnessSpec::default()
        };
        let graph = if optional {
            HarnessGraph::from_spec_with_optional_budget(spec, None, || {
                coding_scheduler(config.clone())
            })
        } else {
            HarnessGraph::from_spec(spec, || coding_scheduler(config.clone()))
        };
        let (services, _, calls, _) = services(64, None);
        let log = MemoryLog::new();
        let settled = run(
            graph.actor(),
            services,
            &log,
            None,
            &CancellationToken::new(),
        )
        .await
        .unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), if optional { 64 } else { 50 });
        assert_eq!(
            settled.view.tool_budget,
            if optional { None } else { Some(50) }
        );
        assert_eq!(
            settled.log.iter().any(|fact| fact.kind == "budget.denied"),
            !optional
        );
    }
}
