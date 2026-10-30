//! Original closeout of one goal slice: the owner's outcome, its completion
//! boundary, its accepted step and its usage. Unknown usage stays unknown; an
//! unknown tool outcome refuses until the original receipt path settles it.

use super::*;
use crate::llm::TokenUsage;

closed_set!(GoalSliceOutcomeV1 {
    Completed => "completed", Interrupted => "interrupted",
});

/// Advisory judgment data; the enclosing settlement binds goal and plan revisions.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GoalStepFeedback {
    pub(crate) step_id: String,
    pub(crate) remaining_criteria: Vec<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SettledBody {
    run_id: String,
    run_sequence: u64,
    outcome: GoalSliceOutcomeV1,
    #[serde(deserialize_with = "Option::<u64>::deserialize")]
    completion_sequence: Option<u64>,
    #[serde(deserialize_with = "Option::<u64>::deserialize")]
    step_sequence: Option<u64>,
    #[serde(deserialize_with = "Option::<TokenUsage>::deserialize")]
    usage: Option<TokenUsage>,
    /// The slice's four-dimension progress (specs/010 P2). Absent on facts
    /// written before the field existed: those settle mutation-only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    progress: Option<crate::harness_loop::TurnProgressDelta>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    feedback: Option<GoalStepFeedback>,
}

type SettledFact = GoalFact<SettledBody>;

pub(super) fn operation_id(run_id: &str) -> String {
    crate::content_digest::digest_bytes("a3s.code.goal-settle-operation.v1", run_id.as_bytes())
}

fn digest(view: &DurableGoalDefinitionV1, body: &SettledBody) -> Result<String, GoalControlError> {
    command_digest(&(
        GoalFactKindV1::SliceSettled,
        &view.binding,
        view.plan_revision,
        body,
    ))
}

/// Every recorded slice (sequence, goal, Run) in log order. An unreadable
/// slice keeps an empty Run id, so it can never look settled.
fn slices(facts: &[Fact]) -> Vec<(u64, String, String)> {
    facts
        .iter()
        .filter(|fact| fact.kind == GoalFactKindV1::SliceAdmitted.as_str())
        .map(|fact| match run::RunFact::deserialize(&fact.payload) {
            Ok(stored) => (fact.seq, stored.goal_id, stored.body.run_id),
            Err(_) => (fact.seq, String::new(), String::new()),
        })
        .collect()
}

fn settled(facts: &[Fact], goal_id: &str, run_id: &str) -> bool {
    goal_fact_key(goal_id, &operation_id(run_id), GoalFactKindV1::SliceSettled)
        .is_ok_and(|key| facts.iter().any(|fact| fact.key == key))
}

/// Recorded slices whose original Run has no closeout yet.
pub(crate) fn unsettled_slices(facts: &[Fact]) -> Vec<String> {
    slices(facts)
        .into_iter()
        .filter(|(_, goal, run)| !settled(facts, goal, run))
        .map(|(_, _, run)| run)
        .collect()
}

/// The completion boundary and accepted step inside this slice's own window,
/// which ends at the next recorded slice.
fn closeout_refs(
    facts: &[Fact],
    goal_id: &str,
    run_id: &str,
    run_sequence: u64,
) -> Result<(Option<u64>, Option<u64>), GoalControlError> {
    let end = facts
        .iter()
        .find(|fact| fact.seq > run_sequence && fact.kind == GoalFactKindV1::SliceAdmitted.as_str())
        .map_or(u64::MAX, |fact| fact.seq);
    let window = facts
        .iter()
        .filter(|fact| fact.seq > run_sequence && fact.seq < end);
    let step_key = goal_fact_key(
        goal_id,
        &step::operation_id(run_id),
        GoalFactKindV1::StepAccepted,
    )?;
    let mut completion = None;
    let mut step = None;
    for fact in window {
        if fact.kind == crate::fact_control::TURN_COMPLETE_FACT_KIND {
            completion = Some(fact.seq);
        } else if fact.key == step_key {
            step = Some(fact.seq);
        }
    }
    Ok((completion, step))
}

/// Closeout for `run_id`, or `None` when it recorded no slice or is already
/// settled. `usage` is the owner's proven Run usage; it is kept only for a Run
/// that reached its completion boundary, otherwise usage stays unknown.
pub(crate) fn prepare_settlement(
    facts: &[Fact],
    session_id: &str,
    workspace: &WorkspaceRef,
    policy_digest: Option<&str>,
    run_id: &str,
    usage: Option<TokenUsage>,
    progress: Option<crate::harness_loop::TurnProgressDelta>,
    feedback: Option<GoalStepFeedback>,
) -> Result<Option<NewFact>, GoalControlError> {
    let Some(view) = definition_from_facts(facts, session_id, workspace, policy_digest)? else {
        return Ok(None);
    };
    let goal_id = view.binding.goal_id.as_str();
    let Some(run_sequence) = slices(facts)
        .into_iter()
        .find(|(_, goal, run)| goal == goal_id && run == run_id)
        .map(|(seq, _, _)| seq)
    else {
        return Ok(None);
    };
    if settled(facts, goal_id, run_id) {
        return Ok(None);
    }
    if let Some(feedback) = &feedback {
        let admission = facts
            .iter()
            .find(|fact| fact.seq == run_sequence)
            .and_then(|fact| run::RunFact::deserialize(&fact.payload).ok())
            .ok_or_else(|| refused(GoalRefusal::Conflict, "judgment admission is absent"))?;
        if feedback.step_id != admission.body.step_id {
            return Err(refused(
                GoalRefusal::Conflict,
                "judgment feedback differs from its original step",
            ));
        }
    }
    if crate::fact_control::has_unsettled_effects(facts) {
        return Err(refused(
            GoalRefusal::ReceiptUnsettled,
            "slice execution outcome is unknown",
        ));
    }
    let (completion_sequence, step_sequence) = closeout_refs(facts, goal_id, run_id, run_sequence)?;
    let (outcome, usage) = match (completion_sequence, usage) {
        (Some(_), Some(usage)) => (GoalSliceOutcomeV1::Completed, Some(usage)),
        _ => (GoalSliceOutcomeV1::Interrupted, None),
    };
    let body = SettledBody {
        run_id: run_id.into(),
        run_sequence,
        outcome,
        completion_sequence,
        step_sequence,
        usage,
        progress,
        feedback,
    };
    let operation_id = operation_id(run_id);
    let stored = SettledFact {
        schema: GOAL_FACT_SCHEMA_V1.into(),
        session_id: session_id.into(),
        goal_id: goal_id.into(),
        operation_id: operation_id.clone(),
        command_digest: digest(&view, &body)?,
        goal_revision: view.binding.goal_revision,
        plan_revision: view.plan_revision,
        work_policy_digest: view.binding.work_policy_digest.clone(),
        body,
    };
    Ok(Some(NewFact {
        kind: GoalFactKindV1::SliceSettled.as_str().into(),
        key: goal_fact_key(goal_id, &operation_id, GoalFactKindV1::SliceSettled)?,
        payload: serde_json::to_value(stored)?,
    }))
}

/// Close every slice whose owner stopped before its closeout. Nothing runs,
/// replays or is re-accepted, and usage stays unknown.
pub(crate) fn reconcile_slices(
    facts: &[Fact],
    session_id: &str,
    workspace: &WorkspaceRef,
    policy_digest: Option<&str>,
) -> Result<Vec<NewFact>, GoalControlError> {
    let mut pending = Vec::new();
    for run_id in unsettled_slices(facts) {
        if let Some(fact) = prepare_settlement(
            facts,
            session_id,
            workspace,
            policy_digest,
            &run_id,
            None,
            None,
            None,
        )? {
            pending.push(fact);
        }
    }
    Ok(pending)
}

/// Closeout changes no goal state, so it leaves `goal_sequence` (the reference
/// controls and ticks compare) where the Run or its accepted step put it.
pub(super) fn fold_settle(
    fact: &Fact,
    current: Option<&DurableGoalDefinitionV1>,
    facts: &[Fact],
    session_id: &str,
    workspace: &WorkspaceRef,
    policy_digest: Option<&str>,
    continuation: &mut run::ContinuationUsage,
) -> Result<(String, String, DurableGoalDefinitionV1, bool), GoalControlError> {
    let current = current.ok_or_else(|| refused(GoalRefusal::NotFound, "goal is absent"))?;
    let stored = SettledFact::deserialize(&fact.payload)
        .map_err(|_| refused(GoalRefusal::Conflict, "invalid slice settlement fact"))?;
    stored.validate_source(fact, session_id, policy_digest, Some(current))?;
    let prefix = &facts[..facts
        .iter()
        .position(|source| source.seq == fact.seq)
        .unwrap_or(facts.len())];
    let body = &stored.body;
    let slice = prefix
        .iter()
        .find(|source| source.seq == body.run_sequence)
        .ok_or_else(|| refused(GoalRefusal::Conflict, "settled slice is absent"))?;
    let refs = closeout_refs(
        prefix,
        &current.binding.goal_id,
        &body.run_id,
        body.run_sequence,
    )?;
    let consistent = match body.outcome {
        GoalSliceOutcomeV1::Completed => body.completion_sequence.is_some() && body.usage.is_some(),
        GoalSliceOutcomeV1::Interrupted => body.usage.is_none(),
    };
    if slice.kind != GoalFactKindV1::SliceAdmitted.as_str()
        || slice.key
            != goal_fact_key(
                &current.binding.goal_id,
                &body.run_id,
                GoalFactKindV1::SliceAdmitted,
            )?
        || !consistent
        || refs != (body.completion_sequence, body.step_sequence)
        || crate::fact_control::has_unsettled_effects(prefix)
        || stored.goal_id != current.binding.goal_id
        || stored.goal_revision != current.binding.goal_revision
        || stored.plan_revision != current.plan_revision
        || &current.binding.workspace != workspace
        || stored.operation_id != operation_id(&body.run_id)
        || stored.command_digest != digest(current, body)?
    {
        return Err(refused(
            GoalRefusal::Conflict,
            "slice settlement differs from its original Run",
        ));
    }
    let (legacy_mutated, changes) = crate::fact_control::goal_slice_workspace_evidence(
        prefix,
        body.run_sequence,
        session_id,
        &body.run_id,
    );
    let admission = run::RunFact::deserialize(&slice.payload)
        .map_err(|_| refused(GoalRefusal::Conflict, "invalid settled slice admission"))?;
    if body
        .feedback
        .as_ref()
        .is_some_and(|feedback| feedback.step_id != admission.body.step_id)
    {
        return Err(refused(
            GoalRefusal::Conflict,
            "judgment feedback differs from its original step",
        ));
    }
    let basis = admission
        .body
        .continuation
        .as_ref()
        .and_then(|source| source.progress_basis);
    let strict = basis == Some(run::ProgressBasis::TurnProgressDeltaV3);
    let net_progress = continuation.observe_workspace_changes(changes)?;
    let mutated = if strict { net_progress } else { legacy_mutated };
    let reduction_only = admission.body.continuation.as_ref().is_some_and(|source| {
        matches!(
            source.progress_basis,
            Some(run::ProgressBasis::TurnProgressDeltaV2 | run::ProgressBasis::TurnProgressDeltaV3)
        )
    });
    // Preserve historical admissions; new slices cannot earn retries by growing debt.
    let progressed = mutated
        || (strict && body.step_sequence.is_some())
        || body.progress.is_some_and(|delta| {
            let uncertainty_progress = if reduction_only {
                delta.uncertainty_delta > 0
            } else {
                delta.uncertainty_delta != 0
            };
            delta.information_delta > 0
                || (!strict && delta.mutation_delta > 0)
                || uncertainty_progress
                || (!strict && delta.plan_delta > 0)
        });
    Ok((
        stored.operation_id,
        stored.command_digest,
        current.clone(),
        progressed,
    ))
}

/// Replayed operations retain their original input cut, excluding their own judgment.
pub(crate) fn tick_feedback(
    facts: &[Fact],
    command: &crate::goal_control::GoalControlCommandV1,
    step_id: &str,
) -> Result<Option<GoalStepFeedback>, GoalControlError> {
    let replay = run::matches_tick_run(facts, command)?;
    let cutoff = if replay {
        facts
            .iter()
            .find(|fact| {
                fact.kind == GoalFactKindV1::SliceAdmitted.as_str()
                    && fact.payload["body"]["run_id"].as_str() == Some(&command.operation_id)
            })
            .map_or(u64::MAX, |fact| fact.seq)
    } else {
        u64::MAX
    };
    for fact in facts
        .iter()
        .rev()
        .filter(|fact| fact.seq < cutoff && fact.kind == GoalFactKindV1::SliceSettled.as_str())
    {
        let stored = SettledFact::deserialize(&fact.payload)
            .map_err(|_| refused(GoalRefusal::Conflict, "invalid judgment feedback source"))?;
        if stored.session_id == command.session_id
            && stored.goal_id == command.goal_id
            && stored.goal_revision == command.expected_goal_revision
            && stored.plan_revision == command.expected_plan_revision
            && stored.work_policy_digest == command.work_policy_digest
        {
            if let Some(feedback) = stored
                .body
                .feedback
                .filter(|feedback| feedback.step_id == step_id)
            {
                return Ok(Some(feedback));
            }
        }
    }
    Ok(None)
}
