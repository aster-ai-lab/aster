//! ApiEnvelope — aligned with apps/api docs/API.md + WORK-CONNECT.

use axum::http::HeaderMap;
use chrono::Utc;
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
pub struct ApiEnvelope<T: Serialize> {
    pub success: bool,
    pub code: i64,
    pub message: String,
    pub data: Option<T>,
    pub request_id: String,
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<Vec<FieldError>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FieldError {
    pub field: String,
    pub message: String,
}

pub fn ok_envelope<T: Serialize>(data: T, request_id: &str) -> ApiEnvelope<T> {
    ok_envelope_msg(data, request_id, "OK")
}

pub fn ok_envelope_msg<T: Serialize>(data: T, request_id: &str, message: &str) -> ApiEnvelope<T> {
    ApiEnvelope {
        success: true,
        code: 0,
        message: message.to_string(),
        data: Some(data),
        request_id: request_id.to_string(),
        timestamp: Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        errors: None,
        error_code: None,
    }
}

pub struct ErrBuilt {
    pub status: u16,
    pub body: ApiEnvelope<()>,
}

pub fn err_envelope(
    request_id: &str,
    http_hint: u16,
    message: impl Into<String>,
    error_code: impl Into<String>,
) -> ErrBuilt {
    let code = match http_hint {
        401 => 40100,
        403 => 40300,
        404 => 40400,
        429 => 42900,
        400 => 40000,
        _ => 50000,
    };
    ErrBuilt {
        status: http_hint,
        body: ApiEnvelope {
            success: false,
            code,
            message: message.into(),
            data: None,
            request_id: request_id.to_string(),
            timestamp: Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            errors: None,
            error_code: Some(error_code.into()),
        },
    }
}

pub fn request_id_from(headers: &HeaderMap) -> String {
    if let Some(h) = headers.get("x-request-id").and_then(|v| v.to_str().ok()) {
        let t = h.trim();
        if Uuid::parse_str(t).is_ok() {
            return t.to_string();
        }
    }
    Uuid::new_v4().to_string()
}
