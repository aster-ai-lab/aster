use super::decision::{
    DecisionDefinition, DecisionError, DecisionRequest, DecisionResponse, DecisionResult,
    DecisionService, OrdinalResult,
};
use a3s_apofasi::infer::{DeviceRequest, NeuralEngine};
use a3s_apofasi::{
    Answer, Criteria, DecisionKind, Question, State, StateTruncation, SystemOneRequest,
    SystemOneResponse,
};
use indexmap::IndexMap;
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

const DECISION_TIMEOUT: Duration = Duration::from_secs(5);
const DECISION_MAX_LEN: usize = 256;

pub(crate) struct ApofasiDecisionService {
    engine: Arc<NeuralEngine>,
    slots: Arc<tokio::sync::Semaphore>,
    timeout: Duration,
}

impl ApofasiDecisionService {
    pub(crate) fn load(
        checkpoint: impl AsRef<std::path::Path>,
        concurrency: usize,
    ) -> Result<Self, DecisionError> {
        Self::load_with_timeout(checkpoint, concurrency, DECISION_TIMEOUT)
    }

    fn load_with_timeout(
        checkpoint: impl AsRef<std::path::Path>,
        concurrency: usize,
        timeout: Duration,
    ) -> Result<Self, DecisionError> {
        let engine = NeuralEngine::load_with(checkpoint, DeviceRequest::Cpu)
            .map_err(|error| DecisionError::Unreachable(error.to_string()))?;
        Ok(Self {
            engine: Arc::new(engine),
            slots: Arc::new(tokio::sync::Semaphore::new(concurrency.max(1))),
            timeout,
        })
    }
}

#[async_trait::async_trait]
impl DecisionService for ApofasiDecisionService {
    async fn decide(&self, request: &DecisionRequest) -> Result<DecisionResponse, DecisionError> {
        let decision_request = request.clone();
        let request = system_one_request(request)?;
        let required = required_system_one_request(&decision_request, &request);
        let truncation = if decision_request.history_partial {
            StateTruncation::Tail
        } else {
            StateTruncation::TailAndMark
        };
        let engine = self.engine.clone();
        let slots = self.slots.clone();
        tokio::time::timeout(self.timeout, async move {
            let permit = slots
                .acquire_owned()
                .await
                .map_err(|_| DecisionError::Unreachable("apofasi decision queue closed".into()))?;
            tokio::task::spawn_blocking(move || {
                let _permit = permit;
                let response = engine
                    .decide_with_required_and_len(
                        &request,
                        Some(&required),
                        truncation,
                        DECISION_MAX_LEN,
                    )
                    .map_err(|error| DecisionError::Unreachable(error.to_string()))?;
                map_system_one_response(&decision_request, &response)
            })
            .await
            .map_err(|error| DecisionError::Unreachable(error.to_string()))?
        })
        .await
        .map_err(|_| DecisionError::Unreachable("apofasi decision timed out".into()))?
    }
}

fn system_one_request(request: &DecisionRequest) -> Result<SystemOneRequest, DecisionError> {
    request.validate()?;
    if request.assessment_target.is_some() {
        return Err(DecisionError::Invalid(
            "apofasi assessment targets are not supported".into(),
        ));
    }
    let state = State::Text(decision_state_text(request));
    let mut questions = IndexMap::new();
    match &request.decision {
        DecisionDefinition::Ordinal {
            instructions,
            levels,
        } => {
            questions.insert("decision".into(), score_question(instructions, levels)?);
        }
        DecisionDefinition::Categorical {
            instructions,
            options,
        } => {
            let choices = options
                .iter()
                .map(|option| (option.id.clone(), Some(json!(option.criterion))))
                .collect::<IndexMap<_, _>>();
            questions.insert(
                "decision".into(),
                Question::new(
                    DecisionKind::Choice,
                    json!(instructions),
                    Some(Criteria::Choice(choices)),
                )
                .map_err(|error| DecisionError::Invalid(error.to_string()))?,
            );
            for option in options {
                if let Some(refinement) = &option.refinement {
                    questions.insert(
                        format!("refinement:{}", option.id),
                        score_question(&refinement.instructions, &refinement.levels)?,
                    );
                }
            }
        }
    }
    Ok(SystemOneRequest {
        model: None,
        state,
        questions,
    })
}

fn required_system_one_request(
    decision: &DecisionRequest,
    full: &SystemOneRequest,
) -> SystemOneRequest {
    SystemOneRequest {
        model: None,
        state: State::Text(format!(
            "{}\n\n[history is partial]",
            decision_content_text(&decision.latest_user)
        )),
        questions: full.questions.clone(),
    }
}

fn decision_content_text(parts: &[super::decision::DecisionContentPart]) -> String {
    parts
        .iter()
        .map(|part| match part {
            super::decision::DecisionContentPart::Text { text } => text.clone(),
            super::decision::DecisionContentPart::Unavailable { source_kind } => {
                format!("[unavailable: {source_kind}]")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn decision_state_text(request: &DecisionRequest) -> String {
    use super::decision::DecisionStepPart;

    let mut sections = Vec::new();
    for turn in &request.visible_conversation {
        sections.push(format!(
            "Previous user:\n{}",
            decision_content_text(&turn.user)
        ));
        for step in &turn.steps {
            let text = step
                .iter()
                .map(|part| match part {
                    DecisionStepPart::Text { text } => text.clone(),
                    DecisionStepPart::Unavailable { source_kind } => {
                        format!("[unavailable: {source_kind}]")
                    }
                    DecisionStepPart::ToolActivity { tool, status } => {
                        format!("[tool {tool}: {status:?}]")
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");
            sections.push(format!("Previous step:\n{text}"));
        }
    }
    if request.history_partial {
        sections.push("[history is partial]".into());
    }
    sections.push(format!(
        "Latest user:\n{}",
        decision_content_text(&request.latest_user)
    ));
    sections.join("\n\n")
}

fn score_question(
    instructions: &str,
    levels: &[super::decision::DecisionLevel],
) -> Result<Question, DecisionError> {
    Question::new(
        DecisionKind::Score,
        json!(instructions),
        Some(Criteria::Score(
            levels.iter().map(|level| json!(level.criterion)).collect(),
        )),
    )
    .map_err(|error| DecisionError::Invalid(error.to_string()))
}

fn map_system_one_response(
    request: &DecisionRequest,
    response: &SystemOneResponse,
) -> Result<DecisionResponse, DecisionError> {
    let decision = match &request.decision {
        DecisionDefinition::Ordinal { levels, .. } => DecisionResult::Ordinal {
            probabilities: score_probabilities(
                response.answers.get("decision"),
                &levels
                    .iter()
                    .map(|level| level.id.clone())
                    .collect::<Vec<_>>(),
            )?,
        },
        DecisionDefinition::Categorical { options, .. } => {
            let Some(Answer::Choice { choice, .. }) = response.answers.get("decision") else {
                return Err(DecisionError::Invalid(
                    "apofasi omitted the categorical choice".into(),
                ));
            };
            let option = options.iter().find(|option| &option.id == choice);
            let refinement = option
                .and_then(|option| option.refinement.as_ref())
                .map(|refinement| {
                    Ok(OrdinalResult {
                        probabilities: score_probabilities(
                            response.answers.get(&format!("refinement:{choice}")),
                            &refinement
                                .levels
                                .iter()
                                .map(|level| level.id.clone())
                                .collect::<Vec<_>>(),
                        )?,
                    })
                })
                .transpose()?;
            DecisionResult::Categorical {
                choice: choice.clone(),
                refinement,
            }
        }
    };
    let response = DecisionResponse {
        decision,
        assessment: None,
    };
    let encoded =
        serde_json::to_vec(&response).map_err(|error| DecisionError::Invalid(error.to_string()))?;
    DecisionResponse::decode_validated(&encoded, request)
}

fn score_probabilities(
    answer: Option<&Answer>,
    level_ids: &[String],
) -> Result<BTreeMap<String, f64>, DecisionError> {
    let Some(Answer::Score { probabilities, .. }) = answer else {
        return Err(DecisionError::Invalid(
            "apofasi omitted the ordinal probabilities".into(),
        ));
    };
    let sum = probabilities
        .values()
        .map(|value| f64::from(*value))
        .sum::<f64>();
    if !sum.is_finite() || sum <= 0.0 || probabilities.len() != level_ids.len() {
        return Err(DecisionError::Invalid(
            "apofasi returned invalid ordinal probabilities".into(),
        ));
    }
    level_ids
        .iter()
        .enumerate()
        .map(|(index, id)| {
            let probability = probabilities
                .get(&index.to_string())
                .ok_or_else(|| DecisionError::Invalid("apofasi omitted a score level".into()))?;
            Ok((id.clone(), f64::from(*probability) / sum))
        })
        .collect()
}

#[cfg(test)]
#[path = "decision_apofasi_tests.rs"]
mod tests;
