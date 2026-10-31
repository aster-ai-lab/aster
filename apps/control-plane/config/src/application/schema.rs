//! Value shape of a runtime builtin. The console renders its form from this and the
//! service rejects writes that do not match, so the two never drift apart.

use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ValueSchema {
    Boolean,
    Integer {
        min: i64,
        max: i64,
        #[serde(skip_serializing_if = "Option::is_none")]
        unit: Option<IntegerUnit>,
    },
    Text {
        max_len: usize,
        #[serde(skip_serializing_if = "std::ops::Not::not")]
        multiline: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        placeholder: Option<&'static str>,
        /// Applies to non-empty text; emptiness is governed by `required_when`.
        #[serde(skip_serializing_if = "Option::is_none")]
        pattern: Option<Pattern>,
    },
    /// Absolute `http(s)` URL.
    Url {
        #[serde(skip_serializing_if = "Option::is_none")]
        placeholder: Option<&'static str>,
    },
    /// Credential. Reads return [`SECRET_MASK`] plus the last four characters; a write
    /// carrying that mask keeps the stored value, and an empty string clears it.
    Secret {
        max_len: usize,
        #[serde(skip_serializing_if = "Option::is_none")]
        pattern: Option<Pattern>,
    },
    Enum {
        options: Vec<EnumOption>,
    },
    /// `data:image/…;base64,…` or `null`.
    ImageDataUrl {
        max_bytes: usize,
    },
    Object {
        fields: Vec<FieldSchema>,
    },
}

/// Leading mask on secrets read back from the service. Real credentials never contain it.
pub const SECRET_MASK: &str = "••••••••";

/// A JavaScript-compatible regular expression and the message shown when it fails.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Pattern {
    pub regex: &'static str,
    pub message: &'static str,
}

impl Pattern {
    pub const fn new(regex: &'static str, message: &'static str) -> Self {
        Self { regex, message }
    }

    fn check(&self, path: &str, text: &str) -> Result<(), String> {
        let matcher = regex::Regex::new(self.regex)
            .map_err(|err| format!("{path} has an invalid pattern: {err}"))?;
        if matcher.is_match(text) {
            Ok(())
        } else {
            Err(format!("{path}: {}", self.message))
        }
    }
}

/// Units the console can display and convert (e.g. hours shown as days).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IntegerUnit {
    Megabytes,
    Seconds,
    Hours,
    Items,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EnumOption {
    pub value: &'static str,
    pub label: &'static str,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FieldSchema {
    pub name: &'static str,
    pub label: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<&'static str>,
    /// Shown only while this sibling boolean field is `true`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visible_when: Option<&'static str>,
    /// Must be non-empty while this sibling boolean field is `true`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required_when: Option<&'static str>,
    pub schema: ValueSchema,
}

impl ValueSchema {
    pub fn integer(min: i64, max: i64, unit: IntegerUnit) -> Self {
        Self::Integer {
            min,
            max,
            unit: Some(unit),
        }
    }

    pub fn url(placeholder: &'static str) -> Self {
        Self::Url {
            placeholder: Some(placeholder),
        }
    }

    pub fn enabled_only() -> Self {
        Self::Object {
            fields: vec![FieldSchema::new("enabled", "启用", Self::Boolean)],
        }
    }

    /// Checks `value` against this shape; the message names the offending path.
    pub fn check(&self, value: &Value) -> Result<(), String> {
        self.check_at("value", value)
    }

    fn check_at(&self, path: &str, value: &Value) -> Result<(), String> {
        match self {
            Self::Boolean => value
                .as_bool()
                .map(|_| ())
                .ok_or_else(|| format!("{path} must be true or false")),
            Self::Integer { min, max, .. } => match value.as_i64() {
                Some(n) if (*min..=*max).contains(&n) => Ok(()),
                Some(_) => Err(format!("{path} must be between {min} and {max}")),
                None => Err(format!("{path} must be an integer")),
            },
            Self::Text {
                max_len, pattern, ..
            }
            | Self::Secret { max_len, pattern } => {
                let text = value
                    .as_str()
                    .ok_or_else(|| format!("{path} must be a string"))?;
                if text.chars().count() > *max_len {
                    return Err(format!("{path} must be at most {max_len} characters"));
                }
                if matches!(self, Self::Secret { .. }) && text.chars().any(char::is_whitespace) {
                    return Err(format!("{path} must not contain whitespace"));
                }
                match pattern {
                    Some(pattern) if !text.is_empty() => pattern.check(path, text),
                    _ => Ok(()),
                }
            }
            Self::Url { .. } => {
                let text = value
                    .as_str()
                    .ok_or_else(|| format!("{path} must be a string"))?;
                let rest = text
                    .strip_prefix("https://")
                    .or_else(|| text.strip_prefix("http://"))
                    .ok_or_else(|| format!("{path} must be an http(s) URL"))?;
                let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
                if host.is_empty() || text.chars().any(char::is_whitespace) || text.len() > 2048 {
                    return Err(format!("{path} must be an http(s) URL"));
                }
                Ok(())
            }
            Self::Enum { options } => {
                let text = value
                    .as_str()
                    .ok_or_else(|| format!("{path} must be a string"))?;
                if options.iter().any(|option| option.value == text) {
                    Ok(())
                } else {
                    let allowed: Vec<_> = options.iter().map(|option| option.value).collect();
                    Err(format!("{path} must be one of {}", allowed.join(", ")))
                }
            }
            Self::ImageDataUrl { max_bytes } => match value {
                Value::Null => Ok(()),
                Value::String(text) => {
                    if !text.starts_with("data:image/") || !text.contains(";base64,") {
                        return Err(format!("{path} must be a data:image/… base64 URL"));
                    }
                    if text.len() > *max_bytes {
                        return Err(format!("{path} must be at most {max_bytes} bytes"));
                    }
                    Ok(())
                }
                _ => Err(format!("{path} must be a data URL or null")),
            },
            Self::Object { fields } => {
                let object = value
                    .as_object()
                    .ok_or_else(|| format!("{path} must be an object"))?;
                if let Some(unknown) = object
                    .keys()
                    .find(|key| !fields.iter().any(|field| field.name == key.as_str()))
                {
                    return Err(format!("{path}.{unknown} is not a known field"));
                }
                for field in fields {
                    let child = format!("{path}.{}", field.name);
                    let value = object
                        .get(field.name)
                        .ok_or_else(|| format!("{child} is required"))?;
                    field.schema.check_at(&child, value)?;
                    let required = field
                        .required_when
                        .is_some_and(|flag| object.get(flag) == Some(&Value::Bool(true)));
                    if required && value.as_str().is_some_and(|text| text.trim().is_empty()) {
                        return Err(format!("{child} is required"));
                    }
                }
                Ok(())
            }
        }
    }
}

impl ValueSchema {
    /// The value with every secret replaced by its mask, safe to hand to a browser.
    pub fn mask_secrets(&self, value: &Value) -> Value {
        match (self, value) {
            (Self::Secret { .. }, Value::String(text)) if !text.is_empty() => {
                Value::String(mask(text))
            }
            (Self::Object { fields }, Value::Object(object)) => {
                let mut masked = object.clone();
                for field in fields {
                    if let Some(child) = object.get(field.name) {
                        masked.insert(field.name.to_string(), field.schema.mask_secrets(child));
                    }
                }
                Value::Object(masked)
            }
            _ => value.clone(),
        }
    }

    /// Puts the stored credential back wherever `incoming` echoes a mask.
    pub fn restore_secrets(&self, incoming: &mut Value, stored: Option<&Value>) {
        match (self, incoming) {
            (Self::Secret { .. }, Value::String(text)) if text.starts_with(SECRET_MASK) => {
                *text = stored
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
            }
            (Self::Object { fields }, Value::Object(object)) => {
                for field in fields {
                    if let Some(child) = object.get_mut(field.name) {
                        let previous = stored.and_then(|value| value.get(field.name));
                        field.schema.restore_secrets(child, previous);
                    }
                }
            }
            _ => {}
        }
    }
}

fn mask(secret: &str) -> String {
    let chars: Vec<char> = secret.chars().collect();
    if chars.len() <= 8 {
        return SECRET_MASK.to_string();
    }
    let tail: String = chars[chars.len() - 4..].iter().collect();
    format!("{SECRET_MASK}{tail}")
}

impl FieldSchema {
    pub fn new(name: &'static str, label: &'static str, schema: ValueSchema) -> Self {
        Self {
            name,
            label,
            description: None,
            visible_when: None,
            required_when: None,
            schema,
        }
    }

    pub fn describe(mut self, description: &'static str) -> Self {
        self.description = Some(description);
        self
    }

    pub fn visible_when(mut self, field: &'static str) -> Self {
        self.visible_when = Some(field);
        self
    }

    pub fn required_when(mut self, field: &'static str) -> Self {
        self.required_when = Some(field);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn scalars_enforce_type_and_bounds() {
        let mb = ValueSchema::integer(1, 1024, IntegerUnit::Megabytes);
        assert!(mb.check(&json!(16)).is_ok());
        assert!(mb.check(&json!(0)).is_err());
        assert!(mb.check(&json!(2048)).is_err());
        assert!(mb.check(&json!("16")).is_err());
        assert!(mb.check(&json!(1.5)).is_err());

        let url = ValueSchema::url("https://huggingface.co");
        assert!(url.check(&json!("https://hf-mirror.com")).is_ok());
        assert!(url.check(&json!("http://127.0.0.1:8080/api")).is_ok());
        assert!(url.check(&json!("ftp://x")).is_err());
        assert!(url.check(&json!("https://")).is_err());
        assert!(url.check(&json!("https://a b")).is_err());

        let hub = ValueSchema::Enum {
            options: vec![EnumOption {
                value: "huggingface",
                label: "Hugging Face",
            }],
        };
        assert!(hub.check(&json!("huggingface")).is_ok());
        assert!(hub.check(&json!("other")).is_err());

        let secret = ValueSchema::Secret {
            max_len: 4,
            pattern: None,
        };
        assert!(secret.check(&json!("")).is_ok());
        assert!(secret.check(&json!("12345")).is_err());

        let logo = ValueSchema::ImageDataUrl { max_bytes: 64 };
        assert!(logo.check(&json!(null)).is_ok());
        assert!(logo.check(&json!("data:image/png;base64,iVBOR")).is_ok());
        assert!(logo.check(&json!("https://x/logo.png")).is_err());
        assert!(logo
            .check(&json!(format!("data:image/png;base64,{}", "A".repeat(64))))
            .is_err());
    }

    #[test]
    fn objects_require_every_field_and_reject_unknown_ones() {
        let schema = ValueSchema::Object {
            fields: vec![
                FieldSchema::new("enabled", "启用", ValueSchema::Boolean),
                FieldSchema::new(
                    "message",
                    "提示",
                    ValueSchema::Text {
                        max_len: 5,
                        multiline: true,
                        placeholder: None,
                        pattern: None,
                    },
                )
                .visible_when("enabled"),
            ],
        };
        assert!(schema
            .check(&json!({ "enabled": true, "message": "hi" }))
            .is_ok());
        assert_eq!(
            schema.check(&json!({ "enabled": true })),
            Err("value.message is required".into())
        );
        assert_eq!(
            schema.check(&json!({ "enabled": "yes", "message": "" })),
            Err("value.enabled must be true or false".into())
        );
        assert!(schema
            .check(&json!({ "enabled": true, "message": "", "extra": 1 }))
            .is_err());
        assert!(schema.check(&json!([])).is_err());
    }

    #[test]
    fn serializes_as_tagged_json_for_the_console() {
        let schema = ValueSchema::Object {
            fields: vec![FieldSchema::new(
                "ttl",
                "TTL",
                ValueSchema::integer(1, 10, IntegerUnit::Hours),
            )
            .visible_when("enabled")],
        };
        assert_eq!(
            serde_json::to_value(&schema).unwrap(),
            json!({
                "type": "object",
                "fields": [{
                    "name": "ttl",
                    "label": "TTL",
                    "visible_when": "enabled",
                    "schema": { "type": "integer", "min": 1, "max": 10, "unit": "hours" }
                }]
            })
        );
    }

    #[test]
    fn patterns_and_required_when_guard_enabled_providers() {
        let schema = ValueSchema::Object {
            fields: vec![
                FieldSchema::new("enabled", "启用", ValueSchema::Boolean),
                FieldSchema::new(
                    "app_id",
                    "AppID",
                    ValueSchema::Text {
                        max_len: 18,
                        multiline: false,
                        placeholder: None,
                        pattern: Some(Pattern::new("^wx[0-9a-f]{16}$", "AppID 格式不正确")),
                    },
                )
                .required_when("enabled"),
            ],
        };
        assert!(schema
            .check(&json!({ "enabled": false, "app_id": "" }))
            .is_ok());
        assert_eq!(
            schema.check(&json!({ "enabled": true, "app_id": "  " })),
            Err("value.app_id: AppID 格式不正确".into())
        );
        assert_eq!(
            schema.check(&json!({ "enabled": true, "app_id": "" })),
            Err("value.app_id is required".into())
        );
        assert!(schema
            .check(&json!({ "enabled": true, "app_id": "wx0123456789abcdef" }))
            .is_ok());
        assert!(schema
            .check(&json!({ "enabled": false, "app_id": "wx123" }))
            .is_err());

        let secret = ValueSchema::Secret {
            max_len: 64,
            pattern: None,
        };
        assert!(secret.check(&json!("abc def")).is_err());
    }

    #[test]
    fn secrets_leave_masked_and_come_back_as_the_stored_value() {
        let schema = ValueSchema::Object {
            fields: vec![
                FieldSchema::new("enabled", "启用", ValueSchema::Boolean),
                FieldSchema::new(
                    "secret",
                    "Secret",
                    ValueSchema::Secret {
                        max_len: 64,
                        pattern: None,
                    },
                ),
            ],
        };
        let stored = json!({ "enabled": true, "secret": "0123456789abcdef" });
        let masked = schema.mask_secrets(&stored);
        assert_eq!(masked, json!({ "enabled": true, "secret": "••••••••cdef" }));
        assert_eq!(
            schema.mask_secrets(&json!({ "enabled": true, "secret": "" })),
            json!({ "enabled": true, "secret": "" })
        );

        let mut echoed = json!({ "enabled": false, "secret": "••••••••cdef" });
        schema.restore_secrets(&mut echoed, Some(&stored));
        assert_eq!(
            echoed,
            json!({ "enabled": false, "secret": "0123456789abcdef" })
        );

        let mut replaced = json!({ "enabled": true, "secret": "new" });
        schema.restore_secrets(&mut replaced, Some(&stored));
        assert_eq!(replaced["secret"], "new");

        let mut first = json!({ "enabled": true, "secret": "••••••••" });
        schema.restore_secrets(&mut first, None);
        assert_eq!(first["secret"], "");
    }
}
