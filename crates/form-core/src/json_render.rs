//! Closed JSON Render contract shared by the agent and native form hosts.

use std::collections::{BTreeMap, HashSet};

use serde::Deserialize;
use serde_json::{json, Value};

pub const MAX_JSON_RENDER_BYTES: usize = 65_536;

#[derive(Debug, thiserror::Error)]
#[error("invalid JSON Render document: {0}")]
pub struct JsonRenderError(&'static str);

macro_rules! components {
    ($($name:ident($children:literal; $($prop:literal),*)),* $(,)?) => {
        #[derive(Clone, Copy, PartialEq, Eq)]
        enum Component { $($name),* }
        impl Component {
            fn parse(value: &str) -> Result<Self, JsonRenderError> {
                match value { $(stringify!($name) => Ok(Self::$name),)* _ => Err(JsonRenderError("unsupported component")) }
            }
            fn as_str(self) -> &'static str { match self { $(Self::$name => stringify!($name),)* } }
            fn props(self) -> &'static [&'static str] { match self { $(Self::$name => &[$($prop),*],)* } }
            fn children(self) -> bool { match self { $(Self::$name => $children,)* } }
        }
        pub fn json_render_component_catalog() -> Value {
            let mut catalog = serde_json::Map::new();
            $(catalog.insert(Component::$name.as_str().into(),json!({"props":Component::$name.props(),"children":Component::$name.children()}));)*
            Value::Object(catalog)
        }
    }
}

components! {
    Form(true; "submitAction"),
    Card(true; "title"),
    Stack(true; "direction", "gap"),
    Grid(true; "columns"),
    Text(false; "content"),
    Input(false; "label", "value", "type", "min", "max", "step", "required", "placeholder"),
    Textarea(false; "label", "value", "required", "placeholder"),
    Select(false; "label", "value", "options", "required", "placeholder"),
    Slider(false; "label", "value", "min", "max", "step"),
    Checkbox(false; "label", "checked", "required"),
    Metric(false; "label", "value", "unit"),
    BarChart(false; "labels", "values"),
    Button(false; "action", "label"),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Source {
    root: String,
    elements: BTreeMap<String, Element>,
    #[serde(default)]
    state: State,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct State {
    #[serde(default)]
    form: BTreeMap<String, Value>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Element {
    #[serde(rename = "type")]
    kind: String,
    props: BTreeMap<String, Value>,
    #[serde(default)]
    children: Vec<String>,
}

/// Native fields in visual order, ready for the terminal's existing schema form.
pub struct JsonRenderDocument {
    pub fields: Vec<JsonRenderField>,
    pub text: Vec<String>,
}

pub struct JsonRenderField {
    pub name: String,
    pub schema: Value,
    pub initial: Value,
    pub required: bool,
}

fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 48
        && value.as_bytes()[0].is_ascii_alphabetic()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        && !matches!(value, "__proto__" | "prototype" | "constructor")
}

fn text(value: Option<&Value>) -> Result<&str, JsonRenderError> {
    value
        .and_then(Value::as_str)
        .ok_or(JsonRenderError("text is required"))
}

pub fn parse_json_render(source: &str) -> Result<JsonRenderDocument, JsonRenderError> {
    let canonical = crate::parse_json_with_limit(source.as_bytes(), MAX_JSON_RENDER_BYTES)
        .map_err(|_| JsonRenderError("invalid, duplicate or oversized JSON"))?;
    let value = serde_json::to_value(canonical).map_err(|_| JsonRenderError("invalid JSON"))?;
    let source: Source = serde_json::from_value(value)
        .map_err(|_| JsonRenderError("expected root/elements/state"))?;
    if source.elements.is_empty()
        || source.elements.len() > 160
        || source.elements.keys().any(|id| !identifier(id))
        || source.state.form.len() > 64
    {
        return Err(JsonRenderError("document bound exceeded"));
    }
    let mut document = JsonRenderDocument {
        fields: Vec::new(),
        text: Vec::new(),
    };
    let mut seen = HashSet::new();
    let mut names = HashSet::new();
    walk(
        &source.root,
        &source,
        &mut seen,
        &mut names,
        &mut document,
        0,
    )?;
    if seen.len() != source.elements.len()
        || source.state.form.keys().any(|key| !names.contains(key))
    {
        return Err(JsonRenderError("unreachable elements or unbound state"));
    }
    Ok(document)
}

fn walk(
    id: &str,
    source: &Source,
    seen: &mut HashSet<String>,
    names: &mut HashSet<String>,
    document: &mut JsonRenderDocument,
    depth: usize,
) -> Result<(), JsonRenderError> {
    if depth > 20 || !seen.insert(id.to_owned()) {
        return Err(JsonRenderError("cyclic, shared or deep element"));
    }
    let node = source
        .elements
        .get(id)
        .ok_or(JsonRenderError("missing element"))?;
    let kind = Component::parse(&node.kind)?;
    if node
        .props
        .keys()
        .any(|key| !kind.props().contains(&key.as_str()))
        || node.children.len() > 64
        || (!kind.children() && !node.children.is_empty())
    {
        return Err(JsonRenderError("unsupported properties or children"));
    }
    let p = &node.props;
    match kind {
        Component::Input
        | Component::Textarea
        | Component::Select
        | Component::Slider
        | Component::Checkbox => {
            let slot = if kind == Component::Checkbox {
                "checked"
            } else {
                "value"
            };
            let binding = p
                .get(slot)
                .and_then(Value::as_object)
                .ok_or(JsonRenderError("field requires $bindState"))?;
            let name = binding
                .get("$bindState")
                .and_then(Value::as_str)
                .and_then(|value| value.strip_prefix("/form/"))
                .ok_or(JsonRenderError("field must bind /form/name"))?;
            if binding.len() != 1
                || !identifier(name)
                || !names.insert(name.to_owned())
                || names.len() > 64
            {
                return Err(JsonRenderError("invalid or duplicate field binding"));
            }
            let label = text(p.get("label"))?;
            if label.trim().is_empty() {
                return Err(JsonRenderError("field label is empty"));
            }
            let required = match p.get("required") {
                None => false,
                Some(Value::Bool(value)) => *value,
                _ => return Err(JsonRenderError("required must be boolean")),
            };
            let value_kind = if kind == Component::Checkbox {
                "boolean"
            } else if kind == Component::Slider {
                "number"
            } else if kind == Component::Input {
                match p.get("type").and_then(Value::as_str).unwrap_or("text") {
                    "number" => "number",
                    "text" | "email" | "tel" => "string",
                    _ => return Err(JsonRenderError("unsupported input type")),
                }
            } else {
                "string"
            };
            if kind == Component::Input && p.get("type").is_some_and(|value| !value.is_string()) {
                return Err(JsonRenderError("input type must be text"));
            }
            if kind == Component::Input && value_kind != "number" {
                for key in ["min", "max", "step"] {
                    if p.get(key)
                        .is_some_and(|value| value.as_f64().is_none_or(|n| !n.is_finite()))
                    {
                        return Err(JsonRenderError("numeric bound must be finite"));
                    }
                }
            }
            let initial =
                source
                    .state
                    .form
                    .get(name)
                    .cloned()
                    .unwrap_or_else(|| match value_kind {
                        "boolean" => false.into(),
                        "number" => 0.into(),
                        _ => "".into(),
                    });
            let valid = match value_kind {
                "boolean" => initial.is_boolean(),
                "number" => initial
                    .as_f64()
                    .is_some_and(|n| n.is_finite() && n.abs() <= 9_007_199_254_740_991.0),
                _ => initial.as_str().is_some_and(|s| s.len() <= 4096),
            };
            if !valid {
                return Err(JsonRenderError("field state has the wrong type or size"));
            }
            let mut schema = json!({"type":value_kind,"title":label,"default":initial});
            if let Some(placeholder) = p.get("placeholder") {
                text(Some(placeholder))?;
                schema["description"] = placeholder.clone();
            }
            if value_kind == "number" {
                for (key, target) in [
                    ("min", "minimum"),
                    ("max", "maximum"),
                    ("step", "multipleOf"),
                ] {
                    if let Some(value) = p.get(key) {
                        let number = value
                            .as_f64()
                            .ok_or(JsonRenderError("numeric bound must be finite"))?;
                        if !number.is_finite() || (key == "step" && number <= 0.0) {
                            return Err(JsonRenderError("invalid numeric bound"));
                        }
                        if key != "step" {
                            schema[target] = value.clone();
                        }
                    }
                }
                let min = p.get("min").and_then(Value::as_f64);
                let max = p.get("max").and_then(Value::as_f64);
                let value = initial
                    .as_f64()
                    .ok_or(JsonRenderError("numeric state required"))?;
                if min.zip(max).is_some_and(|(a, b)| a >= b)
                    || min.is_some_and(|min| value < min)
                    || max.is_some_and(|max| value > max)
                    || (kind == Component::Slider && (min.is_none() || max.is_none()))
                {
                    return Err(JsonRenderError("invalid field range"));
                }
            }
            if kind == Component::Select {
                let options = p
                    .get("options")
                    .and_then(Value::as_array)
                    .ok_or(JsonRenderError("Select requires options"))?;
                let mut unique = HashSet::new();
                if options.is_empty()
                    || options.len() > 64
                    || options
                        .iter()
                        .any(|v| v.as_str().is_none_or(|s| s.is_empty() || !unique.insert(s)))
                    || (initial != "" && !options.contains(&initial))
                {
                    return Err(JsonRenderError("invalid Select options or state"));
                }
                schema["enum"] = Value::Array(options.clone());
            }
            document.fields.push(JsonRenderField {
                name: name.to_owned(),
                schema,
                initial,
                required,
            });
        }
        Component::Card => {
            if let Some(title) = p.get("title") {
                document.text.push(text(Some(title))?.to_owned());
            }
        }
        Component::Text => document.text.push(text(p.get("content"))?.to_owned()),
        Component::Button => {
            if p.get("action").and_then(Value::as_str) != Some("submit") {
                return Err(JsonRenderError("unsupported action"));
            }
            text(p.get("label"))?;
        }
        Component::Form => {
            if p.get("submitAction")
                .is_some_and(|value| value.as_str() != Some("submit"))
            {
                return Err(JsonRenderError("unsupported form action"));
            }
        }
        Component::Grid => {
            if p.get("columns")
                .is_some_and(|value| !matches!(value.as_f64(), Some(1.0 | 2.0 | 3.0)))
            {
                return Err(JsonRenderError("invalid grid columns"));
            }
        }
        Component::Stack => {
            if p.get("direction")
                .is_some_and(|v| !matches!(v.as_str(), Some("vertical" | "horizontal")))
                || p.get("gap")
                    .is_some_and(|v| v.as_f64().is_none_or(|n| !(0.0..=24.0).contains(&n)))
            {
                return Err(JsonRenderError("invalid stack layout"));
            }
        }
        Component::Metric => {
            text(p.get("label"))?;
            if !p
                .get("value")
                .is_some_and(|v| v.is_string() || v.is_number())
            {
                return Err(JsonRenderError("invalid metric value"));
            }
            let unit = p
                .get("unit")
                .map(|unit| text(Some(unit)))
                .transpose()?
                .unwrap_or("");
            let value = p
                .get("value")
                .ok_or(JsonRenderError("invalid metric value"))?;
            let value = value
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| value.to_string());
            document
                .text
                .push(format!("{}: {value}{unit}", text(p.get("label"))?));
        }
        Component::BarChart => {
            let labels = p
                .get("labels")
                .and_then(Value::as_array)
                .ok_or(JsonRenderError("invalid chart labels"))?;
            let values = p
                .get("values")
                .and_then(Value::as_array)
                .ok_or(JsonRenderError("invalid chart values"))?;
            if labels.len() > 64
                || labels.len() != values.len()
                || labels.iter().any(|v| !v.is_string())
                || values
                    .iter()
                    .any(|v| v.as_f64().is_none_or(|n| !n.is_finite() || n < 0.0))
            {
                return Err(JsonRenderError("invalid chart data"));
            }
            for (label, value) in labels.iter().zip(values) {
                document
                    .text
                    .push(format!("{}: {value}", text(Some(label))?));
            }
        }
    }
    for child in &node.children {
        walk(child, source, seen, names, document, depth + 1)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "json_render_tests.rs"]
mod tests;
