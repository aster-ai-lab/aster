//! Classify Connect errors without exposing opaque details or bearer values.

use anyhow::anyhow;
use serde_json::Value;

pub(super) fn connect(error: &Value, secret: &str) -> anyhow::Error {
    let code = error
        .get("code")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let details = error
        .get("details")
        .and_then(Value::as_array)
        .and_then(|details| {
            details.iter().find(|detail| {
                detail.get("type").and_then(Value::as_str) == Some("aiserver.v1.ErrorDetails")
            })
        })
        .and_then(|detail| detail.pointer("/debug/details"));
    let title = details
        .and_then(|details| details.get("title"))
        .and_then(Value::as_str);
    let detail = details
        .and_then(|details| details.get("detail"))
        .and_then(Value::as_str);
    let message = format!(
        "cursor account error: {}{}{}",
        match code {
            "invalid_argument"
            | "unauthenticated"
            | "permission_denied"
            | "failed_precondition"
            | "not_found"
            | "unimplemented"
            | "resource_exhausted"
            | "unavailable"
            | "internal"
            | "deadline_exceeded"
            | "cancelled"
            | "aborted"
            | "out_of_range"
            | "data_loss"
            | "unknown" => code,
            _ => "unknown",
        },
        title
            .map(|title| format!("; {}", super::redact(title, secret)))
            .unwrap_or_default(),
        detail
            .map(|detail| format!("; {}", super::redact(detail, secret)))
            .unwrap_or_default(),
    );
    if matches!(
        code,
        "invalid_argument"
            | "unauthenticated"
            | "permission_denied"
            | "failed_precondition"
            | "not_found"
            | "unimplemented"
    ) || details
        .and_then(|details| details.get("isRetryable"))
        .and_then(Value::as_bool)
        == Some(false)
    {
        anyhow!(super::super::NonRetryableLlmError::new(message))
    } else {
        anyhow!(message)
    }
}
