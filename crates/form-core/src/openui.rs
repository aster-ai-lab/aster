//! The component and positional-argument contract shared by native form hosts and renderers.

use std::collections::{BTreeMap, HashSet};

use a3s_acl::Value;
use serde_json::json;

pub const MAX_OPENUI_BYTES: usize = 65_536;

#[derive(Debug, thiserror::Error)]
pub enum OpenUiError {
    #[error("invalid OpenUI form: {0}")]
    Invalid(&'static str),
    #[error("form exceeds its size or graph bound")]
    Limit,
    #[error("unsupported OpenUI form component: {0}")]
    Component(String),
}

macro_rules! components {
    ($($name:ident($children:literal; $($prop:literal),*)),* $(,)?) => {
        #[derive(Clone, Copy, PartialEq, Eq)]
        enum Component { $($name),* }
        impl Component {
            fn parse(value: &str) -> Result<Self, OpenUiError> {
                match value {
                    $(stringify!($name) => Ok(Self::$name),)*
                    _ => Err(OpenUiError::Component(value.to_owned())),
                }
            }
            fn as_str(self) -> &'static str { match self { $(Self::$name => stringify!($name),)* } }
            fn props(self) -> &'static [&'static str] { match self { $(Self::$name => &[$($prop),*],)* } }
            fn children(self) -> bool { match self { $(Self::$name => $children,)* } }
        }
        pub fn openui_component_catalog() -> serde_json::Value {
            json!({ $(stringify!($name): {"props": [$($prop),*], "children": $children}),* })
        }
    }
}

components! {
    Card(true; "title"),
    Stack(true;),
    Grid(true; "columns"),
    Text(true;),
    Input(false; "name", "label", "initial", "type", "min", "max", "step", "required", "placeholder"),
    Textarea(false; "name", "label", "initial", "required", "placeholder"),
    Slider(false; "name", "label", "initial", "min", "max", "step"),
    Select(false; "name", "label", "initial", "options", "required", "placeholder"),
    Checkbox(false; "name", "label", "initial", "required"),
    Metric(false; "label", "value", "unit"),
    BarChart(false; "labels", "values"),
    Button(false; "action", "label"),
}

pub fn openui_parameter_table() -> String {
    let catalog = openui_component_catalog();
    let Some(entries) = catalog.as_object() else {
        return String::new();
    };
    entries
        .iter()
        .filter_map(|(name, entry)| {
            let mut props = entry["props"]
                .as_array()?
                .iter()
                .filter_map(|value| value.as_str())
                .collect::<Vec<_>>();
            if entry["children"] == true {
                props.push("children");
            }
            let component = Component::parse(name).ok()?;
            Some(format!("{}({})", component.as_str(), props.join(", ")))
        })
        .collect::<Vec<_>>()
        .join("; ")
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

fn literal(value: &Value) -> bool {
    match value {
        Value::String(_) | Value::Bool(_) | Value::Null => true,
        Value::Number(value) => value.is_finite(),
        Value::List(values) => values.iter().all(literal),
        _ => false,
    }
}

struct Node<'a> {
    kind: Component,
    args: &'a [Value],
}

/// Compile the bounded form graph before a host publishes a waiting request.
pub fn validate_openui(source: &str) -> Result<(), OpenUiError> {
    use OpenUiError::{Invalid, Limit};
    if source.len() > MAX_OPENUI_BYTES {
        return Err(Limit);
    }
    let lines = source
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    if lines.is_empty() || lines.len() > 160 {
        return Err(Invalid("statements are required"));
    }
    for line in &lines {
        let Some((name, call)) = line.split_once(" = ") else {
            return Err(Invalid("component assignment required"));
        };
        if !identifier(name) || !call.ends_with(')') {
            return Err(Invalid("component assignment required"));
        }
    }
    let document = a3s_acl::parse_with_limits(
        source,
        a3s_acl::ParseLimits {
            max_document_bytes: 65_536,
            max_nesting_depth: 16,
            max_collection_items: 160,
            max_token_bytes: 4096,
            ..Default::default()
        },
    )
    .map_err(|_| Invalid("component statements could not be parsed"))?;
    let mut nodes = BTreeMap::new();
    for block in &document.blocks {
        if !block.labels.is_empty() || !block.blocks.is_empty() || block.attributes.len() != 1 {
            return Err(Invalid("one component per statement required"));
        }
        let Some(Value::Call(kind, args)) = block.attributes.get(&block.name) else {
            return Err(Invalid("component assignment required"));
        };
        let kind = Component::parse(kind)?;
        if args.len() > kind.props().len() + usize::from(kind.children())
            || !args.iter().all(literal)
        {
            return Err(Invalid("component arguments do not match the catalog"));
        }
        if matches!(kind, Component::Card | Component::Grid | Component::Stack)
            && !matches!(args.get(kind.props().len()), Some(Value::List(_)))
        {
            return Err(Invalid(
                "container children must be a list of node references",
            ));
        }
        if nodes
            .insert(block.name.as_str(), Node { kind, args })
            .is_some()
        {
            return Err(Invalid("duplicate component assignment"));
        }
    }
    let mut fields = HashSet::new();
    walk("root", &nodes, &mut HashSet::new(), &mut fields, &mut 0)
}

fn walk<'a>(
    id: &str,
    nodes: &BTreeMap<&'a str, Node<'a>>,
    path: &mut HashSet<String>,
    fields: &mut HashSet<String>,
    count: &mut usize,
) -> Result<(), OpenUiError> {
    use OpenUiError::{Invalid, Limit};
    if path.len() > 20 || *count >= 160 {
        return Err(Limit);
    }
    if !path.insert(id.to_owned()) {
        return Err(Invalid("cyclic component references"));
    }
    *count += 1;
    let node = nodes
        .get(id)
        .ok_or(Invalid("missing component reference"))?;
    let get = |key: &str| {
        node.kind
            .props()
            .iter()
            .position(|name| *name == key)
            .and_then(|index| node.args.get(index))
    };
    let text = |key| match get(key) {
        Some(Value::String(value)) => Some(value.as_str()),
        _ => None,
    };
    let number = |key| match get(key) {
        Some(Value::Number(value)) => Some(*value),
        _ => None,
    };
    let kind = node.kind;
    if matches!(
        kind,
        Component::Input
            | Component::Textarea
            | Component::Slider
            | Component::Select
            | Component::Checkbox
    ) {
        let name = text("name").ok_or(Invalid("field name required"))?;
        if !identifier(name) || !fields.insert(name.to_owned()) || fields.len() > 64 {
            return Err(Invalid("unique bounded field names required"));
        }
        if text("label").is_none_or(|label| label.trim().is_empty()) {
            return Err(Invalid("field label required"));
        }
        if get("required").is_some_and(|value| !matches!(value, Value::Bool(_)))
            || get("placeholder").is_some_and(|value| !matches!(value, Value::String(_)))
        {
            return Err(Invalid("invalid required or placeholder property"));
        }
        let numeric = kind == Component::Slider || text("type") == Some("number");
        if let Some(initial) = get("initial").filter(|value| !matches!(value, Value::Null)) {
            let valid = if kind == Component::Checkbox {
                matches!(initial, Value::Bool(_))
            } else if numeric {
                matches!(initial, Value::Number(_))
            } else {
                matches!(initial, Value::String(value) if value.len() <= 4096)
            };
            if !valid {
                return Err(Invalid("initial value has the wrong field type"));
            }
        }
        if kind == Component::Input
            && get("type").is_some()
            && !matches!(text("type"), Some("text" | "number" | "email" | "tel"))
        {
            return Err(Invalid("unsupported input type"));
        }
        if matches!(kind, Component::Input | Component::Slider) {
            for name in ["min", "max", "step"] {
                if get(name).is_some() && number(name).is_none() {
                    return Err(Invalid("numeric bounds required"));
                }
            }
            if numeric
                && (number("step").is_some_and(|step| step <= 0.0)
                    || number("min")
                        .zip(number("max"))
                        .is_some_and(|(min, max)| min >= max)
                    || number("initial")
                        .zip(number("min"))
                        .is_some_and(|(value, min)| value < min)
                    || number("initial")
                        .zip(number("max"))
                        .is_some_and(|(value, max)| value > max))
            {
                return Err(Invalid("invalid numeric range"));
            }
            if kind == Component::Slider
                && ["initial", "min", "max"]
                    .iter()
                    .any(|name| number(name).is_none())
            {
                return Err(Invalid("slider requires an initial value and bounds"));
            }
        }
        if kind == Component::Select {
            let Some(Value::List(options)) = get("options") else {
                return Err(Invalid("select options required"));
            };
            let mut seen = HashSet::new();
            for option in options {
                let Value::String(value) = option else {
                    return Err(Invalid("select options must be strings"));
                };
                if value.is_empty() || !seen.insert(value.as_str()) {
                    return Err(Invalid("select options must be unique and nonempty"));
                }
            }
            let initial = text("initial").unwrap_or("");
            if options.is_empty() || (!initial.is_empty() && !seen.contains(initial)) {
                return Err(Invalid("invalid select value"));
            }
        }
    }
    if kind == Component::Button && (text("action") != Some("submit") || text("label").is_none()) {
        return Err(Invalid("invalid submit button"));
    }
    if kind == Component::Grid
        && get("columns").is_some()
        && !matches!(number("columns"), Some(1.0 | 2.0 | 3.0))
    {
        return Err(Invalid("invalid grid columns"));
    }
    if kind == Component::BarChart {
        let (Some(Value::List(labels)), Some(Value::List(values))) = (get("labels"), get("values"))
        else {
            return Err(Invalid("chart arrays required"));
        };
        if labels.len() != values.len()
            || labels.iter().any(|v| !matches!(v, Value::String(_)))
            || values
                .iter()
                .any(|v| !matches!(v, Value::Number(n) if *n >= 0.0))
        {
            return Err(Invalid("invalid chart values"));
        }
    }
    if kind.children() {
        if let Some(children) = node.args.get(kind.props().len()) {
            match children {
                Value::List(children) => {
                    for child in children {
                        let Value::String(child) = child else {
                            return Err(Invalid("child references required"));
                        };
                        walk(child, nodes, path, fields, count)?;
                    }
                }
                Value::String(_) => {}
                _ => return Err(Invalid("unsupported children")),
            }
        }
    }
    path.remove(id);
    Ok(())
}

#[cfg(test)]
#[path = "openui_tests.rs"]
mod tests;
