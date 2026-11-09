//! Control-loop end-to-end tasks on the DeepSeek model from `.aster/config.acl`.
//!
//! Covers the session/Run cybernetic analogues already in aster-core:
//! sensor (mutation / tool evidence / wait) → controller (`preview_goal_tick` /
//! `decide_completion`) → actuator (`tick_goal` / tools) → feedback (settle /
//! reopen). No Mock LLM. No HarnessMM decision reuse. No repo-outer CI loop.
//!
//! Matrix IDs (see inventory in the evidence OUTCOME):
//! - R1*: Run-inner completion gate (this file + `test_harness_loop_live_e2e`)
//! - R2*: Goal-tick controller decisions (this file)
//! - R3*: Multi-slice task across pause/reopen (this file)
//! - R4/R5: mounted from existing live suites in the OUTCOME inventory
//!
//! ```text
//! ASTER_CONFIG_FILE=/abs/path/.aster/config.acl \
//!   cargo test -p aster-core --test test_control_loop_deepseek_e2e \
//!   -- --ignored --test-threads=1 --nocapture
//! ```

use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};
use std::time::{Duration, Instant};

use aster_core::budget::BudgetGuard;
use aster_core::goal_control::{
    GoalCommandKindV1, GoalControlCommandV1, GoalDefinitionCommandV1, GoalPlanCommandV1, GoalState,
    GoalTickDecisionV1, GoalWaitCommandV1, GoalWaitConditionV1, GoalWaitEvidenceV1,
};
use aster_core::harness_loop::CompletionTerminal;
use aster_core::llm::TokenUsage;
use aster_core::permissions::{PermissionDecision, PermissionPolicy};
use aster_core::planning::{AgentGoal, Complexity, ExecutionPlan, Task, TaskStatus};
use aster_core::store::FileSessionStore;
use aster_core::tools::{Tool, ToolCapabilities, ToolContext, ToolOutput};
use aster_core::verification::{VerificationCheck, VerificationReport, VerificationStatus};
use aster_core::workspace::WorkspaceServices;
use aster_core::{
    Agent, AgentEvent, AgentRunSpawn, AgentSession, CodeConfig, CompletionAttestor, GoalTickAdmission,
    MutatedPathRecord, PlanningMode, SessionOptions,
};

mod support;
use support::layer_c_model::repo_config_path;

const MODEL_TIMEOUT: Duration = Duration::from_secs(420);
const SLICE_TIMEOUT: Duration = Duration::from_secs(420);

#[derive(Default)]
struct UsageProbe(Mutex<Vec<TokenUsage>>);

#[async_trait::async_trait]
impl BudgetGuard for UsageProbe {
    async fn record_after_llm(&self, _: &str, usage: &TokenUsage) {
        self.0.lock().unwrap().push(usage.clone());
    }
}

struct TokenFileAttestor {
    root: PathBuf,
    expected: String,
    calls: AtomicUsize,
}

impl CompletionAttestor for TokenFileAttestor {
    fn attest(
        &self,
        digest: &str,
        paths: &[MutatedPathRecord],
    ) -> Option<VerificationReport> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if paths.is_empty() || digest.is_empty() {
            return None;
        }
        let path = self.root.join("loop_out.txt");
        let Ok(bytes) = std::fs::read(&path) else {
            return None;
        };
        if bytes != self.expected.as_bytes() {
            return None;
        }
        if !paths.iter().any(|record| record.path == "loop_out.txt") {
            return None;
        }
        Some(
            VerificationReport::new(
                "host:control-loop-token",
                vec![VerificationCheck::required(
                    "loop_out-bytes",
                    "host_token_bytes",
                    "loop_out.txt matches the host token",
                )
                .with_status(VerificationStatus::Passed)],
            )
            .with_effect_digest(digest.to_string()),
        )
    }
}

struct VerifySource(String);

#[async_trait::async_trait]
impl Tool for VerifySource {
    fn name(&self) -> &str {
        "verify_source"
    }
    fn description(&self) -> &str {
        "Read source.txt and independently verify its bytes against the host expected value."
    }
    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({"type":"object","properties":{},"additionalProperties":false})
    }
    fn capabilities(&self, _: &serde_json::Value) -> ToolCapabilities {
        ToolCapabilities::read_only_paginated(1)
    }
    async fn execute(
        &self,
        _: &serde_json::Value,
        ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        let value = std::fs::read_to_string(ctx.workspace.join("source.txt"))?;
        let status = if value == self.0 {
            VerificationStatus::Passed
        } else {
            VerificationStatus::Failed
        };
        let report = VerificationReport::new(
            "host:source-check",
            vec![VerificationCheck::required(
                "inspect",
                "source-bytes",
                "The bytes read from source.txt match the host expected value",
            )
            .with_status(status)],
        );
        let output = serde_json::to_string(&serde_json::json!({
            "path": "source.txt",
            "observed": value,
            "verification_report": report,
        }))?;
        Ok(
            ToolOutput::success(output).with_metadata(serde_json::json!({
                "verification_author": "host",
                "verification_report": report,
            })),
        )
    }
}

/// Resolve a DeepSeek route declared in `.aster/config.acl`.
///
/// Preference order:
/// 1. `ASTER_TEST_MODEL` when it names a declared DeepSeek route
/// 2. Layer C pins (`boyue/deepseek-v4-flash`, bailian Flash peer)
/// 3. `deepseek/deepseek-flash`
/// 4. any declared `provider/model` whose model id contains `deepseek`
fn resolve_deepseek_route(config: &CodeConfig) -> String {
    let env_pin = std::env::var("ASTER_TEST_MODEL")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    if let Some(pin) = env_pin {
        assert!(
            route_is_deepseek(&pin),
            "ASTER_TEST_MODEL={pin} must be a DeepSeek route for this suite"
        );
        assert!(
            provider_has_route(config, &pin),
            "ASTER_TEST_MODEL={pin} is not declared in {}",
            repo_config_path().display()
        );
        return pin;
    }

    const CANDIDATES: &[&str] = &[
        "boyue/deepseek-v4-flash",
        "boyue/bailian/deepseek-v4.1-flash",
        "deepseek/deepseek-flash",
        "gateway/deepseek-v4.1-flash",
    ];
    for candidate in CANDIDATES {
        if provider_has_route(config, candidate) {
            return (*candidate).to_string();
        }
    }

    let mut discovered = Vec::new();
    for provider in &config.providers {
        for model in &provider.models {
            if model.id.to_ascii_lowercase().contains("deepseek") {
                discovered.push(format!("{}/{}", provider.name, model.id));
            }
        }
    }
    discovered.sort();
    discovered.dedup();
    assert!(
        !discovered.is_empty(),
        "{} must declare a DeepSeek model for control-loop E2E",
        repo_config_path().display()
    );
    discovered[0].clone()
}

fn route_is_deepseek(route: &str) -> bool {
    route.to_ascii_lowercase().contains("deepseek")
}

fn provider_has_route(config: &CodeConfig, route: &str) -> bool {
    let Some((provider, model)) = route.split_once('/') else {
        return false;
    };
    config
        .find_provider(provider)
        .map(|entry| entry.models.iter().any(|item| item.id == model))
        .unwrap_or(false)
}

fn load_deepseek_config() -> CodeConfig {
    let path = repo_config_path();
    let mut config = CodeConfig::from_file(&path)
        .unwrap_or_else(|error| panic!("failed to load {}: {error}", path.display()));
    let pin = resolve_deepseek_route(&config);
    if config.default_model.as_deref() != Some(pin.as_str()) {
        eprintln!(
            "pinning default_model to {pin} (config had {:?})",
            config.default_model
        );
    }
    config.default_model = Some(pin.clone());
    eprintln!("CONTROL-LOOP using default_model={pin}");
    config
}

fn with_goal_bounds(mut config: CodeConfig, max_slices: u64, max_no_progress: u64) -> CodeConfig {
    let deadline = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis()
        + 3_600_000;
    config.goal_control = CodeConfig::from_acl(&format!(
        r#"goal_control {{
            automatic_enabled = true
            max_slices = "{max_slices}"
            max_no_progress_slices = "{max_no_progress}"
            deadline_unix_ms = "{deadline}"
        }}"#
    ))
    .unwrap()
    .goal_control;
    config.memory = Some(aster_core::memory::MemoryConfig {
        llm_extraction: false,
        prune_policy: None,
        ..Default::default()
    });
    config
}

fn tick_request(session: &AgentSession, operation_id: &str) -> GoalControlCommandV1 {
    let goal = session.goal_definition().unwrap().unwrap();
    GoalControlCommandV1 {
        kind: GoalCommandKindV1::Tick,
        operation_id: operation_id.into(),
        goal_id: goal.binding.goal_id,
        session_id: goal.binding.session_id,
        workspace: goal.binding.workspace,
        work_policy_digest: goal.binding.work_policy_digest,
        expected_goal_revision: goal.binding.goal_revision,
        expected_plan_revision: goal.plan_revision,
        expected_goal_sequence: goal.goal_sequence,
    }
}

fn create_goal(session: &AgentSession, workspace: &Path, description: &str, criteria: Vec<String>) {
    session
        .apply_goal_definition(GoalDefinitionCommandV1 {
            kind: GoalCommandKindV1::Create,
            import_source: None,
            operation_id: "create".into(),
            goal_id: "control-loop".into(),
            session_id: session.session_id().into(),
            workspace: WorkspaceServices::local(workspace).workspace_ref().clone(),
            goal: AgentGoal::new(description).with_criteria(criteria),
            non_goals: vec!["Do not change unrelated files".into()],
            scope_fact_sequences: vec![],
            work_policy_digest: None,
            expected_goal_revision: None,
            expected_plan_revision: None,
            expected_goal_sequence: None,
        })
        .unwrap();
}

fn accept_plan(session: &AgentSession, operation: &str, steps: Vec<Task>) {
    let goal = session.goal_definition().unwrap().unwrap();
    let mut plan = ExecutionPlan::new("control-loop plan", Complexity::Simple);
    for step in steps {
        plan.upsert_step(step);
    }
    let mut proposal = GoalPlanCommandV1 {
        kind: GoalCommandKindV1::ProposePlan,
        operation_id: operation.into(),
        goal_id: goal.binding.goal_id,
        session_id: goal.binding.session_id,
        workspace: goal.binding.workspace,
        work_policy_digest: None,
        expected_goal_revision: goal.binding.goal_revision,
        expected_plan_revision: goal.plan_revision,
        expected_goal_sequence: goal.goal_sequence,
        plan: Some(plan),
        proposal_sequence: None,
    };
    let proposed = session.apply_goal_plan(proposal.clone()).unwrap();
    proposal.kind = GoalCommandKindV1::AcceptPlan;
    proposal.operation_id = format!("{operation}-accept");
    proposal.expected_plan_revision = proposed.plan_revision;
    proposal.expected_goal_sequence = proposed.goal_sequence;
    proposal.plan = None;
    proposal.proposal_sequence = proposed.plan_fact_sequence;
    session.apply_goal_plan(proposal).unwrap();
}

fn step_status(session: &AgentSession, step: &str) -> TaskStatus {
    let goal = session.goal_definition().unwrap().unwrap();
    goal.plan
        .as_ref()
        .unwrap()
        .steps
        .iter()
        .find(|task| task.id == step)
        .unwrap()
        .status
        .clone()
}

fn wait_cmd(
    session: &AgentSession,
    operation: &str,
    step: &str,
    condition: Option<GoalWaitConditionV1>,
    evidence: Option<GoalWaitEvidenceV1>,
) -> GoalWaitCommandV1 {
    let goal = session.goal_definition().unwrap().unwrap();
    GoalWaitCommandV1 {
        kind: if condition.is_some() {
            GoalCommandKindV1::DeclareWait
        } else {
            GoalCommandKindV1::ObserveWait
        },
        operation_id: operation.into(),
        goal_id: goal.binding.goal_id,
        session_id: goal.binding.session_id,
        workspace: goal.binding.workspace,
        work_policy_digest: goal.binding.work_policy_digest,
        expected_goal_revision: goal.binding.goal_revision,
        expected_plan_revision: goal.plan_revision,
        expected_goal_sequence: goal.goal_sequence,
        step_id: step.into(),
        condition,
        evidence,
    }
}

async fn admit_and_join(session: &AgentSession, operation: &str) -> String {
    let started = Instant::now();
    let GoalTickAdmission::Admitted(AgentRunSpawn::Started { worker, .. }) = session
        .tick_goal(&tick_request(session, operation))
        .await
        .unwrap()
    else {
        panic!("{operation} must admit one Run");
    };
    tokio::time::timeout(SLICE_TIMEOUT, worker)
        .await
        .unwrap_or_else(|_| panic!("{operation} timed out"))
        .unwrap();
    let snapshot = session.run_snapshot(operation).await.unwrap();
    eprintln!(
        "CONTROL-LOOP slice op={operation} secs={:.1} status={:?} text={:?}",
        started.elapsed().as_secs_f64(),
        snapshot.status,
        snapshot.result_text
    );
    snapshot.result_text.unwrap_or_default()
}

fn assert_real_provider_calls(guard: &UsageProbe, at_least: usize, label: &str) {
    let calls = guard.0.lock().unwrap();
    assert!(
        calls.len() >= at_least,
        "{label}: expected ≥{at_least} DeepSeek calls, got {}",
        calls.len()
    );
    assert!(
        calls.iter().any(|usage| usage.total_tokens > 0),
        "{label}: provider usage must be non-zero"
    );
}

/// R1d — Run-inner sensor → Verified completion on DeepSeek.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires a DeepSeek model configured in .aster/config.acl"]
async fn r1_deepseek_verified_write_completes() {
    let token = format!("CL-{}", uuid::Uuid::new_v4());
    let config = load_deepseek_config();
    let agent = Agent::from_config(config).await.expect("agent");
    let workspace = tempfile::tempdir().unwrap();
    let attestor = Arc::new(TokenFileAttestor {
        root: workspace.path().to_path_buf(),
        expected: token.clone(),
        calls: AtomicUsize::new(0),
    });
    let policy = PermissionPolicy {
        default_decision: PermissionDecision::Deny,
        ..PermissionPolicy::default()
    }
    .allow("read(**)")
    .allow("write(**)")
    .allow("bash(**)");
    let session = agent
        .session_async(
            workspace.path().to_string_lossy().to_string(),
            Some(
                SessionOptions::new()
                    .with_session_id("cl-r1-verified")
                    .with_permission_policy(policy)
                    .with_default_security()
                    .with_planning(false)
                    .with_auto_delegation_enabled(false)
                    .with_manual_delegation_enabled(false)
                    .with_max_tool_rounds(8)
                    .with_llm_api_timeout(180_000)
                    .with_temperature(0.0)
                    .with_continuation(false)
                    .with_completion_attestor(attestor.clone() as Arc<dyn CompletionAttestor>),
            ),
        )
        .await
        .unwrap();

    let prompt = format!(
        "Create loop_out.txt containing exactly the token {token} using the write tool. \
         Then stop. Do not invent verification; the host will attest the bytes."
    );
    let result = tokio::time::timeout(MODEL_TIMEOUT, session.send(&prompt, None))
        .await
        .expect("R1 timed out")
        .expect("host-attested write must Allow(Verified)");
    let path = workspace.path().join("loop_out.txt");
    assert!(path.is_file(), "R1 must write loop_out.txt");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), token);
    assert!(
        matches!(result.completion, CompletionTerminal::Verified { .. }),
        "expected Verified, got {:?}",
        result.completion
    );
    assert!(
        attestor.calls.load(Ordering::SeqCst) > 0,
        "host attestor must run"
    );
    session.close().await;
    agent.close().await;
}

/// R2a + R2b + R2e — Advance admits one Run; unsettled op reconciles; budget ends.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires a DeepSeek model configured in .aster/config.acl"]
async fn r2_deepseek_advance_reconcile_and_budget() {
    let workspace = tempfile::tempdir().unwrap();
    let value = format!("VERIFIED-{}", uuid::Uuid::new_v4());
    std::fs::write(workspace.path().join("source.txt"), &value).unwrap();

    let config = with_goal_bounds(load_deepseek_config(), 1, 1);
    let agent = Agent::from_config(config).await.unwrap();
    let guard = Arc::new(UsageProbe::default());
    let mut permission = PermissionPolicy::new().allow("verify_source(*)");
    permission.default_decision = PermissionDecision::Deny;
    let session = agent
        .session_async(
            workspace.path().display().to_string(),
            Some(
                SessionOptions::new()
                    .with_session_id("cl-r2-budget")
                    .with_planning_mode(PlanningMode::Enabled)
                    .with_goal_tracking(true)
                    .with_auto_compact(false)
                    .with_auto_delegation_enabled(false)
                    .with_manual_delegation_enabled(false)
                    .with_continuation(false)
                    .with_permission_policy(permission)
                    .with_budget_guard(guard.clone())
                    .with_max_tool_rounds(4)
                    .with_llm_api_timeout(180_000)
                    .with_temperature(0.0),
            ),
        )
        .await
        .unwrap();
    session
        .register_dynamic_tool(Arc::new(VerifySource(value.clone())))
        .unwrap();

    create_goal(
        &session,
        workspace.path(),
        "Verify the source, then report a host-only release code",
        vec![
            "The current source value is independently verified".into(),
            "The owner's release code is reported exactly".into(),
        ],
    );
    accept_plan(
        &session,
        "plan",
        vec![
            Task::new(
                "inspect",
                "Read and independently verify source.txt, then quote its exact current value",
            )
            .with_success_criteria(
                "The current source value is independently verified and reported exactly",
            ),
            // Keeps the goal open so max_slices=1 surfaces BudgetExhausted (R2e),
            // not Complete, after the first admitted slice.
            Task::new(
                "release-code",
                "Report the host-only release code that no tool in this session can read",
            )
            .with_dependencies(vec!["inspect".into()])
            .with_success_criteria("The owner's release code is reported exactly"),
        ],
    );

    let wake = tick_request(&session, "wake-1");
    assert_eq!(
        session.preview_goal_tick(&wake).await.unwrap(),
        GoalTickDecisionV1::Advance,
        "R2a: first preview must Advance"
    );

    let GoalTickAdmission::Admitted(AgentRunSpawn::Started { worker, .. }) =
        session.tick_goal(&wake).await.unwrap()
    else {
        panic!("R2a: first tick must admit a Started Run");
    };

    // R2b: same operation while the slice is live must not start a second Run.
    match session.tick_goal(&wake).await.unwrap() {
        GoalTickAdmission::Admitted(AgentRunSpawn::Replayed { snapshot }) => {
            assert_eq!(snapshot.id, "wake-1");
        }
        GoalTickAdmission::Decided(GoalTickDecisionV1::Reconcile) => {}
        GoalTickAdmission::Admitted(AgentRunSpawn::Started { .. }) => {
            panic!("R2b: second tick must not start another Run")
        }
        GoalTickAdmission::Decided(decision) => {
            panic!("R2b: expected Replayed or Reconcile, got {decision:?}")
        }
    }

    tokio::time::timeout(SLICE_TIMEOUT, worker)
        .await
        .expect("R2a slice timed out")
        .unwrap();
    let text = session
        .run_snapshot("wake-1")
        .await
        .unwrap()
        .result_text
        .unwrap_or_default();
    assert!(
        text.contains(&value),
        "R2a slice must report the verified value: {text}"
    );
    assert_eq!(step_status(&session, "inspect"), TaskStatus::Completed);
    assert_real_provider_calls(&guard, 1, "R2a");
    let calls_after_slice = guard.0.lock().unwrap().len();

    // R2e: max_slices=1 → next tick is BudgetExhausted.
    assert_eq!(
        session
            .preview_goal_tick(&tick_request(&session, "wake-2"))
            .await
            .unwrap(),
        GoalTickDecisionV1::BudgetExhausted,
        "R2e: budget must exhaust after the single admitted slice"
    );
    match session
        .tick_goal(&tick_request(&session, "wake-2"))
        .await
        .unwrap()
    {
        GoalTickAdmission::Decided(GoalTickDecisionV1::BudgetExhausted) => {}
        GoalTickAdmission::Decided(decision) => {
            panic!("R2e: expected BudgetExhausted, got {decision:?}")
        }
        GoalTickAdmission::Admitted(_) => panic!("R2e: exhausted tick must not admit a Run"),
    }
    assert_eq!(
        guard.0.lock().unwrap().len(),
        calls_after_slice,
        "R2e: exhausted tick must not call the model again"
    );

    session.close().await;
    agent.close().await;
}

/// R2c + R2d + R2f — NeedsInput wait, evidence unblocks Advance, pause/resume.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires a DeepSeek model configured in .aster/config.acl"]
async fn r2_deepseek_wait_evidence_pause_resume() {
    let workspace = tempfile::tempdir().unwrap();
    let value = format!("VERIFIED-{}", uuid::Uuid::new_v4());
    let code = format!("REL-{}", uuid::Uuid::new_v4());
    std::fs::write(workspace.path().join("source.txt"), &value).unwrap();

    let config = with_goal_bounds(load_deepseek_config(), 3, 2);
    let agent = Agent::from_config(config).await.unwrap();
    let guard = Arc::new(UsageProbe::default());
    let mut permission = PermissionPolicy::new().allow("verify_source(*)");
    permission.default_decision = PermissionDecision::Deny;
    let session = agent
        .session_async(
            workspace.path().display().to_string(),
            Some(
                SessionOptions::new()
                    .with_session_id("cl-r2-wait")
                    .with_planning_mode(PlanningMode::Enabled)
                    .with_goal_tracking(true)
                    .with_auto_compact(false)
                    .with_auto_delegation_enabled(false)
                    .with_manual_delegation_enabled(false)
                    .with_continuation(false)
                    .with_permission_policy(permission)
                    .with_budget_guard(guard.clone())
                    .with_max_tool_rounds(4)
                    .with_llm_api_timeout(180_000)
                    .with_temperature(0.0),
            ),
        )
        .await
        .unwrap();
    session
        .register_dynamic_tool(Arc::new(VerifySource(value.clone())))
        .unwrap();

    create_goal(
        &session,
        workspace.path(),
        "Verify the source and report the owner's release code",
        vec![
            "The current source is independently verified".into(),
            "The owner's release code is reported exactly".into(),
        ],
    );
    accept_plan(
        &session,
        "plan",
        vec![
            Task::new(
                "inspect",
                "Read and independently verify source.txt, then quote its exact current value",
            )
            .with_success_criteria(
                "The current source value is independently verified and reported exactly",
            ),
            Task::new("release-code", "Report the owner's release code exactly")
                .with_dependencies(vec!["inspect".into()])
                .with_success_criteria("The owner's release code is reported exactly"),
        ],
    );

    assert!(
        session
            .apply_goal_wait(wait_cmd(
                &session,
                "w-code",
                "release-code",
                Some(GoalWaitConditionV1::QuestionAnswer {
                    question: "Which release code should be reported?".into(),
                }),
                None,
            ))
            .unwrap()
            .recorded
    );

    let inspected = admit_and_join(&session, "wake-inspect").await;
    assert!(inspected.contains(&value));
    assert_eq!(step_status(&session, "inspect"), TaskStatus::Completed);
    let calls_after_inspect = guard.0.lock().unwrap().len();
    assert_real_provider_calls(&guard, 1, "R2 wait inspect");

    // R2c: open question → NeedsInput, no model call.
    assert!(matches!(
        session
            .tick_goal(&tick_request(&session, "wake-idle"))
            .await
            .unwrap(),
        GoalTickAdmission::Decided(GoalTickDecisionV1::NeedsInput)
    ));
    assert_eq!(guard.0.lock().unwrap().len(), calls_after_inspect);

    // R2d: evidence unblocks the next Advance.
    assert!(
        session
            .apply_goal_wait(wait_cmd(
                &session,
                "o-code",
                "release-code",
                None,
                Some(GoalWaitEvidenceV1::QuestionAnswer {
                    answer: format!("The release code is {code}."),
                }),
            ))
            .unwrap()
            .recorded
    );

    let GoalTickAdmission::Admitted(AgentRunSpawn::Started { worker, .. }) = session
        .tick_goal(&tick_request(&session, "wake-code"))
        .await
        .unwrap()
    else {
        panic!("R2d: evidence must allow Advance");
    };

    // R2f: pause while the slice is live.
    let mut pause = tick_request(&session, "pause");
    pause.kind = GoalCommandKindV1::Pause;
    let paused = match session
        .control_goal(pause, Duration::from_secs(30))
        .await
    {
        Ok(paused) => paused,
        Err(error) => {
            eprintln!("CONTROL-LOOP pause retry after {error}");
            let mut retry = tick_request(&session, "pause-retry");
            retry.kind = GoalCommandKindV1::Pause;
            session
                .control_goal(retry, Duration::from_secs(30))
                .await
                .unwrap()
        }
    };
    assert_eq!(paused.state, GoalState::Paused);
    tokio::time::timeout(Duration::from_secs(120), worker)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(
        session
            .preview_goal_tick(&tick_request(&session, "wake-paused"))
            .await
            .unwrap(),
        GoalTickDecisionV1::Paused
    );

    let mut resume = tick_request(&session, "resume");
    resume.kind = GoalCommandKindV1::Resume;
    assert_eq!(
        session
            .control_goal(resume, Duration::from_secs(30))
            .await
            .unwrap()
            .state,
        GoalState::Active
    );

    // After an interrupted slice, the controller may Advance again or exhaust —
    // both are valid kernel outcomes; Paused must be cleared.
    let after_resume = session
        .preview_goal_tick(&tick_request(&session, "wake-resumed"))
        .await
        .unwrap();
    assert!(
        matches!(
            after_resume,
            GoalTickDecisionV1::Advance | GoalTickDecisionV1::BudgetExhausted
        ),
        "R2f: resume must leave Paused; got {after_resume:?}"
    );

    session.close().await;
    agent.close().await;
}

/// R3 — write+attest task with FileSessionStore reopen on DeepSeek.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires a DeepSeek model configured in .aster/config.acl"]
async fn r3_deepseek_write_goal_across_reopen() {
    let workspace = tempfile::tempdir().unwrap();
    let storage = tempfile::tempdir().unwrap();
    let token = format!("CL-R3-{}", uuid::Uuid::new_v4());
    let session_id = "cl-r3-reopen";

    let config = with_goal_bounds(load_deepseek_config(), 3, 2);
    let agent = Agent::from_config(config.clone()).await.unwrap();
    let guard = Arc::new(UsageProbe::default());
    let store = Arc::new(FileSessionStore::new(storage.path()).await.unwrap());
    let attestor = Arc::new(TokenFileAttestor {
        root: workspace.path().to_path_buf(),
        expected: token.clone(),
        calls: AtomicUsize::new(0),
    });
    let policy = PermissionPolicy {
        default_decision: PermissionDecision::Deny,
        ..PermissionPolicy::default()
    }
    .allow("read(**)")
    .allow("write(**)")
    .allow("bash(**)");

    let session = agent
        .session_async(
            workspace.path().to_string_lossy().to_string(),
            Some(
                SessionOptions::new()
                    .with_session_id(session_id)
                    .with_session_store(store.clone())
                    .with_permission_policy(policy.clone())
                    .with_default_security()
                    .with_planning_mode(PlanningMode::Enabled)
                    .with_goal_tracking(true)
                    .with_auto_compact(false)
                    .with_auto_delegation_enabled(false)
                    .with_manual_delegation_enabled(false)
                    .with_continuation(false)
                    .with_budget_guard(guard.clone())
                    .with_max_tool_rounds(8)
                    .with_llm_api_timeout(180_000)
                    .with_temperature(0.0)
                    .with_completion_attestor(attestor.clone() as Arc<dyn CompletionAttestor>),
            ),
        )
        .await
        .unwrap();

    create_goal(
        &session,
        workspace.path(),
        &format!("Create loop_out.txt containing exactly the token {token}"),
        vec![format!("loop_out.txt contains exactly {token}")],
    );
    accept_plan(
        &session,
        "plan",
        vec![Task::new(
            "write-token",
            format!(
                "Create loop_out.txt containing exactly the token {token} using the write tool, then stop"
            ),
        )
        .with_success_criteria(format!("loop_out.txt contains exactly {token}"))],
    );

    let before = serde_json::to_value(session.goal_definition().unwrap().unwrap()).unwrap();
    let text = admit_and_join(&session, "slice-1").await;
    let path = workspace.path().join("loop_out.txt");
    assert!(
        path.is_file(),
        "R3 slice must write loop_out.txt; model text was {text:?}"
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), token);
    assert_real_provider_calls(&guard, 1, "R3 slice-1");
    assert!(
        attestor.calls.load(Ordering::SeqCst) > 0
            || session
                .run_events("slice-1")
                .await
                .iter()
                .any(|record| matches!(record.event, AgentEvent::ToolEnd { .. })),
        "R3 must exercise tools or host attestation"
    );

    session.save().await.unwrap();
    session.close().await;
    agent.close().await;

    // Reopen from the file store: binding must survive; controller stays coherent.
    let agent = Agent::from_config(config).await.unwrap();
    let store = Arc::new(FileSessionStore::new(storage.path()).await.unwrap());
    let resumed = agent
        .resume_session_async(
            session_id,
            SessionOptions::new()
                .with_session_id(session_id)
                .with_session_store(store)
                .with_permission_policy(policy)
                .with_default_security()
                .with_planning_mode(PlanningMode::Enabled)
                .with_goal_tracking(true)
                .with_auto_compact(false)
                .with_auto_delegation_enabled(false)
                .with_manual_delegation_enabled(false)
                .with_continuation(false)
                .with_budget_guard(guard.clone())
                .with_max_tool_rounds(8)
                .with_llm_api_timeout(180_000)
                .with_temperature(0.0)
                .with_completion_attestor(Arc::new(TokenFileAttestor {
                    root: workspace.path().to_path_buf(),
                    expected: token.clone(),
                    calls: AtomicUsize::new(0),
                }) as Arc<dyn CompletionAttestor>),
        )
        .await
        .unwrap();

    let after = serde_json::to_value(resumed.goal_definition().unwrap().unwrap()).unwrap();
    assert_eq!(
        after["binding"]["goal_id"], before["binding"]["goal_id"],
        "R3 reopen must keep the same goal binding"
    );
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        token,
        "R3 reopen must keep the written token bytes"
    );

    let decision = resumed
        .preview_goal_tick(&tick_request(&resumed, "after-reopen"))
        .await
        .unwrap();
    eprintln!("CONTROL-LOOP R3 after-reopen decision={decision:?}");
    assert!(
        matches!(
            decision,
            GoalTickDecisionV1::Advance
                | GoalTickDecisionV1::BudgetExhausted
                | GoalTickDecisionV1::Complete
                | GoalTickDecisionV1::NeedsInput
                | GoalTickDecisionV1::Wait
                | GoalTickDecisionV1::Reconcile
        ),
        "R3 reopen preview must be a closed GoalTickDecision, got {decision:?}"
    );

    // Reused operation id must not start a second admission of slice-1.
    match resumed.tick_goal(&tick_request(&resumed, "slice-1")).await {
        Ok(GoalTickAdmission::Admitted(AgentRunSpawn::Replayed { snapshot })) => {
            assert_eq!(snapshot.id, "slice-1");
        }
        Ok(GoalTickAdmission::Decided(_)) => {}
        Ok(GoalTickAdmission::Admitted(AgentRunSpawn::Started { .. })) => {
            panic!("R3: reused operation id must not start a fresh Run")
        }
        Err(error) => {
            let message = error.to_string();
            assert!(
                message.contains("conflict") || message.contains("operation"),
                "R3: reused op must fail closed or decide, got {message}"
            );
        }
    }

    resumed.close().await;
    agent.close().await;
}
