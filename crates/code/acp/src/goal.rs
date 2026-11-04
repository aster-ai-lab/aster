//! Host-side `/goal` for the pager.
//!
//! The command stores one durable objective per ACP session and turns Core
//! planning on or off with [`AgentSession::set_planning_mode`]. It does not
//! start a second planner, classifier, or background workflow.

use aster_core::goal_control::{
    DurableGoalDefinitionV1, GoalCommandKindV1, GoalControlCommandV1, GoalControlError,
    GoalRefusal, GoalState, GoalTickDecisionV1,
};
use aster_core::PlanningMode;

/// One user-set objective for an ACP session.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DurableGoal {
    pub objective: String,
    pub paused: bool,
}

mod import;
pub(crate) use import::{commit_import, preview_import, GoalImportPreviewRequest};
mod persistence;
pub(crate) use persistence::{apply_session_goal, load_goal, retire_achieved_durable_goal};

#[derive(Clone, Copy, Default)]
pub(crate) enum GoalLanguage {
    #[default]
    English,
    Chinese,
}

impl GoalLanguage {
    pub(crate) fn from_host(language: Option<&str>) -> Self {
        match language {
            Some("zh-CN") => Self::Chinese,
            _ => Self::English,
        }
    }

    pub(crate) fn decision(self, decision: GoalTickDecisionV1) -> String {
        if matches!(self, Self::English) {
            return match decision {
                GoalTickDecisionV1::Advance => "Ready for the next step.",
                GoalTickDecisionV1::Wait => "Waiting for a required result.",
                GoalTickDecisionV1::NeedsInput => "Your input is needed to continue.",
                GoalTickDecisionV1::Paused => "Use /goal resume to continue.",
                GoalTickDecisionV1::BudgetExhausted => {
                    "The execution limit has been reached. Automatic work has stopped."
                }
                GoalTickDecisionV1::Reconcile => {
                    "Execution records need review before work can continue."
                }
                GoalTickDecisionV1::Complete => "All completion criteria are satisfied.",
            }
            .into();
        }
        match decision {
            GoalTickDecisionV1::Advance => "可继续执行下一步。",
            GoalTickDecisionV1::Wait => "等待所需结果后继续。",
            GoalTickDecisionV1::NeedsInput => "需要你的输入后才能继续。",
            GoalTickDecisionV1::Paused => "发送 /goal resume 可继续执行。",
            GoalTickDecisionV1::BudgetExhausted => "已达到执行上限，自动执行已停止。",
            GoalTickDecisionV1::Reconcile => "需要核对执行记录后继续。",
            GoalTickDecisionV1::Complete => "目标已满足完成条件。",
        }
        .into()
    }

    pub(crate) fn for_user_text(self, text: &str) -> Self {
        let fallback = match self {
            Self::English => "en",
            Self::Chinese => "zh-CN",
        };
        let language = aster_core::resolve_product_output_language(Some(fallback), text);
        Self::from_host(language.as_deref())
    }

    fn for_goal(self, session: &aster_core::AgentSession, goal: &DurableGoalDefinitionV1) -> Self {
        session
            .reply_language()
            .map(|tag| Self::from_host(Some(&tag)))
            .unwrap_or_else(|| self.for_user_text(&goal.goal.description))
    }

    pub(crate) fn legacy_reply(self, goal: Option<&DurableGoal>, fallback: &str) -> String {
        match self {
            Self::English => fallback.into(),
            Self::Chinese => match goal {
                Some(goal) if goal.paused => "目标已暂停，可发送 /goal resume 继续。",
                Some(_) => "目标进行中。",
                None => "当前未设置目标。发送 /goal <目标内容> 开始。",
            }
            .into(),
        }
    }
}

pub(crate) fn host_clock_ms() -> anyhow::Result<u64> {
    use anyhow::Context;
    u64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .context("host clock")?
            .as_millis(),
    )
    .context("host clock overflow")
}

/// Product-host bounds; a trusted ACL declaration, including disabled, wins.
pub(crate) fn default_continuation_policy(
    now_ms: u64,
) -> anyhow::Result<aster_core::goal_control::GoalContinuationPolicyV1> {
    use anyhow::Context;
    use std::num::NonZeroU64;
    let deadline = now_ms
        .checked_add(24 * 60 * 60 * 1_000)
        .context("goal deadline overflow")?;
    let policy = aster_core::goal_control::GoalContinuationPolicyV1 {
        automatic_enabled: true,
        max_slices: NonZeroU64::new(8).context("goal slice bound")?,
        max_no_progress_slices: NonZeroU64::new(2).context("goal progress bound")?,
        deadline_unix_ms: NonZeroU64::new(deadline).context("goal deadline")?,
    };
    policy.check_admission(now_ms)?;
    Ok(policy)
}

/// What the user asked `/goal` to do.
#[derive(Debug, PartialEq, Eq)]
pub enum GoalOp {
    Status,
    Pause,
    Resume,
    Clear,
    Set(String),
}

/// How the next Core turn should treat planning.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GoalPlan {
    Unchanged,
    Enabled,
    Disabled,
    ClearOverride,
}

impl GoalPlan {
    pub fn mode(self) -> Option<PlanningMode> {
        match self {
            Self::Enabled => Some(PlanningMode::Enabled),
            Self::Disabled => Some(PlanningMode::Disabled),
            Self::Unchanged | Self::ClearOverride => None,
        }
    }

    pub fn clears_override(self) -> bool {
        matches!(self, Self::ClearOverride)
    }
}

/// Result of applying one `/goal` operation to the stored objective.
#[derive(Debug, PartialEq, Eq)]
pub struct AppliedGoal {
    /// `None` removes the stored objective.
    pub next: Option<DurableGoal>,
    pub plan: GoalPlan,
    /// Shown to the user without a model turn.
    pub reply: Option<String>,
    /// Sent to Core when the operation starts work.
    pub model_prompt: Option<String>,
}

const USAGE: &str = "Usage: /goal <objective> | status | pause | resume | clear";

/// Parse a whole prompt that is a `/goal` command.
///
/// A longer token such as `/goalie` is not a goal command.
pub fn parse_goal_command(prompt: &str) -> Option<GoalOp> {
    let trimmed = prompt.trim();
    let rest = trimmed.strip_prefix("/goal")?;
    if !rest.is_empty() && !rest.starts_with(char::is_whitespace) {
        return None;
    }
    let args = rest.trim();
    match args.to_ascii_lowercase().as_str() {
        "" | "status" => Some(GoalOp::Status),
        "pause" => Some(GoalOp::Pause),
        "resume" => Some(GoalOp::Resume),
        "clear" => Some(GoalOp::Clear),
        _ => Some(GoalOp::Set(args.to_string())),
    }
}

fn none_message() -> String {
    format!("No goal is set.\n{USAGE}")
}

fn status_line(goal: &DurableGoal) -> String {
    if goal.paused {
        "Goal paused.".into()
    } else {
        "Goal in progress.".into()
    }
}

/// Prompt Core sees for a newly set goal.
pub fn goal_turn_prompt(objective: &str) -> String {
    format!(
        "{objective}\n\nThis is a durable goal. Work until it is met, then stop. \
Do not ask whether to continue unless you are blocked."
    )
}

pub fn apply_goal(current: Option<DurableGoal>, op: GoalOp) -> AppliedGoal {
    match op {
        GoalOp::Status => AppliedGoal {
            reply: Some(
                current
                    .as_ref()
                    .map(status_line)
                    .unwrap_or_else(none_message),
            ),
            next: current,
            plan: GoalPlan::Unchanged,
            model_prompt: None,
        },
        GoalOp::Pause => match current {
            None => AppliedGoal {
                next: None,
                plan: GoalPlan::Unchanged,
                reply: Some(none_message()),
                model_prompt: None,
            },
            Some(mut goal) => {
                goal.paused = true;
                let reply = status_line(&goal);
                AppliedGoal {
                    next: Some(goal),
                    plan: GoalPlan::Disabled,
                    reply: Some(reply),
                    model_prompt: None,
                }
            }
        },
        GoalOp::Resume => match current {
            None => AppliedGoal {
                next: None,
                plan: GoalPlan::Unchanged,
                reply: Some(none_message()),
                model_prompt: None,
            },
            Some(mut goal) => {
                let prompt = format!(
                    "Continue this durable goal until it is met:\n\n{}",
                    goal.objective
                );
                goal.paused = false;
                AppliedGoal {
                    next: Some(goal),
                    plan: GoalPlan::Enabled,
                    reply: None,
                    model_prompt: Some(prompt),
                }
            }
        },
        GoalOp::Clear => {
            if current.is_none() {
                AppliedGoal {
                    next: None,
                    plan: GoalPlan::Unchanged,
                    reply: Some(none_message()),
                    model_prompt: None,
                }
            } else {
                AppliedGoal {
                    next: None,
                    plan: GoalPlan::ClearOverride,
                    reply: Some("Goal cleared.".to_string()),
                    model_prompt: None,
                }
            }
        }
        GoalOp::Set(objective) => AppliedGoal {
            next: Some(DurableGoal {
                objective: objective.clone(),
                paused: false,
            }),
            plan: GoalPlan::Enabled,
            reply: None,
            model_prompt: Some(goal_turn_prompt(&objective)),
        },
    }
}

/// `aster/goal/state` request: the bound definition of one live session.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GoalStateRequest {
    pub session_id: String,
}

/// A bound `/goal` reply, and whether this operation wakes bounded execution.
pub(crate) struct BoundGoalReply {
    pub reply: String,
    pub tick: bool,
}

fn control_command(
    current: &DurableGoalDefinitionV1,
    kind: GoalCommandKindV1,
    operation_id: String,
) -> GoalControlCommandV1 {
    GoalControlCommandV1 {
        kind,
        operation_id,
        goal_id: current.binding.goal_id.clone(),
        session_id: current.binding.session_id.clone(),
        workspace: current.binding.workspace.clone(),
        work_policy_digest: current.binding.work_policy_digest.clone(),
        expected_goal_revision: current.binding.goal_revision,
        expected_plan_revision: current.plan_revision,
        expected_goal_sequence: current.goal_sequence,
    }
}

/// Bound sessions use the original Core facts, never the legacy workspace file.
/// A first `set` binds the exact explicit task; `set` and `resume` wake the
/// original bounded driver. Core gates each slice on the trusted host policy.
pub(crate) async fn apply_bound_goal(
    session: &aster_core::AgentSession,
    op: &GoalOp,
    language: GoalLanguage,
) -> anyhow::Result<Option<BoundGoalReply>> {
    let created = if let GoalOp::Set(task) = op {
        if session.goal_definition()?.is_none()
            && load_goal(session.workspace(), session.session_id())
                .await?
                .is_none()
        {
            bind_task_goal(session, task).await?;
            true
        } else {
            false
        }
    } else {
        false
    };
    let Some(mut current) = session.goal_definition()? else {
        return Ok(None);
    };
    let control = |current: &DurableGoalDefinitionV1, kind: GoalCommandKindV1| {
        let operation_id = format!("acp-{}-{}", kind.as_str(), current.goal_sequence);
        session.apply_goal_control(control_command(current, kind, operation_id))
    };
    let mut tick = false;
    match op {
        GoalOp::Status => {}
        GoalOp::Pause if current.state != GoalState::Paused => {
            current = control(&current, GoalCommandKindV1::Pause)?;
        }
        GoalOp::Pause => {}
        GoalOp::Resume => {
            if current.state == GoalState::Paused {
                current = control(&current, GoalCommandKindV1::Resume)?;
            }
            tick = !matches!(current.state, GoalState::Completed | GoalState::Cancelled);
            if tick && current.import_source.is_none() && current.plan.is_none() {
                bind_task_goal(session, &current.goal.description).await?;
                current = session
                    .goal_definition()?
                    .ok_or_else(|| anyhow::anyhow!("goal.not_found"))?;
            }
        }
        GoalOp::Clear if current.state != GoalState::Cancelled => {
            current = control(&current, GoalCommandKindV1::Cancel)?;
        }
        GoalOp::Clear => {}
        GoalOp::Set(_) if created => tick = true,
        GoalOp::Set(_) => {
            return Err(GoalControlError::Refused {
                refusal: GoalRefusal::ConditionUnsupported,
                reason: "a bound Goal is revised only through aster/goal/definition",
            }
            .into());
        }
    }
    let language = language.for_goal(session, &current);
    let mut reply = bound_status(session, &current, language).await?;
    if matches!(op, GoalOp::Clear) {
        reply.push_str(match language {
            GoalLanguage::Chinese => "\n可新建会话开始其他任务。",
            GoalLanguage::English => "\nUse /new to start a different goal or ordinary work.",
        });
    }
    Ok(Some(BoundGoalReply { reply, tick }))
}

/// State, accepted plan progress and the next decision Core would take.
pub(crate) async fn bound_status(
    session: &aster_core::AgentSession,
    current: &DurableGoalDefinitionV1,
    language: GoalLanguage,
) -> anyhow::Result<String> {
    let language = language.for_goal(session, current);
    let preview = control_command(current, GoalCommandKindV1::Tick, "acp-status".into());
    let decision = if matches!(current.state, GoalState::Completed | GoalState::Cancelled) {
        None
    } else {
        Some(session.preview_goal_tick(&preview).await?)
    };
    let heading = match (language, current.state) {
        (GoalLanguage::English, GoalState::Active)
            if matches!(
                decision,
                Some(
                    GoalTickDecisionV1::NeedsInput
                        | GoalTickDecisionV1::BudgetExhausted
                        | GoalTickDecisionV1::Reconcile
                )
            ) =>
        {
            "Goal not complete yet."
        }
        (GoalLanguage::English, GoalState::Active) => "Goal in progress.",
        (GoalLanguage::English, GoalState::Waiting) => "Goal waiting for a result.",
        (GoalLanguage::English, GoalState::Paused) => "Goal paused.",
        (GoalLanguage::English, GoalState::Completed) => "Goal complete.",
        (GoalLanguage::English, GoalState::Cancelled) => "Goal cancelled.",
        (GoalLanguage::Chinese, GoalState::Active)
            if matches!(
                decision,
                Some(
                    GoalTickDecisionV1::NeedsInput
                        | GoalTickDecisionV1::BudgetExhausted
                        | GoalTickDecisionV1::Reconcile
                )
            ) =>
        {
            "目标尚未完成"
        }
        (GoalLanguage::Chinese, GoalState::Active) => "目标进行中",
        (GoalLanguage::Chinese, GoalState::Waiting) => "目标等待中",
        (GoalLanguage::Chinese, GoalState::Paused) => "目标已暂停",
        (GoalLanguage::Chinese, GoalState::Completed) => "目标已完成",
        (GoalLanguage::Chinese, GoalState::Cancelled) => "目标已取消",
    };
    let mut lines = vec![heading.to_string()];
    if let Some(plan) = &current.plan {
        let done = plan
            .steps
            .iter()
            .filter(|step| step.status == aster_core::planning::TaskStatus::Completed)
            .count();
        if current.state != GoalState::Completed {
            lines.push(match language {
                GoalLanguage::English => format!("Completed {done} of {} steps.", plan.steps.len()),
                GoalLanguage::Chinese => format!("已完成 {done}/{} 项。", plan.steps.len()),
            });
        }
        if !current.plan_accepted {
            lines.push(
                match language {
                    GoalLanguage::English => "Review and accept the plan to continue.",
                    GoalLanguage::Chinese => "请确认计划后继续。",
                }
                .into(),
            );
        }
        if decision == Some(GoalTickDecisionV1::Advance) && current.plan_accepted {
            if let Some(step) = plan.get_ready_steps().first() {
                lines.push(match language {
                    GoalLanguage::English => format!("Next: {}", step.content),
                    GoalLanguage::Chinese => format!("下一步：{}", step.content),
                });
            }
        } else if let Some(decision) = decision {
            lines.push(language.decision(decision));
        }
        if decision == Some(GoalTickDecisionV1::NeedsInput) && current.plan_accepted {
            if let Some(step) = plan.get_ready_steps().first() {
                if let Some(feedback) = session.goal_tick_feedback(&preview, &step.id)? {
                    if !feedback.remaining_criteria.is_empty() {
                        lines.push(
                            match language {
                                GoalLanguage::English => "Still to verify:",
                                GoalLanguage::Chinese => "仍需验证：",
                            }
                            .into(),
                        );
                        lines.extend(
                            feedback
                                .remaining_criteria
                                .iter()
                                .map(|criterion| format!("- {criterion}")),
                        );
                    }
                }
            }
        }
    } else if !matches!(current.state, GoalState::Completed | GoalState::Cancelled) {
        lines.push(
            match language {
                GoalLanguage::English => "A plan is needed before work can continue.",
                GoalLanguage::Chinese => "需要先制定计划后才能继续。",
            }
            .into(),
        );
    }
    Ok(lines.join("\n"))
}

/// What the host should do with a bound goal after a slice, or before the
/// first one. `Advance` carries a fresh tick command. A preview does not
/// admit a Run.
pub(crate) enum BoundGoalStep {
    Advance(GoalControlCommandV1),
    Finished,
    Stopped(GoalTickDecisionV1),
}

/// A terminal unattended admission refusal is a failure to finish the goal,
/// even when the most recent individual turn was verified.
#[derive(Debug)]
pub(crate) struct GoalContinuationStopped(pub GoalTickDecisionV1);

impl std::fmt::Display for GoalContinuationStopped {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "goal.continuation_stopped: the task goal is unfinished; next={}",
            self.0.as_str()
        )
    }
}

impl std::error::Error for GoalContinuationStopped {}

pub(crate) async fn bound_goal_step(
    session: &aster_core::AgentSession,
) -> anyhow::Result<BoundGoalStep> {
    let Some(current) = session.goal_definition()? else {
        return Ok(BoundGoalStep::Finished);
    };
    if matches!(current.state, GoalState::Completed | GoalState::Cancelled) {
        return Ok(BoundGoalStep::Finished);
    }
    let Some(command) = bound_tick_command(session)? else {
        return Ok(BoundGoalStep::Finished);
    };
    let decision = session.preview_goal_tick(&command).await?;
    Ok(match decision {
        aster_core::goal_control::GoalTickDecisionV1::Advance => BoundGoalStep::Advance(command),
        aster_core::goal_control::GoalTickDecisionV1::Complete => BoundGoalStep::Finished,
        other => BoundGoalStep::Stopped(other),
    })
}

/// Bind the explicit objective and exact context facts, then accept a model's
/// typed proposal. Checklist statuses never grant accepted progress.
pub(crate) async fn bind_task_goal(
    session: &aster_core::AgentSession,
    task: &str,
) -> anyhow::Result<GoalControlCommandV1> {
    use aster_core::goal_control::{GoalDefinitionCommandV1, GoalPlanCommandV1};
    use aster_core::planning::AgentGoal;
    let criterion = "Every requirement of the task holds in the workspace.";
    let definition = if let Some(current) = session.goal_definition()? {
        anyhow::ensure!(
            current.goal.description == task,
            "goal.conflict: the bound objective differs"
        );
        if current.plan_accepted {
            return bound_tick_command(session)?.ok_or_else(|| anyhow::anyhow!("goal.not_found"));
        }
        current
    } else {
        let thread = aster_core::fact_control::thread_for_session(session.session_id());
        let source_sequences = || {
            let workspace = session.workspace().to_path_buf();
            let thread = thread.clone();
            tokio_util::task::AbortOnDropHandle::new(tokio::task::spawn_blocking(
                move || -> anyhow::Result<Vec<u64>> {
                    let facts =
                        aster_core::fact_control::read_workspace_facts(&workspace, &thread)?;
                    let active = facts
                        .iter()
                        .rposition(|fact| fact.kind == "compaction.done")
                        .unwrap_or(0);
                    Ok(facts[active..]
                        .iter()
                        .filter(|fact| {
                            matches!(
                                fact.kind.as_str(),
                                "user.message" | "model.turn" | "compaction.done"
                            )
                        })
                        .map(|fact| fact.seq)
                        .collect())
                },
            ))
        };
        let mut scope_fact_sequences = source_sequences().await??;
        if scope_fact_sequences.len() > 32 {
            session.compact_conversation(Some("Preserve the authorized plan, original requirements, constraints, unfinished work and failures. Checklist completion is not accepted Goal progress.")).await?;
            scope_fact_sequences = source_sequences().await??;
        }
        anyhow::ensure!(
            scope_fact_sequences.len() <= 32,
            "goal.scope_too_large: compact the conversation before binding its plan"
        );
        session.apply_goal_definition(GoalDefinitionCommandV1 {
            kind: GoalCommandKindV1::Create,
            import_source: None,
            operation_id: "unattended-create".into(),
            goal_id: "unattended-task".into(),
            session_id: session.session_id().into(),
            workspace: aster_core::workspace::WorkspaceServices::local(session.workspace())
                .workspace_ref()
                .clone(),
            goal: AgentGoal::new(task).with_criteria(vec![criterion.into()]),
            non_goals: vec![],
            scope_fact_sequences,
            work_policy_digest: session.work_policy_digest()?,
            expected_goal_revision: None,
            expected_plan_revision: None,
            expected_goal_sequence: None,
        })?
    };
    let mut proposal = GoalPlanCommandV1 {
        kind: GoalCommandKindV1::ProposePlan,
        operation_id: format!("unattended-plan-{}", definition.goal_sequence),
        goal_id: definition.binding.goal_id.clone(),
        session_id: definition.binding.session_id.clone(),
        workspace: definition.binding.workspace.clone(),
        work_policy_digest: definition.binding.work_policy_digest.clone(),
        expected_goal_revision: definition.binding.goal_revision,
        expected_plan_revision: definition.plan_revision,
        expected_goal_sequence: definition.goal_sequence,
        plan: None,
        proposal_sequence: None,
    };
    let proposed = if definition.plan.is_some() {
        definition
    } else {
        proposal.plan = Some(session.generate_goal_plan().await?);
        session.apply_goal_plan(proposal.clone())?
    };
    proposal.kind = GoalCommandKindV1::AcceptPlan;
    proposal.operation_id = format!("unattended-accept-{}", proposed.goal_sequence);
    proposal.expected_plan_revision = proposed.plan_revision;
    proposal.expected_goal_sequence = proposed.goal_sequence;
    proposal.plan = None;
    proposal.proposal_sequence = proposed.plan_fact_sequence;
    session.apply_goal_plan(proposal)?;
    bound_tick_command(session)?
        .ok_or_else(|| anyhow::anyhow!("goal.not_found: the bound task goal is missing"))
}

pub(crate) fn bound_tick_command(
    session: &aster_core::AgentSession,
) -> anyhow::Result<Option<GoalControlCommandV1>> {
    Ok(session.goal_definition()?.map(|current| {
        let operation_id = format!("acp-tick-{}", uuid::Uuid::new_v4());
        control_command(&current, GoalCommandKindV1::Tick, operation_id)
    }))
}

/// The slice's Run outcome (when one ran) and the goal status after it.
pub(crate) async fn bound_slice_report(
    session: &aster_core::AgentSession,
    run_id: Option<&str>,
    language: GoalLanguage,
) -> anyhow::Result<String> {
    let current = session.goal_definition()?;
    let language = current
        .as_ref()
        .map(|goal| language.for_goal(session, goal))
        .unwrap_or(language);
    let mut lines = Vec::new();
    if let Some(run_id) = run_id {
        let snapshot = session
            .run_snapshot(run_id)
            .await
            .ok_or_else(|| anyhow::anyhow!("goal slice {run_id} has no Run record"))?;
        tracing::debug!(run_id = %snapshot.id, status = ?snapshot.status, "Goal execution status");
        let message = match (language, snapshot.status) {
            (_, aster_core::run::RunStatus::Completed) => None,
            (GoalLanguage::English, aster_core::run::RunStatus::Failed) => {
                Some("The latest execution failed.")
            }
            (GoalLanguage::Chinese, aster_core::run::RunStatus::Failed) => Some("本次执行未完成。"),
            (GoalLanguage::English, aster_core::run::RunStatus::Cancelled) => {
                Some("The latest execution was cancelled.")
            }
            (GoalLanguage::Chinese, aster_core::run::RunStatus::Cancelled) => {
                Some("本次执行已取消。")
            }
            (GoalLanguage::English, aster_core::run::RunStatus::Verifying) => {
                Some("Verifying the latest results.")
            }
            (GoalLanguage::Chinese, aster_core::run::RunStatus::Verifying) => {
                Some("正在验证本次结果。")
            }
            (GoalLanguage::English, _) => Some("The current step is running."),
            (GoalLanguage::Chinese, _) => Some("正在执行当前步骤。"),
        };
        if let Some(message) = message {
            lines.push(message.into());
        }
    }
    if let Some(after) = session.goal_definition()? {
        lines.push(bound_status(session, &after, language).await?);
    }
    Ok(lines.join("\n\n"))
}

/// Later user turns carry an active objective until it is paused or cleared.
pub fn standing_goal_prompt(goal: Option<&DurableGoal>, user: &str) -> String {
    match goal {
        Some(goal) if !goal.paused && !goal.objective.trim().is_empty() => format!(
            "Note: a durable goal is set for this session: \"{}\". It stays in force until \"/goal clear\".\
If the user's message asks to work on this goal or clearly continues it, proceed; a greeting or \
an unrelated question needs only a normal reply — do not start executing the goal unprompted.\n\n{user}",
            goal.objective.trim()
        ),
        _ => user.to_string(),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn legacy_commands_follow_the_host_language_without_repeating_the_objective() {
        use super::*;
        let language = GoalLanguage::from_host(Some("zh-CN"));
        let goal = DurableGoal {
            objective: "The original English task".into(),
            paused: false,
        };
        let active = apply_goal(Some(goal), GoalOp::Status);
        assert_eq!(
            language.legacy_reply(active.next.as_ref(), active.reply.as_deref().unwrap()),
            "目标进行中。"
        );
        let paused = apply_goal(active.next, GoalOp::Pause);
        assert_eq!(
            language.legacy_reply(paused.next.as_ref(), paused.reply.as_deref().unwrap()),
            "目标已暂停，可发送 /goal resume 继续。"
        );
        let cleared = apply_goal(paused.next, GoalOp::Clear);
        assert_eq!(
            language.legacy_reply(cleared.next.as_ref(), cleared.reply.as_deref().unwrap()),
            "当前未设置目标。发送 /goal <目标内容> 开始。"
        );
        assert_eq!(
            GoalLanguage::English.legacy_reply(None, "Cleared the durable goal."),
            "Cleared the durable goal."
        );
    }

    #[test]
    fn product_default_bounds_are_finite_and_explicit() {
        let policy = super::default_continuation_policy(5_000).unwrap();
        assert!(policy.automatic_enabled);
        assert_eq!(policy.max_slices.get(), 8);
        assert_eq!(policy.max_no_progress_slices.get(), 2);
        assert_eq!(policy.deadline_unix_ms.get(), 5_000 + 24 * 60 * 60 * 1_000);
        assert!(policy.check_limits(5_000, 8, 0).is_err());
        assert!(policy.check_limits(5_000, 0, 2).is_err());
        assert!(policy
            .check_limits(policy.deadline_unix_ms.get(), 0, 0)
            .is_err());
        assert!(super::default_continuation_policy(u64::MAX).is_err());
    }

    use super::*;

    #[test]
    fn parse_rejects_a_longer_token() {
        assert!(parse_goal_command("/goalie ship it").is_none());
        assert!(parse_goal_command("please /goal ship it").is_none());
    }

    #[test]
    fn parse_verbs_and_objectives() {
        assert_eq!(parse_goal_command("/goal"), Some(GoalOp::Status));
        assert_eq!(parse_goal_command("  /goal status"), Some(GoalOp::Status));
        assert_eq!(parse_goal_command("/goal PAUSE"), Some(GoalOp::Pause));
        assert_eq!(parse_goal_command("/goal resume"), Some(GoalOp::Resume));
        assert_eq!(parse_goal_command("/goal clear"), Some(GoalOp::Clear));
        assert_eq!(
            parse_goal_command("/goal Pause the rollout"),
            Some(GoalOp::Set("Pause the rollout".to_string()))
        );
    }

    #[test]
    fn set_enables_planning_and_keeps_the_objective() {
        let applied = apply_goal(None, GoalOp::Set("Ship the fix".into()));
        assert_eq!(applied.plan, GoalPlan::Enabled);
        assert_eq!(
            applied.next,
            Some(DurableGoal {
                objective: "Ship the fix".into(),
                paused: false,
            })
        );
        let prompt = applied.model_prompt.expect("model prompt");
        assert!(prompt.starts_with("Ship the fix\n"));
        assert!(!prompt.contains("/goal"));
        assert!(applied.reply.is_none());
    }

    #[test]
    fn status_pause_resume_and_clear_round_trip() {
        let set = apply_goal(None, GoalOp::Set("Ship the fix".into()));
        let goal = set.next.clone().expect("goal");

        let status = apply_goal(Some(goal.clone()), GoalOp::Status);
        assert_eq!(status.plan, GoalPlan::Unchanged);
        assert_eq!(status.reply.as_deref(), Some("Goal in progress."));
        assert!(status.model_prompt.is_none());

        let paused = apply_goal(Some(goal.clone()), GoalOp::Pause);
        assert_eq!(paused.plan, GoalPlan::Disabled);
        assert!(paused.next.as_ref().is_some_and(|goal| goal.paused));
        assert!(paused.model_prompt.is_none());

        let resumed = apply_goal(paused.next, GoalOp::Resume);
        assert_eq!(resumed.plan, GoalPlan::Enabled);
        assert!(resumed.next.as_ref().is_some_and(|goal| !goal.paused));
        assert!(resumed
            .model_prompt
            .as_deref()
            .is_some_and(|prompt| prompt.contains("Ship the fix")));

        let cleared = apply_goal(resumed.next, GoalOp::Clear);
        assert_eq!(cleared.next, None);
        assert!(cleared.plan.clears_override());
        assert_eq!(cleared.reply.as_deref(), Some("Goal cleared."));
    }

    #[test]
    fn missing_goal_does_not_start_a_turn() {
        for op in [GoalOp::Status, GoalOp::Pause, GoalOp::Resume, GoalOp::Clear] {
            let applied = apply_goal(None, op);
            assert_eq!(applied.next, None);
            assert_eq!(applied.plan, GoalPlan::Unchanged);
            assert!(applied.model_prompt.is_none());
            assert!(applied.reply.unwrap().contains("No goal is set."));
        }
    }
}
