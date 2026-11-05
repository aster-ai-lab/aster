//! Stateless Responses requests mapped to the existing Chat Completions wire.
//! Server-side state and fields without a declared mapping refuse.

use super::{TranslationError, TranslationResult};
use serde_json::{Map, Value};
use std::collections::BTreeSet;

pub(super) fn to_openai(request: &Value) -> TranslationResult {
    let request = fields(
        request,
        &[
            "model",
            "input",
            "instructions",
            "max_output_tokens",
            "temperature",
            "top_p",
            "stream",
            "tools",
            "tool_choice",
            "parallel_tool_calls",
            "reasoning",
            "text",
            "metadata",
            "user",
            "service_tier",
            "prompt_cache_key",
            "prompt_cache_retention",
            "safety_identifier",
            "store",
            "previous_response_id",
        ],
        "request",
    )?;
    if request
        .get("previous_response_id")
        .is_some_and(|value| !value.is_null())
    {
        return Err(TranslationError::field(
            "previous_response_id",
            "stateful responses require a responses-native upstream",
        ));
    }
    let mut output = Map::new();
    if let Some(model) = request.get("model") {
        output.insert("model".into(), model.clone());
    }
    let mut messages = Vec::new();
    if let Some(value) = request.get("instructions").filter(|value| !value.is_null()) {
        let text = value
            .as_str()
            .ok_or_else(|| TranslationError::field("instructions", "must be a string"))?;
        if !text.is_empty() {
            messages.push(serde_json::json!({"role":"system", "content":text}));
        }
    }
    match request.get("input") {
        Some(Value::String(text)) => {
            messages.push(serde_json::json!({"role":"user", "content":text}))
        }
        Some(Value::Array(items)) => append_input(items, &mut messages)?,
        _ => {
            return Err(TranslationError::field(
                "input",
                "required as a text string or an item list",
            ))
        }
    }
    if messages.is_empty() {
        return Err(TranslationError::field(
            "input",
            "no messages remain to translate",
        ));
    }
    output.insert("messages".into(), Value::Array(messages));
    let max_tokens = match request
        .get("max_output_tokens")
        .filter(|value| !value.is_null())
    {
        Some(value) => value.as_u64().filter(|value| *value > 0).ok_or_else(|| {
            TranslationError::field("max_output_tokens", "must be a positive integer")
        })?,
        None => 1024,
    };
    output.insert("max_tokens".into(), max_tokens.into());
    for name in ["temperature", "top_p"] {
        if let Some(value) = request.get(name).filter(|value| !value.is_null()) {
            if !value.is_number() {
                return Err(TranslationError::field(name, "must be a number"));
            }
            output.insert(name.into(), value.clone());
        }
    }
    for name in [
        "user",
        "service_tier",
        "prompt_cache_key",
        "prompt_cache_retention",
        "safety_identifier",
    ] {
        if let Some(value) = request.get(name).filter(|value| !value.is_null()) {
            if !value.is_string() {
                return Err(TranslationError::field(name, "must be a string"));
            }
            output.insert(name.into(), value.clone());
        }
    }
    if let Some(metadata) = request.get("metadata").filter(|value| !value.is_null()) {
        if !metadata
            .as_object()
            .is_some_and(|object| object.values().all(Value::is_string))
        {
            return Err(TranslationError::field(
                "metadata",
                "must be an object of string values",
            ));
        }
        output.insert("metadata".into(), metadata.clone());
    }
    for name in ["stream", "parallel_tool_calls", "store"] {
        if let Some(value) = request.get(name).filter(|value| !value.is_null()) {
            let enabled = value
                .as_bool()
                .ok_or_else(|| TranslationError::field(name, "must be a boolean"))?;
            if name == "store" && enabled {
                return Err(TranslationError::field(
                    "store",
                    "stored Responses require a responses-native upstream",
                ));
            }
            if name != "stream" || enabled {
                output.insert(name.into(), value.clone());
            }
            if name == "stream" && enabled {
                output.insert(
                    "stream_options".into(),
                    serde_json::json!({"include_usage":true}),
                );
            }
        }
    }
    if let Some(tools) = request.get("tools").filter(|value| !value.is_null()) {
        let tools = tools
            .as_array()
            .ok_or_else(|| TranslationError::field("tools", "must be an array"))?;
        output.insert(
            "tools".into(),
            Value::Array(tools.iter().map(function_tool).collect::<Result<_, _>>()?),
        );
    }
    if let Some(choice) = request.get("tool_choice").filter(|value| !value.is_null()) {
        let choice = match choice {
            Value::String(value) if matches!(value.as_str(), "auto" | "none" | "required") => {
                choice.clone()
            }
            Value::Object(_) => {
                let object = fields(choice, &["type", "name"], "tool_choice")?;
                if object.get("type").and_then(Value::as_str) != Some("function") {
                    return Err(TranslationError::field(
                        "tool_choice.type",
                        "expected function",
                    ));
                }
                serde_json::json!({"type":"function", "function":{"name":required_identifier(object, "name")?}})
            }
            _ => {
                return Err(TranslationError::field(
                    "tool_choice",
                    "expected auto, none, required or a named function",
                ))
            }
        };
        output.insert("tool_choice".into(), choice);
    }
    if let Some(reasoning) = request.get("reasoning").filter(|value| !value.is_null()) {
        let object = fields(reasoning, &["effort"], "reasoning")?;
        if let Some(effort) = object.get("effort").filter(|value| !value.is_null()) {
            if !effort.is_string() {
                return Err(TranslationError::field(
                    "reasoning.effort",
                    "must be a string",
                ));
            }
            output.insert("reasoning_effort".into(), effort.clone());
        }
    }
    if let Some(text) = request.get("text").filter(|value| !value.is_null()) {
        let text = fields(text, &["format", "verbosity"], "text")?;
        if let Some(verbosity) = text.get("verbosity").filter(|value| !value.is_null()) {
            if !verbosity.is_string() {
                return Err(TranslationError::field(
                    "text.verbosity",
                    "must be a string",
                ));
            }
            output.insert("verbosity".into(), verbosity.clone());
        }
        if let Some(format) = text.get("format").filter(|value| !value.is_null()) {
            output.insert("response_format".into(), response_format(format)?);
        }
    }
    Ok(Value::Object(output))
}

fn append_input(items: &[Value], messages: &mut Vec<Value>) -> Result<(), TranslationError> {
    let mut pending = BTreeSet::new();
    let mut seen = BTreeSet::new();
    for item in items {
        let object = item
            .as_object()
            .ok_or_else(|| TranslationError::field("input", "each item must be an object"))?;
        // Item id/status are provenance; Chat tool linkage retains call_id.
        if object
            .get("id")
            .is_some_and(|value| !value.is_null() && !value.is_string())
        {
            return Err(TranslationError::field(
                "input.id",
                "must be a string or null",
            ));
        }
        if object.get("status").is_some_and(|value| {
            !value.is_null()
                && !matches!(
                    value.as_str(),
                    Some("in_progress" | "completed" | "incomplete")
                )
        }) {
            return Err(TranslationError::field(
                "input.status",
                "expected in_progress, completed or incomplete",
            ));
        }
        let kind = match object.get("type") {
            None => "message",
            Some(value) => value
                .as_str()
                .ok_or_else(|| TranslationError::field("input.type", "must be a string"))?,
        };
        match kind {
            "message" => {
                let object = fields(item, &["type", "role", "content", "id", "status"], "input.message")?;
                let role = required_string(object, "role")?;
                if !matches!(role, "user" | "assistant" | "system" | "developer") { return Err(TranslationError::field("input.role", "expected user, assistant, system or developer")); }
                if !pending.is_empty() && (role != "assistant" || messages.last().is_some_and(|message| message["role"] == "tool")) { return Err(TranslationError::field("input.type", "function_call requires its output before the next turn")); }
                let content = object.get("content").ok_or_else(|| TranslationError::field("input.content", "required"))?;
                if role == "assistant" {
                    let (content, refusal) = assistant_content(content)?;
                    let message = assistant_message(messages);
                    if let Some(text) = content.as_str() { append_text(message, "content", text); }
                    if let Some(refusal) = refusal { append_text(message, "refusal", &refusal); }
                } else {
                    messages.push(serde_json::json!({"role":role, "content":message_content(content, role == "user")?}));
                }
            }
            "reasoning" => {
                let object = fields(item, &["type", "id", "status", "content", "summary", "encrypted_content"], "input.reasoning")?;
                if object.get("encrypted_content").is_some_and(|value| !value.is_null()) || object.get("summary").is_some_and(|value| !value.as_array().is_some_and(Vec::is_empty)) {
                    return Err(TranslationError::field("input.reasoning", "encrypted content and reasoning summaries require a responses-native upstream"));
                }
                if !pending.is_empty() && messages.last().is_some_and(|message| message["role"] == "tool") {
                    return Err(TranslationError::field("input.type", "all pending outputs must precede the next reasoning turn"));
                }
                let parts = object.get("content").and_then(Value::as_array).filter(|parts| !parts.is_empty()).ok_or_else(|| TranslationError::field("input.reasoning.content", "non-empty plaintext reasoning parts required; opaque state requires a responses-native upstream"))?;
                let message = assistant_message(messages);
                for part in parts {
                    let part = fields(part, &["type", "text"], "input.reasoning.content")?;
                    if part.get("type").and_then(Value::as_str) != Some("reasoning_text") { return Err(TranslationError::field("input.reasoning.content.type", "expected reasoning_text")); }
                    append_text(message, "reasoning_content", required_string(part, "text")?);
                }
            }
            "function_call" => {
                let object = fields(item, &["type", "call_id", "name", "arguments", "id", "status"], "input.function_call")?;
                if !pending.is_empty() && messages.last().is_some_and(|message| message["role"] == "tool") {
                    return Err(TranslationError::field("input.type", "all pending outputs must precede the next function_call batch"));
                }
                let call_id = required_identifier(object, "call_id")?;
                if !seen.insert(call_id) { return Err(TranslationError::field("function_call.call_id", "must be unique in the supplied history")); }
                pending.insert(call_id);
                let call = serde_json::json!({"id":call_id, "type":"function", "function":{"name":required_identifier(object,"name")?,"arguments":required_string(object,"arguments")?}});
                let last = assistant_message(messages);
                if last.get("tool_calls").is_none() { last["tool_calls"] = serde_json::json!([]); }
                // Every assistant tool_calls value is constructed as an array here.
                last["tool_calls"].as_array_mut().expect("constructed tool_calls array").push(call);
            }
            "function_call_output" => {
                let object = fields(item, &["type", "call_id", "output", "id", "status"], "input.function_call_output")?;
                let call_id = required_identifier(object, "call_id")?;
                if !pending.remove(call_id) { return Err(TranslationError::field("function_call_output.call_id", "has no pending call in the supplied history")); }
                let content = object.get("output").ok_or_else(|| TranslationError::field("function_call_output.output", "required"))?;
                messages.push(serde_json::json!({"role":"tool","tool_call_id":call_id,"content":message_content(content, false)?}));
            }
            _ => return Err(TranslationError::field("input.type", "expected message, reasoning, function_call or function_call_output; item references and opaque state require a responses-native upstream")),
        }
    }
    if !pending.is_empty() {
        return Err(TranslationError::field(
            "input.type",
            "function_call requires a matching function_call_output in the supplied history",
        ));
    }
    Ok(())
}

fn assistant_message(messages: &mut Vec<Value>) -> &mut Value {
    if !messages
        .last()
        .is_some_and(|message| message["role"] == "assistant")
    {
        messages.push(serde_json::json!({"role":"assistant", "content":null}));
    }
    // A missing assistant message was just appended above.
    messages.last_mut().expect("constructed assistant message")
}

fn append_text(message: &mut Value, field: &str, text: &str) {
    if let Some(Value::String(value)) = message.get_mut(field) {
        value.push_str(text);
    } else {
        message[field] = Value::String(text.to_owned());
    }
}

fn assistant_content(content: &Value) -> Result<(Value, Option<String>), TranslationError> {
    let Some(parts) = content.as_array() else {
        return Ok((message_content(content, false)?, None));
    };
    let mut text_parts = Vec::new();
    let mut refusal = None::<String>;
    for part in parts {
        if part.get("type").and_then(Value::as_str) == Some("refusal") {
            let part = fields(part, &["type", "refusal"], "content.refusal")?;
            refusal
                .get_or_insert_with(String::new)
                .push_str(required_string(part, "refusal")?);
        } else {
            text_parts.push(part.clone());
        }
    }
    let content = if text_parts.is_empty() && refusal.is_some() {
        Value::Null
    } else {
        message_content(&Value::Array(text_parts), false)?
    };
    Ok((content, refusal))
}

fn message_content(content: &Value, images: bool) -> TranslationResult {
    if let Some(text) = content.as_str() {
        return Ok(Value::String(text.to_owned()));
    }
    let parts = content.as_array().ok_or_else(|| {
        TranslationError::field("content", "must be a text string or a part list")
    })?;
    let mixed = parts
        .iter()
        .any(|part| part.get("type").and_then(Value::as_str) == Some("input_image"));
    let mut text = String::new();
    let mut output = Vec::new();
    for part in parts {
        match part.get("type").and_then(Value::as_str) {
            Some("input_text" | "output_text") => {
                let object = fields(part, &["type", "text", "annotations"], "content.text")?;
                if object.get("annotations").is_some_and(|value| !value.as_array().is_some_and(Vec::is_empty)) { return Err(TranslationError::field("annotations", "non-empty annotations require a responses-native upstream")); }
                let value = required_string(object, "text")?;
                if mixed { output.push(serde_json::json!({"type":"text", "text":value})); } else { text.push_str(value); }
            }
            Some("input_image") if images => {
                let object = fields(part, &["type", "image_url", "detail"], "content.input_image")?;
                let mut image = serde_json::json!({"url":required_string(object,"image_url")?});
                if let Some(detail) = object.get("detail").filter(|value| !value.is_null()) {
                    if !matches!(detail.as_str(), Some("auto" | "low" | "high")) { return Err(TranslationError::field("detail", "expected auto, low or high")); }
                    image["detail"] = detail.clone();
                }
                output.push(serde_json::json!({"type":"image_url", "image_url":image}));
            }
            _ => return Err(TranslationError::field("content.type", "expected text parts or a user input_image URL; other content requires a responses-native upstream")),
        }
    }
    Ok(if mixed {
        Value::Array(output)
    } else {
        Value::String(text)
    })
}

fn function_tool(tool: &Value) -> TranslationResult {
    let object = fields(
        tool,
        &["type", "name", "description", "parameters", "strict"],
        "tools",
    )?;
    if object.get("type").and_then(Value::as_str) != Some("function") {
        return Err(TranslationError::field(
            "tools.type",
            "only function tools can translate",
        ));
    }
    let mut function = Map::new();
    function.insert(
        "name".into(),
        Value::String(required_identifier(object, "name")?.to_owned()),
    );
    for name in ["description", "parameters", "strict"] {
        if let Some(value) = object.get(name) {
            let valid = value.is_null()
                || match name {
                    "description" => value.is_string(),
                    "parameters" => value.is_object(),
                    _ => value.is_boolean(),
                };
            if !valid {
                return Err(TranslationError::field(
                    name,
                    "invalid function definition field type",
                ));
            }
            function.insert(name.into(), value.clone());
        }
    }
    Ok(serde_json::json!({"type":"function","function":function}))
}

fn response_format(format: &Value) -> TranslationResult {
    let object = fields(
        format,
        &["type", "name", "description", "schema", "strict"],
        "text.format",
    )?;
    match object.get("type").and_then(Value::as_str) {
        Some("text" | "json_object") if object.len() == 1 => Ok(format.clone()),
        Some("json_schema") => {
            let mut schema = object.clone();
            schema.remove("type");
            required_identifier(&schema, "name")?;
            if !schema.get("schema").is_some_and(Value::is_object) {
                return Err(TranslationError::field(
                    "text.format.schema",
                    "required as an object",
                ));
            }
            if schema
                .get("strict")
                .is_some_and(|value| !value.is_null() && !value.is_boolean())
            {
                return Err(TranslationError::field(
                    "text.format.strict",
                    "must be a boolean or null",
                ));
            }
            if schema
                .get("description")
                .is_some_and(|value| !value.is_null() && !value.is_string())
            {
                return Err(TranslationError::field(
                    "text.format.description",
                    "must be a string or null",
                ));
            }
            Ok(serde_json::json!({"type":"json_schema","json_schema":schema}))
        }
        _ => Err(TranslationError::field(
            "text.format",
            "expected text, json_object or a named json_schema",
        )),
    }
}

fn fields<'a>(
    value: &'a Value,
    allowed: &[&str],
    field: &str,
) -> Result<&'a Map<String, Value>, TranslationError> {
    let object = value
        .as_object()
        .ok_or_else(|| TranslationError::field(field, "must be an object"))?;
    if let Some(name) = object.keys().find(|name| !allowed.contains(&name.as_str())) {
        return Err(TranslationError::field(
            field,
            &format!("unsupported field {name}"),
        ));
    }
    Ok(object)
}

fn required_string<'a>(
    object: &'a Map<String, Value>,
    name: &str,
) -> Result<&'a str, TranslationError> {
    object
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| TranslationError::field(name, "required as a string"))
}

fn required_identifier<'a>(
    object: &'a Map<String, Value>,
    name: &str,
) -> Result<&'a str, TranslationError> {
    let value = required_string(object, name)?;
    if value.is_empty() {
        return Err(TranslationError::field(name, "must be non-empty"));
    }
    Ok(value)
}
