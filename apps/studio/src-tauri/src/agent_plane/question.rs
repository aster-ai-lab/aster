//! Agent questions use the owned form channel, never a permission grant.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::forms::FormValues;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Question {
    pub question: String,
    pub choices: Vec<Choice>,
    pub allow_free_text: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Choice {
    pub id: String,
    pub label: String,
}

impl Question {
    pub fn from_permission(params: &Value) -> Result<Option<Self>, &'static str> {
        let Some(marker) = params.pointer("/_meta/question") else {
            return Ok(None);
        };
        let allow_free_text = marker
            .get("allowFreeText")
            .and_then(Value::as_bool)
            .ok_or("question.allowFreeText is required")?;
        let has_choices = marker
            .get("hasChoices")
            .and_then(Value::as_bool)
            .ok_or("question.hasChoices is required; update the ACP agent")?;
        let question = params
            .pointer("/toolCall/title")
            .and_then(Value::as_str)
            .filter(|text| !text.trim().is_empty() && text.len() <= 4096)
            .ok_or("question text is required")?
            .to_owned();
        let options = params
            .get("options")
            .and_then(Value::as_array)
            .filter(|options| !options.is_empty() && options.len() <= 8)
            .ok_or("question options are invalid")?;
        let mut ids = HashSet::new();
        let mut choices = Vec::new();
        for option in options {
            let id = option
                .get("optionId")
                .and_then(Value::as_str)
                .filter(|id| {
                    id.strip_prefix("question-").is_some_and(|index| {
                        index
                            .parse::<usize>()
                            .is_ok_and(|n| n < 8 && *id == format!("question-{n}"))
                    })
                })
                .ok_or("question option identity is invalid")?;
            let label = option
                .get("name")
                .and_then(Value::as_str)
                .filter(|text| !text.trim().is_empty() && text.len() <= 4096)
                .ok_or("question option label is invalid")?;
            if option.get("kind").and_then(Value::as_str) != Some("reject_once") || !ids.insert(id)
            {
                return Err("question options cannot grant permissions or repeat identities");
            }
            if has_choices {
                choices.push(Choice {
                    id: id.to_owned(),
                    label: label.to_owned(),
                });
            }
        }
        if !has_choices && (!allow_free_text || options.len() != 1) {
            return Err("question has no answer surface");
        }
        Ok(Some(Self {
            question,
            choices,
            allow_free_text,
        }))
    }

    pub fn answer(&self, values: &FormValues) -> Option<Value> {
        if values
            .keys()
            .any(|name| name != "choice" && name != "answer")
        {
            return None;
        }
        let text = |name| match values.get(name) {
            None => Some(""),
            Some(Value::String(value)) => Some(value.as_str()),
            _ => None,
        };
        let choice = text("choice")?;
        let answer = text("answer")?.trim();
        match (!choice.is_empty(), !answer.is_empty()) {
            (true, false) if self.choices.iter().any(|option| option.id == choice) => {
                Some(json!({"outcome":{"outcome":"selected","optionId":choice}}))
            }
            (false, true) if self.allow_free_text => {
                Some(json!({"outcome":{"outcome":"cancelled"},"_meta":{"followup_message":answer}}))
            }
            _ => None,
        }
    }
}

#[cfg(test)]
#[path = "question_tests.rs"]
mod tests;
