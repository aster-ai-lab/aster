use super::*;
use crate::llm::structured::{
    generate_blocking_with_cancellation, StructuredMode, StructuredRequest,
};
use serde::{Deserialize, Serialize};

const FOLLOW_UP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
const MAX_PROMPT_CHARS: usize = 4_000;
const MAX_RESPONSE_CHARS: usize = 6_000;
const MAX_QUESTION_CHARS: usize = 160;

const FOLLOW_UP_SYSTEM: &str = "\
You suggest optional next questions after a coding assistant has finished a task.
Return JSON only. Do not include markdown.
Write one specific, useful primary question in the user's language, plus up to two alternatives.
Each question must offer a concrete next step that builds on the completed work, such as verification, risk review, documentation, or a relevant extension.
Do not ask generic questions like 'What should I do next?', do not repeat the completed answer, and do not use internal orchestration terms.
These are optional suggestions, not a request for missing information and never a new task started by the assistant.";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FollowUpSuggestions {
    pub primary: String,
    #[serde(default)]
    pub alternatives: Vec<String>,
}

fn bounded(text: &str, cap: usize) -> String {
    text.chars().take(cap).collect()
}

fn question(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty()
        || value.chars().count() > MAX_QUESTION_CHARS
        || (!value.contains('?') && !value.contains('？'))
    {
        return None;
    }
    Some(value.to_string())
}

fn validated(value: serde_json::Value) -> Option<FollowUpSuggestions> {
    let primary = question(value.get("primary")?.as_str()?)?;
    let mut alternatives = Vec::new();
    for candidate in value
        .get("alternatives")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_str)
        .filter_map(question)
    {
        if candidate != primary && !alternatives.contains(&candidate) {
            alternatives.push(candidate);
        }
        if alternatives.len() == 2 {
            break;
        }
    }
    Some(FollowUpSuggestions {
        primary,
        alternatives,
    })
}

async fn generate_with_client(
    client: &dyn LlmClient,
    prompt: &str,
    response: &str,
    cancellation: tokio_util::sync::CancellationToken,
) -> Option<FollowUpSuggestions> {
    let request = StructuredRequest {
        prompt: format!(
            "Completed user request:\n{}\n\nFinal assistant answer:\n{}",
            bounded(prompt, MAX_PROMPT_CHARS),
            bounded(response, MAX_RESPONSE_CHARS)
        ),
        system: Some(FOLLOW_UP_SYSTEM.to_string()),
        schema: serde_json::json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["primary"],
            "properties": {
                "primary": { "type": "string", "minLength": 1, "maxLength": MAX_QUESTION_CHARS },
                "alternatives": {
                    "type": "array",
                    "maxItems": 2,
                    "items": { "type": "string", "minLength": 1, "maxLength": MAX_QUESTION_CHARS }
                }
            }
        }),
        schema_name: "task_follow_up_suggestions".to_string(),
        schema_description: Some("Optional follow-up questions after a completed task".to_string()),
        mode: StructuredMode::Auto,
        max_repair_attempts: 0,
    };
    match tokio::time::timeout(
        FOLLOW_UP_TIMEOUT,
        generate_blocking_with_cancellation(client, &request, cancellation),
    )
    .await
    {
        Ok(Ok(result)) => validated(result.object),
        Ok(Err(error)) => {
            tracing::debug!(%error, "follow-up suggestion generation failed");
            None
        }
        Err(_) => None,
    }
}

impl AgentSession {
    /// Generate optional next questions without opening another agent run or
    /// changing the settled turn. Failures only suppress this convenience surface.
    pub async fn generate_follow_up_suggestions(
        &self,
        prompt: &str,
        response: &str,
    ) -> Option<FollowUpSuggestions> {
        if !self.config.follow_up_suggestions
            || self.closed.load(std::sync::atomic::Ordering::Relaxed)
            || prompt.trim().is_empty()
            || response.trim().is_empty()
        {
            return None;
        }
        let client = self
            .llm_client
            .fork_for_session(&format!("{}:follow-up", self.session_id))
            .unwrap_or_else(|| Arc::clone(&self.llm_client));
        generate_with_client(
            client.as_ref(),
            prompt,
            response,
            self.session_cancel.child_token(),
        )
        .await
    }
}

#[cfg(test)]
#[path = "follow_up_runtime_tests.rs"]
mod tests;
