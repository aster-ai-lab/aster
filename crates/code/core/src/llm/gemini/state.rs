//! Persisted Gemini parts bound to the unchanged public message projection.

use super::{GeminiError, MAX_BODY_BYTES};
use crate::llm::{ContentBlock, ImageSource, Message};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::collections::HashSet;

/// Exact native parts and call bindings, retained only for the original model.
/// Debug output omits all native payloads. Restoring JSON validates the state.
#[derive(Clone, Serialize)]
pub struct GeminiState {
    provider: String,
    model: String,
    parts: Vec<Value>,
    call_ids: Vec<String>,
    #[serde(skip)]
    projection_digest: String,
    #[serde(skip)]
    wire_chars: usize,
}

impl std::fmt::Debug for GeminiState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GeminiState")
            .finish_non_exhaustive()
    }
}

impl GeminiState {
    pub(crate) fn message(
        provider: &str,
        model: &str,
        parts: Vec<Value>,
    ) -> Result<Message, GeminiError> {
        let (state, mut message) = Self::admit(provider, model, parts, None)?;
        message.content.push(ContentBlock::GeminiState { state });
        Ok(message)
    }

    pub(super) fn message_with_ids(
        provider: &str,
        model: &str,
        parts: Vec<Value>,
        ids: Vec<String>,
    ) -> Result<Message, GeminiError> {
        let (state, mut message) = Self::admit(provider, model, parts, Some(ids))?;
        message.content.push(ContentBlock::GeminiState { state });
        Ok(message)
    }

    pub(crate) fn replay<'a>(
        &'a self,
        message: &Message,
        provider: &str,
        model: &str,
    ) -> Result<&'a [Value], GeminiError> {
        if self.provider != provider
            || self.model != model
            || message.role != "assistant"
            || projection_digest(message)? != self.projection_digest
        {
            return Err(GeminiError::StateMismatch);
        }
        Ok(&self.parts)
    }

    pub(crate) fn wire_chars(&self) -> usize {
        self.wire_chars
    }

    fn admit(
        provider: &str,
        model: &str,
        parts: Vec<Value>,
        saved_ids: Option<Vec<String>>,
    ) -> Result<(Self, Message), GeminiError> {
        if [provider, model].into_iter().any(|value| {
            value.is_empty()
                || value.len() > 256
                || value.trim() != value
                || value.chars().any(char::is_control)
        }) {
            return Err(GeminiError::Invalid("invalid Gemini state origin"));
        }
        let wire_chars = serde_json::to_vec(&parts)?.len();
        if wire_chars > MAX_BODY_BYTES || parts.is_empty() {
            return Err(GeminiError::Invalid("invalid Gemini parts size"));
        }
        let mut message = Message::assistant("");
        message.content.clear();
        let mut reasoning = String::new();
        let mut call_ids = Vec::new();
        let mut seen_ids = HashSet::new();
        for part in &parts {
            if !part.as_object().is_some_and(|part| !part.is_empty()) {
                return Err(GeminiError::Invalid(
                    "Gemini part must be a nonempty object",
                ));
            }
            let data_fields = [
                "text",
                "inlineData",
                "inline_data",
                "functionCall",
                "function_call",
                "functionResponse",
                "function_response",
                "fileData",
                "file_data",
                "executableCode",
                "executable_code",
                "codeExecutionResult",
                "code_execution_result",
                "toolCall",
                "tool_call",
                "toolResponse",
                "tool_response",
            ];
            if data_fields
                .iter()
                .filter(|name| part.get(**name).is_some())
                .count()
                > 1
            {
                return Err(GeminiError::Invalid(
                    "Gemini part has conflicting data fields",
                ));
            }
            if let Some(signature) = field(part, "thoughtSignature", "thought_signature")? {
                if !signature.is_string() {
                    return Err(GeminiError::Invalid("invalid Gemini thought signature"));
                }
            }
            let thought = part
                .get("thought")
                .map(|value| {
                    value
                        .as_bool()
                        .ok_or(GeminiError::Invalid("invalid Gemini thought flag"))
                })
                .transpose()?
                .unwrap_or(false);
            if let Some(text) = part.get("text") {
                let text = text
                    .as_str()
                    .ok_or(GeminiError::Invalid("invalid Gemini text"))?;
                if thought {
                    reasoning.push_str(text);
                } else {
                    message
                        .content
                        .push(ContentBlock::Text { text: text.into() });
                }
            }
            if let Some(call) = field(part, "functionCall", "function_call")? {
                let name = call
                    .get("name")
                    .and_then(Value::as_str)
                    .filter(|name| !name.is_empty())
                    .ok_or(GeminiError::Invalid("invalid Gemini function name"))?;
                let input = call
                    .get("args")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({}));
                if !input.is_object() {
                    return Err(GeminiError::Invalid(
                        "Gemini function args must be an object",
                    ));
                }
                let native_id = call
                    .get("id")
                    .map(|value| {
                        value
                            .as_str()
                            .ok_or(GeminiError::Invalid("invalid Gemini function id"))
                    })
                    .transpose()?;
                let id = match &saved_ids {
                    Some(ids) => ids
                        .get(call_ids.len())
                        .cloned()
                        .ok_or(GeminiError::Invalid("missing Gemini call id"))?,
                    None => native_id
                        .map(str::to_owned)
                        .unwrap_or_else(|| format!("call_{}", uuid::Uuid::new_v4())),
                };
                if id.is_empty()
                    || id.len() > 1024
                    || id.chars().any(char::is_control)
                    || native_id.is_some_and(|native| native != id)
                    || !seen_ids.insert(id.clone())
                {
                    return Err(GeminiError::Invalid("invalid Gemini call binding"));
                }
                message.content.push(ContentBlock::ToolUse {
                    id: id.clone(),
                    name: name.into(),
                    input,
                });
                call_ids.push(id);
            }
            if let Some(data) = field(part, "inlineData", "inline_data")? {
                let mime = field(data, "mimeType", "mime_type")?
                    .and_then(Value::as_str)
                    .ok_or(GeminiError::Invalid("invalid Gemini inline MIME type"))?;
                let data = data
                    .get("data")
                    .and_then(Value::as_str)
                    .ok_or(GeminiError::Invalid("invalid Gemini inline data"))?;
                if mime.starts_with("image/") && !thought {
                    message.content.push(ContentBlock::Image {
                        source: ImageSource {
                            source_type: "base64".into(),
                            media_type: mime.into(),
                            data: data.into(),
                        },
                    });
                }
            }
        }
        if saved_ids
            .as_ref()
            .is_some_and(|ids| ids.len() != call_ids.len())
        {
            return Err(GeminiError::Invalid("unexpected Gemini call id"));
        }
        message.reasoning_content = (!reasoning.is_empty()).then_some(reasoning);
        let projection_digest = projection_digest(&message)?;
        Ok((
            Self {
                provider: provider.into(),
                model: model.into(),
                parts,
                call_ids,
                projection_digest,
                wire_chars,
            },
            message,
        ))
    }
}

impl<'de> Deserialize<'de> for GeminiState {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Saved {
            provider: String,
            model: String,
            parts: Vec<Value>,
            call_ids: Vec<String>,
        }
        let saved = Saved::deserialize(deserializer)?;
        Self::admit(
            &saved.provider,
            &saved.model,
            saved.parts,
            Some(saved.call_ids),
        )
        .map(|(state, _)| state)
        .map_err(serde::de::Error::custom)
    }
}

fn projection_digest(message: &Message) -> Result<String, GeminiError> {
    let content = message
        .content
        .iter()
        .filter(|block| !matches!(block, ContentBlock::GeminiState { .. }))
        .collect::<Vec<_>>();
    crate::session_checkpoint::canonical_identity_digest(
        "aster.gemini.projection.v1",
        &(content, &message.reasoning_content),
    )
    .map_err(GeminiError::Identity)
}

pub(super) fn field<'a>(
    value: &'a Value,
    camel: &str,
    snake: &str,
) -> Result<Option<&'a Value>, GeminiError> {
    match (value.get(camel), value.get(snake)) {
        (Some(_), Some(_)) => Err(GeminiError::Invalid("ambiguous Gemini field aliases")),
        (a, b) => Ok(a.or(b)),
    }
}
