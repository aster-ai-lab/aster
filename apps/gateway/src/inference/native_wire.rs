//! Native OpenAI / Anthropic wire rendering for `wire = "openai"` routes.
//!
//! Gateway-originated inference errors are built once as Work Connect
//! envelopes and tagged with [`EnvelopeError`]. When the matched route speaks
//! the native wire, the request handler marks the task with
//! [`mark_native_wire`] and the outermost layer re-renders tagged errors in
//! the shape SDKs parse.

use bytes::Bytes;
use http::StatusCode;
use serde_json::json;
use std::cell::Cell;
use std::future::Future;

/// Error facts retained on an envelope response for native re-rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct EnvelopeError {
    pub(crate) status: StatusCode,
    pub(crate) message: &'static str,
    pub(crate) error_code: &'static str,
}

/// Native error dialect selected by the request path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NativeWire {
    OpenAi,
    Anthropic,
}

tokio::task_local! {
    static NATIVE_WIRE: Cell<Option<NativeWire>>;
}

/// Run one request handler with a fresh wire marker and return the marker
/// set while it ran.
pub(crate) async fn with_native_wire_scope<F: Future>(
    future: F,
) -> (F::Output, Option<NativeWire>) {
    NATIVE_WIRE
        .scope(Cell::new(None), async move {
            let output = future.await;
            (output, NATIVE_WIRE.with(Cell::get))
        })
        .await
}

/// Mark the current request as served on the native wire.
pub(crate) fn mark_native_wire(wire: NativeWire) {
    let _ = NATIVE_WIRE.try_with(|marker| marker.set(Some(wire)));
}

/// Render one Gateway error in the native dialect.
pub(crate) fn render_native_error(wire: NativeWire, error: &EnvelopeError) -> Bytes {
    let body = match wire {
        NativeWire::OpenAi => {
            let (kind, code) = match error.status.as_u16() {
                401 => ("authentication_error", "invalid_api_key"),
                403 => ("permission_error", "model_not_found"),
                404 => ("invalid_request_error", "unknown_url"),
                429 => ("rate_limit_error", "rate_limit_exceeded"),
                400 | 413 | 415 => ("invalid_request_error", "invalid_request"),
                _ => ("api_error", "service_unavailable"),
            };
            json!({
                "error": {
                    "message": error.message,
                    "type": kind,
                    "param": null,
                    "code": code,
                    "aster_error_code": error.error_code,
                }
            })
        }
        NativeWire::Anthropic => {
            let kind = match error.status.as_u16() {
                401 => "authentication_error",
                403 => "permission_error",
                404 => "not_found_error",
                413 => "request_too_large",
                429 => "rate_limit_error",
                400 | 415 => "invalid_request_error",
                503 => "overloaded_error",
                _ => "api_error",
            };
            json!({
                "type": "error",
                "error": { "type": kind, "message": error.message },
            })
        }
    };
    Bytes::from(serde_json::to_vec(&body).unwrap_or_default())
}

/// Native OpenAI model list.
pub(crate) fn native_models_body(models: &[String]) -> Bytes {
    let data = models
        .iter()
        .map(|model| {
            json!({
                "id": model,
                "object": "model",
                "created": 0,
                "owned_by": "aster"
            })
        })
        .collect::<Vec<_>>();
    Bytes::from(serde_json::to_vec(&json!({ "object": "list", "data": data })).unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn error(status: StatusCode) -> EnvelopeError {
        EnvelopeError {
            status,
            message: "nope",
            error_code: "auth.invalid_key",
        }
    }

    #[test]
    fn openai_errors_use_the_sdk_shape() {
        let body: serde_json::Value = serde_json::from_slice(&render_native_error(
            NativeWire::OpenAi,
            &error(StatusCode::UNAUTHORIZED),
        ))
        .unwrap();
        assert_eq!(body["error"]["message"], "nope");
        assert_eq!(body["error"]["type"], "authentication_error");
        assert_eq!(body["error"]["code"], "invalid_api_key");
        assert!(body.get("success").is_none());
    }

    #[test]
    fn anthropic_errors_use_the_sdk_shape() {
        let body: serde_json::Value = serde_json::from_slice(&render_native_error(
            NativeWire::Anthropic,
            &error(StatusCode::TOO_MANY_REQUESTS),
        ))
        .unwrap();
        assert_eq!(body["type"], "error");
        assert_eq!(body["error"]["type"], "rate_limit_error");
    }

    #[tokio::test]
    async fn marker_is_scoped_to_one_request() {
        let ((), wire) = with_native_wire_scope(async {
            mark_native_wire(NativeWire::Anthropic);
        })
        .await;
        assert_eq!(wire, Some(NativeWire::Anthropic));
        let ((), wire) = with_native_wire_scope(async {}).await;
        assert_eq!(wire, None);
        mark_native_wire(NativeWire::OpenAi);
    }
}
