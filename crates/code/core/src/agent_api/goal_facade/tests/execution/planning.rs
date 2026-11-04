use super::*;

const INVALID_COMPLEXITY: &str = r#"{"intent":"plan","requires_planning":true,"goal":{"description":"Repair callers","success_criteria":["Tests pass"]},"execution_plan":{"complexity":"Unknown","stages":[]},"optimized_input":"Repair callers"}"#;

fn bind_source(session: &AgentSession, text: &str) {
    let log = session
        .run_admission
        .fact_log_slot()
        .lock()
        .unwrap()
        .clone()
        .unwrap();
    let thread = crate::fact_control::thread_for_session(session.session_id());
    let facts = log
        .append(
            &thread,
            &[aster_effect::NewFact {
                kind: "user.message".into(),
                key: "authorized-plan".into(),
                payload: serde_json::json!({"text":text}),
            }],
            None,
        )
        .unwrap();
    let before = session.goal_definition().unwrap().unwrap();
    let mut revision = command(session, None);
    revision.kind = GoalCommandKindV1::Revise;
    revision.operation_id = "scope-revision".into();
    revision.expected_goal_revision = Some(before.binding.goal_revision);
    revision.expected_plan_revision = before.plan_revision;
    revision.expected_goal_sequence = Some(before.goal_sequence);
    revision.scope_fact_sequences = vec![facts.last().unwrap().seq];
    session.apply_goal_definition(revision).unwrap();
}

#[derive(Default)]
struct PlanningBudget {
    checks: AtomicUsize,
    usage: Mutex<Vec<TokenUsage>>,
    deny: bool,
}

#[async_trait::async_trait]
impl crate::budget::BudgetGuard for PlanningBudget {
    async fn check_before_llm(&self, _: &str, _: usize) -> crate::budget::BudgetDecision {
        self.checks.fetch_add(1, Ordering::SeqCst);
        if self.deny {
            crate::budget::BudgetDecision::Deny {
                resource: "tokens".into(),
                reason: "Planning budget exhausted".into(),
            }
        } else {
            crate::budget::BudgetDecision::Allow
        }
    }

    async fn record_after_llm(&self, _: &str, usage: &TokenUsage) {
        self.usage.lock().unwrap().push(usage.clone());
    }
}

#[tokio::test]
async fn generated_plan_is_typed_and_does_not_accept_progress_or_replace_the_plan() {
    let root = tempfile::tempdir().unwrap();
    let budget = Arc::new(PlanningBudget::default());
    let contract = crate::verification::VerificationContractV1::new(
        "caller-check",
        "cargo test --locked --lib",
        ["src/**"],
    )
    .unwrap();
    let (agent, session, client) = fixture(
        root.path(),
        false,
        options("generated-plan")
            .with_budget_guard(budget.clone())
            .with_verification_contract(contract.clone()),
    )
    .await;
    client.scripted.lock().unwrap().push_back(LlmResponse {
        message: Message::assistant(r#"{"intent":"plan","requires_planning":true,"goal":{"description":"Repair callers","success_criteria":["Original tests pass"]},"execution_plan":{"complexity":"Medium","stages":[{"id":"repair","description":"Repair callers","success_criteria":"Current callers compile"},{"id":"verify","description":"Run original tests","dependencies":["repair"],"success_criteria":"Original tests pass"}]},"optimized_input":"Repair callers and verify"}"#),
        usage: TokenUsage { prompt_tokens: 11, completion_tokens: 7, total_tokens: 18, ..Default::default() },
        stop_reason: None, token_logprobs: vec![], meta: None,
    });
    let before = session.goal_definition().unwrap().unwrap();
    let plan = session.generate_goal_plan().await.unwrap();
    assert_eq!(plan.steps.len(), 2);
    assert_eq!(plan.steps[1].dependencies, ["repair"]);
    assert!(plan
        .steps
        .iter()
        .all(|step| step.status == crate::planning::TaskStatus::Pending));
    let after = session.goal_definition().unwrap().unwrap();
    assert_eq!(before.goal_sequence, after.goal_sequence);
    assert_eq!(before.plan_revision, after.plan_revision);
    assert!(!after.plan_accepted);
    assert_eq!(client.calls.load(Ordering::SeqCst), 1);
    assert_eq!(budget.checks.load(Ordering::SeqCst), 1);
    assert_eq!(budget.usage.lock().unwrap()[0].total_tokens, 18);
    assert_eq!(session.recorded_usage_tokens(), (11, 7, 18));
    assert!(client.inputs.lock().unwrap()[0].contains("Repair existing callers"));
    assert!(client.inputs.lock().unwrap()[0]
        .contains(&serde_json::to_value(&contract).unwrap().to_string()));
    assert!(client.systems.lock().unwrap()[0]
        .contains("Each stage requires fresh independent verification with status Passed"));
    assert!(client.systems.lock().unwrap()[0].contains("execution_plan.stages"));
    session.close().await;
    agent.close().await;
}

#[tokio::test]
async fn denied_planning_budget_never_enters_the_provider() {
    let root = tempfile::tempdir().unwrap();
    let budget = Arc::new(PlanningBudget {
        deny: true,
        ..Default::default()
    });
    let (agent, session, client) = fixture(
        root.path(),
        false,
        options("plan-denied").with_budget_guard(budget.clone()),
    )
    .await;
    assert!(matches!(
        session.generate_goal_plan().await,
        Err(crate::error::CodeError::BudgetExhausted { .. })
    ));
    assert_eq!(budget.checks.load(Ordering::SeqCst), 1);
    assert_eq!(client.calls.load(Ordering::SeqCst), 0);
    assert!(budget.usage.lock().unwrap().is_empty());
    assert_eq!(session.recorded_usage_tokens(), (0, 0, 0));
    session.close().await;
    agent.close().await;
}

#[tokio::test]
async fn malformed_goal_stage_responses_fail_without_accepting_a_plan() {
    for response in [
        "[]",
        r#"{"execution_plan":null}"#,
        r#"{"execution_plan":{"steps":[]}}"#,
        r#"{"execution_plan":{"stages":null}}"#,
    ] {
        let root = tempfile::tempdir().unwrap();
        let (agent, session, client) = fixture(root.path(), false, options("invalid-stages")).await;
        client.scripted.lock().unwrap().push_back(LlmResponse {
            message: Message::assistant(response),
            usage: TokenUsage::default(),
            stop_reason: None,
            token_logprobs: vec![],
            meta: None,
        });
        let before = session.goal_definition().unwrap().unwrap();
        assert!(session.generate_goal_plan().await.is_err(), "{response}");
        let after = session.goal_definition().unwrap().unwrap();
        assert_eq!(before.goal_sequence, after.goal_sequence);
        assert!(!after.plan_accepted);
        session.close().await;
        agent.close().await;
    }
}

#[tokio::test]
async fn a_noncanonical_complexity_uses_the_bounded_repair_and_counts_both_responses() {
    let root = tempfile::tempdir().unwrap();
    let (agent, session, client) = fixture(root.path(), false, options("repair-complexity")).await;
    let mut response = serde_json::json!({"intent":"Plan","requires_planning":true,
        "goal":{"description":"Repair callers","success_criteria":["Current tests pass"]},
        "execution_plan":{"complexity":"moderate","stages":[{"id":"repair","description":"Repair and verify callers","success_criteria":"Current tests pass"}]},
        "optimized_input":"Repair callers and verify"});
    for (complexity, prompt_tokens, completion_tokens) in [("moderate", 5, 3), ("Medium", 7, 4)] {
        response["execution_plan"]["complexity"] = complexity.into();
        client.scripted.lock().unwrap().push_back(LlmResponse {
            message: Message::assistant(&response.to_string()),
            usage: TokenUsage {
                prompt_tokens,
                completion_tokens,
                total_tokens: prompt_tokens + completion_tokens,
                ..Default::default()
            },
            stop_reason: None,
            token_logprobs: vec![],
            meta: None,
        });
    }
    let before = session.goal_definition().unwrap().unwrap().goal_sequence;
    let plan = session.generate_goal_plan().await.unwrap();
    assert_eq!(plan.complexity, crate::planning::Complexity::Medium);
    assert_eq!(client.calls.load(Ordering::SeqCst), 2);
    assert_eq!(session.recorded_usage_tokens(), (12, 7, 19));
    let after = session.goal_definition().unwrap().unwrap();
    assert_eq!(after.goal_sequence, before);
    assert!(!after.plan_accepted);
    session.close().await;
    agent.close().await;
}

#[tokio::test]
async fn dropping_a_planner_releases_admission_without_accepting_a_plan() {
    let root = tempfile::tempdir().unwrap();
    let (agent, session, client) = fixture(root.path(), false, options("plan-cancelled")).await;
    let sequence = session.goal_definition().unwrap().unwrap().goal_sequence;
    client.stall_at.store(1, Ordering::SeqCst);
    let entered = Arc::new(tokio::sync::Notify::new());
    let notify = entered.clone();
    *client.before_response.lock().unwrap() = Some(Box::new(move |_| notify.notify_one()));
    client.scripted.lock().unwrap().push_back(LlmResponse {
        message: Message::assistant(INVALID_COMPLEXITY),
        usage: TokenUsage::default(),
        stop_reason: None,
        token_logprobs: vec![],
        meta: None,
    });
    let mut planning = Box::pin(session.generate_goal_plan());
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        tokio::select! {
            result = &mut planning => panic!("the model must stay pending: {result:?}"),
            _ = entered.notified() => {}
        }
    })
    .await
    .unwrap();
    assert!(matches!(
        session.generate_goal_plan().await,
        Err(crate::error::CodeError::SessionBusy { .. })
    ));
    drop(planning);
    assert_eq!(
        session.goal_definition().unwrap().unwrap().goal_sequence,
        sequence
    );
    assert!(session.generate_goal_plan().await.is_err());
    assert_eq!(
        client.calls.load(Ordering::SeqCst),
        4,
        "the new planner acquires admission and retains its two bounded repairs"
    );
    session.close().await;
    agent.close().await;
}

#[tokio::test]
async fn planning_sources_are_the_exact_bound_facts() {
    let root = tempfile::tempdir().unwrap();
    let (agent, session, client) = fixture(root.path(), false, options("plan-sources")).await;
    bind_source(&session, "AUTHORIZED-STAGES: repair callers, then run original tests. Do not claim either accepted yet.");
    assert!(
        session.generate_goal_plan().await.is_err(),
        "the model fixture returns malformed content"
    );
    let input = client.inputs.lock().unwrap()[0].clone();
    assert!(input.contains("AUTHORIZED-STAGES"));
    assert!(
        !input.contains("\"key\":\"plan\""),
        "unbound historical proposals are excluded"
    );
    session.close().await;
    agent.close().await;
}

#[tokio::test]
async fn oversized_planning_context_is_refused_before_the_provider() {
    let root = tempfile::tempdir().unwrap();
    let (agent, session, client) = fixture(
        root.path(),
        false,
        options("plan-window").with_max_context_tokens(12288),
    )
    .await;
    bind_source(&session, &"1234 ".repeat(16_384));
    assert!(matches!(
        session.generate_goal_plan().await,
        Err(crate::error::CodeError::Context(_))
    ));
    assert_eq!(client.calls.load(Ordering::SeqCst), 0);
    session.close().await;
    agent.close().await;
}

#[tokio::test]
async fn malformed_model_plan_is_not_replaced_with_a_generic_task() {
    let root = tempfile::tempdir().unwrap();
    let (agent, session, client) = fixture(root.path(), false, options("invalid-plan")).await;
    let before = session.goal_definition().unwrap().unwrap().goal_sequence;
    client.scripted.lock().unwrap().push_back(LlmResponse {
        message: Message::assistant(INVALID_COMPLEXITY),
        usage: TokenUsage {
            prompt_tokens: 11,
            completion_tokens: 7,
            total_tokens: 18,
            ..Default::default()
        },
        stop_reason: None,
        token_logprobs: vec![],
        meta: None,
    });
    assert!(session.generate_goal_plan().await.is_err());
    assert_eq!(
        session.recorded_usage_tokens(),
        (11, 7, 18),
        "a rejected plan still consumed a successful provider response"
    );
    assert_eq!(
        session.goal_definition().unwrap().unwrap().goal_sequence,
        before
    );
    assert_eq!(client.calls.load(Ordering::SeqCst), 3);
    session.close().await;
    assert!(matches!(
        session.generate_goal_plan().await,
        Err(crate::error::CodeError::SessionClosed { .. })
    ));
    agent.close().await;
}

#[tokio::test]
async fn the_model_can_select_the_original_single_step_without_rewriting_the_objective() {
    let root = tempfile::tempdir().unwrap();
    let (agent, session, client) = fixture(root.path(), false, options("simple-plan")).await;
    client.scripted.lock().unwrap().push_back(LlmResponse {
        message: Message::assistant(r#"{"intent":"GeneralPurpose","requires_planning":false,"goal":{"description":"A shorter model label","success_criteria":[]},"execution_plan":{"complexity":"Simple","stages":[]},"optimized_input":"A rewritten request"}"#),
        usage: TokenUsage::default(), stop_reason: None, token_logprobs: vec![], meta: None,
    });
    let plan = session.generate_goal_plan().await.unwrap();
    assert_eq!(plan.goal, "Repair existing callers");
    assert_eq!(plan.steps.len(), 1);
    assert_eq!(plan.steps[0].content, "Repair existing callers");
    assert_eq!(
        plan.steps[0].success_criteria.as_deref(),
        Some("All original checks pass")
    );
    assert_eq!(client.calls.load(Ordering::SeqCst), 1);
    session.close().await;
    agent.close().await;
}

#[tokio::test]
async fn durable_stage_plans_preserve_more_than_five_steps_in_order() {
    let root = tempfile::tempdir().unwrap();
    let (agent, session, client) = fixture(root.path(), false, options("eight-stage-plan")).await;
    let steps = (0..8).map(|index| serde_json::json!({"id":format!("stage-{index}"),"description":format!("Deliver stage {index}"),"dependencies":if index == 0 {vec![]} else {vec![format!("stage-{}", index-1)]},"success_criteria":format!("Artifact {index} passes its original check")})).collect::<Vec<_>>();
    let response = serde_json::json!({"intent":"Plan","requires_planning":true,"goal":{"description":"Deliver the eight stages","success_criteria":["All eight artifacts pass"]},"execution_plan":{"complexity":"VeryComplex","stages":steps},"optimized_input":"Deliver the eight authorized stages"});
    client.scripted.lock().unwrap().push_back(LlmResponse {
        message: Message::assistant(&response.to_string()),
        usage: TokenUsage::default(),
        stop_reason: None,
        token_logprobs: vec![],
        meta: None,
    });
    let plan = session.generate_goal_plan().await.unwrap();
    assert_eq!(plan.steps.len(), 8);
    for (index, step) in plan.steps.iter().enumerate() {
        assert_eq!(step.id, format!("stage-{index}"));
    }
    assert_eq!(plan.steps[7].dependencies, ["stage-6"]);
    session.close().await;
    agent.close().await;
}

#[tokio::test]
async fn the_goal_deadline_cancels_auxiliary_planning_and_refuses_a_late_retry() {
    use super::automatic::{now_ms, policy};
    let root = tempfile::tempdir().unwrap();
    let (agent, session, client) = fixture_with(
        root.path(),
        false,
        options("plan-deadline"),
        Some(policy(true, 8, 4, now_ms() + 3_000)),
    )
    .await;
    client.stall_at.store(1, Ordering::SeqCst);
    let error = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        session.generate_goal_plan(),
    )
    .await
    .unwrap()
    .unwrap_err();
    let is_deadline = |error: crate::error::CodeError| {
        matches!(error,
        crate::error::CodeError::Internal(source) if matches!(source.downcast_ref::<crate::goal_control::GoalControlError>(), Some(crate::goal_control::GoalControlError::Refused { refusal: crate::goal_control::GoalRefusal::DeadlineExpired, .. })))
    };
    assert!(is_deadline(error));
    assert!(is_deadline(session.generate_goal_plan().await.unwrap_err()));
    assert_eq!(client.calls.load(Ordering::SeqCst), 1);
    assert!(!session.goal_definition().unwrap().unwrap().plan_accepted);
    session.close().await;
    agent.close().await;
}

#[tokio::test]
async fn planning_deadline_includes_waiting_for_the_shared_scheduler() {
    use super::automatic::{now_ms, policy};
    let root = tempfile::tempdir().unwrap();
    let (agent, session, client) = fixture_with(
        root.path(),
        false,
        options("queued-plan-deadline"),
        Some(policy(true, 8, 4, now_ms() + 3_000)),
    )
    .await;
    let scheduler = &session.task_scheduler;
    let capacity = scheduler.stats().await.unwrap().max_active;
    let cancellation = tokio_util::sync::CancellationToken::new();
    let mut occupied = Vec::new();
    for index in 0..capacity {
        occupied.push(
            scheduler
                .acquire(
                    crate::task_scheduler::TaskPriority::Foreground,
                    format!("occupied-{index}"),
                    &cancellation,
                )
                .await
                .unwrap(),
        );
    }
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(4),
        session.generate_goal_plan(),
    )
    .await;
    assert!(
        result.is_ok(),
        "the Goal deadline must include scheduler waiting"
    );
    assert!(
        matches!(result.unwrap().unwrap_err(), crate::error::CodeError::Internal(source)
        if matches!(source.downcast_ref::<crate::goal_control::GoalControlError>(), Some(crate::goal_control::GoalControlError::Refused { refusal: crate::goal_control::GoalRefusal::DeadlineExpired, .. })))
    );
    assert_eq!(client.calls.load(Ordering::SeqCst), 0);
    drop(occupied);
    session.close().await;
    agent.close().await;
}

#[test]
fn queued_fact_reads_respect_the_deadline_and_release_original_admission() {
    use super::automatic::{now_ms, policy};
    tokio::runtime::Builder::new_current_thread().enable_all().max_blocking_threads(1)
        .build().unwrap().block_on(async {
            let root = tempfile::tempdir().unwrap();
            let (agent, session, client) = fixture_with(root.path(), false, options("queued-fact-read"), Some(policy(true, 8, 4, now_ms()+3_000))).await;
            let sequence = session.goal_definition().unwrap().unwrap().goal_sequence;
            let (entered, waiting) = tokio::sync::oneshot::channel();
            let (release, blocked) = std::sync::mpsc::channel();
            let blocker = tokio::task::spawn_blocking(move || {
                entered.send(()).unwrap();
                blocked.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
            });
            waiting.await.unwrap();
            let result = tokio::time::timeout(std::time::Duration::from_secs(4), session.generate_goal_plan()).await;
            release.send(()).unwrap();
            blocker.await.unwrap();
            tokio::task::spawn_blocking(|| ()).await.unwrap();
            let error = result.expect("a blocked pool must not block the async deadline").unwrap_err();
            assert!(matches!(error, crate::error::CodeError::Internal(source) if matches!(source.downcast_ref::<crate::goal_control::GoalControlError>(), Some(crate::goal_control::GoalControlError::Refused { refusal: crate::goal_control::GoalRefusal::DeadlineExpired, .. }))));
            assert_eq!(client.calls.load(Ordering::SeqCst), 0);
            assert_eq!(session.goal_definition().unwrap().unwrap().goal_sequence, sequence);
            let lease = session.run_admission.try_acquire(session.session_id()).unwrap();
            drop(lease);
            session.close().await;
            agent.close().await;
        });
}
