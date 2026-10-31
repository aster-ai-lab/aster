//! Work Connect ApiEnvelope errors for managed inference admission.

use bytes::Bytes;
use http::header::WWW_AUTHENTICATE;
use http::{HeaderValue, Response, StatusCode};
use uuid::Uuid;

use super::work_envelope::error_envelope_response;

/// Stable native inference authorization and admission failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum InferenceAccessError {
    /// No valid inference credential was presented.
    Unauthorized,
    /// The credential was explicitly revoked.
    Revoked,
    /// The authenticated credential has no matching route, endpoint, or model grant.
    Denied,
    /// Path matched an inference router but is not a managed OpenAI endpoint.
    UnsupportedEndpoint,
    /// The complete authorization snapshot or local verifier is unavailable.
    Unavailable,
    /// Required durable usage evidence cannot be accepted locally.
    UsageUnavailable,
    /// The credential grant has exhausted its local request-rate allowance.
    RateLimited { retry_after_secs: u64 },
    /// The credential grant has exhausted its local rolling spend budget.
    SpendLimited { retry_after_secs: u64 },
    /// The credential grant has reached its local in-flight request cap.
    ConcurrencyLimited,
    /// The model pool's bounded waiting room is full.
    PoolQueueFull,
    /// The model pool could not admit the request before its queue deadline.
    PoolQueueTimeout,
    /// A cross-protocol target refused to translate this request body. The
    /// field-level reason is logged at the dispatch site; the wire keeps a
    /// stable code.
    TranslationRefused,
    /// The grant's guardrails policy blocked this body (request leg) or
    /// withheld the completion (response leg).
    GuardrailBlocked { reason: String },
    /// Streaming over a cross-protocol target is not available yet.
    CrossProtocolStreaming,
    /// Compiled fallback candidates were exhausted without a usable upstream.
    UpstreamExhausted,
}

impl InferenceAccessError {
    /// Convert a failure into a Work Connect ApiEnvelope response.
    pub(crate) fn into_response(self) -> Response<Bytes> {
        self.into_response_with_request_id(Uuid::new_v4())
    }

    /// Same as [`Self::into_response`], correlating with an existing request id.
    pub(crate) fn into_response_with_request_id(self, request_id: Uuid) -> Response<Bytes> {
        let (status, message, error_code, retry_after_secs) = match self {
            Self::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "Invalid authentication credentials.",
                "auth.invalid_key",
                None,
            ),
            Self::Revoked => (
                StatusCode::UNAUTHORIZED,
                "Inference credential has been revoked.",
                "auth.revoked",
                None,
            ),
            Self::Denied => (
                StatusCode::FORBIDDEN,
                "Listing or model is not entitled for this credential.",
                "entitlement.denied",
                None,
            ),
            Self::UnsupportedEndpoint => (
                StatusCode::NOT_FOUND,
                "Path is not a managed inference endpoint.",
                "request.invalid",
                None,
            ),
            Self::Unavailable => (
                StatusCode::SERVICE_UNAVAILABLE,
                "Inference route or snapshot is not ready.",
                "route.not_ready",
                None,
            ),
            Self::UsageUnavailable => (
                StatusCode::SERVICE_UNAVAILABLE,
                "Durable inference usage is temporarily unavailable.",
                "route.not_ready",
                None,
            ),
            Self::RateLimited { retry_after_secs } => (
                StatusCode::TOO_MANY_REQUESTS,
                "Inference request rate or quota exceeded.",
                "quota.exceeded",
                Some(retry_after_secs),
            ),
            Self::SpendLimited { retry_after_secs } => (
                StatusCode::TOO_MANY_REQUESTS,
                "Inference spend budget exceeded.",
                "spend.exceeded",
                Some(retry_after_secs),
            ),
            Self::ConcurrencyLimited => (
                StatusCode::TOO_MANY_REQUESTS,
                "Inference concurrency limit exceeded.",
                "quota.exceeded",
                Some(1),
            ),
            Self::PoolQueueFull => (
                StatusCode::TOO_MANY_REQUESTS,
                "Inference pool queue is full.",
                "quota.exceeded",
                Some(1),
            ),
            Self::TranslationRefused => (
                StatusCode::BAD_REQUEST,
                "Inference request cannot be translated for the selected target.",
                "translation.refused",
                None,
            ),
            Self::GuardrailBlocked { .. } => (
                StatusCode::FORBIDDEN,
                "Inference request blocked by the guardrails policy.",
                "guardrail.blocked",
                None,
            ),
            Self::CrossProtocolStreaming => (
                StatusCode::BAD_REQUEST,
                "Streaming over a cross-protocol target is not supported.",
                "translation.streaming_unsupported",
                None,
            ),
            Self::PoolQueueTimeout => (
                StatusCode::TOO_MANY_REQUESTS,
                "Inference pool queue deadline elapsed.",
                "quota.exceeded",
                Some(1),
            ),
            Self::UpstreamExhausted => (
                StatusCode::BAD_GATEWAY,
                "Upstream providers exhausted after compiled fallback.",
                "upstream.exhausted",
                None,
            ),
        };
        let mut response = error_envelope_response(request_id, status, message, error_code);
        if matches!(self, Self::Unauthorized | Self::Revoked) {
            response.headers_mut().insert(
                WWW_AUTHENTICATE,
                HeaderValue::from_static(r#"Bearer realm="aster-inference""#),
            );
        }
        if let Some(retry_after_secs) = retry_after_secs {
            if let Ok(retry_after) = HeaderValue::from_str(&retry_after_secs.to_string()) {
                response.headers_mut().insert("retry-after", retry_after);
            }
        }
        response
    }
}
