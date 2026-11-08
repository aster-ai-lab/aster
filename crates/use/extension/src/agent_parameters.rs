//! Closed JSON Schema subset for an agent call.
//!
//! Install inspection and turn start both use these functions. A schema that
//! is not this subset is refused. Values must be a flat map of the declared
//! scalars.

use std::collections::BTreeSet;

use aster_use_core::{UseError, UseResult};
use serde_json::{Map, Value};

pub const MAX_AGENT_PARAMETER_SCHEMA_BYTES: usize = 65_536;
pub const MAX_AGENT_PARAMETERS: usize = 12;
pub const MAX_PARAMETER_STRING_BYTES: usize = 4096;

const RESERVED_NAMES: &[&str] = &["root", "submit", "__proto__", "prototype", "constructor"];

#[derive(Debug, Clone, PartialEq)]
pub struct AgentParameterSchema {
    pub fields: Vec<AgentParameterField>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AgentParameterField {
    pub name: String,
    pub title: String,
    pub kind: AgentParameterKind,
    pub required: bool,
    pub minimum: Option<f64>,
    pub maximum: Option<f64>,
    pub max_length: Option<usize>,
    pub enum_values: Vec<Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentParameterKind {
    String,
    Number,
    Integer,
    Boolean,
}

pub fn parse_agent_parameter_schema(bytes: &[u8]) -> UseResult<AgentParameterSchema> {
    if bytes.is_empty() || bytes.len() > MAX_AGENT_PARAMETER_SCHEMA_BYTES {
        return Err(invalid("Agent parameter schema is empty or too large."));
    }
    if bytes.contains(&0) {
        return Err(invalid("Agent parameter schema must be UTF-8 JSON."));
    }
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|_| invalid("Agent parameter schema must be UTF-8 JSON."))?;
    let root = value
        .as_object()
        .ok_or_else(|| invalid("Agent parameter schema root must be an object."))?;
    if root.get("type").and_then(Value::as_str) != Some("object") {
        return Err(invalid("Agent parameter schema root type must be object."));
    }
    for key in root.keys() {
        if key != "type" && key != "properties" && key != "required" {
            return Err(invalid(
                "Agent parameter schema only allows type, properties, and required.",
            ));
        }
    }
    let properties = root
        .get("properties")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("Agent parameter schema properties must be an object."))?;
    if properties.is_empty() || properties.len() > MAX_AGENT_PARAMETERS {
        return Err(invalid(
            "Agent parameter schema must declare 1 to 12 properties.",
        ));
    }
    let required = required_names(root.get("required"), properties)?;
    let mut fields = Vec::with_capacity(properties.len());
    for (name, spec) in properties {
        if !parameter_name_ok(name) {
            return Err(invalid("Agent parameter name is not an identifier."));
        }
        let spec = spec
            .as_object()
            .ok_or_else(|| invalid("Agent parameter property must be an object."))?;
        fields.push(parse_field(name, spec, required.contains(name))?);
    }
    Ok(AgentParameterSchema { fields })
}

pub fn validate_agent_parameter_values(
    schema: &AgentParameterSchema,
    values: &Map<String, Value>,
) -> UseResult<Map<String, Value>> {
    if values.len() > MAX_AGENT_PARAMETERS {
        return Err(invalid("Agent parameter values exceed 12 keys."));
    }
    let mut checked = Map::new();
    for field in &schema.fields {
        match values.get(&field.name) {
            None if field.required => {
                return Err(invalid("A required agent parameter is missing."));
            }
            None => {}
            Some(value) => {
                checked.insert(field.name.clone(), check_value(field, value)?);
            }
        }
    }
    if values.len() != checked.len() {
        return Err(invalid(
            "Agent parameter values contain a key the schema does not declare.",
        ));
    }
    Ok(checked)
}

fn required_names(
    value: Option<&Value>,
    properties: &Map<String, Value>,
) -> UseResult<BTreeSet<String>> {
    let Some(value) = value else {
        return Ok(BTreeSet::new());
    };
    let items = value
        .as_array()
        .ok_or_else(|| invalid("Agent parameter required must be an array."))?;
    let mut names = BTreeSet::new();
    for item in items {
        let name = item
            .as_str()
            .ok_or_else(|| invalid("Agent parameter required entries must be strings."))?;
        if !properties.contains_key(name) || !names.insert(name.to_owned()) {
            return Err(invalid(
                "Agent parameter required names must be unique declared properties.",
            ));
        }
    }
    Ok(names)
}

fn parse_field(
    name: &str,
    spec: &Map<String, Value>,
    required: bool,
) -> UseResult<AgentParameterField> {
    for key in spec.keys() {
        if !matches!(
            key.as_str(),
            "type" | "title" | "enum" | "minimum" | "maximum" | "maxLength"
        ) {
            return Err(invalid(
                "Agent parameter property uses a keyword outside the closed subset.",
            ));
        }
    }
    let kind = match spec.get("type").and_then(Value::as_str) {
        Some("string") => AgentParameterKind::String,
        Some("number") => AgentParameterKind::Number,
        Some("integer") => AgentParameterKind::Integer,
        Some("boolean") => AgentParameterKind::Boolean,
        _ => {
            return Err(invalid(
                "Agent parameter type must be string, number, integer, or boolean.",
            ))
        }
    };
    let title = match spec.get("title") {
        None => name.to_owned(),
        Some(Value::String(text)) if !text.trim().is_empty() && text.chars().count() <= 80 => {
            text.trim().to_owned()
        }
        _ => return Err(invalid("Agent parameter title must be 1 to 80 characters.")),
    };
    let max_length = match spec.get("maxLength") {
        None => None,
        Some(value) if kind == AgentParameterKind::String => Some(bounded_length(value)?),
        _ => return Err(invalid("maxLength is only valid on a string parameter.")),
    };
    let minimum = optional_bound(spec.get("minimum"), kind)?;
    let maximum = optional_bound(spec.get("maximum"), kind)?;
    if let (Some(min), Some(max)) = (minimum, maximum) {
        if min > max {
            return Err(invalid("Agent parameter minimum is greater than maximum."));
        }
    }
    let enum_values = match spec.get("enum") {
        None => Vec::new(),
        Some(_) if kind == AgentParameterKind::Boolean => {
            return Err(invalid("A boolean parameter cannot declare enum."));
        }
        Some(value) => parse_enum(value, kind)?,
    };
    Ok(AgentParameterField {
        name: name.to_owned(),
        title,
        kind,
        required,
        minimum,
        maximum,
        max_length,
        enum_values,
    })
}

fn bounded_length(value: &Value) -> UseResult<usize> {
    let length = value
        .as_u64()
        .ok_or_else(|| invalid("maxLength must be an integer."))?;
    if length == 0 || length > MAX_PARAMETER_STRING_BYTES as u64 {
        return Err(invalid("maxLength must be from 1 to 4096."));
    }
    Ok(length as usize)
}

fn optional_bound(value: Option<&Value>, kind: AgentParameterKind) -> UseResult<Option<f64>> {
    let Some(value) = value else {
        return Ok(None);
    };
    if !matches!(
        kind,
        AgentParameterKind::Number | AgentParameterKind::Integer
    ) {
        return Err(invalid(
            "minimum and maximum are only valid on numeric parameters.",
        ));
    }
    let number = finite_number(value)
        .ok_or_else(|| invalid("Agent parameter bound must be a finite number."))?;
    if kind == AgentParameterKind::Integer && number.fract() != 0.0 {
        return Err(invalid(
            "An integer parameter bound must be a whole number.",
        ));
    }
    Ok(Some(number))
}

fn parse_enum(value: &Value, kind: AgentParameterKind) -> UseResult<Vec<Value>> {
    let items = value
        .as_array()
        .ok_or_else(|| invalid("Agent parameter enum must be an array."))?;
    if items.is_empty() || items.len() > 8 {
        return Err(invalid("Agent parameter enum must contain 1 to 8 values."));
    }
    let mut values = Vec::with_capacity(items.len());
    for item in items {
        let checked = match kind {
            AgentParameterKind::String => {
                let text = item
                    .as_str()
                    .ok_or_else(|| invalid("A string enum value must be a string."))?;
                if text.is_empty() || text.len() > 80 {
                    return Err(invalid("A string enum value must be 1 to 80 bytes."));
                }
                Value::String(text.to_owned())
            }
            AgentParameterKind::Number | AgentParameterKind::Integer => {
                let number = finite_number(item)
                    .ok_or_else(|| invalid("A numeric enum value must be a finite number."))?;
                if kind == AgentParameterKind::Integer && number.fract() != 0.0 {
                    return Err(invalid("An integer enum value must be a whole number."));
                }
                serde_json::Number::from_f64(number)
                    .map(Value::Number)
                    .ok_or_else(|| invalid("A numeric enum value must be a finite number."))?
            }
            AgentParameterKind::Boolean => {
                return Err(invalid("A boolean parameter cannot declare enum."));
            }
        };
        if values.iter().any(|existing| existing == &checked) {
            return Err(invalid("Agent parameter enum values must be unique."));
        }
        values.push(checked);
    }
    Ok(values)
}

fn check_value(field: &AgentParameterField, value: &Value) -> UseResult<Value> {
    let checked = match field.kind {
        AgentParameterKind::String => {
            let text = value
                .as_str()
                .ok_or_else(|| invalid("A string parameter must be a string."))?;
            if text.len() > field.max_length.unwrap_or(MAX_PARAMETER_STRING_BYTES)
                || text.contains('\0')
            {
                return Err(invalid("A string parameter is too long."));
            }
            if field.required && text.trim().is_empty() {
                return Err(invalid("A required string parameter is empty."));
            }
            Value::String(text.to_owned())
        }
        AgentParameterKind::Number => {
            let number = finite_number(value)
                .ok_or_else(|| invalid("A number parameter must be a finite number."))?;
            Value::Number(number_value(number)?)
        }
        AgentParameterKind::Integer => {
            let number = finite_number(value)
                .ok_or_else(|| invalid("An integer parameter must be a whole number."))?;
            if number.fract() != 0.0 || number < i64::MIN as f64 || number > i64::MAX as f64 {
                return Err(invalid("An integer parameter must be a whole number."));
            }
            Value::Number(number_value(number)?)
        }
        AgentParameterKind::Boolean => Value::Bool(
            value
                .as_bool()
                .ok_or_else(|| invalid("A boolean parameter must be a boolean."))?,
        ),
    };
    if !field.enum_values.is_empty()
        && !field
            .enum_values
            .iter()
            .any(|item| enum_match(item, &checked))
    {
        return Err(invalid("An agent parameter is outside its enum."));
    }
    if let Some(min) = field.minimum {
        let number =
            finite_number(&checked).ok_or_else(|| invalid("A numeric parameter is required."))?;
        if number < min {
            return Err(invalid("An agent parameter is below its minimum."));
        }
    }
    if let Some(max) = field.maximum {
        let number =
            finite_number(&checked).ok_or_else(|| invalid("A numeric parameter is required."))?;
        if number > max {
            return Err(invalid("An agent parameter is above its maximum."));
        }
    }
    Ok(checked)
}

fn enum_match(expected: &Value, actual: &Value) -> bool {
    if expected == actual {
        return true;
    }
    match (finite_number(expected), finite_number(actual)) {
        (Some(left), Some(right)) => left == right,
        _ => false,
    }
}

fn finite_number(value: &Value) -> Option<f64> {
    let number = value.as_f64()?;
    number.is_finite().then_some(number)
}

fn number_value(number: f64) -> UseResult<serde_json::Number> {
    if number.fract() == 0.0 && (i64::MIN as f64..=i64::MAX as f64).contains(&number) {
        return Ok(serde_json::Number::from(number as i64));
    }
    serde_json::Number::from_f64(number)
        .ok_or_else(|| invalid("A numeric parameter must be finite."))
}

fn parameter_name_ok(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_alphabetic() || name.len() > 48 || RESERVED_NAMES.contains(&name) {
        return false;
    }
    chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

fn invalid(message: &str) -> UseError {
    UseError::new("use.extension.agent_parameters_invalid", message.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEGAL: &str = r#"{
      "type": "object",
      "properties": {
        "topic": { "type": "string", "title": "主题", "maxLength": 200 },
        "days": { "type": "integer", "title": "天数", "minimum": 1, "maximum": 14 },
        "agreed": { "type": "boolean", "title": "同意" },
        "kind": { "type": "string", "title": "类型", "enum": ["a", "b"] }
      },
      "required": ["days", "kind"]
    }"#;

    #[test]
    fn a_legal_schema_accepts_a_flat_value_map() {
        let schema = parse_agent_parameter_schema(LEGAL.as_bytes()).unwrap();
        let values = serde_json::json!({ "days": 5, "kind": "a", "agreed": false });
        let checked =
            validate_agent_parameter_values(&schema, values.as_object().unwrap()).unwrap();
        assert_eq!(checked.get("days").and_then(Value::as_i64), Some(5));
        assert!(checked.get("topic").is_none());
    }

    #[test]
    fn nested_objects_arrays_and_unknown_keywords_are_refused() {
        for source in [
            r#"{"type":"object","properties":{"body":{"type":"object"}},"additionalProperties":false}"#,
            r#"{"type":"object","properties":{"tags":{"type":"array"}}}"#,
            r#"{"type":"object","properties":{"topic":{"type":"string","pattern":"x"}}}"#,
            r#"{"type":"object","properties":{"bad-name":{"type":"string"}}}"#,
            r#"{"type":"object","properties":{}}"#,
        ] {
            assert!(
                parse_agent_parameter_schema(source.as_bytes()).is_err(),
                "{source}"
            );
        }
    }

    #[test]
    fn a_missing_required_value_is_refused() {
        let schema = parse_agent_parameter_schema(LEGAL.as_bytes()).unwrap();
        let values = serde_json::json!({ "kind": "a" });
        assert!(validate_agent_parameter_values(&schema, values.as_object().unwrap()).is_err());
    }
}
