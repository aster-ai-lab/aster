//! Explicit degrade middleware — short-circuit with a static fallback response.
//!
//! Use this when an operator wants a route to return a known-good degraded
//! payload instead of contacting upstream (maintenance, feature kill-switch,
//! or intentional shed). Circuit breaking and failover remain separate
//! policies; this middleware is the deliberate degrade path.

use crate::config::MiddlewareConfig;
use crate::error::{GatewayError, Result};
use crate::middleware::{Middleware, RequestContext};
use async_trait::async_trait;
use http::{header, HeaderValue, Response, StatusCode};

/// Static fallback / degrade response middleware.
pub struct FallbackMiddleware {
    status: StatusCode,
    body: Vec<u8>,
    content_type: String,
}

impl FallbackMiddleware {
    pub fn try_new(config: &MiddlewareConfig) -> Result<Self> {
        let status_code = config.status_code.unwrap_or(503);
        let status = StatusCode::from_u16(status_code as u16).map_err(|_| {
            GatewayError::Config(format!(
                "fallback middleware status_code {status_code} is not a valid HTTP status"
            ))
        })?;
        let body = config
            .value
            .clone()
            .unwrap_or_else(|| r#"{"error":"service degraded"}"#.to_string())
            .into_bytes();
        let content_type = config
            .header
            .clone()
            .unwrap_or_else(|| "application/json".to_string());
        Ok(Self {
            status,
            body,
            content_type,
        })
    }
}

#[async_trait]
impl Middleware for FallbackMiddleware {
    async fn handle_request(
        &self,
        _req: &mut http::request::Parts,
        _ctx: &RequestContext,
    ) -> Result<Option<Response<Vec<u8>>>> {
        let mut response = Response::builder()
            .status(self.status)
            .body(self.body.clone())
            .map_err(|error| {
                GatewayError::Config(format!("fallback response build failed: {error}"))
            })?;
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_str(&self.content_type).map_err(|error| {
                GatewayError::Config(format!(
                    "fallback middleware content type is invalid: {error}"
                ))
            })?,
        );
        response.headers_mut().insert(
            header::HeaderName::from_static("x-aster-degraded"),
            HeaderValue::from_static("1"),
        );
        Ok(Some(response))
    }

    fn name(&self) -> &str {
        "fallback"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::MiddlewareConfig;

    #[tokio::test]
    async fn fallback_short_circuits_with_configured_body() {
        let mw = FallbackMiddleware::try_new(&MiddlewareConfig {
            middleware_type: "fallback".to_string(),
            status_code: Some(503),
            value: Some(r#"{"error":"maintenance"}"#.to_string()),
            header: Some("application/json".to_string()),
            ..MiddlewareConfig::default()
        })
        .unwrap();

        let mut parts = http::Request::builder()
            .uri("/anything")
            .body(())
            .unwrap()
            .into_parts()
            .0;
        let ctx = RequestContext {
            client_ip: "127.0.0.1".into(),
            entrypoint: "web".into(),
            router: "api".into(),
        };
        let response = mw
            .handle_request(&mut parts, &ctx)
            .await
            .unwrap()
            .expect("short-circuit");
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(response.headers().get("x-aster-degraded").unwrap(), "1");
        assert_eq!(response.body(), br#"{"error":"maintenance"}"#);
    }
}
