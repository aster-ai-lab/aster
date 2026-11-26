//! Repeated host refusals under the same original execution authority.

use super::*;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

pub(in crate::fact_control) enum ExecutionControl {
    Replan { tool: String, kind: RefusalKind },
    Refused(String),
    ObservedState(String),
}

impl ExecutionControl {
    pub(in crate::fact_control) fn apply(
        self,
        messages: &mut Vec<Message>,
    ) -> Result<(), ActorError> {
        match self {
            Self::Replan { tool, kind } => {
                messages.push(Message::user_wire(&format!(
                    "[host tool recovery] The host refused '{tool}' before execution ({}). \
                     Use tools already authorized for a different way to achieve the original task. \
                     Do not retry the refused action without new authorization. If no authorized route \
                     exists, report the specific blocker and preserve verified work.",
                    kind.as_str(),
                )));
                Ok(())
            }
            Self::Refused(tool) => Err(ActorError::Defect(format!(
                "run.tool_refusal_cycle: tool '{tool}' repeated a refusal without fresh original progress; change the approach or obtain the required host input"
            ))),
            Self::ObservedState(tool) => Err(ActorError::Defect(format!(
                "run.tool_state_cycle: tool '{tool}' exhausted its duplicate allowance while revisiting already observed workspace states without fresh original information; revise the approach or obtain new host input"
            ))),
        }
    }
}

#[derive(Clone, Copy)]
pub(in crate::fact_control) enum RefusalKind {
    PermissionDenied,
    ConfirmationRequired,
    ExecutionUnavailable,
}

impl RefusalKind {
    fn parse(code: &str) -> Option<Self> {
        match code {
            "permission_denied" => Some(Self::PermissionDenied),
            "confirmation_required" => Some(Self::ConfirmationRequired),
            "execution_unavailable" => Some(Self::ExecutionUnavailable),
            _ => None,
        }
    }

    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::PermissionDenied => "permission_denied",
            Self::ConfirmationRequired => "confirmation_required",
            Self::ExecutionUnavailable => "execution_unavailable",
        }
    }

    fn from_result(result: &ToolExecutionResultV1) -> Option<Self> {
        let envelope = decode_fact_tool_result(result.output.as_str()?)?;
        Self::parse(envelope.metadata?.get("host_refusal")?.as_str()?)
    }
}

pub(in crate::fact_control) fn execution_control(
    facts: &[aster_effect::Fact],
    session_id: &str,
    run_id: &str,
    binding: &str,
    duplicate_threshold: u32,
) -> Option<ExecutionControl> {
    fold_execution_control(
        facts,
        Some((session_id, run_id, binding)),
        duplicate_threshold,
    )
}

/// Only the host-owned, exclusively fenced ChildRun log may use unbound receipts.
pub(in crate::fact_control) fn child_execution_control(
    facts: &[aster_effect::Fact],
    duplicate_threshold: u32,
) -> Option<ExecutionControl> {
    fold_execution_control(facts, None, duplicate_threshold)
}

fn fold_execution_control(
    facts: &[aster_effect::Fact],
    admission: Option<(&str, &str, &str)>,
    duplicate_threshold: u32,
) -> Option<ExecutionControl> {
    // None is a private child-log authority, never inferred from a missing Run id.
    let current = |result: &ToolExecutionResultV1| match (admission, &result.identity.admission) {
        (Some((session_id, run_id, binding)), Some(source))
            if source.thread_id == session_id
                && (source.run_id != run_id || source.capability_binding_digest == binding) =>
        {
            Some(source.run_id == run_id)
        }
        (None, None) => Some(true),
        _ => None,
    };
    let start = facts
        .iter()
        .rposition(|fact| fact.kind == "user.message")
        .map_or(0, |index| index + 1);
    // Only possible recovery or repeated actions need the full fold.
    let mut denied = 0u32;
    let mut recoverable = false;
    let mut mutated: BTreeMap<(String, String), u32> = BTreeMap::new();
    let mut preparations = None;
    let mut state_candidate = false;
    for result in facts[start..]
        .iter()
        .filter(|fact| fact.kind == "tool.result")
        .filter_map(|fact| ToolExecutionResultV1::deserialize(fact.payload.get("receipt")?).ok())
        .filter(|result| current(result) == Some(true))
    {
        if result.outcome == ToolExecutionOutcome::Denied {
            denied += 1;
            recoverable |= RefusalKind::from_result(&result).is_some();
        }
        if duplicate_threshold > 0
            && super::super::goal_evidence::observed_workspace_mutation(&result.effects)
        {
            let actions = preparations.get_or_insert_with(|| prepared_actions(facts));
            if let Some(action) = actions.get(&result.identity.key().ok()?) {
                let count = mutated.entry(action.clone()).or_default();
                *count = count.saturating_add(1);
                state_candidate |= *count > duplicate_threshold;
            }
        }
        if recoverable || denied >= 2 || state_candidate {
            break;
        }
    }
    if !recoverable && denied < 2 && !state_candidate {
        return None;
    }
    let debt = fold_cache_evidence(facts);
    if debt.integrity_invalid || !debt.unresolved.is_empty() {
        return None;
    }
    let preparations = preparations.unwrap_or_else(|| prepared_actions(facts));
    let mut information = BTreeSet::new();
    let mut refused = BTreeSet::new();
    let mut executions: BTreeMap<(String, String), u32> = BTreeMap::new();
    let mut workspace = crate::goal_control::MutationProgress::default();
    let mut cycle = None;
    let mut first_refusal: Option<((u64, u64), String, RefusalKind)> = None;
    for projection in &debt.projection_order {
        let index = debt.projection_fact_index(projection)?;
        let ReceiptProjection::Tool(tool_index) = projection else {
            if index >= start
                && admission.is_some_and(|(_, run_id, _)| {
                    facts[index].payload["source_run"].as_str() == Some(run_id)
                })
            {
                refused.clear();
                executions.clear();
                cycle = None;
                first_refusal = None;
            }
            continue;
        };
        let (tool, result) = debt.results.get(*tool_index)?;
        let Some(is_current) = current(result) else {
            continue;
        };
        let (_, args) = preparations.get(&result.identity.key().ok()?)?;
        let novel = novelty::information_identity(tool, result, args)
            .ok()?
            .is_some_and(|identity| information.insert(identity));
        if !is_current || index < start {
            continue;
        }
        let changes = result
            .effects
            .changes
            .iter()
            .filter(|change| !crate::porcelain::is_harness_path(&change.path))
            .cloned()
            .collect();
        let changed = workspace.observe(Some(changes)).ok()?;
        let coherent = workspace.last_observation_is_coherent();
        // A gap starts a fresh allowance without disabling later detection.
        if novel || changed || !coherent {
            refused.clear();
            executions.clear();
            cycle = None;
            first_refusal = None;
        }
        let round = (result.identity.turn, result.identity.cycle);
        if let Some((first, tool, _)) = &first_refusal {
            if *first != round {
                cycle = Some(ExecutionControl::Refused(tool.clone()));
            }
        }
        if duplicate_threshold > 0
            && coherent
            && matches!(
                result.outcome,
                ToolExecutionOutcome::Succeeded | ToolExecutionOutcome::Failed
            )
            && result.effects.completeness == ToolObservationCompleteness::Complete
            && super::super::goal_evidence::observed_workspace_mutation(&result.effects)
        {
            let count = executions.entry((tool.clone(), args.clone())).or_default();
            *count = count.saturating_add(1);
            if *count > duplicate_threshold {
                cycle = Some(ExecutionControl::ObservedState(tool.clone()));
            }
        }
        if result.outcome == ToolExecutionOutcome::Denied
            && result.effects.completeness == ToolObservationCompleteness::Complete
            && result.effects.changes.is_empty()
        {
            if !refused.insert((tool.clone(), args.clone())) {
                cycle = Some(ExecutionControl::Refused(tool.clone()));
            }
            if let Some(kind) = RefusalKind::from_result(result) {
                first_refusal.get_or_insert_with(|| (round, tool.clone(), kind));
            }
        }
    }
    cycle.or_else(|| first_refusal.map(|(_, tool, kind)| ExecutionControl::Replan { tool, kind }))
}

fn prepared_actions(facts: &[aster_effect::Fact]) -> BTreeMap<String, (String, String)> {
    facts
        .iter()
        .filter(|fact| fact.kind == "tool.prepared")
        .filter_map(|fact| {
            let prepared: ToolPreparedV1 =
                ToolPreparedV1::deserialize(fact.payload.get("prepared")?).ok()?;
            Some((
                prepared.identity.key().ok()?,
                (prepared.tool_name, prepared.effective_args_digest),
            ))
        })
        .collect()
}
