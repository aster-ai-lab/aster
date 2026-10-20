//! Axum router for Gateway `/v1` product data-plane.

use std::time::Instant;

use axum::extract::{Path, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde_json::{json, Value};
use tower::ServiceBuilder;
use tower_http_cors_lite::CorsLayer;

use crate::auth::authenticate;
use crate::entitlement::{
    is_entitled_api, is_entitled_model, spend_cap_usd, ENTITLED_API_SKUS, ENTITLED_MODEL_IDS,
    ENTITLED_TOOL_SKUS,
};
use crate::envelope::{err_envelope, ok_envelope, request_id_from, ApiEnvelope};
use crate::providers::platform::deliver_platform_model;
use crate::routes::{parse_service_input, service_invoke_path, ServiceKind};
use crate::usage::{UsageInput, UsageStatus, ACCEPTED_UNITS};
use crate::vault::{is_loopback_host, VaultProviderId, VaultScope};
use crate::{AppState, SCOPE_ENV, SCOPE_ORG, SCOPE_PROJECT};

/// Tiny CORS without tower-http dep — middleware via axum layer.
mod tower_http_cors_lite {
    use axum::http::{header, HeaderValue, Method};
    use tower::{Layer, Service};

    #[derive(Clone, Default)]
    pub struct CorsLayer;

    impl<S> Layer<S> for CorsLayer {
        type Service = CorsService<S>;
        fn layer(&self, inner: S) -> Self::Service {
            CorsService { inner }
        }
    }

    #[derive(Clone)]
    pub struct CorsService<S> {
        inner: S,
    }

    impl<S, ReqBody> Service<axum::http::Request<ReqBody>> for CorsService<S>
    where
        S: Service<axum::http::Request<ReqBody>, Response = axum::response::Response>
            + Clone
            + Send
            + 'static,
        S::Future: Send + 'static,
        ReqBody: Send + 'static,
    {
        type Response = S::Response;
        type Error = S::Error;
        type Future = std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>,
        >;

        fn poll_ready(
            &mut self,
            cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Result<(), Self::Error>> {
            self.inner.poll_ready(cx)
        }

        fn call(&mut self, req: axum::http::Request<ReqBody>) -> Self::Future {
            let mut inner = self.inner.clone();
            Box::pin(async move {
                if req.method() == Method::OPTIONS {
                    let mut res = axum::response::Response::new(axum::body::Body::empty());
                    *res.status_mut() = axum::http::StatusCode::NO_CONTENT;
                    apply_cors(res.headers_mut());
                    return Ok(res);
                }
                let mut res = inner.call(req).await?;
                apply_cors(res.headers_mut());
                Ok(res)
            })
        }
    }

    fn apply_cors(headers: &mut axum::http::HeaderMap) {
        headers.insert(
            header::ACCESS_CONTROL_ALLOW_ORIGIN,
            HeaderValue::from_static("*"),
        );
        headers.insert(
            header::ACCESS_CONTROL_ALLOW_METHODS,
            HeaderValue::from_static("GET,POST,PUT,OPTIONS"),
        );
        headers.insert(
            header::ACCESS_CONTROL_ALLOW_HEADERS,
            HeaderValue::from_static("Authorization,Content-Type,Accept,x-request-id,x-api-key"),
        );
    }
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/v1/api/health", get(health))
        .route("/v1/api/services/models", get(models))
        .route("/v1/api/models", get(models))
        .route("/v1/api/usage", get(usage))
        .route("/v1/api/usage/batch", post(usage_batch))
        .route("/v1/api/analytics", get(analytics))
        .route("/v1/api/events/recent", get(events_recent))
        .route(
            "/v1/api/services/{kind}/invoke",
            post(service_invoke_with_parameters),
        )
        .route(
            "/v1/api/services/{kind}/invoke/{*sku}",
            post(service_invoke),
        )
        .route("/v1/api/__local/vault/materialize", post(vault_materialize))
        .route("/v1/api/__local/vault/status", get(vault_status))
        .route("/v1/api/__local/vault/receipts", get(vault_receipts))
        .route("/v1/api/__local/queue/vault-apply", post(vault_apply))
        .route("/v1/api/__local/upstream-secrets", put(upstream_secrets))
        // aliases without /v1 prefix (Console loopback)
        .route("/__local/vault/materialize", post(vault_materialize))
        .route("/__local/vault/status", get(vault_status))
        .route("/__local/vault/receipts", get(vault_receipts))
        .route("/__local/queue/vault-apply", post(vault_apply))
        .route("/__local/upstream-secrets", put(upstream_secrets))
        .fallback(not_found)
        .layer(ServiceBuilder::new().layer(CorsLayer))
        .with_state(state)
}

fn json_ok<T: serde::Serialize>(
    status: StatusCode,
    body: ApiEnvelope<T>,
    request_id: &str,
) -> Response {
    let mut res = (status, Json(body)).into_response();
    if let Ok(v) = HeaderValue::from_str(request_id) {
        res.headers_mut().insert("x-request-id", v);
    }
    res
}

fn json_err(built: crate::envelope::ErrBuilt) -> Response {
    let status = StatusCode::from_u16(built.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let rid = built.body.request_id.clone();
    json_ok(status, built.body, &rid)
}

fn require_auth(headers: &HeaderMap, request_id: &str) -> Result<crate::auth::AuthOk, Response> {
    authenticate(headers).map_err(|e| json_err(err_envelope(request_id, 401, e.message, e.code)))
}

fn is_loopback(headers: &HeaderMap) -> bool {
    if let Some(host) = headers.get("host").and_then(|v| v.to_str().ok()) {
        if is_loopback_host(host) {
            return true;
        }
    }
    if let Some(xf) = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(str::trim)
    {
        if xf == "127.0.0.1" || xf == "::1" {
            return true;
        }
    }
    false
}

async fn health(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let request_id = request_id_from(&headers);
    let vault = state.vault.read().await;
    let body = ok_envelope(
        json!({
            "service": "a3s-runtime",
            "role": "data-plane-runtime",
            "entitled_tools": ENTITLED_TOOL_SKUS.len(),
            "entitled_apis": ENTITLED_API_SKUS.len(),
            "entitled_models": ENTITLED_MODEL_IDS.len(),
            "boyue_vault": vault.get_credential(VaultProviderId::Boyue, None).is_some(),
            "siliconflow_vault": vault.get_credential(VaultProviderId::Siliconflow, None).is_some(),
            "vault": vault.status(None),
        }),
        &request_id,
    );
    json_ok(StatusCode::OK, body, &request_id)
}

async fn models(State(_state): State<AppState>, headers: HeaderMap) -> Response {
    let request_id = request_id_from(&headers);
    if let Err(res) = require_auth(&headers, &request_id) {
        return res;
    }
    let data: Vec<Value> = ENTITLED_MODEL_IDS
        .iter()
        .map(|id| {
            json!({
                "id": id,
                "object": "model",
                "created": 0,
                "owned_by": "a3s",
            })
        })
        .collect();
    json_ok(
        StatusCode::OK,
        ok_envelope(json!({ "object": "list", "data": data }), &request_id),
        &request_id,
    )
}

async fn usage(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let request_id = request_id_from(&headers);
    let auth = match require_auth(&headers, &request_id) {
        Ok(a) => a,
        Err(res) => return res,
    };
    let store = state.usage.read().await;
    let items = store.list(100);
    let spend = store.spend_for_prefix(&auth.prefix);
    json_ok(
        StatusCode::OK,
        ok_envelope(
            json!({
                "items": items,
                "spend_usd": (spend * 1_000_000.0).round() / 1_000_000.0,
                "spend_cap_usd": spend_cap_usd(),
                "entitled_tools": ENTITLED_TOOL_SKUS,
                "entitled_apis": ENTITLED_API_SKUS,
                "entitled_models": ENTITLED_MODEL_IDS,
                "lifecycle_schema": "a3s.gateway.usage-lifecycle.v1",
                "batch_schema": "a3s.gateway.usage-batch.v1",
                "lifecycle_events": 0,
            }),
            &request_id,
        ),
        &request_id,
    )
}

async fn usage_batch(
    State(_state): State<AppState>,
    headers: HeaderMap,
    body: Option<Json<Value>>,
) -> Response {
    let request_id = request_id_from(&headers);
    if let Err(res) = require_auth(&headers, &request_id) {
        return res;
    }
    // Loose validation / stub: accept empty or object; return empty batch when nothing to export.
    if let Some(Json(v)) = &body {
        if !(v.is_null() || v.is_object() || v.is_array()) {
            return json_err(err_envelope(
                &request_id,
                400,
                "body must be JSON object or array",
                "request.invalid",
            ));
        }
    }
    json_ok(
        StatusCode::OK,
        ok_envelope(
            json!({
                "schema": "a3s.gateway.usage-batch.v1",
                "gateway_id": "a3s-runtime",
                "batch_id": request_id,
                "records": [],
                "detail": "stub: lifecycle export not yet wired in Rust runtime",
            }),
            &request_id,
        ),
        &request_id,
    )
}

async fn analytics(State(_state): State<AppState>, headers: HeaderMap) -> Response {
    let request_id = request_id_from(&headers);
    if let Err(res) = require_auth(&headers, &request_id) {
        return res;
    }
    json_ok(
        StatusCode::OK,
        ok_envelope(
            json!({
                "schema": "a3s.os.api-analytics.v1",
                "items": [],
            }),
            &request_id,
        ),
        &request_id,
    )
}

async fn events_recent(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let request_id = request_id_from(&headers);
    let items = state.events.read().await.recent(50);
    json_ok(
        StatusCode::OK,
        ok_envelope(json!({ "items": items }), &request_id),
        &request_id,
    )
}

async fn service_invoke(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((kind_raw, sku_raw)): Path<(String, String)>,
    Json(body): Json<Value>,
) -> Response {
    invoke_service(
        state,
        headers,
        kind_raw,
        urlencoding_decode(sku_raw.trim()),
        body,
    )
    .await
}

/// `POST /v1/api/services/{kind}/invoke` with the listing sku in additional parameters.
async fn service_invoke_with_parameters(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(kind_raw): Path<String>,
    Json(body): Json<Value>,
) -> Response {
    let request_id = request_id_from(&headers);
    let Some(sku) = sku_from_additional_parameters(&body) else {
        return json_err(err_envelope(
            &request_id,
            400,
            "sku is required in additional parameters",
            "request.invalid",
        ));
    };
    invoke_service(state, headers, kind_raw, sku, body).await
}

fn sku_from_additional_parameters(body: &Value) -> Option<String> {
    let obj = body.as_object()?;
    let direct = obj
        .get("sku")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|sku| !sku.is_empty());
    if let Some(sku) = direct {
        return Some(sku.to_string());
    }
    for key in ["params", "extra", "extras"] {
        let nested = obj
            .get(key)
            .and_then(|value| value.as_object())
            .and_then(|value| value.get("sku"))
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|sku| !sku.is_empty());
        if let Some(sku) = nested {
            return Some(sku.to_string());
        }
    }
    None
}

async fn invoke_service(
    state: AppState,
    headers: HeaderMap,
    kind_raw: String,
    sku: String,
    body: Value,
) -> Response {
    let request_id = request_id_from(&headers);
    let kind = match ServiceKind::parse(kind_raw.trim()) {
        Some(k) => k,
        None => {
            return json_err(err_envelope(
                &request_id,
                404,
                format!("no route: POST /v1/api/services/{kind_raw}/invoke"),
                "route.not_ready",
            ));
        }
    };
    let route = service_invoke_path(kind, &sku);

    if kind == ServiceKind::Model {
        return handle_model_invoke(state, headers, sku, route, request_id, body).await;
    }

    let auth = match require_auth(&headers, &request_id) {
        Ok(a) => a,
        Err(res) => return res,
    };

    if kind == ServiceKind::Agent {
        return json_err(err_envelope(
            &request_id,
            404,
            format!("no provider for sku: {sku}"),
            "route.not_ready",
        ));
    }

    if let Err(msg) = parse_service_input(&body) {
        return json_err(err_envelope(&request_id, 400, msg, "request.invalid"));
    }

    if kind == ServiceKind::Tool {
        record_denied(
            &state,
            &request_id,
            &sku,
            "tool",
            &auth.prefix,
            &route,
            "entitlement.denied",
            None,
        )
        .await;
        return json_err(err_envelope(
            &request_id,
            403,
            format!("SKU not in tool entitlement set: {sku}"),
            "entitlement.denied",
        ));
    }

    if !is_entitled_api(&sku) {
        let listing = if kind == ServiceKind::Feed {
            "feed"
        } else {
            "api"
        };
        record_denied(
            &state,
            &request_id,
            &sku,
            listing,
            &auth.prefix,
            &route,
            "entitlement.denied",
            None,
        )
        .await;
        return json_err(err_envelope(
            &request_id,
            403,
            format!("SKU not in entitlement set: {sku}"),
            "entitlement.denied",
        ));
    }

    if let Some(res) = check_quota(&state, &auth.prefix, &request_id, &sku, &route).await {
        return res;
    }

    json_err(err_envelope(
        &request_id,
        404,
        format!("no provider for sku: {sku}"),
        "route.not_ready",
    ))
}

async fn handle_model_invoke(
    state: AppState,
    headers: HeaderMap,
    model: String,
    route: String,
    request_id: String,
    body: Value,
) -> Response {
    let started = Instant::now();
    let auth = match require_auth(&headers, &request_id) {
        Ok(a) => a,
        Err(res) => return res,
    };

    if !is_entitled_model(&model) {
        let transaction_id = headers
            .get("x-transaction-id")
            .and_then(|value| value.to_str().ok())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| request_id.clone());
        match deliver_platform_model(&state.http, &auth.key, &model, &transaction_id, &body).await {
            Ok(delivery) if delivery.listed && delivery.available => {
                record_ok_once(
                    &state,
                    &transaction_id,
                    &model,
                    "model",
                    &auth.prefix,
                    &route,
                    ACCEPTED_UNITS,
                    Some(elapsed_ms(started)),
                )
                .await;
                return json_ok(
                    StatusCode::OK,
                    ok_envelope(
                        json!({ "body": delivery.body, "available": true }),
                        &request_id,
                    ),
                    &request_id,
                );
            }
            Ok(delivery) if delivery.listed => {
                let reason = if delivery.reason.is_empty() {
                    "no worker".to_string()
                } else {
                    delivery.reason
                };
                return json_ok(
                    StatusCode::SERVICE_UNAVAILABLE,
                    ApiEnvelope {
                        success: false,
                        code: 50300,
                        message: reason.clone(),
                        data: Some(json!({
                            "available": false,
                            "body": "",
                            "reason": reason,
                        })),
                        request_id: request_id.clone(),
                        timestamp: chrono::Utc::now()
                            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
                        errors: None,
                        error_code: Some("inference.unavailable".into()),
                    },
                    &request_id,
                );
            }
            Ok(_) => {}
            Err(error) => {
                let message = match error {
                    crate::providers::ProviderErr::Status { message, .. } => message,
                };
                tracing::warn!(error = %message, "platform model delivery skipped");
            }
        }
        record_denied(
            &state,
            &request_id,
            if model.is_empty() { "unknown" } else { &model },
            "model",
            &auth.prefix,
            &route,
            "entitlement.denied",
            None,
        )
        .await;
        return json_err(err_envelope(
            &request_id,
            403,
            format!(
                "model not in entitlement set: {}",
                if model.is_empty() { "(empty)" } else { &model }
            ),
            "entitlement.denied",
        ));
    }

    if let Some(res) = check_quota(&state, &auth.prefix, &request_id, &model, &route).await {
        return res;
    }

    json_err(err_envelope(
        &request_id,
        404,
        format!("no model provider for sku: {model}"),
        "route.not_ready",
    ))
}

async fn vault_materialize(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    let request_id = request_id_from(&headers);
    if !is_loopback(&headers) {
        return json_err(err_envelope(
            &request_id,
            403,
            "vault materialize is loopback-only",
            "auth.forbidden",
        ));
    }
    let row = body.as_object().cloned().unwrap_or_default();
    let scope_raw = row
        .get("scope")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();
    let provider = row
        .get("provider")
        .and_then(|v| v.as_str())
        .and_then(VaultProviderId::parse);
    let Some(provider) = provider else {
        return json_err(err_envelope(
            &request_id,
            400,
            "provider must be boyue|siliconflow|worldmonitor",
            "request.invalid",
        ));
    };
    let scope = VaultScope {
        organization_id: scope_raw
            .get("organization_id")
            .and_then(|v| v.as_str())
            .unwrap_or("org_local")
            .to_string(),
        project_id: scope_raw
            .get("project_id")
            .and_then(|v| v.as_str())
            .unwrap_or("proj_local")
            .to_string(),
        environment_id: scope_raw
            .get("environment_id")
            .and_then(|v| v.as_str())
            .unwrap_or("env_local")
            .to_string(),
    };
    let secret_ref = row.get("secret_ref").and_then(|v| v.as_str()).unwrap_or("");
    let version = row.get("version").and_then(|v| v.as_i64()).unwrap_or(0);
    let api_key = row.get("apiKey").and_then(|v| v.as_str()).unwrap_or("");
    let base_url = row.get("baseUrl").and_then(|v| v.as_str());

    let mut vault = state.vault.write().await;
    match vault.materialize(scope, provider, secret_ref, version, api_key, base_url) {
        Ok(sealed_at) => json_ok(
            StatusCode::OK,
            ok_envelope(
                json!({
                    "sealed": true,
                    "sealed_at": sealed_at,
                    "secret_ref": secret_ref,
                }),
                &request_id,
            ),
            &request_id,
        ),
        Err(msg) => json_err(err_envelope(&request_id, 400, msg, "request.invalid")),
    }
}

async fn vault_status(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let request_id = request_id_from(&headers);
    if !is_loopback(&headers) {
        return json_err(err_envelope(
            &request_id,
            403,
            "vault status is loopback-only",
            "auth.forbidden",
        ));
    }
    let vault = state.vault.read().await;
    json_ok(
        StatusCode::OK,
        ok_envelope(vault.status(None), &request_id),
        &request_id,
    )
}

async fn vault_receipts(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let request_id = request_id_from(&headers);
    if !is_loopback(&headers) {
        return json_err(err_envelope(
            &request_id,
            403,
            "vault receipts are loopback-only",
            "auth.forbidden",
        ));
    }
    let vault = state.vault.read().await;
    json_ok(
        StatusCode::OK,
        ok_envelope(json!({ "items": vault.list_receipts(20) }), &request_id),
        &request_id,
    )
}

async fn vault_apply(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    let request_id = request_id_from(&headers);
    if !is_loopback(&headers) {
        return json_err(err_envelope(
            &request_id,
            403,
            "vault-apply queue is loopback-only",
            "auth.forbidden",
        ));
    }
    // Fail closed on plaintext fields in queue jobs.
    if body.as_object().is_some_and(|o| {
        o.contains_key("apiKey") || o.contains_key("api_key") || o.contains_key("plaintext")
    }) {
        return json_err(err_envelope(
            &request_id,
            400,
            "plaintext credentials forbidden on vault.apply",
            "vault.plaintext_forbidden",
        ));
    }
    let obj = body.as_object().cloned().unwrap_or_default();
    let provider = obj
        .get("provider")
        .and_then(|v| v.as_str())
        .and_then(VaultProviderId::parse);
    let Some(provider) = provider else {
        return json_err(err_envelope(
            &request_id,
            400,
            "provider must be boyue|siliconflow|worldmonitor",
            "request.invalid",
        ));
    };
    let scope_raw = obj
        .get("scope")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();
    let scope = VaultScope {
        organization_id: scope_raw
            .get("organization_id")
            .and_then(|v| v.as_str())
            .unwrap_or("org_local")
            .to_string(),
        project_id: scope_raw
            .get("project_id")
            .and_then(|v| v.as_str())
            .unwrap_or("proj_local")
            .to_string(),
        environment_id: scope_raw
            .get("environment_id")
            .and_then(|v| v.as_str())
            .unwrap_or("env_local")
            .to_string(),
    };
    let secret_ref = match obj.get("secret_ref") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) => Some(s.as_str()),
        _ => {
            return json_err(err_envelope(
                &request_id,
                400,
                "secret_ref must be string or null",
                "request.invalid",
            ));
        }
    };
    let version = obj.get("version").and_then(|v| v.as_i64()).unwrap_or(0);
    let base_url = obj.get("base_url").and_then(|v| v.as_str());

    let mut vault = state.vault.write().await;
    let receipt = vault.apply_job(scope, provider, secret_ref, version, base_url);
    if receipt.status == "failed" {
        return json_err(err_envelope(
            &request_id,
            503,
            format!(
                "queue down; apply failed: {}",
                receipt.detail.as_deref().unwrap_or("unknown")
            ),
            "queue.unavailable",
        ));
    }
    json_ok(
        StatusCode::OK,
        ok_envelope(
            json!({
                "queued": false,
                "detail": "applied directly (NATS not wired in Rust runtime)",
                "receipt": receipt,
            }),
            &request_id,
        ),
        &request_id,
    )
}

async fn upstream_secrets(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    let request_id = request_id_from(&headers);
    if !is_loopback(&headers) {
        return json_err(err_envelope(
            &request_id,
            403,
            "upstream-secrets sync is loopback-only",
            "auth.forbidden",
        ));
    }
    let mut vault = state.vault.write().await;
    match vault.apply_upstream_secrets_patch(&body) {
        Ok((boyue, siliconflow, worldmonitor)) => {
            let receipt = vault.list_receipts(1).into_iter().next();
            json_ok(
                StatusCode::OK,
                ok_envelope(
                    json!({
                        "detail": "upstream secrets updated (legacy plaintext path)",
                        "boyue": boyue,
                        "siliconflow": siliconflow,
                        "worldmonitor": worldmonitor,
                        "receipt": receipt,
                    }),
                    &request_id,
                ),
                &request_id,
            )
        }
        Err(msg) => json_err(err_envelope(&request_id, 400, msg, "request.invalid")),
    }
}

async fn not_found(method: Method, uri: axum::http::Uri, headers: HeaderMap) -> Response {
    let request_id = request_id_from(&headers);
    json_err(err_envelope(
        &request_id,
        404,
        format!("no route: {method} {}", uri.path()),
        "route.not_ready",
    ))
}

fn elapsed_ms(started: Instant) -> u64 {
    started.elapsed().as_millis() as u64
}

async fn check_quota(
    state: &AppState,
    prefix: &str,
    request_id: &str,
    sku: &str,
    route: &str,
) -> Option<Response> {
    let spent = state.usage.read().await.spend_for_prefix(prefix);
    if spent >= spend_cap_usd() {
        record_denied(
            state,
            request_id,
            sku,
            "api",
            prefix,
            route,
            "quota.exceeded",
            None,
        )
        .await;
        return Some(json_err(err_envelope(
            request_id,
            429,
            format!("Env spend hard-cap reached (${})", spend_cap_usd()),
            "quota.exceeded",
        )));
    }
    None
}

async fn record_denied(
    state: &AppState,
    request_id: &str,
    sku: &str,
    listing_kind: &str,
    prefix: &str,
    route: &str,
    detail: &str,
    duration_ms: Option<u64>,
) {
    let mut usage = state.usage.write().await;
    let row = usage.record(UsageInput {
        request_id: request_id.to_string(),
        sku: sku.to_string(),
        listing_kind: listing_kind.to_string(),
        credential_prefix: prefix.to_string(),
        units: 0.0,
        unit_price_usd: 0.0,
        status: UsageStatus::Denied,
        detail: Some(detail.to_string()),
        route: route.to_string(),
        org_id: SCOPE_ORG.into(),
        project_id: SCOPE_PROJECT.into(),
        env_id: SCOPE_ENV.into(),
        duration_ms,
    });
    drop(usage);
    push_event(
        state,
        "invoke.failed",
        json!({ "sku": sku, "usage_id": row.id, "detail": detail }),
    )
    .await;
}

async fn record_ok_once(
    state: &AppState,
    request_id: &str,
    sku: &str,
    listing_kind: &str,
    prefix: &str,
    route: &str,
    units: f64,
    duration_ms: Option<u64>,
) -> crate::usage::UsageRecord {
    let mut usage = state.usage.write().await;
    usage.record_once(UsageInput {
        request_id: request_id.to_string(),
        sku: sku.to_string(),
        listing_kind: listing_kind.to_string(),
        credential_prefix: prefix.to_string(),
        units,
        unit_price_usd: 0.0,
        status: UsageStatus::Ok,
        detail: None,
        route: route.to_string(),
        org_id: SCOPE_ORG.into(),
        project_id: SCOPE_PROJECT.into(),
        env_id: SCOPE_ENV.into(),
        duration_ms,
    })
}

async fn push_event(state: &AppState, kind: &str, payload: Value) {
    state.events.write().await.push(kind, payload);
}

fn urlencoding_decode(s: &str) -> String {
    // Percent-decode minimal; axum usually already decodes path segments.
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let h = hex_nibble(bytes[i + 1]);
            let l = hex_nibble(bytes[i + 2]);
            if let (Some(h), Some(l)) = (h, l) {
                out.push((h << 4 | l) as char);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

fn hex_nibble(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {

    // Both tests mutate the process-global A3S_API_ORIGIN; serialize them
    // so parallel execution cannot interleave the mutation with the other
    // test's AppState construction. The guard is intentionally held across
    // await points — the current-thread test runtimes do not require Send.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    async fn fake_deliver(Json(body): Json<Value>) -> Json<Value> {
        let sku = body.get("sku").and_then(|item| item.as_str()).unwrap_or("");
        let echoed = body
            .get("body")
            .and_then(|item| item.as_str())
            .unwrap_or("");
        let (listed, available, reason, response_body) = if sku.contains("idle") {
            (true, false, "no worker", "")
        } else if sku.contains("ready") {
            (true, true, "", echoed)
        } else {
            (false, false, "no entitlement", "")
        };
        Json(json!({
            "success": true,
            "code": 0,
            "message": "OK",
            "data": {
                "listed": listed,
                "available": available,
                "body": response_body,
                "reason": reason,
            }
        }))
    }

    async fn call(
        app: &Router,
        method: &str,
        uri: &str,
        key: Option<&str>,
        transaction: Option<&str>,
        body: &str,
    ) -> (StatusCode, Value) {
        let mut builder = Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json");
        if let Some(key) = key {
            builder = builder.header("authorization", format!("Bearer {key}"));
        }
        if let Some(transaction) = transaction {
            builder = builder.header("x-transaction-id", transaction);
        }
        let response = app
            .clone()
            .oneshot(builder.body(Body::from(body.to_string())).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), 1_000_000)
            .await
            .unwrap();
        let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, value)
    }

    fn accepted_units(usage: &Value) -> (usize, f64) {
        let items = usage["data"]["items"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let ok: Vec<_> = items
            .iter()
            .filter(|item| item["status"] == "ok")
            .cloned()
            .collect();
        let units = ok
            .iter()
            .map(|item| item["units"].as_f64().unwrap_or(0.0))
            .sum();
        (ok.len(), units)
    }

    #[tokio::test]
    async fn accepted_platform_call_records_one_usage_quantity() {
        let _env_guard = ENV_LOCK.lock().unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let api = Router::new().route("/v1/api/admin/inference/deliveries", post(fake_deliver));
        let server = tokio::spawn(async move {
            let _ = axum::serve(listener, api).await;
        });
        unsafe { std::env::set_var("A3S_API_ORIGIN", &origin) };
        tokio::time::sleep(std::time::Duration::from_millis(30)).await;

        let dir = std::env::temp_dir().join(format!("runtime-usage-{}", uuid::Uuid::new_v4()));
        let state = crate::AppState::new(dir.clone());
        let app = router(state);
        let key = crate::auth::DEV_CAPABILITY_KEY;
        let prompt = r#"{"prompt":"usage-proof"}"#;
        let echoed = serde_json::from_str::<Value>(prompt).unwrap().to_string();

        let (status, _) = call(
            &app,
            "POST",
            "/v1/api/services/model/invoke/platform-ready",
            None,
            None,
            prompt,
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        let (status, usage) = call(&app, "GET", "/v1/api/usage", Some(key), None, "").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(accepted_units(&usage), (0, 0.0));

        let transaction = uuid::Uuid::new_v4().to_string();
        for _ in 0..2 {
            let (status, body) = call(
                &app,
                "POST",
                "/v1/api/services/model/invoke/platform-ready",
                Some(key),
                Some(&transaction),
                prompt,
            )
            .await;
            assert_eq!(status, StatusCode::OK);
            assert_eq!(body["data"]["available"], true);
            assert_eq!(body["data"]["body"], echoed);
        }
        let (status, usage) = call(&app, "GET", "/v1/api/usage", Some(key), None, "").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(accepted_units(&usage), (1, ACCEPTED_UNITS));
        assert_eq!(usage["data"]["items"][0]["request_id"], transaction);

        for _ in 0..2 {
            let (status, body) = call(
                &app,
                "POST",
                "/v1/api/services/model/invoke/platform-idle",
                Some(key),
                Some(&uuid::Uuid::new_v4().to_string()),
                prompt,
            )
            .await;
            assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
            assert_eq!(body["data"]["available"], false);
            assert_eq!(body["data"]["body"], "");
            assert_eq!(body["data"]["reason"], "no worker");
        }
        let (status, usage) = call(&app, "GET", "/v1/api/usage", Some(key), None, "").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(accepted_units(&usage), (1, ACCEPTED_UNITS));
        println!(
            "usage items={} units={}",
            accepted_units(&usage).0,
            accepted_units(&usage).1
        );

        server.abort();
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn unified_invoke_reads_sku_from_additional_parameters() {
        let _env_guard = ENV_LOCK.lock().unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let api = Router::new().route("/v1/api/admin/inference/deliveries", post(fake_deliver));
        let server = tokio::spawn(async move {
            let _ = axum::serve(listener, api).await;
        });
        unsafe { std::env::set_var("A3S_API_ORIGIN", &origin) };
        tokio::time::sleep(std::time::Duration::from_millis(30)).await;

        let dir = std::env::temp_dir().join(format!("runtime-sku-param-{}", uuid::Uuid::new_v4()));
        let state = crate::AppState::new(dir.clone());
        let app = router(state);
        let key = crate::auth::DEV_CAPABILITY_KEY;
        let body = r#"{"sku":"platform-ready","prompt":"usage-proof"}"#;
        let echoed = serde_json::from_str::<Value>(body).unwrap().to_string();
        let transaction = uuid::Uuid::new_v4().to_string();

        let (status, response) = call(
            &app,
            "POST",
            "/v1/api/services/model/invoke",
            Some(key),
            Some(&transaction),
            body,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(response["data"]["available"], true);
        assert_eq!(response["data"]["body"], echoed);

        let nested = r#"{"params":{"sku":"platform-ready"},"prompt":"nested"}"#;
        let (status, response) = call(
            &app,
            "POST",
            "/v1/api/services/model/invoke",
            Some(key),
            Some(&uuid::Uuid::new_v4().to_string()),
            nested,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(response["data"]["available"], true);

        let (status, missing) = call(
            &app,
            "POST",
            "/v1/api/services/model/invoke",
            Some(key),
            None,
            r#"{"prompt":"usage-proof"}"#,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(missing["error_code"], "request.invalid");

        let (status, _) = call(
            &app,
            "POST",
            "/v1/api/services/api/invoke",
            Some(key),
            None,
            r#"{"sku":"a3s.monitor.missing","input":{}}"#,
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);

        let (status, agent) = call(
            &app,
            "POST",
            "/v1/api/services/agent/invoke",
            Some(key),
            None,
            r#"{"sku":"support-triage","input":{}}"#,
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(agent["error_code"], "route.not_ready");
        assert!(agent["message"]
            .as_str()
            .unwrap_or("")
            .contains("support-triage"));

        server.abort();
        let _ = std::fs::remove_dir_all(dir);
    }
}
