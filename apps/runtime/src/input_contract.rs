//! A3S Work listing input contract — snake_case only on the Work surface.

use serde_json::Value;

pub fn is_illegal_work_field_key(key: &str) -> bool {
    key.chars().any(|c| c.is_ascii_uppercase())
}

pub fn find_illegal_work_field(input: &serde_json::Map<String, Value>) -> Option<String> {
    input.keys().find(|k| is_illegal_work_field_key(k)).cloned()
}

pub fn str_field(input: &serde_json::Map<String, Value>, key: &str) -> Option<String> {
    match input.get(key) {
        Some(Value::String(s)) if !s.trim().is_empty() => Some(s.trim().to_string()),
        Some(Value::Number(n)) => Some(n.to_string()),
        _ => None,
    }
}

pub fn require_str(input: &serde_json::Map<String, Value>, key: &str) -> Result<String, String> {
    str_field(input, key).ok_or_else(|| format!("{key} is required"))
}

pub fn country_code_field(
    input: &serde_json::Map<String, Value>,
    fallback: Option<&str>,
) -> Option<String> {
    let raw = str_field(input, "country_code").or_else(|| fallback.map(|s| s.to_string()))?;
    let code = raw.trim().to_uppercase();
    if code.len() == 2 && code.chars().all(|c| c.is_ascii_uppercase()) {
        Some(code)
    } else {
        None
    }
}
