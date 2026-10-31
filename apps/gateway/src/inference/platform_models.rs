//! Platform OCR and System One, with an optional supplier on the same route.
//!
//! A signed-in caller posts the image or the decision body and names no model.
//! When the route has no supplier, the gateway calls the in-process engines.
//! A bound supplier receives the original body. A missing or unreachable
//! supplier is a 503 and does not fall through to the platform engine.

use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;

use aster_apofasi::{Client, SystemOneRequest};
use aster_use_bge::{EmbeddingClient, EmbeddingRequest, RerankClient, RerankRequest};
use aster_use_ocr::{OcrClient, OcrRequest};
use bytes::Bytes;
use http::{HeaderMap, HeaderValue, Response, StatusCode};
use uuid::Uuid;

use super::access_error::InferenceAccessError;
use super::authorization::{AuthenticatedInference, InferenceAuthorizer};
use super::identity::InferenceRequestIdentity;
use super::observed_tokens::{ObservedTokenUsage, ObservedTokens};
use super::openai_request::{OpenAiRequestError, OpenAiRequestProfile};
use super::work_envelope::error_envelope_response;
use super::{
    PLATFORM_APOFASI_ALIAS, PLATFORM_EMBEDDING_ALIAS, PLATFORM_OCR_ALIAS, PLATFORM_RERANK_ALIAS,
};
use crate::config::{InferenceEndpoint, InferenceProtocol, InferenceTargetConfig};
use crate::service::{Backend, ServiceRegistry};
use crate::usage::{UsageRequestLifecycle, UsageSpool};

const PLATFORM_OCR_MODEL_ID: Uuid = Uuid::from_u128(0x0c10_0000_0000_4000_8000_000000000001);
const PLATFORM_APOFASI_MODEL_ID: Uuid = Uuid::from_u128(0x0c10_0000_0000_4000_8000_000000000002);
const PLATFORM_EMBEDDING_MODEL_ID: Uuid = Uuid::from_u128(0x0c10_0000_0000_4000_8000_000000000003);
const PLATFORM_RERANK_MODEL_ID: Uuid = Uuid::from_u128(0x0c10_0000_0000_4000_8000_000000000004);
const BODY_LIMIT: usize = super::openai_request::OPENAI_REQUEST_BODY_LIMIT;

/// One finished OCR or System One response, plus usage when a spool is configured.
pub(crate) struct ServedPlatformModel {
    pub(crate) response: Response<Bytes>,
    pub(crate) lifecycle: Option<UsageRequestLifecycle>,
}

pub(crate) async fn serve_platform_model(
    registry: &ServiceRegistry,
    usage_spool: Option<std::sync::Arc<UsageSpool>>,
    authorizer: &InferenceAuthorizer,
    authenticated: AuthenticatedInference,
    identity: &mut InferenceRequestIdentity,
    router: &str,
    profile: OpenAiRequestProfile,
    path: &str,
    headers: &HeaderMap,
    body: Bytes,
) -> ServedPlatformModel {
    let request_id = identity.request_id();
    let endpoint = match profile {
        OpenAiRequestProfile::Ocr => InferenceEndpoint::Ocr,
        OpenAiRequestProfile::SystemOne => InferenceEndpoint::SystemOne,
        _ => {
            return failed(
                InferenceAccessError::UnsupportedEndpoint.into_response_with_request_id(request_id),
            );
        }
    };
    if body.is_empty() {
        return failed(reject(
            request_id,
            StatusCode::BAD_REQUEST,
            "Request body is empty.",
        ));
    }
    if let Err(response) = validate_body(profile, headers, request_id) {
        return failed(response);
    }

    let supplier = match authorizer.platform_supplier(router, endpoint) {
        Ok(supplier) => supplier,
        Err(error) => return failed(error.into_response_with_request_id(request_id)),
    };
    let _admission = match authorizer.admit_request(authenticated, chrono::Utc::now()) {
        Ok(admission) => admission,
        Err(error) => return failed(error.into_response_with_request_id(request_id)),
    };

    let served = if let Some(supplier) = supplier {
        let Some(backend) = healthy_backend(registry, &supplier.service) else {
            tracing::warn!(
                service = %supplier.service,
                target_id = %supplier.target_id,
                "Platform model supplier has no healthy backend"
            );
            return failed(
                InferenceAccessError::Unavailable.into_response_with_request_id(request_id),
            );
        };
        let alias = supplier.upstream_model.clone();
        let target_id = supplier.target_id;
        let lifecycle = match begin_usage(
            usage_spool,
            identity,
            authenticated,
            &alias,
            target_id,
            request_id,
        )
        .await
        {
            Ok(lifecycle) => lifecycle,
            Err(served) => return served,
        };
        let response = proxy_supplier(
            &supplier,
            authorizer.upstream_authorization(supplier.target_id),
            &backend,
            path,
            headers,
            body,
            request_id,
        )
        .await;
        ServedPlatformModel {
            response,
            lifecycle,
        }
    } else {
        let (alias, model_id) = platform_identity(endpoint);
        let lifecycle = match begin_usage(
            usage_spool,
            identity,
            authenticated,
            alias,
            model_id,
            request_id,
        )
        .await
        {
            Ok(lifecycle) => lifecycle,
            Err(served) => return served,
        };
        let response = match endpoint {
            InferenceEndpoint::Ocr => recognize(headers, body, request_id).await,
            InferenceEndpoint::SystemOne => decide(body, request_id),
            _ => {
                InferenceAccessError::UnsupportedEndpoint.into_response_with_request_id(request_id)
            }
        };
        ServedPlatformModel {
            response,
            lifecycle,
        }
    };
    served
}

fn failed(response: Response<Bytes>) -> ServedPlatformModel {
    ServedPlatformModel {
        response,
        lifecycle: None,
    }
}

fn platform_identity(endpoint: InferenceEndpoint) -> (&'static str, Uuid) {
    match endpoint {
        InferenceEndpoint::Ocr => (PLATFORM_OCR_ALIAS, PLATFORM_OCR_MODEL_ID),
        InferenceEndpoint::Embeddings => (PLATFORM_EMBEDDING_ALIAS, PLATFORM_EMBEDDING_MODEL_ID),
        InferenceEndpoint::Rerank => (PLATFORM_RERANK_ALIAS, PLATFORM_RERANK_MODEL_ID),
        _ => (PLATFORM_APOFASI_ALIAS, PLATFORM_APOFASI_MODEL_ID),
    }
}

async fn begin_usage(
    usage_spool: Option<std::sync::Arc<UsageSpool>>,
    identity: &mut InferenceRequestIdentity,
    authenticated: AuthenticatedInference,
    alias: &str,
    model_id: Uuid,
    request_id: Uuid,
) -> Result<Option<UsageRequestLifecycle>, ServedPlatformModel> {
    identity.set_model_id(model_id);
    let Some(spool) = usage_spool else {
        return Ok(None);
    };
    let mut lifecycle =
        match UsageRequestLifecycle::begin(spool, identity, authenticated, alias, None).await {
            Ok(lifecycle) => lifecycle,
            Err(error) => {
                tracing::error!(
                    error = %error,
                    request_id = %request_id,
                    "Platform model rejected because durable usage could not start"
                );
                return Err(failed(
                    InferenceAccessError::UsageUnavailable
                        .into_response_with_request_id(request_id),
                ));
            }
        };
    let attempt = identity.begin_attempt(model_id);
    if let Err(error) = lifecycle.begin_attempt(&attempt).await {
        tracing::error!(
            error = %error,
            request_id = %request_id,
            "Platform model rejected because durable attempt usage could not start"
        );
        return Err(ServedPlatformModel {
            response: InferenceAccessError::UsageUnavailable
                .into_response_with_request_id(request_id),
            lifecycle: Some(lifecycle),
        });
    }
    Ok(Some(lifecycle))
}

fn validate_body(
    profile: OpenAiRequestProfile,
    headers: &HeaderMap,
    request_id: Uuid,
) -> Result<(), Response<Bytes>> {
    match profile {
        OpenAiRequestProfile::Ocr => {
            if image_extension(content_type(headers)).is_none() {
                return Err(reject(
                    request_id,
                    StatusCode::UNSUPPORTED_MEDIA_TYPE,
                    "Content-Type must be an image.",
                ));
            }
        }
        OpenAiRequestProfile::SystemOne => {
            if !is_json(content_type(headers)) {
                return Err(OpenAiRequestError::UnsupportedMediaType
                    .into_response_with_request_id(request_id));
            }
        }
        _ => {}
    }
    Ok(())
}

async fn recognize(headers: &HeaderMap, body: Bytes, request_id: Uuid) -> Response<Bytes> {
    let Some(extension) = image_extension(content_type(headers)) else {
        return reject(
            request_id,
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "Content-Type must be an image.",
        );
    };
    let client = match tokio::task::spawn_blocking(platform_ocr_client).await {
        Ok(Ok(client)) => client,
        Ok(Err(error)) => {
            tracing::error!(error = %error, "Platform OCR engine is unavailable");
            return InferenceAccessError::Unavailable.into_response_with_request_id(request_id);
        }
        Err(error) => {
            tracing::error!(error = %error, "Platform OCR engine task failed");
            return InferenceAccessError::Unavailable.into_response_with_request_id(request_id);
        }
    };
    let path = std::env::temp_dir().join(format!("aster-gateway-ocr-{request_id}.{extension}"));
    let _file = TempImage(path.clone());
    if let Err(error) = tokio::fs::write(&path, &body).await {
        tracing::error!(error = %error, "Platform OCR could not store the image");
        return InferenceAccessError::Unavailable.into_response_with_request_id(request_id);
    }
    let extracted = client.extract(OcrRequest { path }).await;
    match extracted {
        Ok(result) => json_response(&result, request_id),
        Err(error) => {
            tracing::error!(error = %error, "Platform OCR recognition failed");
            InferenceAccessError::Unavailable.into_response_with_request_id(request_id)
        }
    }
}

fn decide(body: Bytes, request_id: Uuid) -> Response<Bytes> {
    let mut request: SystemOneRequest = match serde_json::from_slice(&body) {
        Ok(request) => request,
        Err(_) if serde_json::from_slice::<serde_json::Value>(&body).is_ok() => {
            return OpenAiRequestError::InvalidBodyShape.into_response_with_request_id(request_id);
        }
        Err(_) => {
            return OpenAiRequestError::InvalidJson.into_response_with_request_id(request_id);
        }
    };
    // The platform model answers. A caller-supplied model name is not a route.
    request.model = None;
    match Client::lexical().system_one(request) {
        Ok(response) => json_response(&response, request_id),
        Err(error) => {
            tracing::error!(error = %error, "Platform Apofasi decision failed");
            OpenAiRequestError::InvalidBodyShape.into_response_with_request_id(request_id)
        }
    }
}

/// Warms the in-process engines at startup so the first caller pays no
/// model-load latency. Missing installs degrade to a warn — serving starts
/// either way and the endpoints fail closed per request.
pub(crate) fn warm_platform_engines() {
    tokio::spawn(async {
        match platform_embedding_client() {
            Ok(client) => {
                if let Err(error) = client.warm().await {
                    tracing::warn!(engine = "embedding", error = %error, "platform engine warm-up skipped");
                }
            }
            Err(error) => {
                tracing::warn!(engine = "embedding", error = %error, "platform engine client unavailable");
            }
        }
        match platform_rerank_client() {
            Ok(client) => {
                if let Err(error) = client.warm().await {
                    tracing::warn!(engine = "rerank", error = %error, "platform engine warm-up skipped");
                }
            }
            Err(error) => {
                tracing::warn!(engine = "rerank", error = %error, "platform engine client unavailable");
            }
        }
    });
}

fn platform_ocr_client() -> aster_use_ocr::UseResult<OcrClient> {
    static SLOT: OnceLock<OcrClient> = OnceLock::new();
    if let Some(client) = SLOT.get() {
        return Ok(client.clone());
    }
    let client = OcrClient::from_env()?;
    let _ = SLOT.set(client.clone());
    Ok(SLOT.get().cloned().unwrap_or(client))
}

fn platform_embedding_client() -> aster_use_bge::UseResult<EmbeddingClient> {
    static SLOT: OnceLock<EmbeddingClient> = OnceLock::new();
    if let Some(client) = SLOT.get() {
        return Ok(client.clone());
    }
    let client = EmbeddingClient::from_env()?;
    let _ = SLOT.set(client.clone());
    Ok(SLOT.get().cloned().unwrap_or(client))
}

fn platform_rerank_client() -> aster_use_bge::UseResult<RerankClient> {
    static SLOT: OnceLock<RerankClient> = OnceLock::new();
    if let Some(client) = SLOT.get() {
        return Ok(client.clone());
    }
    let client = RerankClient::from_env()?;
    let _ = SLOT.set(client.clone());
    Ok(SLOT.get().cloned().unwrap_or(client))
}

/// Serves `platform/bge-m3` on embeddings and `platform/bge-reranker-v2-m3`
/// on rerank. The caller has already matched the body's model alias to a
/// reserved platform alias; any other alias stays on managed dispatch.
///
/// A bound supplier is the only upstream and does not fall back (the OCR and
/// System One contract). Without one, the in-process engines answer, and a
/// missing install is a named 503 — the gateway never auto-downloads.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn serve_platform_alias_model(
    registry: &ServiceRegistry,
    usage_spool: Option<std::sync::Arc<UsageSpool>>,
    authorizer: &InferenceAuthorizer,
    authenticated: AuthenticatedInference,
    identity: &mut InferenceRequestIdentity,
    router: &str,
    profile: OpenAiRequestProfile,
    headers: &HeaderMap,
    body: Bytes,
) -> ServedPlatformModel {
    let request_id = identity.request_id();
    let endpoint = match profile {
        OpenAiRequestProfile::Embeddings => InferenceEndpoint::Embeddings,
        OpenAiRequestProfile::Rerank => InferenceEndpoint::Rerank,
        _ => {
            return failed(
                InferenceAccessError::UnsupportedEndpoint.into_response_with_request_id(request_id),
            );
        }
    };
    if body.is_empty() {
        return failed(reject(
            request_id,
            StatusCode::BAD_REQUEST,
            "Request body is empty.",
        ));
    }
    if !is_json(content_type(headers)) {
        return failed(
            OpenAiRequestError::UnsupportedMediaType.into_response_with_request_id(request_id),
        );
    }
    let supplier = match authorizer.platform_supplier(router, endpoint) {
        Ok(supplier) => supplier,
        Err(error) => return failed(error.into_response_with_request_id(request_id)),
    };
    let _admission = match authorizer.admit_request(authenticated, chrono::Utc::now()) {
        Ok(admission) => admission,
        Err(error) => return failed(error.into_response_with_request_id(request_id)),
    };

    if let Some(supplier) = supplier {
        let Some(backend) = healthy_backend(registry, &supplier.service) else {
            tracing::warn!(
                service = %supplier.service,
                target_id = %supplier.target_id,
                "Platform alias model supplier has no healthy backend"
            );
            return failed(
                InferenceAccessError::Unavailable.into_response_with_request_id(request_id),
            );
        };
        let alias = supplier.upstream_model.clone();
        let target_id = supplier.target_id;
        let lifecycle = match begin_usage(
            usage_spool,
            identity,
            authenticated,
            &alias,
            target_id,
            request_id,
        )
        .await
        {
            Ok(lifecycle) => lifecycle,
            Err(served) => return served,
        };
        let response = proxy_supplier(
            &supplier,
            authorizer.upstream_authorization(supplier.target_id),
            &backend,
            "/v1/embeddings",
            headers,
            body,
            request_id,
        )
        .await;
        ServedPlatformModel {
            response,
            lifecycle,
        }
    } else {
        let (alias, model_id) = platform_identity(endpoint);
        let lifecycle = match begin_usage(
            usage_spool,
            identity,
            authenticated,
            alias,
            model_id,
            request_id,
        )
        .await
        {
            Ok(lifecycle) => lifecycle,
            Err(served) => return served,
        };
        // The engines measure their own token usage; feeding it to the
        // lifecycle turns the durable record from `unknown` completeness
        // into upstream-measured totals.
        let observed = lifecycle
            .as_ref()
            .map(UsageRequestLifecycle::observed_tokens_handle);
        let response = match endpoint {
            InferenceEndpoint::Embeddings => match platform_embedding_client() {
                Ok(client) => match EmbeddingRequest::from_json(&body) {
                    Ok(request) => match client.embed(request).await {
                        Ok(response) => {
                            record_observed_tokens(&observed, response.usage.total_tokens);
                            json_response(&response, request_id)
                        }
                        Err(error) => engine_unavailable(request_id, "embedding", &error),
                    },
                    Err(error) => engine_request_invalid(request_id, "embedding", &error),
                },
                Err(error) => engine_unavailable(request_id, "embedding", &error),
            },
            _ => match platform_rerank_client() {
                Ok(client) => match RerankRequest::from_json(&body) {
                    Ok(request) => match client.rerank(request).await {
                        Ok(response) => {
                            record_observed_tokens(&observed, response.usage.total_tokens);
                            json_response(&response, request_id)
                        }
                        Err(error) => engine_unavailable(request_id, "rerank", &error),
                    },
                    Err(error) => engine_request_invalid(request_id, "rerank", &error),
                },
                Err(error) => engine_unavailable(request_id, "rerank", &error),
            },
        };
        ServedPlatformModel {
            response,
            lifecycle,
        }
    }
}

fn record_observed_tokens(
    observed: &Option<std::sync::Arc<ObservedTokenUsage>>,
    total_tokens: u64,
) {
    if let Some(handle) = observed {
        // The platform engines report a measured total without a split; the
        // split lanes stay unknown rather than invented.
        handle.record(&ObservedTokens {
            total: Some(total_tokens),
            ..ObservedTokens::default()
        });
    }
}

fn engine_unavailable(
    request_id: Uuid,
    engine: &str,
    error: &aster_use_bge::UseError,
) -> Response<Bytes> {
    tracing::warn!(
        engine,
        code = %error.code,
        error = %error,
        suggestion = ?error.suggestion,
        "Platform {engine} engine is unavailable"
    );
    error_envelope_response(
        request_id,
        StatusCode::SERVICE_UNAVAILABLE,
        "The platform model engine is unavailable; install it with 'aster-use-bge install'.",
        "use.bge.unavailable",
    )
}

fn engine_request_invalid(
    request_id: Uuid,
    engine: &str,
    error: &aster_use_bge::UseError,
) -> Response<Bytes> {
    tracing::warn!(
        engine,
        code = %error.code,
        error = %error,
        "Platform {engine} request was refused"
    );
    error_envelope_response(
        request_id,
        StatusCode::BAD_REQUEST,
        "The platform model request violates a named limit; the engine log names it.",
        "use.bge.request_invalid",
    )
}

async fn proxy_supplier(
    supplier: &InferenceTargetConfig,
    authorization: Option<HeaderValue>,
    backend: &Backend,
    path: &str,
    headers: &HeaderMap,
    body: Bytes,
    request_id: Uuid,
) -> Response<Bytes> {
    let Some(url) = supplier_url(backend, path) else {
        return InferenceAccessError::Unavailable.into_response_with_request_id(request_id);
    };
    let Some(client) = http_client() else {
        return InferenceAccessError::Unavailable.into_response_with_request_id(request_id);
    };
    let mut request = client.post(url).body(body.to_vec());
    if let Some(content_type) = headers.get(http::header::CONTENT_TYPE) {
        request = request.header(http::header::CONTENT_TYPE, content_type);
    }
    if let Some(authorization) = authorization {
        match supplier.protocol {
            InferenceProtocol::Anthropic => {
                request = request.header("x-api-key", authorization);
            }
            InferenceProtocol::Openai | InferenceProtocol::Ocr | InferenceProtocol::SystemOne => {
                request = request.header(http::header::AUTHORIZATION, authorization);
            }
        }
    }
    let response = match request.send().await {
        Ok(response) => response,
        Err(error) => {
            tracing::warn!(error = %error, "Platform model supplier is unreachable");
            return InferenceAccessError::Unavailable.into_response_with_request_id(request_id);
        }
    };
    let status = response.status();
    let content_type = response
        .headers()
        .get(http::header::CONTENT_TYPE)
        .cloned()
        .unwrap_or_else(|| HeaderValue::from_static("application/json"));
    let bytes = match response.bytes().await {
        Ok(bytes) if bytes.len() <= BODY_LIMIT => bytes,
        Ok(_) => {
            return reject(
                request_id,
                StatusCode::BAD_GATEWAY,
                "Supplier response exceeds the body limit.",
            );
        }
        Err(error) => {
            tracing::warn!(error = %error, "Platform model supplier response could not be read");
            return InferenceAccessError::Unavailable.into_response_with_request_id(request_id);
        }
    };
    passthrough(status, content_type, bytes, request_id)
}

fn supplier_url(backend: &Backend, path: &str) -> Option<String> {
    let base = backend.http_base_uri()?;
    let authority = base.authority()?;
    let scheme = base.scheme_str()?;
    let path = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    };
    Some(format!("{scheme}://{authority}{path}"))
}

fn healthy_backend(registry: &ServiceRegistry, service: &str) -> Option<std::sync::Arc<Backend>> {
    let backend = registry.get(service)?.next_backend()?;
    if backend.is_healthy() && backend.http_base_uri().is_some() {
        Some(backend)
    } else {
        None
    }
}

fn http_client() -> Option<&'static reqwest::Client> {
    static CLIENT: OnceLock<Option<reqwest::Client>> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .ok()
        })
        .as_ref()
}

fn json_response(body: &impl serde::Serialize, request_id: Uuid) -> Response<Bytes> {
    let bytes = match serde_json::to_vec(body) {
        Ok(bytes) => Bytes::from(bytes),
        Err(error) => {
            tracing::error!(error = %error, "Platform model response could not be encoded");
            return InferenceAccessError::Unavailable.into_response_with_request_id(request_id);
        }
    };
    passthrough(
        StatusCode::OK,
        HeaderValue::from_static("application/json"),
        bytes,
        request_id,
    )
}

fn passthrough(
    status: StatusCode,
    content_type: HeaderValue,
    body: Bytes,
    request_id: Uuid,
) -> Response<Bytes> {
    let mut response = Response::new(body);
    *response.status_mut() = status;
    response
        .headers_mut()
        .insert(http::header::CONTENT_TYPE, content_type);
    if let Ok(length) = HeaderValue::from_str(&response.body().len().to_string()) {
        response
            .headers_mut()
            .insert(http::header::CONTENT_LENGTH, length);
    }
    if let Ok(value) = HeaderValue::from_str(&request_id.to_string()) {
        response.headers_mut().insert("x-request-id", value);
    }
    response
}

fn reject(request_id: Uuid, status: StatusCode, message: &'static str) -> Response<Bytes> {
    error_envelope_response(request_id, status, message, "request.invalid")
}

fn content_type(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(http::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
}

fn media_type(value: Option<&str>) -> Option<&str> {
    value
        .and_then(|value| value.split(';').next())
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn is_json(value: Option<&str>) -> bool {
    media_type(value).is_some_and(|value| value.eq_ignore_ascii_case("application/json"))
}

fn image_extension(value: Option<&str>) -> Option<&'static str> {
    match media_type(value)?.to_ascii_lowercase().as_str() {
        "image/png" => Some("png"),
        "image/jpeg" | "image/jpg" => Some("jpg"),
        "image/webp" => Some("webp"),
        "image/gif" => Some("gif"),
        "image/bmp" => Some("bmp"),
        "image/tiff" | "image/tif" => Some("tiff"),
        _ => None,
    }
}

struct TempImage(PathBuf);

impl Drop for TempImage {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
