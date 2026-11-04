use super::{GeminiClient, GeminiError, MAX_BODY_BYTES};
use crate::llm::{
    structured, ContentBlock, Message, ToolDefinition, ToolResultContent, ToolResultContentField,
};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;

#[derive(Serialize)]
struct Content {
    role: &'static str,
    parts: Vec<Value>,
}

impl GeminiClient {
    pub(super) fn request(
        &self,
        messages: &[Message],
        system: Option<&str>,
        tools: &[ToolDefinition],
        directive: Option<&structured::StructuredDirective>,
    ) -> Result<Value, GeminiError> {
        if self
            .config
            .headers
            .keys()
            .filter(|name| name.eq_ignore_ascii_case("x-aster-provider-id"))
            .count()
            > 1
        {
            return Err(GeminiError::Invalid("ambiguous Gemini provider identity"));
        }
        let identity = self.provider_identity();
        if identity.is_empty()
            || identity.len() > 256
            || identity.trim() != identity
            || identity.chars().any(char::is_control)
        {
            return Err(GeminiError::Invalid("invalid Gemini provider identity"));
        }
        if self.max_tokens() == 0 {
            return Err(GeminiError::Invalid("Gemini output limit must be positive"));
        }
        let mut contents: Vec<Content> = Vec::new();
        let mut instructions = system
            .map(|text| vec![json!({"text":text})])
            .unwrap_or_default();
        let mut pending = HashMap::<String, (String, Option<String>)>::new();
        for message in messages {
            if message.role == "system" {
                for block in &message.content {
                    let ContentBlock::Text { text } = block else {
                        return Err(GeminiError::Invalid("Gemini system content must be text"));
                    };
                    instructions.push(json!({"text":text}));
                }
                continue;
            }
            let role = match message.role.as_str() {
                "assistant" => "model",
                "user" | "tool" => "user",
                _ => return Err(GeminiError::Invalid("invalid Gemini message role")),
            };
            let states = message
                .content
                .iter()
                .filter_map(|block| match block {
                    ContentBlock::GeminiState { state } => Some(state),
                    _ => None,
                })
                .collect::<Vec<_>>();
            if states.len() > 1 {
                return Err(GeminiError::StateMismatch);
            }
            let mut parts = Vec::new();
            if let Some(state) = states.first() {
                let raw = state.replay(message, self.provider_identity(), &self.config.model)?;
                let calls = raw.iter().filter_map(|part| {
                    part.get("functionCall")
                        .or_else(|| part.get("function_call"))
                });
                for (call, raw) in message.tool_calls().into_iter().zip(calls) {
                    if pending
                        .insert(
                            call.id,
                            (
                                call.name,
                                raw.get("id").and_then(Value::as_str).map(str::to_owned),
                            ),
                        )
                        .is_some()
                    {
                        return Err(GeminiError::Invalid("duplicate Gemini tool call"));
                    }
                }
                if self.config.text_only_input
                    && message
                        .content
                        .iter()
                        .any(|block| matches!(block, ContentBlock::Image { .. }))
                {
                    return Err(GeminiError::Invalid("Gemini model does not accept images"));
                }
                parts.extend_from_slice(raw);
            } else {
                for block in &message.content {
                    match block {
                        ContentBlock::Text { text } => parts.push(json!({"text":text})),
                        ContentBlock::Image { source } => {
                            if self.config.text_only_input || source.source_type != "base64" {
                                return Err(GeminiError::Invalid("unsupported Gemini image input"));
                            }
                            parts.push(json!({"inlineData":{"mimeType":source.media_type,"data":source.data}}));
                        }
                        ContentBlock::ToolUse { id, name, input } => {
                            if role != "model"
                                || !input.is_object()
                                || pending
                                    .insert(id.clone(), (name.clone(), Some(id.clone())))
                                    .is_some()
                            {
                                return Err(GeminiError::Invalid("invalid Gemini tool call"));
                            }
                            parts.push(json!({"functionCall":{"id":id,"name":name,"args":input}}));
                        }
                        ContentBlock::ToolResult {
                            tool_use_id,
                            content,
                            is_error,
                            trust,
                            redaction_reviewed,
                        } => {
                            if role != "user"
                                || (trust.requires_redaction_review() && !redaction_reviewed)
                            {
                                return Err(GeminiError::Invalid("unreviewed Gemini tool result"));
                            }
                            let (name, id) = pending
                                .remove(tool_use_id)
                                .ok_or(GeminiError::Invalid("orphan Gemini tool result"))?;
                            let response = if *is_error == Some(true) {
                                json!({"error":content.as_text()})
                            } else {
                                json!({"output":content.as_text()})
                            };
                            let mut result = json!({"name":name,"response":response});
                            if let Some(id) = id {
                                result["id"] = json!(id);
                            }
                            if let ToolResultContentField::Blocks(blocks) = content {
                                let images = blocks
                                    .iter()
                                    .filter_map(|block| match block {
                                        ToolResultContent::Image { source } => Some(source),
                                        _ => None,
                                    })
                                    .collect::<Vec<_>>();
                                if !images.is_empty() {
                                    if self.config.text_only_input
                                        || images
                                            .iter()
                                            .any(|source| source.source_type != "base64")
                                    {
                                        return Err(GeminiError::Invalid(
                                            "unsupported Gemini tool image",
                                        ));
                                    }
                                    result["parts"]=json!(images.into_iter().map(|source|json!({"inlineData":{"mimeType":source.media_type,"data":source.data}})).collect::<Vec<_>>());
                                }
                            }
                            parts.push(json!({"functionResponse":result}));
                        }
                        ContentBlock::GeminiState { .. } => return Err(GeminiError::StateMismatch),
                    }
                }
            }
            if !parts.is_empty() {
                if let Some(last) = contents.last_mut().filter(|last| last.role == role) {
                    last.parts.extend(parts);
                } else {
                    contents.push(Content { role, parts });
                }
            }
        }
        if contents.is_empty() || !pending.is_empty() {
            return Err(GeminiError::Invalid(
                "Gemini needs content and completed tool results",
            ));
        }
        let mut request = json!({"contents":contents,"generationConfig":{"maxOutputTokens":self.max_tokens(),"candidateCount":1}});
        if !instructions.is_empty() {
            request["systemInstruction"] = json!({"parts":instructions});
        }
        if !tools.is_empty() {
            request["tools"] = json!([{"functionDeclarations":tools.iter().map(|tool|json!({"name":tool.name,"description":tool.description,"parametersJsonSchema":tool.parameters})).collect::<Vec<_>>()}]);
        }
        let config = &mut request["generationConfig"];
        if !self.config.disable_temperature {
            if let Some(temperature) = self.config.temperature {
                config["temperature"] = json!(temperature);
            }
        }
        match (
            self.config.thinking_budget,
            self.config.reasoning_effort.as_deref(),
        ) {
            (Some(_), Some(_)) => {
                return Err(GeminiError::Invalid("conflicting Gemini thinking settings"))
            }
            (Some(budget), None) => {
                config["thinkingConfig"] =
                    json!({"thinkingBudget":budget,"includeThoughts":budget>0})
            }
            (None, Some("none")) => config["thinkingConfig"] = json!({"thinkingBudget":0}),
            (None, Some(level)) => {
                let level = match level {
                    "minimal" => "MINIMAL",
                    "low" => "LOW",
                    "medium" => "MEDIUM",
                    "high" => "HIGH",
                    _ => return Err(GeminiError::Invalid("unsupported Gemini thinking level")),
                };
                config["thinkingConfig"] = json!({"thinkingLevel":level,"includeThoughts":true});
            }
            (None, None) => {}
        }
        if let Some(directive) = directive {
            if let Some(format) = &directive.response_format {
                config["responseMimeType"] = json!("application/json");
                if let structured::ResponseFormat::JsonSchema { schema, .. } = format {
                    config["responseJsonSchema"] = schema.clone();
                }
            }
            if let Some(name) = &directive.force_tool {
                if !tools.iter().any(|tool| tool.name == *name) {
                    return Err(GeminiError::Invalid("Gemini forced tool is not declared"));
                }
                request["toolConfig"] =
                    json!({"functionCallingConfig":{"mode":"ANY","allowedFunctionNames":[name]}});
            }
        }
        if serde_json::to_vec(&request)?.len() > MAX_BODY_BYTES {
            return Err(GeminiError::Invalid("Gemini request exceeds byte limit"));
        }
        Ok(request)
    }

    pub(super) fn endpoint(&self, streaming: bool) -> Result<url::Url, GeminiError> {
        let mut url = url::Url::parse(
            self.config
                .base_url
                .as_deref()
                .unwrap_or("https://generativelanguage.googleapis.com"),
        )
        .map_err(|_| GeminiError::Invalid("invalid Gemini base URL"))?;
        if !matches!(url.scheme(), "http" | "https")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.fragment().is_some()
            || url.query_pairs().any(|(key, _)| key == "key")
        {
            return Err(GeminiError::Invalid(
                "Gemini credentials require a separate API key/header",
            ));
        }
        if self.config.model.is_empty()
            || self.config.model.len() > 256
            || self.config.model.trim() != self.config.model
            || self.config.model.chars().any(char::is_control)
        {
            return Err(GeminiError::Invalid("invalid Gemini model"));
        }
        let path = url.path().trim_end_matches('/');
        let collection = path.ends_with("/models");
        let version = path.ends_with("/v1") || path.ends_with("/v1beta");
        {
            let mut path = url
                .path_segments_mut()
                .map_err(|_| GeminiError::Invalid("invalid Gemini base URL"))?;
            path.pop_if_empty();
            if !collection {
                if !version {
                    path.push("v1beta");
                }
                path.push("models");
            }
            path.push(&format!(
                "{}:{}",
                self.config.model,
                if streaming {
                    "streamGenerateContent"
                } else {
                    "generateContent"
                }
            ));
        }
        let query = url
            .query_pairs()
            .filter(|(name, _)| name != "alt")
            .map(|(name, value)| (name.into_owned(), value.into_owned()))
            .collect::<Vec<_>>();
        url.set_query(None);
        if streaming || !query.is_empty() {
            let mut pairs = url.query_pairs_mut();
            pairs.extend_pairs(query);
            if streaming {
                pairs.append_pair("alt", "sse");
            }
        }
        Ok(url)
    }
}
