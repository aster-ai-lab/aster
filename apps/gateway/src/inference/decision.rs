//! HR0: the task-routing decision contract — the HiRoute decision API
//! subset this gateway consumes. The wire is strict (unknown fields
//! refuse), the request is bounded (64 KiB), and the boundary is
//! model-blind: model ids and routing thresholds never cross it.
//!
//! Content flows ONE way — the gateway sends turn content to the configured
//! decision implementation and persists nothing: no prompt bytes from a
//! decision call enter the spool, logs, or metrics (invariant 7 applies
//! verbatim).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The request bound the HiRoute contract enforces.
pub(crate) const DECISION_REQUEST_LIMIT_BYTES: usize = 64 * 1024;
/// The probabilities must sum to within this of 1.
const PROBABILITY_SUM_TOLERANCE: f64 = 1e-6;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum DecisionContentPart {
    Text { text: String },
    Unavailable { source_kind: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum DecisionTurnStatus {
    Completed,
    Failed,
    Interrupted,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum DecisionStepPart {
    Text {
        text: String,
    },
    Unavailable {
        source_kind: String,
    },
    ToolActivity {
        tool: String,
        status: DecisionTurnStatus,
    },
}

impl From<&DecisionContentPart> for DecisionStepPart {
    fn from(part: &DecisionContentPart) -> Self {
        match part {
            DecisionContentPart::Text { text } => DecisionStepPart::Text { text: text.clone() },
            DecisionContentPart::Unavailable { source_kind } => DecisionStepPart::Unavailable {
                source_kind: source_kind.clone(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DecisionVisibleTurn {
    pub(crate) user: Vec<DecisionContentPart>,
    pub(crate) status: DecisionTurnStatus,
    /// The turn's executed steps, each step a parallel part list.
    pub(crate) steps: Vec<Vec<DecisionStepPart>>,
}

/// An anchored rubric criterion — exactly the scores 0, 0.5, and 1
/// appear once each across a target's criteria.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DecisionCriterion {
    pub(crate) score: f64,
    pub(crate) criterion: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DecisionAssessmentTarget {
    /// The visible-conversation turn index the assessment scores.
    pub(crate) from: u64,
    pub(crate) instructions: String,
    pub(crate) criteria: Vec<DecisionCriterion>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum DecisionDefinition {
    /// Ordered difficulty levels (2–16) with per-level criteria.
    Ordinal {
        instructions: String,
        levels: Vec<DecisionLevel>,
    },
    /// Task categories (2–16) with optional per-category refinement.
    Categorical {
        instructions: String,
        options: Vec<DecisionCategory>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DecisionLevel {
    pub(crate) id: String,
    pub(crate) criterion: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DecisionCategory {
    pub(crate) id: String,
    pub(crate) criterion: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) refinement: Option<OrdinalRefinement>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OrdinalRefinement {
    pub(crate) instructions: String,
    pub(crate) levels: Vec<DecisionLevel>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DecisionRequest {
    pub(crate) decision: DecisionDefinition,
    /// The new user turn's content (at least one part).
    pub(crate) latest_user: Vec<DecisionContentPart>,
    pub(crate) visible_conversation: Vec<DecisionVisibleTurn>,
    pub(crate) history_partial: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) assessment_target: Option<DecisionAssessmentTarget>,
}

impl DecisionRequest {
    /// Serializes for the wire and enforces the contract's size bound
    /// — an oversized turn refuses before any bytes leave.
    pub(crate) fn encode_bounded(&self) -> Result<Vec<u8>, DecisionError> {
        self.validate()?;
        let body = serde_json::to_vec(self).map_err(|error| {
            DecisionError::Invalid(format!("decision request could not be encoded: {error}"))
        })?;
        if body.len() > DECISION_REQUEST_LIMIT_BYTES {
            return Err(DecisionError::Invalid(format!(
                "decision request exceeds the {} byte bound",
                DECISION_REQUEST_LIMIT_BYTES
            )));
        }
        Ok(body)
    }

    /// Fail-closed structural validation (the shapes serde cannot
    /// check: cardinalities, criterion anchoring, non-empty parts).
    pub(crate) fn validate(&self) -> Result<(), DecisionError> {
        let invalid = |message: &str| DecisionError::Invalid(message.to_string());
        if self.latest_user.is_empty() {
            return Err(invalid("decision request carries no user content"));
        }
        match &self.decision {
            DecisionDefinition::Ordinal { levels, .. } => {
                if !(2..=16).contains(&levels.len()) {
                    return Err(invalid("ordinal definition needs 2 to 16 levels"));
                }
            }
            DecisionDefinition::Categorical { options, .. } => {
                if !(2..=16).contains(&options.len()) {
                    return Err(invalid("categorical definition needs 2 to 16 options"));
                }
            }
        }
        if let Some(target) = &self.assessment_target {
            if target.criteria.len() != 3 {
                return Err(invalid("assessment target needs exactly three criteria"));
            }
            let mut scores = vec![false, false, false];
            for criterion in &target.criteria {
                let index = match criterion.score {
                    score if score == 0.0 => 0,
                    score if score == 0.5 => 1,
                    score if score == 1.0 => 2,
                    _ => return Err(invalid("assessment criteria anchor on 0, 0.5, and 1")),
                };
                if scores[index] {
                    return Err(invalid("assessment criteria anchor each score once"));
                }
                scores[index] = true;
            }
            if !scores.into_iter().all(|anchored| anchored) {
                return Err(invalid("assessment criteria anchor on 0, 0.5, and 1"));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DecisionAssessment {
    pub(crate) score: f64,
    pub(crate) partial: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum DecisionResult {
    Ordinal {
        /// Probabilities over exactly the requested level ids.
        probabilities: BTreeMap<String, f64>,
    },
    Categorical {
        choice: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        refinement: Option<OrdinalResult>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OrdinalResult {
    pub(crate) probabilities: BTreeMap<String, f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DecisionResponse {
    pub(crate) decision: DecisionResult,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) assessment: Option<DecisionAssessment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DecisionError {
    Invalid(String),
    /// The transport failed (timeout, connection, non-200). The
    /// composition layer fails CLOSED to the primary group on this.
    Unreachable(String),
}

impl std::fmt::Display for DecisionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(message) => write!(formatter, "decision contract violation: {message}"),
            Self::Unreachable(message) => {
                write!(formatter, "decision service unreachable: {message}")
            }
        }
    }
}

impl DecisionResponse {
    /// Strict decode + full validation against the REQUEST the answer
    /// answers: only requested ids may appear, probabilities live in
    /// [0,1] and sum to 1 within tolerance, and a categorical choice
    /// must name a requested option. Invalid degree assessments fall
    /// back per the HiRoute contract — handled by the caller
    /// (`HR3`/`HR4`), this type only refuses structural violations.
    pub(crate) fn decode_validated(
        bytes: &[u8],
        request: &DecisionRequest,
    ) -> Result<Self, DecisionError> {
        let invalid = |message: &str| DecisionError::Invalid(message.to_string());
        let response: Self =
            serde_json::from_slice(bytes).map_err(|error| invalid(&error.to_string()))?;
        match (&response.decision, &request.decision) {
            (
                DecisionResult::Ordinal { probabilities },
                DecisionDefinition::Ordinal { levels, .. },
            ) => {
                validate_ordinal(
                    probabilities,
                    &levels
                        .iter()
                        .map(|level| level.id.clone())
                        .collect::<Vec<_>>(),
                )?;
            }
            (
                DecisionResult::Categorical { choice, refinement },
                DecisionDefinition::Categorical { options, .. },
            ) => {
                if choice.is_empty() || choice.len() > 128 {
                    return Err(invalid("categorical choice length is invalid"));
                }
                if !options.iter().any(|option| &option.id == choice) {
                    return Err(invalid("categorical choice names no requested option"));
                }
                let option = options.iter().find(|option| &option.id == choice);
                if let (Some(refinement), Some(option)) = (refinement, option) {
                    if let Some(levels) = option.refinement.as_ref() {
                        validate_ordinal(
                            &refinement.probabilities,
                            &levels
                                .levels
                                .iter()
                                .map(|level| level.id.clone())
                                .collect::<Vec<_>>(),
                        )?;
                    } else {
                        return Err(invalid(
                            "categorical refinement answers an undefined degree",
                        ));
                    }
                }
            }
            _ => {
                return Err(invalid(
                    "decision result does not answer its request's shape",
                ))
            }
        }
        if let Some(assessment) = &response.assessment {
            if !(0.0..=1.0).contains(&assessment.score) {
                return Err(invalid("assessment score is out of range"));
            }
        }
        Ok(response)
    }
}

fn validate_ordinal(
    probabilities: &BTreeMap<String, f64>,
    level_ids: &[String],
) -> Result<(), DecisionError> {
    let invalid = |message: &str| DecisionError::Invalid(message.to_string());
    if probabilities.len() < 2 {
        return Err(invalid("ordinal result needs at least two levels"));
    }
    let mut sum = 0.0f64;
    for (id, probability) in probabilities {
        if !level_ids.contains(id) {
            return Err(invalid("ordinal result names an unrequested level"));
        }
        if !(0.0..=1.0).contains(probability) {
            return Err(invalid("ordinal probability is out of range"));
        }
        sum += *probability;
    }
    if (sum - 1.0).abs() > PROBABILITY_SUM_TOLERANCE {
        return Err(invalid("ordinal probabilities do not sum to one"));
    }
    Ok(())
}

/// HR3b: the smart-saving fact extraction — the simple level's
/// probability from an ordinal answer.
pub(crate) fn from_ordinal_simple(
    response: &DecisionResponse,
    simple_level_id: &str,
    floor_bp: u32,
) -> Result<crate::inference::routing::DecisionInputs, DecisionError> {
    let DecisionResult::Ordinal { probabilities } = &response.decision else {
        return Err(DecisionError::Invalid(
            "smart saving expected an ordinal answer".into(),
        ));
    };
    let simple = probabilities
        .get(simple_level_id)
        .copied()
        .ok_or_else(|| DecisionError::Invalid("ordinal answer omits the simple level".into()))?;
    let below_floor = response
        .assessment
        .as_ref()
        .is_some_and(|assessment| !assessment.partial && assessment.score < 0.5);
    Ok(crate::inference::routing::DecisionInputs {
        simple_probability: Some(simple),
        branch_choice: None,
        degree_simple_probability: None,
        fresh_competence_below_floor: below_floor,
    })
}

/// HR3b: the branch fact extraction — the categorical choice and the
/// light degree's probability from a refinement, when present.
pub(crate) fn from_categorical(
    response: &DecisionResponse,
    branch_choice_ids: &[&str],
    light_level_id: &str,
    floor_bp: u32,
) -> Result<crate::inference::routing::DecisionInputs, DecisionError> {
    let DecisionResult::Categorical { choice, refinement } = &response.decision else {
        return Err(DecisionError::Invalid(
            "branch routing expected a categorical answer".into(),
        ));
    };
    if !branch_choice_ids.contains(&choice.as_str()) {
        return Err(DecisionError::Invalid(
            "categorical answer names no declared branch".into(),
        ));
    }
    let degree = refinement
        .as_ref()
        .and_then(|refined| refined.probabilities.get(light_level_id).copied());
    let below_floor = response
        .assessment
        .as_ref()
        .is_some_and(|assessment| !assessment.partial && assessment.score < 0.5);
    Ok(crate::inference::routing::DecisionInputs {
        simple_probability: None,
        branch_choice: Some(choice.clone()),
        degree_simple_probability: degree,
        fresh_competence_below_floor: below_floor,
    })
}

/// The decision-service port. Content crossing this port is never persisted
/// by the gateway, whether the implementation is local or remote.
#[async_trait::async_trait]
pub(crate) trait DecisionService: Send + Sync {
    /// One decision call for one turn. Implementations bound their
    /// own timeout; a retry that could duplicate a turn decision is
    /// the implementor's refusal.
    async fn decide(&self, request: &DecisionRequest) -> Result<DecisionResponse, DecisionError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ordinal_request() -> DecisionRequest {
        DecisionRequest {
            decision: DecisionDefinition::Ordinal {
                instructions: "Judge the turn's difficulty.".into(),
                levels: vec![
                    DecisionLevel {
                        id: "simple".into(),
                        criterion: "a single direct step answers it".into(),
                    },
                    DecisionLevel {
                        id: "complex".into(),
                        criterion: "multi-step reasoning is required".into(),
                    },
                ],
            },
            latest_user: vec![DecisionContentPart::Text {
                text: "summarize this file".into(),
            }],
            visible_conversation: Vec::new(),
            history_partial: false,
            assessment_target: None,
        }
    }

    #[test]
    fn an_ordinal_round_trip_validates() {
        let request = ordinal_request();
        let body = request.encode_bounded().expect("encode");
        assert!(body.len() <= DECISION_REQUEST_LIMIT_BYTES);
        let response = DecisionResponse {
            decision: DecisionResult::Ordinal {
                probabilities: BTreeMap::from([
                    ("simple".to_string(), 0.9),
                    ("complex".to_string(), 0.1),
                ]),
            },
            assessment: None,
        };
        let bytes = serde_json::to_vec(&response).expect("response bytes");
        let decoded = DecisionResponse::decode_validated(&bytes, &request).expect("decode");
        assert_eq!(decoded, response);
    }

    #[test]
    fn unknown_fields_and_wrong_shapes_refuse() {
        let request = ordinal_request();
        let over_broad =
            br#"{"decision":{"kind":"ordinal","probabilities":{"simple":1.0}},"extra":true}"#;
        assert!(DecisionResponse::decode_validated(over_broad, &request).is_err());
        // Probabilities not summing to one refuse.
        let bad_sum = DecisionResponse {
            decision: DecisionResult::Ordinal {
                probabilities: BTreeMap::from([
                    ("simple".to_string(), 0.7),
                    ("complex".to_string(), 0.2),
                ]),
            },
            assessment: None,
        };
        let bytes = serde_json::to_vec(&bad_sum).expect("bytes");
        assert!(DecisionResponse::decode_validated(&bytes, &request).is_err());
        // Unrequested level ids refuse.
        let foreign = DecisionResponse {
            decision: DecisionResult::Ordinal {
                probabilities: BTreeMap::from([
                    ("simple".to_string(), 0.5),
                    ("other".to_string(), 0.5),
                ]),
            },
            assessment: None,
        };
        let bytes = serde_json::to_vec(&foreign).expect("bytes");
        assert!(DecisionResponse::decode_validated(&bytes, &request).is_err());
        // A result answering the other definition shape refuses.
        let categorical = DecisionResponse {
            decision: DecisionResult::Categorical {
                choice: "drafting".into(),
                refinement: None,
            },
            assessment: None,
        };
        let bytes = serde_json::to_vec(&categorical).expect("bytes");
        assert!(DecisionResponse::decode_validated(&bytes, &request).is_err());
    }

    #[test]
    fn categorical_choices_and_refinements_validate_against_the_request() {
        let request = DecisionRequest {
            decision: DecisionDefinition::Categorical {
                instructions: "Classify the task.".into(),
                options: vec![
                    DecisionCategory {
                        id: "drafting".into(),
                        criterion: "producing a draft".into(),
                        refinement: Some(OrdinalRefinement {
                            instructions: "Judge degree.".into(),
                            levels: vec![
                                DecisionLevel {
                                    id: "light".into(),
                                    criterion: "a touch-up".into(),
                                },
                                DecisionLevel {
                                    id: "heavy".into(),
                                    criterion: "a rewrite".into(),
                                },
                            ],
                        }),
                    },
                    DecisionCategory {
                        id: "review".into(),
                        criterion: "critiquing work".into(),
                        refinement: None,
                    },
                ],
            },
            latest_user: vec![DecisionContentPart::Text {
                text: "review my draft".into(),
            }],
            visible_conversation: vec![DecisionVisibleTurn {
                user: vec![DecisionContentPart::Text {
                    text: "write a draft".into(),
                }],
                status: DecisionTurnStatus::Completed,
                steps: vec![vec![DecisionStepPart::ToolActivity {
                    tool: "editor".into(),
                    status: DecisionTurnStatus::Completed,
                }]],
            }],
            history_partial: true,
            assessment_target: Some(DecisionAssessmentTarget {
                from: 0,
                instructions: "Assess the prior stage.".into(),
                criteria: vec![
                    DecisionCriterion {
                        score: 0.0,
                        criterion: "unusable".into(),
                    },
                    DecisionCriterion {
                        score: 0.5,
                        criterion: "usable with rework".into(),
                    },
                    DecisionCriterion {
                        score: 1.0,
                        criterion: "complete".into(),
                    },
                ],
            }),
        };
        request.validate().expect("request validates");
        request.encode_bounded().expect("bounded encode");
        let response = DecisionResponse {
            decision: DecisionResult::Categorical {
                choice: "review".into(),
                refinement: None,
            },
            assessment: Some(DecisionAssessment {
                score: 0.5,
                partial: false,
                reason: Some("the draft needed rework".into()),
            }),
        };
        let bytes = serde_json::to_vec(&response).expect("bytes");
        assert!(DecisionResponse::decode_validated(&bytes, &request).is_ok());
        // A refinement answering a category without one refuses.
        let refined = DecisionResponse {
            decision: DecisionResult::Categorical {
                choice: "review".into(),
                refinement: Some(OrdinalResult {
                    probabilities: BTreeMap::from([
                        ("light".to_string(), 0.5),
                        ("heavy".to_string(), 0.5),
                    ]),
                }),
            },
            assessment: None,
        };
        let bytes = serde_json::to_vec(&refined).expect("bytes");
        assert!(DecisionResponse::decode_validated(&bytes, &request).is_err());
    }

    #[test]
    fn structural_request_violations_refuse_fail_closed() {
        let mut request = ordinal_request();
        request.latest_user = Vec::new();
        assert!(request.encode_bounded().is_err());
        let mut request = ordinal_request();
        request.assessment_target = Some(DecisionAssessmentTarget {
            from: 0,
            instructions: String::new(),
            criteria: vec![
                DecisionCriterion {
                    score: 0.0,
                    criterion: "a".into(),
                },
                DecisionCriterion {
                    score: 0.5,
                    criterion: "b".into(),
                },
                DecisionCriterion {
                    score: 0.5,
                    criterion: "c".into(),
                },
            ],
        });
        assert!(request.validate().is_err());
    }
}
