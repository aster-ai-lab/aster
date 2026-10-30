//! Information credits derived from original preparations and visible results.

use super::*;
use std::collections::{BTreeMap, BTreeSet};

pub(in crate::fact_control) fn novel_information(
    facts: &[a3s_effect::Fact],
    session_id: &str,
    run_id: &str,
) -> u64 {
    let debt = fold_cache_evidence(facts);
    if debt.integrity_invalid || !debt.unresolved.is_empty() {
        return 0;
    }
    let preparations: BTreeMap<_, _> = facts
        .iter()
        .filter(|fact| fact.kind == "tool.prepared")
        .filter_map(|fact| {
            let prepared: ToolPreparedV1 =
                serde_json::from_value(fact.payload.get("prepared")?.clone()).ok()?;
            Some((
                prepared.identity.key().ok()?,
                prepared.effective_args_digest,
            ))
        })
        .collect();
    let mut seen = BTreeSet::new();
    let mut count = 0u64;
    let mut current_capability = None;
    for projection in &debt.projection_order {
        let ReceiptProjection::Tool(index) = projection else {
            continue;
        };
        let Some((tool, result)) = debt.results.get(*index) else {
            return 0;
        };
        let Some(admission) = &result.identity.admission else {
            continue;
        };
        if admission.thread_id != session_id {
            continue;
        }
        if admission.run_id == run_id {
            if current_capability
                .is_some_and(|digest| digest != &admission.capability_binding_digest)
            {
                return 0;
            }
            current_capability = Some(&admission.capability_binding_digest);
        }
        let Some(args) = result
            .identity
            .key()
            .ok()
            .and_then(|key| preparations.get(&key))
        else {
            return 0;
        };
        let identity = match information_identity(tool, result, args) {
            Ok(Some(identity)) => identity,
            Ok(None) => continue,
            Err(_) => return 0,
        };
        if seen.insert(identity) && admission.run_id == run_id {
            count = count.saturating_add(1);
        }
    }
    count
}

// The raw observation excludes host repeat hints and caller-controlled metadata.
pub(super) fn information_identity(
    tool: &str,
    result: &ToolExecutionResultV1,
    args: &str,
) -> Result<Option<String>, ActorError> {
    if !matches!(
        result.outcome,
        ToolExecutionOutcome::Succeeded | ToolExecutionOutcome::Failed
    ) || result.effects.completeness != ToolObservationCompleteness::Complete
        || !result.effects.unresolved_external_operations.is_empty()
    {
        return Ok(None);
    }
    let Some(body) = result.output.as_str() else {
        return Ok(None);
    };
    let observation = match decode_fact_tool_result(body).and_then(|value| value.observation_digest)
    {
        Some(digest) => {
            crate::content_digest::validate_digest(&digest).map_err(|_| {
                ActorError::Schema(
                    "tool_observation_digest: invalid original observation digest".into(),
                )
            })?;
            digest
        }
        None => {
            let visible = message_from_folded_tool(
                "observation",
                body,
                result.outcome == ToolExecutionOutcome::Failed,
            );
            crate::content_digest::digest_json("a3s.code.tool-observation.v1", &visible)
                .map_err(|error| ActorError::Schema(error.to_string()))?
        }
    };
    crate::content_digest::digest_json(
        "a3s.code.tool-information.v1",
        &(tool, args, result.outcome, result.exit_code, observation),
    )
    .map(Some)
    .map_err(|error| ActorError::Schema(error.to_string()))
}
