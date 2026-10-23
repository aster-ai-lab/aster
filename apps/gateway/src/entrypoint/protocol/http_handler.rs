//! Plain HTTP protocol handler

use crate::entrypoint::protocol::body_buffer::{buffer_body_up_to, BufferedBody};
use crate::entrypoint::protocol::{
    full_body, request_is_replayable, retry_upstream, retryable_upstream_status, ProtocolContext,
    ResponseBody,
};
use crate::error::GatewayError;
use crate::inference::{
    track_attributed_token_budget_response, wrap_json_success_body, InferenceAccessError,
};
use crate::observability::access_log::AccessLogGuard;
use crate::proxy::{BackendOperationTracking, ForwardOptions, HttpTimeouts, OwnedStreamingRequest};
use crate::usage::{track_usage_response, UsageRequestLifecycle, UsageTerminalOutcome};
use arc_swap::ArcSwap;
use bytes::Bytes;
use http::header::{CONTENT_LENGTH, CONTENT_TYPE};
use http::Response;
use http_body_util::BodyExt;
use std::sync::Arc;

pub async fn handle_http_dispatch(ctx: ProtocolContext) -> Response<ResponseBody> {
    let inference_admission = ctx.inference_admission;
    let mut inference_attempt = ctx.inference_attempt;
    let mut inference_dispatch = ctx.inference_dispatch;
    let mut backend = ctx.backend;
    let state = ctx.state;
    let mut route = ctx.route;
    let mut req_parts = ctx.req_parts;
    let mut body_bytes = ctx.body_bytes;
    let pipeline = ctx.pipeline;
    let forwarded = ctx.forwarded;
    let prepared_forwarded = ctx.prepared_forwarded;
    let mut timeouts = ctx.timeouts;
    let mut access_log = ctx.access_log;
    let request_start = ctx.request_start;
    let mut sticky_new_session = ctx.sticky_new_session;
    let mut streaming_body = ctx.streaming_body;
    let mut usage_lifecycle = ctx.usage_lifecycle;
    let mut service_request = ctx.service_request;

    loop {
        let forward_opts = ForwardOptions {
            context: Some(forwarded),
            timeouts: Some(HttpTimeouts::new(
                timeouts.request_timeout(),
                timeouts.stream_idle_timeout(),
                timeouts.stream_total_timeout(),
            )),
        };
        let proxy_result = if let Some(incoming) = streaming_body.take() {
            if pipeline.is_empty() && inference_dispatch.is_none() {
                let method = std::mem::replace(&mut req_parts.method, http::Method::GET);
                let uri = std::mem::replace(&mut req_parts.uri, http::Uri::from_static("/"));
                let headers = std::mem::take(&mut req_parts.headers);
                state
                    .http_proxy_for_backend(&route.service_name, &backend)
                    .forward_streaming_exchange_owned(
                        &backend,
                        OwnedStreamingRequest {
                            method,
                            uri,
                            headers,
                            body: incoming,
                        },
                        forward_opts,
                        prepared_forwarded.as_deref(),
                        BackendOperationTracking::Tracked,
                    )
                    .await
            } else {
                state
                    .http_proxy_for_backend(&route.service_name, &backend)
                    .forward_streaming_exchange(
                        &backend,
                        &req_parts.method,
                        &req_parts.uri,
                        &req_parts.headers,
                        incoming,
                        forward_opts,
                    )
                    .await
            }
        } else {
            let retry_policy = if request_is_replayable(&req_parts.method, &req_parts.headers) {
                pipeline.retry_policy()
            } else {
                None
            };
            // Keep the selected backend in an atomic slot shared by the
            // operation and retry observer.  Managed inference deliberately
            // stays pinned to its exact attempt; ordinary replayable routes
            // can move to another healthy member after a discarded attempt.
            let retry_backend = Arc::new(ArcSwap::from(backend.clone()));
            let operation_backend = retry_backend.clone();
            let observer_backend = retry_backend.clone();
            let allow_backend_reselection = inference_dispatch.is_none();
            let service_name = route.service_name.clone();
            let operation_state = state.clone();
            let operation_method = Arc::new(req_parts.method.clone());
            let operation_uri = Arc::new(req_parts.uri.clone());
            let operation_headers = Arc::new(req_parts.headers.clone());
            let operation_body = body_bytes.clone();
            let result = retry_upstream(
                retry_policy,
                || {
                    let backend = operation_backend.load_full();
                    let state = operation_state.clone();
                    let method = operation_method.clone();
                    let uri = operation_uri.clone();
                    let headers = operation_headers.clone();
                    let body = operation_body.clone();
                    let proxy_service = service_name.clone();
                    async move {
                        state
                            .http_proxy_for_backend(&proxy_service, backend.as_ref())
                            .forward_streaming_response_with_options(
                                &backend,
                                method.as_ref(),
                                uri.as_ref(),
                                headers.as_ref(),
                                body,
                                forward_opts,
                            )
                            .await
                    }
                },
                |response| retryable_upstream_status(response.status),
                |result| match result {
                    Ok(response) => {
                        let backend = observer_backend.load_full();
                        pipeline.observe_upstream_response_with_request(
                            &req_parts.extensions,
                            response.status,
                        );
                        if let Some(phc) = state.passive_health.get(&service_name) {
                            phc.record_response(&backend, response.status.as_u16());
                        }
                        if allow_backend_reselection {
                            if let Some(next) = crate::entrypoint::select_retry_backend(
                                &state,
                                &service_name,
                                &backend,
                            ) {
                                if state.metrics_enabled {
                                    state.metrics.record_backend_request_id(next.metric_id());
                                }
                                observer_backend.store(next);
                            }
                        }
                    }
                    Err(error) => {
                        let backend = observer_backend.load_full();
                        pipeline.observe_upstream_failure_with_request(&req_parts.extensions);
                        if let Some(phc) = state.passive_health.get(&service_name) {
                            phc.record_error(&backend, proxy_error_status(error));
                        }
                        if allow_backend_reselection {
                            if let Some(next) = crate::entrypoint::select_retry_backend(
                                &state,
                                &service_name,
                                &backend,
                            ) {
                                if state.metrics_enabled {
                                    state.metrics.record_backend_request_id(next.metric_id());
                                }
                                observer_backend.store(next);
                            }
                        }
                    }
                },
            )
            .await;
            backend = retry_backend.load_full();
            if let Some(access_log) = access_log.as_mut() {
                access_log.set_backend(backend.url.clone());
            }
            result
        };

        match proxy_result {
            Ok(proxy_resp) => {
                // Latency-biased selection samples first-response time per
                // target; the observation map is per-snapshot bounded.
                if let (Some(dispatch), Some(attempt)) =
                    (inference_dispatch.as_ref(), inference_attempt.as_ref())
                {
                    dispatch.observe_first_response(
                        attempt.target_id(),
                        request_start
                            .elapsed()
                            .as_millis()
                            .min(u128::from(u64::MAX)) as u64,
                    );
                }
                let status_code = proxy_resp.status.as_u16();
                pipeline.observe_upstream_response_with_request(
                    &req_parts.extensions,
                    proxy_resp.status,
                );

                if let Some(phc) = state.passive_health.get(&route.service_name) {
                    phc.record_response(&backend, status_code);
                }

                let mut upstream_response = http::Response::new(proxy_resp.body);
                *upstream_response.status_mut() = proxy_resp.status;
                *upstream_response.headers_mut() = proxy_resp.headers;
                let (mut resp_parts, upstream_body) = upstream_response.into_parts();
                if let Err(error) = crate::entrypoint::agent_billing::settle(
                    state.agent_billing.as_ref(),
                    &route.service_name,
                    req_parts.uri.path(),
                    resp_parts.status,
                    &mut resp_parts.headers,
                    &req_parts.headers,
                    None,
                )
                .await
                {
                    drop(upstream_body);
                    drop(inference_admission);
                    drop(service_request);
                    if let Some(lifecycle) = usage_lifecycle.as_mut() {
                        let _ = lifecycle
                            .finish_attempt(UsageTerminalOutcome::Failed, None)
                            .await;
                    }
                    if state.metrics_enabled {
                        state.metrics.record_request(503, 0);
                        state.metrics.record_router_error(&route.router_name);
                        state.metrics.record_service_error(&route.service_name);
                    }
                    if let Some(access_log) = access_log {
                        access_log.finish(503, 0);
                    }
                    return super::usage_not_recorded(&error);
                }
                let upstream_body = ResponseBody::proxy(upstream_body);
                let upstream_body = crate::inference::track_prefix_body(
                    upstream_body,
                    inference_dispatch
                        .as_mut()
                        .and_then(|dispatch| dispatch.take_prefix_lease()),
                    resp_parts.status.is_success(),
                    content_type_is_event_stream(&resp_parts.headers),
                );
                let response_body = if pipeline.is_empty() {
                    upstream_body
                } else {
                    if let Err(e) = pipeline
                        .process_response_with_request(&req_parts.headers, &mut resp_parts)
                        .await
                    {
                        drop(upstream_body);
                        drop(inference_admission);
                        drop(service_request);
                        if let Some(lifecycle) = usage_lifecycle.as_mut() {
                            let _ = lifecycle
                                .finish_attempt(UsageTerminalOutcome::Failed, None)
                                .await;
                        }
                        if state.metrics_enabled {
                            state.metrics.record_request(500, 0);
                            state.metrics.record_router_error(&route.router_name);
                            state.metrics.record_service_error(&route.service_name);
                        }
                        if let Some(access_log) = access_log {
                            access_log.finish(500, 0);
                        }
                        return super::response_middleware_failure(&e);
                    }
                    match pipeline.prepare_response_body(&req_parts.headers, &mut resp_parts) {
                        Some(limit) => match buffer_body_up_to(upstream_body, limit).await {
                            BufferedBody::Complete(mut body) => {
                                if let Err(error) = pipeline
                                    .transform_buffered_response(
                                        &req_parts.headers,
                                        &mut resp_parts,
                                        &mut body,
                                    )
                                    .await
                                {
                                    drop(inference_admission);
                                    drop(service_request);
                                    if let Some(lifecycle) = usage_lifecycle.as_mut() {
                                        let _ = lifecycle
                                            .finish_attempt(UsageTerminalOutcome::Failed, None)
                                            .await;
                                    }
                                    if state.metrics_enabled {
                                        state.metrics.record_request(500, 0);
                                        state.metrics.record_router_error(&route.router_name);
                                        state.metrics.record_service_error(&route.service_name);
                                    }
                                    if let Some(access_log) = access_log {
                                        access_log.finish(500, 0);
                                    }
                                    return super::response_middleware_failure(&error);
                                }
                                full_body(body)
                            }
                            BufferedBody::Streaming(body) => body,
                        },
                        None => upstream_body,
                    }
                };

                // Guardrails response leg (wire feature): restore the
                // request leg's placeholders, then inspect the body
                // before release — a block withholds the completion
                // (fail-closed), a mask pass is response-side redaction.
                // A streamed (text/event-stream) response gates the same
                // contract on the reassembled body: the whole stream
                // buffers to EOF inside the bound, placeholders restore
                // across chunk splits on the reassembled bytes (the
                // wire.rs restoration note), and the gated body delivers as
                // one release — a guardrailed grant trades progressive
                // streaming for the guarantee that no ungated byte ever
                // reaches the agent.
                #[cfg(feature = "wire")]
                let response_body = if let Some((gate, restores)) = inference_dispatch
                    .as_ref()
                    .and_then(|dispatch| dispatch.response_guardrails())
                {
                    if (200..300).contains(&status_code) {
                        // `None` (no gateable body), `Ok` (fully buffered),
                        // or `Err(reason)` (fail-closed shape violation).
                        // The body moves into whichever branch buffers it;
                        // the cell keeps the pass-through path owned.
                        let mut body_cell = Some(response_body);
                        let candidate: Option<Result<Bytes, &'static str>> =
                            if content_type_is_event_stream(&resp_parts.headers) {
                                match buffer_body_up_to(
                                    body_cell.take().expect("the response body buffers once"),
                                    1024 * 1024,
                                )
                                .await
                                {
                                    BufferedBody::Complete(bytes) => Some(Ok(bytes)),
                                    // Over the bound mid-stream: the gate
                                    // never releases an ungated body it
                                    // committed to inspecting.
                                    BufferedBody::Streaming(_) => {
                                        Some(Err("streamed response exceeded the guardrail buffer"))
                                    }
                                }
                            } else {
                                let bound = resp_parts
                                    .headers
                                    .get(CONTENT_LENGTH)
                                    .and_then(|value| value.to_str().ok())
                                    .and_then(|value| value.parse::<usize>().ok())
                                    .unwrap_or(0);
                                if bound > 0 && bound <= 1024 * 1024 {
                                    match buffer_body_up_to(
                                        body_cell.take().expect("the response body buffers once"),
                                        bound,
                                    )
                                    .await
                                    {
                                        BufferedBody::Complete(bytes) if bytes.len() == bound => {
                                            Some(Ok(bytes))
                                        }
                                        // Incomplete after the declared
                                        // length or mid-stream: fail closed.
                                        BufferedBody::Streaming(_) | BufferedBody::Complete(_) => {
                                            Some(Err("incomplete response body"))
                                        }
                                    }
                                } else {
                                    None
                                }
                            };
                        match candidate {
                            Some(Ok(bytes)) => {
                                let restored = match restores {
                                    Some(restores) => gate.ungate_response(
                                        &String::from_utf8_lossy(&bytes),
                                        &restores,
                                    ),
                                    None => String::from_utf8_lossy(&bytes).into_owned(),
                                };
                                match gate.gate_response(&restored) {
                                    crate::wire::Gated::Block { status: _, reason } => {
                                        tracing::warn!(
                                            reason,
                                            "Guardrails withheld an inference response"
                                        );
                                        if let Some(lifecycle) = usage_lifecycle.as_mut() {
                                            let _ = lifecycle
                                                .finish_attempt(UsageTerminalOutcome::Failed, None)
                                                .await;
                                        }
                                        let request_id = inference_attempt
                                            .as_ref()
                                            .map(|identity| identity.request().request_id())
                                            .unwrap_or_else(uuid::Uuid::new_v4);
                                        let bytes_response =
                                            InferenceAccessError::GuardrailBlocked { reason }
                                                .into_response_with_request_id(request_id);
                                        let status = bytes_response.status();
                                        let (parts, body) = bytes_response.into_parts();
                                        let response_bytes = body.len() as u64;
                                        let mut response =
                                            http::Response::from_parts(parts, full_body(body));
                                        *response.status_mut() = status;
                                        if let Some(identity) = inference_attempt.as_ref() {
                                            identity.attach_response_header(&mut response);
                                        }
                                        if let Some(access_log) = access_log {
                                            access_log.finish(status.as_u16(), response_bytes);
                                        }
                                        drop(inference_admission);
                                        drop(service_request);
                                        return track_usage_response(response, usage_lifecycle);
                                    }
                                    crate::wire::Gated::Forward {
                                        body: gated,
                                        redacted,
                                        ..
                                    } => {
                                        if redacted > 0 {
                                            tracing::info!(
                                                redacted,
                                                "Guardrails redacted an inference response"
                                            );
                                        }
                                        let encoded = Bytes::from(gated);
                                        resp_parts.headers.remove(http::header::TRANSFER_ENCODING);
                                        if let Ok(len) =
                                            http::HeaderValue::from_str(&encoded.len().to_string())
                                        {
                                            resp_parts.headers.insert(CONTENT_LENGTH, len);
                                        }
                                        full_body(encoded)
                                    }
                                }
                            }
                            Some(Err(reason)) => {
                                tracing::warn!(
                                    reason,
                                    "Guardrails withheld an ungatable inference response"
                                );
                                if let Some(lifecycle) = usage_lifecycle.as_mut() {
                                    let _ = lifecycle
                                        .finish_attempt(UsageTerminalOutcome::Failed, None)
                                        .await;
                                }
                                let request_id = inference_attempt
                                    .as_ref()
                                    .map(|identity| identity.request().request_id())
                                    .unwrap_or_else(uuid::Uuid::new_v4);
                                let bytes_response = InferenceAccessError::GuardrailBlocked {
                                    reason: reason.into(),
                                }
                                .into_response_with_request_id(request_id);
                                let status = bytes_response.status();
                                let (parts, body) = bytes_response.into_parts();
                                let response_bytes = body.len() as u64;
                                let mut response =
                                    http::Response::from_parts(parts, full_body(body));
                                *response.status_mut() = status;
                                if let Some(identity) = inference_attempt.as_ref() {
                                    identity.attach_response_header(&mut response);
                                }
                                if let Some(access_log) = access_log {
                                    access_log.finish(status.as_u16(), response_bytes);
                                }
                                drop(inference_admission);
                                drop(service_request);
                                return track_usage_response(response, usage_lifecycle);
                            }
                            None => body_cell.take().expect("the response body buffers once"),
                        }
                    } else {
                        response_body
                    }
                } else {
                    response_body
                };

                // Cross-protocol back-translation: a buffered 2xx JSON
                // response from a translated attempt returns in the endpoint
                // dialect before envelope wrapping or native-wire
                // pass-through. The upstream response has already started,
                // so an untranslatable or incomplete body fails closed —
                // no fallback.
                let response_body = if let Some(direction) = inference_dispatch
                    .as_ref()
                    .and_then(|dispatch| dispatch.translate_response())
                {
                    let known_len = resp_parts
                        .headers
                        .get(CONTENT_LENGTH)
                        .and_then(|value| value.to_str().ok())
                        .and_then(|value| value.parse::<usize>().ok());
                    if (200..300).contains(&status_code)
                        && !content_type_is_event_stream(&resp_parts.headers)
                        && known_len.is_some_and(|len| len > 0 && len <= 1024 * 1024)
                    {
                        let expected_len = known_len.expect("content-length");
                        let translated = match buffer_body_up_to(response_body, expected_len).await
                        {
                            BufferedBody::Complete(bytes) if bytes.len() == expected_len => {
                                let created_unix = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .map(|duration| duration.as_secs() as i64)
                                    .unwrap_or(0);
                                serde_json::from_slice(&bytes)
                                    .map_err(|error| error.to_string())
                                    .and_then(|value| {
                                        direction
                                            .translate_json_response(&value, created_unix)
                                            .map_err(|error| error.0)
                                    })
                                    .and_then(|value| {
                                        serde_json::to_vec(&value)
                                            .map_err(|error| error.to_string())
                                    })
                                    .ok()
                            }
                            _ => None,
                        };
                        match translated {
                            Some(translated) => {
                                resp_parts.headers.remove(http::header::TRANSFER_ENCODING);
                                if let Ok(len) =
                                    http::HeaderValue::from_str(&translated.len().to_string())
                                {
                                    resp_parts.headers.insert(CONTENT_LENGTH, len);
                                }
                                full_body(Bytes::from(translated))
                            }
                            None => {
                                tracing::warn!("Cross-protocol response translation failed");
                                if let Some(lifecycle) = usage_lifecycle.as_mut() {
                                    let _ = lifecycle
                                        .finish_attempt(UsageTerminalOutcome::Failed, None)
                                        .await;
                                }
                                if state.metrics_enabled {
                                    state.metrics.record_request(502, 0);
                                    state.metrics.record_router_error(&route.router_name);
                                    state.metrics.record_service_error(&route.service_name);
                                }
                                let request_id = inference_attempt
                                    .as_ref()
                                    .map(|identity| identity.request().request_id())
                                    .unwrap_or_else(uuid::Uuid::new_v4);
                                let bytes_response = InferenceAccessError::UpstreamExhausted
                                    .into_response_with_request_id(request_id);
                                let status = bytes_response.status();
                                let (parts, body) = bytes_response.into_parts();
                                let response_bytes = body.len() as u64;
                                let mut response =
                                    http::Response::from_parts(parts, full_body(body));
                                *response.status_mut() = status;
                                if let Some(identity) = inference_attempt.as_ref() {
                                    identity.attach_response_header(&mut response);
                                }
                                if let Some(access_log) = access_log {
                                    access_log.finish(status.as_u16(), response_bytes);
                                }
                                drop(inference_admission);
                                drop(service_request);
                                return track_usage_response(response, usage_lifecycle);
                            }
                        }
                    } else {
                        response_body
                    }
                } else {
                    response_body
                };

                // Work Connect: wrap non-SSE JSON successes in ApiEnvelope when the
                // upstream advertises a bounded Content-Length. Incomplete bodies
                // fail closed (do not stream a truncated JSON rewrite).
                let known_len = resp_parts
                    .headers
                    .get(CONTENT_LENGTH)
                    .and_then(|value| value.to_str().ok())
                    .and_then(|value| value.parse::<usize>().ok());
                let should_wrap = inference_attempt.is_some()
                    && inference_dispatch.as_ref().is_some_and(|dispatch| {
                        !dispatch.stream_requested() && !dispatch.native_wire()
                    })
                    && (200..300).contains(&status_code)
                    && !content_type_is_event_stream(&resp_parts.headers)
                    && known_len.is_some_and(|len| len > 0 && len <= 1024 * 1024);

                let response_body = if should_wrap {
                    let expected_len = known_len.expect("content-length");
                    match buffer_body_up_to(response_body, expected_len).await {
                        BufferedBody::Complete(bytes) if bytes.len() == expected_len => {
                            let request_id = inference_attempt
                                .as_ref()
                                .expect("inference attempt")
                                .request()
                                .request_id();
                            match wrap_json_success_body(&bytes, request_id) {
                                Some(wrapped) => {
                                    resp_parts.headers.remove(http::header::TRANSFER_ENCODING);
                                    if let Ok(len) =
                                        http::HeaderValue::from_str(&wrapped.len().to_string())
                                    {
                                        resp_parts.headers.insert(CONTENT_LENGTH, len);
                                    }
                                    resp_parts.headers.insert(
                                        CONTENT_TYPE,
                                        http::HeaderValue::from_static("application/json"),
                                    );
                                    full_body(wrapped)
                                }
                                None => full_body(bytes),
                            }
                        }
                        _ => {
                            if let Some(lifecycle) = usage_lifecycle.as_mut() {
                                let _ = lifecycle
                                    .finish_attempt(UsageTerminalOutcome::Failed, None)
                                    .await;
                            }
                            if state.metrics_enabled {
                                state.metrics.record_request(502, 0);
                                state.metrics.record_router_latency(
                                    &route.router_name,
                                    request_start.elapsed().as_micros() as u64,
                                );
                                state.metrics.record_router_error(&route.router_name);
                                state.metrics.record_service_error(&route.service_name);
                            }
                            let request_id = inference_attempt
                                .as_ref()
                                .map(|identity| identity.request().request_id())
                                .unwrap_or_else(uuid::Uuid::new_v4);
                            let bytes_response = InferenceAccessError::UpstreamExhausted
                                .into_response_with_request_id(request_id);
                            let status = bytes_response.status();
                            let (parts, body) = bytes_response.into_parts();
                            let response_bytes = body.len() as u64;
                            let mut response = http::Response::from_parts(parts, full_body(body));
                            *response.status_mut() = status;
                            if let Some(identity) = inference_attempt.as_ref() {
                                identity.attach_response_header(&mut response);
                            }
                            if let Some(access_log) = access_log {
                                access_log.finish(status.as_u16(), response_bytes);
                            }
                            drop(inference_admission);
                            drop(service_request);
                            return track_usage_response(response, usage_lifecycle);
                        }
                    }
                } else {
                    response_body
                };

                if let (Some(new_id), Some(sticky_mgr)) = (
                    &sticky_new_session,
                    state.sticky_managers.get(&route.service_name),
                ) {
                    match http::HeaderValue::from_str(&sticky_mgr.build_cookie(new_id)) {
                        Ok(cookie) => {
                            resp_parts.headers.append(http::header::SET_COOKIE, cookie);
                        }
                        Err(error) => {
                            // Declared sticky affinity must not soft-skip Set-Cookie.
                            drop(inference_admission);
                            drop(service_request);
                            if let Some(lifecycle) = usage_lifecycle.as_mut() {
                                let _ = lifecycle
                                    .finish_attempt(UsageTerminalOutcome::Failed, None)
                                    .await;
                            }
                            if state.metrics_enabled {
                                state.metrics.record_request(500, 0);
                                state.metrics.record_router_error(&route.router_name);
                                state.metrics.record_service_error(&route.service_name);
                            }
                            if let Some(access_log) = access_log {
                                access_log.finish(500, 0);
                            }
                            return super::response_middleware_failure(&format!(
                                "sticky session cookie is invalid: {error}"
                            ));
                        }
                    }
                }

                if state.metrics_enabled {
                    state.metrics.record_request(status_code, 0);
                    state.metrics.record_router_latency(
                        &route.router_name,
                        request_start.elapsed().as_micros() as u64,
                    );
                    if status_code >= 400 {
                        state.metrics.record_router_error(&route.router_name);
                        state.metrics.record_service_error(&route.service_name);
                    }
                }

                let client_status = resp_parts.status.as_u16();
                let response_identity = inference_attempt.clone();
                let response_metrics = state.metrics_enabled.then(|| state.metrics.clone());
                let track_response_body = inference_admission.is_some()
                    || inference_attempt.is_some()
                    || service_request.is_some()
                    || access_log.is_some()
                    || response_metrics.is_some();
                let hold_admission = inference_admission.is_some();
                let response_body = if track_response_body {
                    let mut access_log_guard = AccessLogGuard::new(access_log, client_status);
                    ResponseBody::boxed(response_body.map_frame(move |frame| {
                        let _inference_attempt = &inference_attempt;
                        // Admission is reconciled by track_token_budget_response;
                        // keep a cheap boolean so the body map still runs when
                        // only the guard was present.
                        let _hold_admission = hold_admission;
                        if let Some(bytes) = frame.data_ref() {
                            if !bytes.is_empty() {
                                if let Some(request) = service_request.as_mut() {
                                    request.record_ttft_once();
                                }
                            }
                            access_log_guard.record_bytes(bytes.len() as u64);
                            if let Some(metrics) = response_metrics.as_ref() {
                                metrics.record_response_bytes(bytes.len() as u64);
                            }
                        }
                        frame
                    }))
                } else {
                    response_body
                };
                let mut response = http::Response::from_parts(resp_parts, response_body);
                if let Some(identity) = response_identity.as_ref() {
                    identity.attach_response_header(&mut response);
                }
                let observed_tokens = usage_lifecycle
                    .as_ref()
                    .map(UsageRequestLifecycle::observed_tokens_handle);
                let attribution = inference_dispatch
                    .as_ref()
                    .and_then(|dispatch| dispatch.usage_attribution());
                let response = track_attributed_token_budget_response(
                    response,
                    inference_admission,
                    observed_tokens,
                    attribution,
                );
                return track_usage_response(response, usage_lifecycle);
            }
            Err(error) => {
                let error_status = proxy_error_status(&error);
                if error.permits_pre_response_fallback() {
                    pipeline.observe_upstream_failure_with_request(&req_parts.extensions);
                    if let Some(phc) = state.passive_health.get(&route.service_name) {
                        phc.record_error(&backend, error_status);
                    }
                }

                if error.permits_pre_response_fallback() {
                    if let Some(dispatch) = inference_dispatch.as_mut() {
                        let failed_service = route.service_name.clone();
                        let failed_backend = backend.url.clone();
                        if let Some(identity) = inference_attempt.as_ref() {
                            tracing::warn!(
                                request_id = %identity.request().request_id(),
                                attempt_id = %identity.attempt_id(),
                                target_id = %identity.target_id(),
                                service = %failed_service,
                                backend = %failed_backend,
                                error = %error,
                                "Managed inference attempt failed before response"
                            );
                        }
                        match dispatch.prepare_next(
                            &state,
                            &mut req_parts.headers,
                            access_log.as_mut(),
                        ) {
                            Ok(prepared) => {
                                let usage_error = if let Some(lifecycle) = usage_lifecycle.as_mut()
                                {
                                    if let Err(error) = lifecycle
                                        .finish_attempt(UsageTerminalOutcome::Fallback, None)
                                        .await
                                    {
                                        Some(error)
                                    } else {
                                        lifecycle.begin_attempt(&prepared.identity).await.err()
                                    }
                                } else {
                                    None
                                };
                                if let Some(error) = usage_error {
                                    tracing::error!(
                                        error = %error,
                                        "Managed inference fallback stopped because durable usage became unavailable"
                                    );
                                } else {
                                    if state.metrics_enabled {
                                        state.metrics.record_service_error(&failed_service);
                                    }
                                    if let Some(request) = service_request.as_mut() {
                                        request.retarget(&prepared.service_name);
                                    }
                                    Arc::make_mut(&mut route).service_name = prepared.service_name;
                                    backend = prepared.backend;
                                    body_bytes = prepared.body;
                                    timeouts = prepared.timeouts;
                                    sticky_new_session = prepared.sticky_new_session;
                                    inference_attempt = Some(prepared.identity);
                                    continue;
                                }
                            }
                            Err(preparation_error) => {
                                tracing::warn!(
                                    service = %failed_service,
                                    backend = %failed_backend,
                                    error = ?preparation_error,
                                    "Managed inference fallback exhausted"
                                );
                                if let Some(lifecycle) = usage_lifecycle.as_mut() {
                                    if let Err(usage_error) = lifecycle
                                        .finish_attempt(UsageTerminalOutcome::Failed, None)
                                        .await
                                    {
                                        tracing::error!(
                                            error = %usage_error,
                                            "Managed inference attempt terminal append failed"
                                        );
                                    }
                                }
                                if state.metrics_enabled {
                                    state.metrics.record_request(502, 0);
                                    state.metrics.record_router_latency(
                                        &route.router_name,
                                        request_start.elapsed().as_micros() as u64,
                                    );
                                    state.metrics.record_router_error(&route.router_name);
                                    state.metrics.record_service_error(&failed_service);
                                }
                                let request_id = inference_attempt
                                    .as_ref()
                                    .map(|identity| identity.request().request_id())
                                    .unwrap_or_else(uuid::Uuid::new_v4);
                                let bytes_response = InferenceAccessError::UpstreamExhausted
                                    .into_response_with_request_id(request_id);
                                let status = bytes_response.status();
                                let (parts, body) = bytes_response.into_parts();
                                let response_bytes = body.len() as u64;
                                let mut response =
                                    http::Response::from_parts(parts, full_body(body));
                                *response.status_mut() = status;
                                if let Some(identity) = inference_attempt.as_ref() {
                                    identity.attach_response_header(&mut response);
                                }
                                if let Some(access_log) = access_log {
                                    access_log.finish(status.as_u16(), response_bytes);
                                }
                                drop(inference_admission);
                                drop(service_request);
                                return track_usage_response(response, usage_lifecycle);
                            }
                        }
                    }
                }

                if let Some(lifecycle) = usage_lifecycle.as_mut() {
                    if let Err(usage_error) = lifecycle
                        .finish_attempt(UsageTerminalOutcome::Failed, None)
                        .await
                    {
                        tracing::error!(
                            error = %usage_error,
                            "Managed inference attempt terminal append failed"
                        );
                    }
                }
                tracing::error!(error = %error, backend = backend.url, "Proxy error");
                if state.metrics_enabled {
                    state.metrics.record_request(error_status, 0);
                    state.metrics.record_router_latency(
                        &route.router_name,
                        request_start.elapsed().as_micros() as u64,
                    );
                    state.metrics.record_router_error(&route.router_name);
                    state.metrics.record_service_error(&route.service_name);
                }

                let (mut err_parts, _) = http::Response::builder()
                    .status(error_status)
                    .body(())
                    .unwrap()
                    .into_parts();
                let mut body = Bytes::from(crate::error::json_error_body(error.to_string()));
                if !pipeline.is_empty() {
                    if let Err(mw_err) = pipeline
                        .process_buffered_response(&req_parts.headers, &mut err_parts, &mut body)
                        .await
                    {
                        if let Some(access_log) = access_log {
                            access_log.finish(500, 0);
                        }
                        return track_usage_response(
                            super::response_middleware_failure(&mw_err),
                            usage_lifecycle,
                        );
                    }
                }
                let mut builder = http::Response::builder().status(error_status);
                for (key, value) in err_parts.headers.iter() {
                    builder = builder.header(key, value);
                }
                let response_bytes = body.len() as u64;
                let mut response = builder
                    .body(crate::entrypoint::protocol::full_body(body))
                    .unwrap();
                if let Some(identity) = inference_attempt.as_ref() {
                    identity.attach_response_header(&mut response);
                }
                if let Some(access_log) = access_log {
                    access_log.finish(error_status, response_bytes);
                }
                return track_usage_response(response, usage_lifecycle);
            }
        }
    }
}

pub(in crate::entrypoint) fn proxy_error_status(error: &GatewayError) -> u16 {
    match error {
        GatewayError::UpstreamTimeout(_) => 504,
        GatewayError::ServiceUnavailable(_) | GatewayError::UpstreamTransport(_) => 503,
        _ => 502,
    }
}

/// Count a dial failure against passive health when it is an upstream
/// transport or timeout. `ServiceUnavailable` stays excluded: that variant is
/// also local admission (draining generation), which must not eject a backend.
pub(in crate::entrypoint) fn record_upstream_dial_failure(
    passive_health: Option<&crate::service::passive_health::PassiveHealthCheck>,
    backend: &Arc<crate::service::Backend>,
    error: &GatewayError,
) {
    if !error.permits_pre_response_fallback() {
        return;
    }
    let Some(passive_health) = passive_health else {
        return;
    };
    passive_health.record_error(backend, proxy_error_status(error));
}

pub(super) fn content_type_is_event_stream(headers: &http::HeaderMap) -> bool {
    headers
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.to_ascii_lowercase().contains("text/event-stream"))
}
