//! Unified Work data-plane path helpers.

use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceKind {
    Model,
    Tool,
    Api,
    Feed,
    Agent,
}

impl ServiceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Model => "model",
            Self::Tool => "tool",
            Self::Api => "api",
            Self::Feed => "feed",
            Self::Agent => "agent",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "model" => Some(Self::Model),
            "tool" => Some(Self::Tool),
            "api" => Some(Self::Api),
            "feed" => Some(Self::Feed),
            "agent" => Some(Self::Agent),
            _ => None,
        }
    }
}

pub fn service_invoke_path(kind: ServiceKind, sku: &str) -> String {
    format!("/v1/api/services/{}/invoke/{}", kind.as_str(), sku)
}

pub fn parse_service_input(body: &Value) -> Result<serde_json::Map<String, Value>, String> {
    let obj = body
        .as_object()
        .ok_or_else(|| "body must be a JSON object with input".to_string())?;
    let input = obj
        .get("input")
        .and_then(|v| v.as_object())
        .ok_or_else(|| "body.input must be a JSON object".to_string())?;
    Ok(input.clone())
}
