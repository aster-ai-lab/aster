//! SSE/Streaming protocol handler

use crate::entrypoint::protocol::http_handler::proxy_error_status;
use crate::entrypoint::protocol::{
    full_body, request_is_replayable, retry_upstream, retryable_upstream_status, ProtocolContext,
    ResponseBody,
};

use super::body_buffer::{buffer_body_up_to, BufferedBody};
use crate::inference::{track_attributed_token_budget_response, InferenceAccessError};
use crate::observability::access_log::AccessLogGuard;
use crate::proxy::{ForwardOptions, HttpTimeouts};
use crate::usage::{track_usage_response, UsageRequestLifecycle, UsageTerminalOutcome};
use arc_swap::ArcSwap;
use bytes::Bytes;
use http::Response;
use http_body_util::BodyExt;
use std::sync::Arc;

pub async fn handle_sse_dispatch(ctx: ProtocolContext) -> Response<ResponseBody> {
    let inference_admission = ctx.inference_admission;
    let mut inference_attempt = ctx.inference_attempt;
    let mut inference_dispatch = ctx.inference_dispatch;
    let mut backend = ctx.backend;
    let state = ctx.state.clone();
    let mut route = ctx.route;
    let mut req_parts = ctx.req_parts;
    let mut body_bytes = ctx.body_bytes;
    let pipeline = ctx.pipeline;
    let forwarded = ctx.forwarded;
    let mut access_log = ctx.access_log;
    let request_start = ctx.request_start;
    let mut timeouts = ctx.timeouts;
    let mut sticky_new_session = ctx.sticky_new_session;
    let mut usage_lifecycle = ctx.usage_lifecycle;
    let mut service_request = ctx.service_request;

    loop {
        let retry_policy = if request_is_replayable(&req_parts.method, &req_parts.headers) {
            pipeline.retry_policy()
        } else {
            None
        };
        // Ordinary replayable SSE requests may move to another healthy
        // backend after a discarded pre-response attempt. Managed inference
        // remains pinned to its exact attempt until its fallback state moves.
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
                            ForwardOptions {
                                context: Some(forwarded),
                                timeouts: Some(HttpTimeouts::new(
                                    timeouts.request_timeout(),
                                    timeouts.stream_idle_timeout(),
                                    timeouts.stream_total_timeout(),
                                )),
                            },
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
                        if let Some(next) =
                            crate::entrypoint::select_retry_backend(&state, &service_name, &backend)
                        {
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
                        if let Some(next) =
                            crate::entrypoint::select_retry_backend(&state, &service_name, &backend)
                        {
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
        match result {
            Ok(stream_resp) => {
                let status_code = stream_resp.status.as_u16();
                pipeline.observe_upstream_response_with_request(
                    &req_parts.extensions,
                    stream_resp.status,
                );

                if let Some(phc) = state.passive_health.get(&route.service_name) {
                    phc.record_response(&backend, status_code);
                }

                let mut upstream_response = http::Response::new(stream_resp.body);
                *upstream_response.status_mut() = stream_resp.status;
                *upstream_response.headers_mut() = stream_resp.headers;
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

                if !pipeline.is_empty() {
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
                }

                if let (Some(new_id), Some(sticky_mgr)) = (
                    &sticky_new_session,
                    state.sticky_managers.get(&route.service_name),
                ) {
                    match http::HeaderValue::from_str(&sticky_mgr.build_cookie(new_id)) {
                        Ok(cookie) => {
                            resp_parts.headers.append(http::header::SET_COOKIE, cookie);
                        }
                        Err(error) => {
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

                let client_status = resp_parts.status.as_u16();
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
                let response_identity = inference_attempt.clone();
                let response_metrics = state.metrics_enabled.then(|| state.metrics.clone());
                // Cross-protocol streaming: translate the upstream SSE in
                // the innermost layer so every downstream observer (metrics,
                // admission reconciliation, usage lifecycle) sees frames in
                // the endpoint dialect. Only the Anthropic→OpenAI direction
                // has a stream translator; the reverse direction refused at
                // admission.
                // Guardrails response leg for streamed exchanges — the
                // same contract as the buffered leg in http_handler: the
                // whole stream buffers to EOF inside the bound, the
                // request-leg placeholders restore across chunk splits on
                // the reassembled bytes, and the gate runs before any
                // byte reaches the agent. A block withholds everything
                // (nothing was streamed yet — fail closed); a pass
                // releases the gated body as one delivery, trading
                // progressive streaming for the guarantee that no ungated
                // byte ever leaves.
                #[cfg(feature = "wire")]
                let upstream_body = if let Some((gate, restores)) = inference_dispatch
                    .as_ref()
                    .and_then(|dispatch| dispatch.response_guardrails())
                {
                    if (200..300).contains(&status_code) {
                        match buffer_body_up_to(ResponseBody::proxy(upstream_body), 1024 * 1024)
                            .await
                        {
                            BufferedBody::Complete(bytes) => {
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
                                            "Guardrails withheld a streamed inference response"
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
                                                "Guardrails redacted a streamed inference response"
                                            );
                                        }
                                        let encoded = Bytes::from(gated);
                                        resp_parts.headers.remove(http::header::TRANSFER_ENCODING);
                                        resp_parts.headers.remove(http::header::CONTENT_LENGTH);
                                        if let Ok(len) =
                                            http::HeaderValue::from_str(&encoded.len().to_string())
                                        {
                                            resp_parts
                                                .headers
                                                .insert(http::header::CONTENT_LENGTH, len);
                                        }
                                        full_body(encoded)
                                    }
                                }
                            }
                            // Over the bound mid-stream: the gate never
                            // releases an ungated body it committed to
                            // inspecting.
                            BufferedBody::Streaming(_) => {
                                tracing::warn!(
                                    "Guardrails withheld a streamed response over the buffer bound"
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
                                    reason: "streamed response exceeded the guardrail buffer"
                                        .into(),
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
                        }
                    } else {
                        ResponseBody::proxy(upstream_body)
                    }
                } else {
                    ResponseBody::proxy(upstream_body)
                };
                #[cfg(not(feature = "wire"))]
                let upstream_body = ResponseBody::proxy(upstream_body);
                let upstream_body = crate::inference::track_prefix_body(
                    upstream_body,
                    inference_dispatch
                        .as_mut()
                        .and_then(|dispatch| dispatch.take_prefix_lease()),
                    resp_parts.status.is_success(),
                    super::http_handler::content_type_is_event_stream(&resp_parts.headers),
                );
                let upstream_body = match inference_dispatch
                    .as_ref()
                    .and_then(|dispatch| dispatch.translate_response())
                {
                    Some(direction) => {
                        // The upstream Content-Length describes the source
                        // dialect's framing; the translated stream is a
                        // different length and frames itself.
                        resp_parts.headers.remove(http::header::CONTENT_LENGTH);
                        let proxied = upstream_body;
                        match direction {
                            crate::translation::ResponseTranslation::ToOpenai => {
                                let created_unix = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .map(|duration| duration.as_secs() as i64)
                                    .unwrap_or(0);
                                ResponseBody::boxed(
                                    crate::translation_stream::TranslatedStreamBody::to_openai(
                                        proxied,
                                        created_unix,
                                    ),
                                )
                            }
                            crate::translation::ResponseTranslation::ToAnthropic => {
                                ResponseBody::boxed(
                                    crate::translation_stream::TranslatedStreamBody::to_anthropic(
                                        proxied,
                                    ),
                                )
                            }
                            crate::translation::ResponseTranslation::ResponsesFromChat => {
                                ResponseBody::boxed(
                                    crate::translation_stream::TranslatedStreamBody::to_responses(
                                        proxied,
                                    ),
                                )
                            }
                        }
                    }
                    None => upstream_body,
                };
                let track_response_body = inference_admission.is_some()
                    || inference_attempt.is_some()
                    || service_request.is_some()
                    || access_log.is_some()
                    || response_metrics.is_some();
                let hold_admission = inference_admission.is_some();
                let response_body = if track_response_body {
                    let mut access_log_guard = AccessLogGuard::new(access_log, client_status);
                    ResponseBody::boxed(upstream_body.map_frame(move |frame| {
                        let _inference_attempt = &inference_attempt;
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
                    upstream_body
                };

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
                                "Managed inference SSE attempt failed before response"
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
                                        "Managed inference SSE fallback stopped because durable usage became unavailable"
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
                                    "Managed inference SSE fallback exhausted"
                                );
                                if let Some(lifecycle) = usage_lifecycle.as_mut() {
                                    if let Err(usage_error) = lifecycle
                                        .finish_attempt(UsageTerminalOutcome::Failed, None)
                                        .await
                                    {
                                        tracing::error!(
                                            error = %usage_error,
                                            "Managed inference SSE attempt terminal append failed"
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
                            "Managed inference SSE attempt terminal append failed"
                        );
                    }
                }
                tracing::error!(error = %error, backend = backend.url, "SSE proxy error");
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
                if let Err(mw_err) = pipeline
                    .process_response_with_request(&req_parts.headers, &mut err_parts)
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
                let mut builder = http::Response::builder().status(error_status);
                for (key, value) in err_parts.headers.iter() {
                    builder = builder.header(key, value);
                }
                let body = Bytes::from(crate::error::json_error_body(error.to_string()));
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
