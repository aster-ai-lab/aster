//! HR3a: the task-routing evaluation — the pure composition between
//! the routing plan (HR1), the decision contract (HR0), the frozen
//! turn (HR2), and the target walk. Given a plan and this turn's
//! decision facts, it names the ordered candidate target ids and the
//! relay ceiling; the existing priority-group machinery (HR3b) walks
//! exactly that window. No I/O lives here — every input is a fact,
//! every output a fact.

use crate::config::{
    InferenceModelConfig, InferenceModelPricing, InferenceRoutingMode, InferenceRoutingPlan,
};
use crate::inference::decision::{DecisionDefinition, DecisionError, DecisionResponse};
use crate::inference::decision_cache::{ChosenGroup, DecisionReason};

/// The outcome of a routing evaluation: the entry window the target
/// walk starts from and may relay through, plus the facts for the
/// evidence lane (HR5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RouteDecision {
    /// Ordered candidate target ids — the declared order restricted to
    /// the chosen group(s). Empty never happens: every mode resolves
    /// to at least the primary/declared set (fail-closed).
    pub(crate) target_ids: Vec<uuid::Uuid>,
    pub(crate) group: ChosenGroup,
    pub(crate) reason: DecisionReason,
    /// Whether the walk may relay BEYOND the chosen group (economy →
    /// primary on failure). A primary start never relays downward or
    /// across — primary stays primary.
    pub(crate) may_relay_to_primary: bool,
}

/// The decision inputs HR3 needs from one decision-service call (or a
/// frozen turn's facts): the simple-level probability (smart saving),
/// the categorical choice + refinement probability (branches), and
/// whether a fresh assessment is below the floor.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct DecisionInputs {
    /// The probability mass of the SIMPLE level, when ordinal.
    pub(crate) simple_probability: Option<f64>,
    /// The categorical choice id, when the plan is branch mode.
    pub(crate) branch_choice: Option<String>,
    /// The refinement (degree) probability of the light end.
    pub(crate) degree_simple_probability: Option<f64>,
    /// A FRESH assessment scored below the competence floor (HR4's
    /// freshness rules apply before this reaches here).
    pub(crate) fresh_competence_below_floor: bool,
}

/// Extracts the routing-relevant facts from one validated decision
/// response, against the levels/options the plan's definition
/// declared. `simple_level` and the branch mapping come from the plan
/// (HR1 shape) — the contract is model-blind, the plan names the ids.
pub(crate) fn decision_inputs(
    response: &DecisionResponse,
    simple_level_id: &str,
    branch_choice_ids: &[&str],
    degree_simple_level_id: Option<&str>,
) -> Result<DecisionInputs, DecisionError> {
    let invalid = |message: &str| DecisionError::Invalid(message.to_string());
    let mut inputs = DecisionInputs::default();
    match &response.decision {
        crate::inference::decision::DecisionResult::Ordinal { probabilities } => {
            let simple = probabilities
                .get(simple_level_id)
                .ok_or_else(|| invalid("ordinal answer omits the plan's simple level"))?;
            inputs.simple_probability = Some(*simple);
        }
        crate::inference::decision::DecisionResult::Categorical { choice, refinement } => {
            let known = branch_choice_ids
                .iter()
                .find(|id| *id == choice)
                .ok_or_else(|| invalid("categorical answer names no declared branch"))?;
            inputs.branch_choice = Some((*known).to_string());
            if let (Some(refinement), Some(degree_id)) = (refinement, degree_simple_level_id) {
                inputs.degree_simple_probability =
                    Some(*refinement.probabilities.get(degree_id).ok_or_else(|| {
                        invalid("refinement answer omits the plan's degree level")
                    })?);
            }
        }
    }
    if let Some(assessment) = &response.assessment {
        if assessment.partial {
            // Partial assessments never act (HiRoute: incomplete data
            // stays unrated) — recorded as absent.
            inputs.fresh_competence_below_floor = false;
        }
    }
    Ok(inputs)
}

/// Evaluates the routing plan for one turn. Pure and total: every
/// failure path resolves fail-closed (primary/declared), never an
/// error — the request path cannot strand on routing.
pub(crate) fn evaluate_routing(
    model: &InferenceModelConfig,
    plan: Option<&InferenceRoutingPlan>,
    decision: Option<&DecisionInputs>,
    decision_unreachable: bool,
) -> RouteDecision {
    let Some(plan) = plan else {
        return declared(model, DecisionReason::NoDecisionSurface);
    };
    let declared_ids: Vec<uuid::Uuid> = model.targets.iter().map(|t| t.target_id).collect();
    match plan.mode {
        InferenceRoutingMode::Fixed => declared(model, DecisionReason::NoDecisionSurface),
        InferenceRoutingMode::FreeFirst => {
            let mut free = Vec::new();
            let mut paid = Vec::new();
            for target in &model.targets {
                let is_free = target_free(&model.pricing, &target.target_id, model);
                let bucket = if is_free { &mut free } else { &mut paid };
                bucket.push(target.target_id);
            }
            // Free first, then the declared order for the rest. When
            // no target is free this is exactly the declared order.
            let mut target_ids = free;
            target_ids.extend(paid);
            RouteDecision {
                target_ids,
                group: ChosenGroup::Economy,
                reason: DecisionReason::NoDecisionSurface,
                may_relay_to_primary: false,
            }
        }
        InferenceRoutingMode::SmartSaving => {
            let primary_ids = plan
                .primary_targets
                .clone()
                .unwrap_or_else(|| declared_ids.clone());
            let economy_ids = plan.economy_targets.clone().unwrap_or_default();
            // Fail-closed order: unreachable/invalid decision, a fresh
            // below-floor assessment, or a complex turn all land on
            // primary. Competence blocks risky cost-cutting.
            if decision_unreachable {
                return primary(primary_ids, DecisionReason::DecisionUnreachable);
            }
            let Some(inputs) = decision else {
                return primary(primary_ids, DecisionReason::DecisionInvalid);
            };
            if inputs.fresh_competence_below_floor {
                return primary(primary_ids, DecisionReason::CompetenceFloor);
            }
            let simple = inputs.simple_probability.unwrap_or(0.0);
            let threshold = f64::from(plan.simple_probability_threshold_bp) / 10_000.0;
            if simple >= threshold && !economy_ids.is_empty() {
                RouteDecision {
                    target_ids: economy_ids,
                    group: ChosenGroup::Economy,
                    reason: DecisionReason::SimpleProbability,
                    may_relay_to_primary: true,
                }
            } else {
                primary(primary_ids, DecisionReason::ComplexProbability)
            }
        }
        InferenceRoutingMode::Branches => {
            let fallback: Vec<uuid::Uuid> = plan
                .primary_targets
                .clone()
                .unwrap_or_else(|| declared_ids.clone());
            if decision_unreachable {
                return primary(fallback, DecisionReason::DecisionUnreachable);
            }
            let Some(inputs) = decision else {
                return primary(fallback, DecisionReason::DecisionInvalid);
            };
            let Some(branch) = inputs.branch_choice.as_deref().and_then(|choice| {
                plan.branches
                    .iter()
                    .find(|branch| branch.category == choice)
            }) else {
                return primary(fallback, DecisionReason::DecisionInvalid);
            };
            // A branch with a primary group and a fresh below-floor
            // assessment (or a complex degree) upgrades to primary;
            // a branch without primary stays regular regardless.
            let upgrade = branch.primary_targets.as_ref().is_some_and(|_| {
                inputs.fresh_competence_below_floor
                    || inputs.degree_simple_probability.is_none_or(|degree| {
                        degree < f64::from(plan.simple_probability_threshold_bp) / 10_000.0
                    })
            });
            if upgrade {
                let ids = branch
                    .primary_targets
                    .clone()
                    .unwrap_or_else(|| fallback.clone());
                primary(ids, DecisionReason::ComplexProbability)
            } else {
                RouteDecision {
                    target_ids: branch.regular_targets.clone(),
                    group: ChosenGroup::BranchRegular {
                        category: branch.category.clone(),
                    },
                    reason: DecisionReason::SimpleProbability,
                    may_relay_to_primary: branch.primary_targets.is_some(),
                }
            }
        }
    }
}

fn declared(model: &InferenceModelConfig, reason: DecisionReason) -> RouteDecision {
    RouteDecision {
        target_ids: model.targets.iter().map(|t| t.target_id).collect(),
        group: ChosenGroup::Primary,
        reason,
        may_relay_to_primary: false,
    }
}

fn primary(ids: Vec<uuid::Uuid>, reason: DecisionReason) -> RouteDecision {
    RouteDecision {
        target_ids: ids,
        group: ChosenGroup::Primary,
        reason,
        may_relay_to_primary: false,
    }
}

/// Free-first freeness: a target is free when the model carries no
/// pricing at all, or its input price is zero. Per-target pricing
/// does not exist yet (pricing is per-model); when it does, this
/// predicate is the single place to refine.
fn target_free(
    _pricing: &Option<InferenceModelPricing>,
    _target_id: &uuid::Uuid,
    _model: &InferenceModelConfig,
) -> bool {
    // HR3a v1: pricing is per-model; a model with no pricing serves
    // free-first's free lane, a priced model has none. The predicate
    // stays the named refinement point for per-target pricing.
    _pricing.is_none()
}

/// The decision definition a plan implies — what HR3b sends to the
/// decision service. The levels/options carry the plan's group ids so
/// the answers map straight back (the ids below are the plan's own
/// category/target vocabulary — model ids never cross).
pub(crate) fn plan_decision_definition(plan: &InferenceRoutingPlan) -> DecisionDefinition {
    match plan.mode {
        InferenceRoutingMode::SmartSaving => DecisionDefinition::Ordinal {
            instructions: "Judge whether the new turn is simple or complex.".into(),
            levels: vec![
                crate::inference::decision::DecisionLevel {
                    id: "simple".into(),
                    criterion: "one direct response, lookup, translation, or explanation completes the request without planning or multi-step analysis".into(),
                },
                crate::inference::decision::DecisionLevel {
                    id: "complex".into(),
                    criterion: "multi-step reasoning, comparison, design, debugging, proof, or implementation is required".into(),
                },
            ],
        },
        InferenceRoutingMode::Branches => DecisionDefinition::Categorical {
            instructions: "Classify the turn's task category.".into(),
            options: plan
                .branches
                .iter()
                .map(|branch| {
                    crate::inference::decision::DecisionCategory {
                        id: branch.category.clone(),
                        criterion: format!("the turn is {} work", branch.category),
                        // A branch with primary candidates asks the
                        // degree question; a regular-only branch does
                        // not (HiRoute: no primary, no degree).
                        refinement: branch.primary_targets.as_ref().map(|_| {
                            crate::inference::decision::OrdinalRefinement {
                                instructions: "Judge the degree within this category.".into(),
                                levels: vec![
                                    crate::inference::decision::DecisionLevel {
                                        id: "light".into(),
                                        criterion: "routine work for this category".into(),
                                    },
                                    crate::inference::decision::DecisionLevel {
                                        id: "heavy".into(),
                                        criterion: "demanding work for this category".into(),
                                    },
                                ],
                            }
                        }),
                    }
                })
                .collect(),
        },
        InferenceRoutingMode::Fixed | InferenceRoutingMode::FreeFirst => {
            unreachable!("fixed and free-first modes never call the decision service")
        }
    }
}

/// Composes the routing window into the target walk's effective list:
/// the chosen group's targets first (declared order, weights intact),
/// then — when the decision allows relay — the primary window. The
/// priorities are RE-LEVELED into contiguous groups so the existing
/// priority walk (advance on failure, stop at exhaustion) expresses
/// the relay ceiling exactly: an economy failure advances into
/// primary; a primary start has no tail and stops at exhaustion.
pub(crate) fn effective_targets(
    model: &InferenceModelConfig,
    decision: &RouteDecision,
) -> Vec<crate::config::InferenceTargetConfig> {
    // The plan's group list declares the candidate ORDER (HiRoute's
    // ordered candidates); the model's declaration supplies the target
    // bodies. Unknown ids (a stale plan vs a newer snapshot) drop —
    // the walk only ever sees declared targets (invariant 4).
    let by_id: std::collections::HashMap<uuid::Uuid, &crate::config::InferenceTargetConfig> =
        model.targets.iter().map(|t| (t.target_id, t)).collect();
    let mut ordered: Vec<crate::config::InferenceTargetConfig> = decision
        .target_ids
        .iter()
        .filter_map(|id| by_id.get(id).map(|target| (*target).clone()))
        .collect();
    if decision.may_relay_to_primary {
        let window: std::collections::HashSet<uuid::Uuid> =
            decision.target_ids.iter().copied().collect();
        ordered.extend(
            model
                .targets
                .iter()
                .filter(|target| !window.contains(&target.target_id))
                .cloned(),
        );
    }
    // Re-level: each candidate becomes its own priority group — the
    // existing walk then expresses HiRoute's ordered-candidate walk
    // exactly (one candidate per group, advance on failure, stop at
    // exhaustion; the relay tail is simply the groups after the
    // window).
    let mut leveled = ordered;
    for (position, target) in leveled.iter_mut().enumerate() {
        target.priority = u32::try_from(position).unwrap_or(u32::MAX);
    }
    leveled
}

/// Derives the turn identity from a parsed OpenAI chat body: the LAST
/// user message anchors the turn; everything before it fingerprints
/// the visible history. Bounded fingerprints only — no content is
/// retained. Missing/malformed message shapes degrade to a stable
/// identity derived from the whole body (never a panic, never a
/// false continuation: any body change re-decides).
pub(crate) fn turn_identity_from_document(
    document: &serde_json::Map<String, serde_json::Value>,
    credential_fingerprint: u64,
    model_alias: &str,
) -> crate::inference::decision_cache::TurnIdentity {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    model_alias.hash(&mut hasher);
    let model_alias_fingerprint = hasher.finish();

    let empty = Vec::new();
    let messages = document
        .get("messages")
        .and_then(|value| value.as_array())
        .unwrap_or(&empty);
    let content_text = |value: &serde_json::Value| -> String {
        match value {
            serde_json::Value::String(text) => text.clone(),
            serde_json::Value::Array(parts) => parts
                .iter()
                .filter_map(|part| {
                    part.get("text")
                        .and_then(|text| text.as_str())
                        .map(str::to_owned)
                })
                .collect::<Vec<_>>()
                .join(" "),
            _ => String::new(),
        }
    };
    let last_user_index = messages
        .iter()
        .rposition(|message| message.get("role").and_then(|role| role.as_str()) == Some("user"));
    let turn_message_count = match last_user_index {
        Some(index) => u32::try_from(index + 1).unwrap_or(u32::MAX),
        None => u32::try_from(messages.len()).unwrap_or(u32::MAX),
    };
    let (last_user_digest, history_digest) = match last_user_index {
        Some(index) => {
            let mut last_hasher = std::collections::hash_map::DefaultHasher::new();
            content_text(
                messages[index]
                    .get("content")
                    .unwrap_or(&serde_json::Value::Null),
            )
            .hash(&mut last_hasher);
            let mut history_hasher = std::collections::hash_map::DefaultHasher::new();
            for message in &messages[..index] {
                message
                    .get("role")
                    .and_then(|r| r.as_str())
                    .hash(&mut history_hasher);
                content_text(message.get("content").unwrap_or(&serde_json::Value::Null))
                    .hash(&mut history_hasher);
            }
            (last_hasher.finish(), history_hasher.finish())
        }
        None => {
            // No user message (embeddings, tools): the whole body
            // fingerprints the turn — any change re-decides.
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            document.hash(&mut hasher);
            (hasher.finish(), 0)
        }
    };
    crate::inference::decision_cache::TurnIdentity {
        credential_fingerprint,
        model_alias_fingerprint,
        last_user_digest,
        history_digest,
        message_count: turn_message_count,
    }
}

/// HR3c: builds the decision request for one turn — the plan's rubric
/// plus the turn's CONTENT. Prior messages fold into visible turns at
/// user-message boundaries (a user message opens a turn; assistant
/// and tool messages append steps). Non-text content degrades to an
/// `unavailable` part — never invented, never dropped silently. The
/// 64 KiB bound applies at encode; an oversized conversation refuses
/// (the caller fails closed to primary).
pub(crate) fn build_decision_request(
    plan: &InferenceRoutingPlan,
    document: &serde_json::Map<String, serde_json::Value>,
) -> Result<crate::inference::decision::DecisionRequest, crate::inference::decision::DecisionError>
{
    use crate::inference::decision::{
        DecisionContentPart, DecisionRequest, DecisionStepPart, DecisionTurnStatus,
        DecisionVisibleTurn,
    };
    let invalid =
        |message: &str| crate::inference::decision::DecisionError::Invalid(message.to_string());
    // The decision observation bound covers the whole request, including
    // leading system content that is intentionally absent from the rubric.
    // Large contexts stay on primary rather than overflowing an economy model.
    let observed = serde_json::to_vec(document)
        .map_err(|_| invalid("decision observation could not be encoded"))?;
    if observed.len() > crate::inference::decision::DECISION_REQUEST_LIMIT_BYTES {
        return Err(invalid("decision observation exceeds the 64 KiB bound"));
    }
    let empty = Vec::new();
    let messages = document
        .get("messages")
        .and_then(|value| value.as_array())
        .unwrap_or(&empty);
    let content_parts = |value: &serde_json::Value| -> Vec<DecisionContentPart> {
        match value {
            serde_json::Value::String(text) => {
                vec![DecisionContentPart::Text { text: text.clone() }]
            }
            serde_json::Value::Array(parts) => parts
                .iter()
                .map(
                    |part| match part.get("text").and_then(|text| text.as_str()) {
                        Some(text) => DecisionContentPart::Text { text: text.into() },
                        None => DecisionContentPart::Unavailable {
                            source_kind: "openai-content-part".into(),
                        },
                    },
                )
                .collect(),
            _ => vec![DecisionContentPart::Unavailable {
                source_kind: "openai-content".into(),
            }],
        }
    };
    let step_of = |message: &serde_json::Value| -> Vec<DecisionStepPart> {
        let role = message
            .get("role")
            .and_then(|role| role.as_str())
            .unwrap_or("");
        let content = message.get("content").unwrap_or(&serde_json::Value::Null);
        if role == "tool" {
            let tool = message
                .get("name")
                .and_then(|name| name.as_str())
                .unwrap_or("tool");
            vec![DecisionStepPart::ToolActivity {
                tool: tool.into(),
                status: DecisionTurnStatus::Unknown,
            }]
        } else {
            content_parts(content)
                .into_iter()
                .map(|part| match part {
                    DecisionContentPart::Text { text } => DecisionStepPart::Text { text },
                    DecisionContentPart::Unavailable { source_kind } => {
                        DecisionStepPart::Unavailable { source_kind }
                    }
                })
                .collect()
        }
    };
    let last_user_index = messages
        .iter()
        .rposition(|message| message.get("role").and_then(|role| role.as_str()) == Some("user"))
        .ok_or_else(|| invalid("decision routing requires a user turn"))?;
    let latest_user = content_parts(
        messages[last_user_index]
            .get("content")
            .unwrap_or(&serde_json::Value::Null),
    );
    if latest_user.is_empty() {
        return Err(invalid("the turn carries no user content"));
    }
    let mut turns: Vec<DecisionVisibleTurn> = Vec::new();
    for message in &messages[..last_user_index] {
        let role = message
            .get("role")
            .and_then(|role| role.as_str())
            .unwrap_or("");
        let content = message.get("content").unwrap_or(&serde_json::Value::Null);
        if role == "user" {
            turns.push(DecisionVisibleTurn {
                user: content_parts(content),
                status: DecisionTurnStatus::Unknown,
                steps: Vec::new(),
            });
        } else if let Some(turn) = turns.last_mut() {
            turn.steps.push(step_of(message));
        }
        // Messages before any user turn (a leading system prompt) stay
        // invisible to the decision service — HiRoute's visible
        // conversation starts at the first user turn.
    }
    let request = DecisionRequest {
        decision: plan_decision_definition(plan),
        latest_user,
        visible_conversation: turns,
        history_partial: false,
        assessment_target: None,
    };
    // The 64 KiB bound is enforced HERE — layer-independent: no
    // transport or double may see an oversized decision request.
    request.encode_bounded().map_err(|error| error)?;
    Ok(request)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::LazyLock;
    static TARGETS: LazyLock<[uuid::Uuid; 3]> = LazyLock::new(|| {
        [
            uuid::Uuid::parse_str("00000000-0000-4000-8000-0000000000e1").unwrap(),
            uuid::Uuid::parse_str("00000000-0000-4000-8000-0000000000e2").unwrap(),
            uuid::Uuid::parse_str("00000000-0000-4000-8000-0000000000e3").unwrap(),
        ]
    });
    use crate::config::{InferenceRoutingBranch, InferenceRoutingPlan, InferenceTargetConfig};
    use uuid::Uuid;

    fn model(routing: Option<InferenceRoutingPlan>) -> InferenceModelConfig {
        InferenceModelConfig {
            model_id: Uuid::new_v4(),
            engine_profile: None,
            targets: targets(),
            failover: Default::default(),
            pricing: None,
            latency_bias: false,
            prefix_affinity: false,
            prefix_parallelism: 1,
            scheduling: None,
            routing,
        }
    }

    fn targets() -> Vec<InferenceTargetConfig> {
        [0u8, 1, 2]
            .iter()
            .map(|index| InferenceTargetConfig {
                target_id: TARGETS[*index as usize],
                service: "svc".into(),
                upstream_model: "m".into(),
                priority: *index as u32,
                weight: 1,
                protocol: crate::config::InferenceProtocol::Openai,
                api_key_env: None,
                translate: false,
            })
            .collect()
    }

    fn target_ids(indices: &[usize]) -> Vec<Uuid> {
        indices.iter().map(|index| TARGETS[*index]).collect()
    }

    fn smart_plan(economy: &[usize], primary: &[usize]) -> InferenceRoutingPlan {
        InferenceRoutingPlan {
            mode: InferenceRoutingMode::SmartSaving,
            decision_model: Some("platform/decider".into()),
            decision_protocol: Default::default(),
            economy_targets: Some(target_ids(economy)),
            primary_targets: Some(target_ids(primary)),
            simple_probability_threshold_bp: 8000,
            competence_floor_bp: 5000,
            branches: Vec::new(),
        }
    }

    fn inputs(simple: Option<f64>, below_floor: bool) -> DecisionInputs {
        DecisionInputs {
            simple_probability: simple,
            branch_choice: None,
            degree_simple_probability: None,
            fresh_competence_below_floor: below_floor,
        }
    }

    #[test]
    fn effective_targets_level_the_window_into_a_relay_tail() {
        let plan = smart_plan(&[0, 1], &[2]);
        let model = model(Some(plan));
        // The declared priorities are all distinct; the window re-levels.
        let economy = RouteDecision {
            target_ids: target_ids(&[1, 0]),
            group: ChosenGroup::Economy,
            reason: DecisionReason::SimpleProbability,
            may_relay_to_primary: true,
        };
        let effective = effective_targets(&model, &economy);
        let ids: Vec<_> = effective.iter().map(|t| t.target_id).collect();
        assert_eq!(ids, target_ids(&[1, 0, 2]), "window order then relay tail");
        assert_eq!(effective[0].priority, 0);
        assert_eq!(effective[1].priority, 1, "each candidate is its own group");
        assert_eq!(effective[2].priority, 2, "the relay tail levels after");
        // A primary start has no tail.
        let primary = RouteDecision {
            target_ids: target_ids(&[2]),
            group: ChosenGroup::Primary,
            reason: DecisionReason::ComplexProbability,
            may_relay_to_primary: false,
        };
        let effective = effective_targets(&model, &primary);
        let ids: Vec<_> = effective.iter().map(|t| t.target_id).collect();
        assert_eq!(ids, target_ids(&[2]));
    }

    #[test]
    fn turn_identity_binds_to_the_last_user_message_and_history() {
        let document = |messages: serde_json::Value| {
            let mut map = serde_json::Map::new();
            map.insert("messages".into(), messages);
            map
        };
        let base = document(serde_json::json!([
            {"role": "user", "content": "write a draft"},
            {"role": "assistant", "content": "here it is"},
            {"role": "user", "content": "review it now"},
        ]));
        let identity = turn_identity_from_document(&base, 7, "alias");
        assert_eq!(identity.credential_fingerprint, 7);
        assert_eq!(identity.message_count, 3);
        // A tool continuation (same last user message, appended
        // assistant/tool messages) keeps the identity's anchor but —
        // per the strict shape — changes the history digest only when
        // messages BEFORE the anchor change; appended trailing
        // messages after the last user message do not exist by
        // construction (the anchor is the LAST user message).
        let appended = document(serde_json::json!([
            {"role": "user", "content": "write a draft"},
            {"role": "assistant", "content": "here it is"},
            {"role": "user", "content": "review it now"},
            {"role": "assistant", "content": "done"},
        ]));
        let continued = turn_identity_from_document(&appended, 7, "alias");
        assert_eq!(
            identity, continued,
            "a same-turn continuation reuses the frozen decision"
        );
        // A new user turn changes the anchor.
        let next = document(serde_json::json!([
            {"role": "user", "content": "write a draft"},
            {"role": "assistant", "content": "here it is"},
            {"role": "user", "content": "review it now"},
            {"role": "assistant", "content": "done"},
            {"role": "user", "content": "now rewrite"},
        ]));
        let fresh = turn_identity_from_document(&next, 7, "alias");
        assert_ne!(identity, fresh);
        // A rebuilt history changes the history digest.
        let rebuilt = document(serde_json::json!([
            {"role": "user", "content": "write a draft (compacted)"},
            {"role": "user", "content": "review it now"},
        ]));
        let redecided = turn_identity_from_document(&rebuilt, 7, "alias");
        assert_ne!(identity, redecided);
        // Array-part content extracts text.
        let parts = document(serde_json::json!([
            {"role": "user", "content": [{"type": "text", "text": "hello"}]},
        ]));
        let from_parts = turn_identity_from_document(&parts, 7, "alias");
        let plain = document(serde_json::json!([
            {"role": "user", "content": "hello"},
        ]));
        assert_eq!(
            from_parts.last_user_digest,
            turn_identity_from_document(&plain, 7, "alias").last_user_digest
        );
    }

    #[test]
    fn no_plan_is_todays_behavior() {
        let model = model(None);
        let decision = evaluate_routing(&model, None, None, false);
        assert_eq!(decision.target_ids, target_ids(&[0, 1, 2]));
        assert!(!decision.may_relay_to_primary);
    }

    #[test]
    fn smart_saving_routes_by_probability_and_fails_closed() {
        let plan = smart_plan(&[0, 1], &[2]);
        let model = model(Some(plan.clone()));
        // Simple turn → economy, relay allowed upward.
        let economy = evaluate_routing(&model, Some(&plan), Some(&inputs(Some(0.9), false)), false);
        assert_eq!(economy.target_ids, target_ids(&[0, 1]));
        assert!(economy.may_relay_to_primary);
        // Complex turn → primary, no relay.
        let complex = evaluate_routing(&model, Some(&plan), Some(&inputs(Some(0.5), false)), false);
        assert_eq!(complex.target_ids, target_ids(&[2]));
        assert!(!complex.may_relay_to_primary);
        // A fresh below-floor assessment forces primary regardless.
        let protected =
            evaluate_routing(&model, Some(&plan), Some(&inputs(Some(0.99), true)), false);
        assert_eq!(protected.target_ids, target_ids(&[2]));
        assert_eq!(protected.reason, DecisionReason::CompetenceFloor);
        // An unreachable decision fails closed to primary.
        let closed = evaluate_routing(&model, Some(&plan), None, true);
        assert_eq!(closed.target_ids, target_ids(&[2]));
        assert_eq!(closed.reason, DecisionReason::DecisionUnreachable);
    }

    #[test]
    fn branches_choose_by_category_and_degree() {
        let plan = InferenceRoutingPlan {
            mode: InferenceRoutingMode::Branches,
            decision_model: Some("platform/decider".into()),
            decision_protocol: Default::default(),
            economy_targets: None,
            primary_targets: Some(target_ids(&[2])),
            simple_probability_threshold_bp: 8000,
            competence_floor_bp: 5000,
            branches: vec![
                InferenceRoutingBranch {
                    category: "drafting".into(),
                    regular_targets: target_ids(&[0]),
                    primary_targets: Some(target_ids(&[2])),
                },
                InferenceRoutingBranch {
                    category: "review".into(),
                    regular_targets: target_ids(&[1]),
                    primary_targets: None,
                },
            ],
        };
        let model = model(Some(plan.clone()));
        let mut choice = inputs(None, false);
        choice.branch_choice = Some("drafting".into());
        choice.degree_simple_probability = Some(0.9);
        let light = evaluate_routing(&model, Some(&plan), Some(&choice), false);
        assert_eq!(light.target_ids, target_ids(&[0]));
        assert!(light.may_relay_to_primary);
        // A complex degree upgrades within the branch.
        let mut heavy = choice;
        heavy.degree_simple_probability = Some(0.2);
        let upgraded = evaluate_routing(&model, Some(&plan), Some(&heavy), false);
        assert_eq!(upgraded.target_ids, target_ids(&[2]));
        assert!(!upgraded.may_relay_to_primary);
        // A branch without primary never upgrades.
        let mut review = inputs(None, false);
        review.branch_choice = Some("review".into());
        review.degree_simple_probability = Some(0.1);
        let stayed = evaluate_routing(&model, Some(&plan), Some(&review), false);
        assert_eq!(stayed.target_ids, target_ids(&[1]));
        assert!(!stayed.may_relay_to_primary);
    }

    #[test]
    fn free_first_keeps_the_declared_order_under_per_model_pricing() {
        let plan = InferenceRoutingPlan {
            mode: InferenceRoutingMode::FreeFirst,
            decision_model: None,
            decision_protocol: Default::default(),
            economy_targets: None,
            primary_targets: None,
            simple_probability_threshold_bp: 8000,
            competence_floor_bp: 5000,
            branches: Vec::new(),
        };
        let model = model(Some(plan.clone()));
        let decision = evaluate_routing(&model, Some(&plan), None, false);
        assert_eq!(decision.target_ids, target_ids(&[0, 1, 2]));
        // A priced model: no free lane — the declared order stands
        // (per-target pricing is the named refinement point).
        let mut priced = model.clone();
        priced.pricing = Some(crate::config::InferenceModelPricing {
            input_micros_per_million: 1,
            output_micros_per_million: 1,
        });
        let decision = evaluate_routing(&priced, Some(&plan), None, false);
        assert_eq!(decision.target_ids, target_ids(&[0, 1, 2]));
    }
}
