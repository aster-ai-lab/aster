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
                     Do not retry earlier refused actions without a change in the blocking condition. \
                     Ask for specific missing task information when needed. Tool permission requires the host's approval channel. \
                     If no authorized route exists, report the specific blocker and preserve verified work.",
                    kind.as_str(),
                )));
                Ok(())
            }
            Self::Refused(tool) => Err(ActorError::Defect(format!(
                "run.tool_refusal_cycle: recovery after a refusal from '{tool}' stalled without fresh original progress; change the approach or obtain the required host input"
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

struct RecoveryAttempt<'a> {
    round: (u64, u64),
    alternate_refusal: Option<(&'a str, RefusalKind)>,
}

struct RefusedArg<'a> {
    round: (u64, u64),
    batch_index: u32,
    call_id: &'a str,
}

struct RefusedTool<'a> {
    first_round: (u64, u64),
    args: BTreeMap<&'a str, RefusedArg<'a>>,
}

struct PreparedAction {
    tool: String,
    args: String,
    wave: u64,
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
        .rposition(|fact| matches!(fact.kind.as_str(), "user.message" | "question.answered"))
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
            let (actions, _) = preparations.get_or_insert_with(|| prepared_actions(facts));
            if let Some(action) = actions.get(&result.identity.key().ok()?) {
                let count = mutated
                    .entry((action.tool.clone(), action.args.clone()))
                    .or_default();
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
    let (preparations, has_model_turns) = preparations.unwrap_or_else(|| prepared_actions(facts));
    let mut information = BTreeSet::new();
    let mut refused: BTreeMap<&str, RefusedTool<'_>> = BTreeMap::new();
    let mut executions: BTreeMap<(String, String), u32> = BTreeMap::new();
    let mut workspace = crate::goal_control::MutationProgress::default();
    let mut cycle = None;
    let mut first_refusal: Option<((u64, u64), &str, RefusalKind)> = None;
    let mut recovery_attempt = None;
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
                recovery_attempt = None;
            }
            continue;
        };
        let (tool, result) = debt.results.get(*tool_index)?;
        let Some(is_current) = current(result) else {
            continue;
        };
        let prepared = preparations.get(&result.identity.key().ok()?)?;
        let args = &prepared.args;
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
            recovery_attempt = None;
        }
        let round = if has_model_turns {
            (result.identity.turn, prepared.wave)
        } else {
            (result.identity.turn, result.identity.cycle)
        };
        if let Some((first_round, first_tool, _)) = first_refusal {
            if first_round != round {
                let attempt = recovery_attempt.get_or_insert(RecoveryAttempt {
                    round,
                    alternate_refusal: None,
                });
                if attempt.round != round {
                    let blocked_tool = attempt
                        .alternate_refusal
                        .map_or(first_tool, |(alternate, _)| alternate);
                    cycle = Some(ExecutionControl::Refused(blocked_tool.to_owned()));
                }
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
            let batch_index = result.identity.batch_index;
            let entry = refused.entry(tool.as_str()).or_insert_with(|| RefusedTool {
                first_round: round,
                args: BTreeMap::new(),
            });
            let first_denied_round = entry.first_round;
            let repeated_same_args = match entry.args.get(args.as_str()) {
                Some(prior) => {
                    // Calls emitted in the same model turn (including batches
                    // split on `needs_confirmation`) share one wave before the
                    // model observes any refusal.
                    if has_model_turns {
                        prior.round != round
                            || prior.call_id == result.identity.tool_call_id.as_str()
                    } else {
                        prior.round != round || prior.batch_index == batch_index
                    }
                }
                None => {
                    entry.args.insert(
                        args.as_str(),
                        RefusedArg {
                            round,
                            batch_index,
                            call_id: result.identity.tool_call_id.as_str(),
                        },
                    );
                    false
                }
            };
            if repeated_same_args || (first_refusal.is_some() && first_denied_round != round) {
                cycle = Some(ExecutionControl::Refused(tool.clone()));
            }
            if let Some(kind) = RefusalKind::from_result(result) {
                first_refusal.get_or_insert((round, tool, kind));
                // Surface a new tool's refusal from the first recovery wave.
                // It buys one decision, with no new allowance for later waves.
                if let Some(attempt) = &mut recovery_attempt {
                    if attempt.round == round && first_denied_round == round {
                        attempt.alternate_refusal.get_or_insert((tool, kind));
                    }
                }
            }
        }
    }
    cycle.or_else(|| {
        let (_, first_tool, first_kind) = first_refusal?;
        let (tool, kind) = match recovery_attempt {
            None => (first_tool, first_kind),
            Some(attempt) => match attempt.alternate_refusal {
                Some(alternate) => alternate,
                None => return Some(ExecutionControl::Refused(first_tool.to_owned())),
            },
        };
        Some(ExecutionControl::Replan {
            tool: tool.to_owned(),
            kind,
        })
    })
}

fn prepared_actions(facts: &[aster_effect::Fact]) -> (BTreeMap<String, PreparedAction>, bool) {
    let mut wave = 0u64;
    let mut actions = BTreeMap::new();
    for fact in facts {
        if fact.kind == "model.turn" {
            wave = wave.saturating_add(1);
            continue;
        }
        if fact.kind != "tool.prepared" {
            continue;
        }
        let Some(prepared) = fact
            .payload
            .get("prepared")
            .and_then(|value| ToolPreparedV1::deserialize(value).ok())
        else {
            continue;
        };
        let Ok(key) = prepared.identity.key() else {
            continue;
        };
        actions.insert(
            key,
            PreparedAction {
                tool: prepared.tool_name,
                args: prepared.effective_args_digest,
                wave,
            },
        );
    }
    (actions, wave > 0)
}
