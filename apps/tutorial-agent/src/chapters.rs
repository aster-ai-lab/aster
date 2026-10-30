use a3s_code_core::context::{ContextItem, ContextType, StaticContextProvider};
use a3s_code_core::durable_memory::{DurableMemoryRecallPolicy, DurableMemorySession};
use a3s_code_core::goal_control::{
    DurableGoalDefinitionV1, GoalCommandKindV1, GoalControlCommandV1, GoalDefinitionCommandV1,
    GoalPlanCommandV1,
};
use a3s_code_core::planning::{AgentGoal, Complexity, ExecutionPlan, Task};
use a3s_code_core::workspace::{WorkspaceRef, WorkspaceServices};
use a3s_code_core::{
    AgentSession, GoalTickStream, HarnessComposeOptions, SessionOptions, WorkPolicyV1,
};
use a3s_memory::repository::{FileMemoryRepository, MemoryNamespace};
use std::sync::Arc;

pub fn coding_harness() -> anyhow::Result<HarnessComposeOptions> {
    let mut harness = HarnessComposeOptions::compose(
        ["system", "tools", "budget", "compact", "infer"]
            .map(str::to_owned)
            .to_vec(),
        Some(12),
        Some(120_000),
        vec!["修改前阅读调用方；修改后运行项目已有检查并说明结果。".into()],
    )?;
    harness.step_limit = Some(96);
    harness.model_attempts = Some(2);
    Ok(harness)
}

pub fn add_project_notes(options: SessionOptions) -> SessionOptions {
    let provider = StaticContextProvider::new("tutorial-project").with_item(
        ContextItem::new(
            "project-test-entry",
            ContextType::Resource,
            "本项目的验收命令是仓库已有测试；修改前先检查对应调用方。",
        )
        .with_relevance(0.9),
    );
    options.with_context_provider(Arc::new(provider))
}

pub async fn project_memory(state_dir: &std::path::Path) -> anyhow::Result<DurableMemorySession> {
    let repository = Arc::new(FileMemoryRepository::open(state_dir).await?);
    let namespace = MemoryNamespace::try_new("tutorial-tenant", "tutorial-user", "tutorial-demo")?;
    let policy = DurableMemoryRecallPolicy::try_new(8, 1.0)?;
    Ok(DurableMemorySession::active_recall(
        repository, namespace, policy,
    ))
}

pub fn goal_options(
    workspace: &std::path::Path,
    options: SessionOptions,
) -> anyhow::Result<(SessionOptions, a3s_code_core::workspace::WorkspaceRef)> {
    let services = WorkspaceServices::local_with_manifest(workspace);
    let workspace_ref = services.workspace_ref().clone();
    let policy = WorkPolicyV1::new(
        "tutorial-engineering",
        1,
        "先检查调用方；每次修改后运行对应检查；保留失败原因。",
    )?;
    Ok((
        options
            .with_workspace_backend(services)
            .with_work_policy(policy)
            .with_planning(true)
            .with_goal_tracking(true),
        workspace_ref,
    ))
}

pub fn create_goal(session: &AgentSession, workspace: WorkspaceRef) -> anyhow::Result<()> {
    session.apply_goal_definition(GoalDefinitionCommandV1 {
        kind: GoalCommandKindV1::Create,
        operation_id: "create-demo-goal".into(),
        goal_id: "repair-demo".into(),
        session_id: session.session_id().into(),
        workspace,
        goal: AgentGoal::new("修复练习项目的失败测试")
            .with_criteria(vec!["原有检查通过，并保留对应修改与验证记录".into()]),
        non_goals: vec!["不增加新功能，不发布软件".into()],
        scope_fact_sequences: vec![],
        work_policy_digest: session.work_policy_digest()?,
        expected_goal_revision: None,
        expected_plan_revision: None,
        expected_goal_sequence: None,
        import_source: None,
    })?;
    Ok(())
}

pub fn repair_plan() -> ExecutionPlan {
    let mut plan = ExecutionPlan::new("修复练习项目", Complexity::Medium);
    plan.upsert_step(
        Task::new("inspect", "检查失败测试与调用方")
            .with_success_criteria("已确认失败原因和受影响调用方"),
    );
    plan.upsert_step(
        Task::new("verify", "修改并执行对应检查")
            .with_dependencies(vec!["inspect".into()])
            .with_success_criteria("当前修改有对应的通过报告"),
    );
    plan
}

pub fn propose_plan(
    session: &AgentSession,
    operation_id: &str,
) -> anyhow::Result<DurableGoalDefinitionV1> {
    let current = session
        .goal_definition()?
        .ok_or_else(|| anyhow::anyhow!("尚未创建目标"))?;
    let binding = &current.binding;
    Ok(session.apply_goal_plan(GoalPlanCommandV1 {
        kind: GoalCommandKindV1::ProposePlan,
        operation_id: operation_id.into(),
        goal_id: binding.goal_id.clone(),
        session_id: binding.session_id.clone(),
        workspace: binding.workspace.clone(),
        work_policy_digest: binding.work_policy_digest.clone(),
        expected_goal_revision: binding.goal_revision,
        expected_plan_revision: current.plan_revision,
        expected_goal_sequence: current.goal_sequence,
        plan: Some(repair_plan()),
        proposal_sequence: None,
    })?)
}

pub fn accept_plan(
    session: &AgentSession,
    shown: &DurableGoalDefinitionV1,
    operation_id: &str,
) -> anyhow::Result<()> {
    let binding = &shown.binding;
    session.apply_goal_plan(GoalPlanCommandV1 {
        kind: GoalCommandKindV1::AcceptPlan,
        operation_id: operation_id.into(),
        goal_id: binding.goal_id.clone(),
        session_id: binding.session_id.clone(),
        workspace: binding.workspace.clone(),
        work_policy_digest: binding.work_policy_digest.clone(),
        expected_goal_revision: binding.goal_revision,
        expected_plan_revision: shown.plan_revision,
        expected_goal_sequence: shown.goal_sequence,
        plan: None,
        proposal_sequence: shown.plan_fact_sequence,
    })?;
    Ok(())
}

pub async fn advance_once(
    session: &AgentSession,
    operation_id: &str,
) -> anyhow::Result<GoalTickStream> {
    let current = session
        .goal_definition()?
        .ok_or_else(|| anyhow::anyhow!("尚未创建目标"))?;
    let binding = current.binding;
    let command = GoalControlCommandV1 {
        kind: GoalCommandKindV1::Tick,
        operation_id: operation_id.into(),
        goal_id: binding.goal_id,
        session_id: binding.session_id,
        workspace: binding.workspace,
        work_policy_digest: binding.work_policy_digest,
        expected_goal_revision: binding.goal_revision,
        expected_plan_revision: current.plan_revision,
        expected_goal_sequence: current.goal_sequence,
    };
    Ok(session.stream_goal_tick(&command).await?)
}
