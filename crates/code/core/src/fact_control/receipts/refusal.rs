//! Repeated host refusals under the same original execution authority.

use super::*;
use std::collections::{BTreeMap, BTreeSet};

pub(in crate::fact_control) enum ExecutionCycle {
    Refused(String),
    ObservedState(String),
}

impl ExecutionCycle {
    pub(in crate::fact_control) fn diagnostic(self) -> String {
        match self {
            Self::Refused(tool) => format!("run.tool_refusal_cycle: tool '{tool}' repeated a refused invocation without fresh original progress; revise the approach or obtain new host input"),
            Self::ObservedState(tool) => format!("run.tool_state_cycle: tool '{tool}' exhausted its duplicate allowance while revisiting already observed workspace states without fresh original information; revise the approach or obtain new host input"),
        }
    }
}

pub(in crate::fact_control) fn execution_cycle(
    facts: &[a3s_effect::Fact],
    session_id: &str,
    run_id: &str,
    binding: &str,
    duplicate_threshold: u32,
) -> Option<ExecutionCycle> {
    fold_execution_cycle(
        facts,
        Some((session_id, run_id, binding)),
        duplicate_threshold,
    )
}

/// Only the host-owned, exclusively fenced ChildRun log may use unbound receipts.
pub(in crate::fact_control) fn child_execution_cycle(
    facts: &[a3s_effect::Fact],
    duplicate_threshold: u32,
) -> Option<ExecutionCycle> {
    fold_execution_cycle(facts, None, duplicate_threshold)
}

fn fold_execution_cycle(
    facts: &[a3s_effect::Fact],
    admission: Option<(&str, &str, &str)>,
    duplicate_threshold: u32,
) -> Option<ExecutionCycle> {
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
    // Only repeated actions need the full workspace-state fold.
    let mut denied = 0u32;
    let mut mutated: BTreeMap<(String, String), u32> = BTreeMap::new();
    let mut preparations = None;
    let mut state_candidate = false;
    for result in facts[start..]
        .iter()
        .filter(|fact| fact.kind == "tool.result")
        .filter_map(|fact| {
            serde_json::from_value::<ToolExecutionResultV1>(fact.payload.get("receipt")?.clone())
                .ok()
        })
        .filter(|result| current(result) == Some(true))
    {
        denied += u32::from(result.outcome == ToolExecutionOutcome::Denied);
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
        if denied >= 2 || state_candidate {
            break;
        }
    }
    if denied < 2 && !state_candidate {
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
                cycle = Some(ExecutionCycle::ObservedState(tool.clone()));
            }
        }
        if result.outcome == ToolExecutionOutcome::Denied
            && result.effects.completeness == ToolObservationCompleteness::Complete
            && result.effects.changes.is_empty()
            && !refused.insert((tool.clone(), args.clone()))
        {
            cycle = Some(ExecutionCycle::Refused(tool.clone()));
        }
    }
    cycle
}

fn prepared_actions(facts: &[a3s_effect::Fact]) -> BTreeMap<String, (String, String)> {
    facts
        .iter()
        .filter(|fact| fact.kind == "tool.prepared")
        .filter_map(|fact| {
            let prepared: ToolPreparedV1 =
                serde_json::from_value(fact.payload.get("prepared")?.clone()).ok()?;
            Some((
                prepared.identity.key().ok()?,
                (prepared.tool_name, prepared.effective_args_digest),
            ))
        })
        .collect()
}
