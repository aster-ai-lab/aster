//! A wire projection and exact-name lookup over the model's admitted tool list.

use crate::llm::{ContentBlock, Message, ToolCall, ToolDefinition, TranscriptVisibility};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

pub(crate) const LOOKUP_NAME: &str = "load_tool_definitions";
const MAX_LOOKUP_NAMES: usize = 8;
const MAX_CATALOG_BYTES: usize = 64 * 1024;
const MIN_SAVED_BYTES: usize = 1_024;
const MAX_SCHEMA_DEPTH: usize = 24;
const MAX_FIELD_NOTE_BYTES: usize = 128;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LookupRequest {
    names: Vec<String>,
}

/// The source is already filtered for this Run and this model turn. A name
/// conflict, oversized catalog, or negligible saving keeps the source intact.
pub(crate) fn project(
    source: &[ToolDefinition],
    expanded: &BTreeSet<String>,
) -> Option<Vec<ToolDefinition>> {
    if source
        .iter()
        .any(|definition| definition.name == LOOKUP_NAME)
    {
        return None;
    }
    encoded_len(source)?;
    let mut original_wire_bytes = 0usize;
    let mut tools = Vec::with_capacity(source.len() + 1);
    for original in source {
        let definition = wire_definition(original);
        let schema = &definition.parameters;
        if !schema.is_object() {
            return None;
        }
        original_wire_bytes += serde_json::to_vec(&definition).ok()?.len();
        if expanded.contains(&definition.name) {
            tools.push(original.clone());
            continue;
        }
        let mut description = definition.description;
        description.push('\n');
        if describe_schema(schema, &mut description, 0).is_none() {
            tools.push(original.clone());
            continue;
        }
        tools.push(ToolDefinition {
            name: definition.name,
            description,
            parameters: required_values(schema),
        });
    }
    tools.push(ToolDefinition {
        name: LOOKUP_NAME.into(),
        description: "Open exact definitions for named tools. Signatures use ? for optional parameters. Call this alone if you need constraints, formats, or examples.".into(),
        parameters: json!({
            "type": "object",
            "properties": {"names": {"type": "array", "items": {"type": "string"}, "minItems": 1, "maxItems": MAX_LOOKUP_NAMES}},
            "required": ["names"],
            "additionalProperties": false,
        }),
    });
    let projected_wire: Vec<_> = tools.iter().map(wire_definition).collect();
    (original_wire_bytes.saturating_sub(encoded_len(&projected_wire)?) >= MIN_SAVED_BYTES)
        .then_some(tools)
}

/// Verify the exact provider-facing projection of the submitted tool subset.
pub(crate) fn verify(source: &[ToolDefinition], tools: &[ToolDefinition]) -> bool {
    let submitted: BTreeMap<_, _> = tools
        .iter()
        .map(|tool| (tool.name.as_str(), tool))
        .collect();
    let subset: Vec<_> = source
        .iter()
        .filter(|tool| submitted.contains_key(tool.name.as_str()))
        .cloned()
        .collect();
    let expanded = subset
        .iter()
        .filter(|tool| {
            let actual = submitted[tool.name.as_str()];
            actual.description == tool.description && actual.parameters == tool.parameters
        })
        .map(|tool| tool.name.clone())
        .collect();
    project(&subset, &expanded).is_some_and(|expected| {
        expected.len() == tools.len()
            && expected.iter().zip(tools).all(|(left, right)| {
                left.name == right.name
                    && left.description == right.description
                    && left.parameters == right.parameters
            })
    })
}

/// Expand an exclusively local lookup against this call's catalog. The next
/// request carries the exact definitions in its native tools, once per name.
pub(crate) fn lookup(
    source: &[ToolDefinition],
    call: &ToolCall,
    messages: &[Message],
    expanded: &mut BTreeSet<String>,
) -> Option<Message> {
    if call.name != LOOKUP_NAME
        || messages.iter().any(|message| {
            message
                .content
                .iter()
                .any(|block| matches!(block, ContentBlock::ToolUse { id, .. } if id == &call.id))
        })
    {
        return None;
    }
    let args = LookupRequest::deserialize(&call.args).ok()?;
    if args.names.is_empty() || args.names.len() > MAX_LOOKUP_NAMES {
        return None;
    }
    let names: BTreeSet<_> = args.names.into_iter().collect();
    let available = source
        .iter()
        .filter(|tool| names.contains(tool.name.as_str()))
        .count();
    if available != names.len() || names.is_subset(expanded) {
        return None;
    }
    expanded.extend(names);
    let mut response = Message::tool_result(
        &call.id,
        "The requested exact definitions are in the current tool list.",
        false,
    );
    response.transcript_visibility = TranscriptVisibility::Wire;
    Some(response)
}

fn wire_definition(tool: &ToolDefinition) -> ToolDefinition {
    ToolDefinition {
        name: tool.name.clone(),
        description: tool.description.clone(),
        parameters: crate::llm::wire_tool_parameters_deepseek(&tool.parameters, true),
    }
}

fn required_values(schema: &Value) -> Value {
    let required = schema.get("required").and_then(Value::as_array);
    let mut properties = Map::new();
    if let Some(required) = required {
        for name in required.iter().filter_map(Value::as_str) {
            let field = &schema["properties"][name];
            if field.get("enum").is_some() || field.get("const").is_some() {
                let values = ["type", "enum", "const"]
                    .into_iter()
                    .filter_map(|key| Some((key.into(), field.get(key)?.clone())))
                    .collect::<Map<_, _>>();
                properties.insert(name.into(), Value::Object(values));
            }
        }
    }
    if properties.is_empty() {
        json!({"type": "object"})
    } else {
        let required: Vec<_> = properties.keys().collect();
        json!({"type": "object", "required": required, "properties": properties})
    }
}

fn describe_schema(schema: &Value, output: &mut String, depth: usize) -> Option<()> {
    if depth >= MAX_SCHEMA_DEPTH || output.len() > MAX_CATALOG_BYTES {
        return None;
    }
    if let Some(value) = schema.get("const") {
        write!(output, "{value}").ok()?;
    } else if let Some(values) = schema.get("enum").and_then(Value::as_array) {
        for (index, value) in values.iter().enumerate() {
            if index > 0 {
                output.push('|');
            }
            match value.as_str() {
                Some(text) => output.push_str(text),
                None => write!(output, "{value}").ok()?,
            }
        }
    } else if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
        output.push('{');
        for (index, (name, field)) in properties.iter().enumerate() {
            if index > 0 {
                output.push(',');
            }
            output.push_str(name);
            if !schema["required"]
                .as_array()
                .is_some_and(|required| required.iter().any(|value| value.as_str() == Some(name)))
            {
                output.push('?');
            }
            output.push(':');
            describe_schema(field, output, depth + 1)?;
        }
        output.push('}');
    } else if schema["type"] == "array" {
        output.push('[');
        describe_schema(&schema["items"], output, depth + 1)?;
        output.push(']');
    } else {
        output.push_str(match schema["type"].as_str() {
            Some("string") => "str",
            Some("integer") => "int",
            Some("number") => "num",
            Some("boolean") => "bool",
            Some("object") => "obj",
            _ => "value",
        });
    }
    for (key, separator) in [("oneOf", '|'), ("anyOf", '|'), ("allOf", '&')] {
        if let Some(branches) = schema[key].as_array() {
            output.push_str(" & (");
            for (index, branch) in branches.iter().enumerate() {
                if index > 0 {
                    output.push(separator);
                }
                describe_schema(branch, output, depth + 1)?;
            }
            output.push(')');
        }
    }
    if let Some(note) = schema["description"]
        .as_str()
        .filter(|note| !note.is_empty())
    {
        // Keep the exact tool when a field note does not fit the signature.
        if note.len() > MAX_FIELD_NOTE_BYTES || note.contains('\n') {
            return None;
        }
        write!(output, " ({note})").ok()?;
    }
    (output.len() <= MAX_CATALOG_BYTES).then_some(())
}

fn encoded_len(tools: &[ToolDefinition]) -> Option<usize> {
    let mut bytes = 0usize;
    for tool in tools {
        bytes = bytes.checked_add(serde_json::to_vec(tool).ok()?.len())?;
        if bytes > MAX_CATALOG_BYTES {
            return None;
        }
    }
    Some(bytes)
}

#[cfg(test)]
mod tests;
