//! Replayable upstream-attempt preparation for managed inference.

use super::{inference_service_is_available, GatewayState};
use crate::config::InferenceProtocol;
use crate::inference::{
    AuthenticatedInference, InferenceAccessError, InferenceAttemptIdentity, InferenceAuthorizer,
    InferenceDispatchTarget, InferenceRequestIdentity, InferenceWorkerCandidate,
    InferenceWorkerPairSelection, InferenceWorkerPairSelectionRequest, InferenceWorkerSelection,
    InferenceWorkerSelectionRequest, OpenAiJsonRequest, PrefixHint, PrefixLease, PrefixTarget,
};
use crate::observability::access_log::RequestAccessLog;
use crate::service::{Backend, ServiceTimeouts};
use crate::translation::ResponseTranslation;
use bytes::Bytes;
use http::header::CONTENT_LENGTH;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Request state retained until an upstream response becomes available.
///
/// The validated JSON document is intentionally retained only in memory and
/// only for the lifetime of one request. A fallback can therefore rewrite the
/// model and replay the request without reparsing client input.
pub(crate) struct InferenceDispatchState {
    authorizer: Arc<InferenceAuthorizer>,
    authenticated: AuthenticatedInference,
    model_alias: String,
    request: OpenAiJsonRequest,
    request_identity: InferenceRequestIdentity,
    next_priority: Option<u32>,
    /// HR3b: the routing window's effective targets, computed on the
    /// first attempt (None until then, or when no plan applies).
    effective_targets: Option<Vec<crate::config::InferenceTargetConfig>>,
    /// HR5: the routing facts for the usage lifecycle (prompt-free).
    routing_facts: Option<crate::usage::RoutingEvidence>,
    attempt_count: u32,
    scheduled_target: Option<InferenceDispatchTarget>,
    attempted_worker_units: HashSet<String>,
    native_wire: bool,
    /// Back-translation for the currently dispatched cross-protocol attempt.
    translate_response: Option<ResponseTranslation>,
    prefix_hint: Option<PrefixHint>,
    prefix_signature: Option<[u8; 32]>,
    prefix_lease: Option<PrefixLease>,
    /// Placeholder restores from the request-leg guardrail mask; the
    /// response leg unmasks with these before its own inspection.
    #[cfg(feature = "wire")]
    guardrail_restores: Option<HashMap<String, String>>,
}

/// One concrete upstream attempt prepared from the active snapshot.
pub(crate) struct PreparedInferenceAttempt {
    pub(crate) service_name: String,
    pub(crate) backend: Arc<Backend>,
    pub(crate) body: Bytes,
    pub(crate) timeouts: ServiceTimeouts,
    pub(crate) sticky_new_session: Option<String>,
    pub(crate) identity: InferenceAttemptIdentity,
    pub(crate) distributed: Option<PreparedDistributedAttempt>,
}

/// Exact pair and runtime policy retained for Gateway-owned P/D orchestration.
pub(crate) struct PreparedDistributedAttempt {
    pub(crate) prefill_backend: Arc<Backend>,
    pub(crate) workers: InferenceWorkerPairSelection,
    pub(crate) config: crate::config::InferenceDistributedServingConfig,
}

impl InferenceDispatchState {
    pub(crate) fn new(
        authorizer: Arc<InferenceAuthorizer>,
        authenticated: AuthenticatedInference,
        model_alias: String,
        request: OpenAiJsonRequest,
        request_identity: InferenceRequestIdentity,
    ) -> Self {
        Self {
            authorizer,
            authenticated,
            model_alias,
            request,
            request_identity,
            next_priority: Some(0),
            effective_targets: None,
            routing_facts: None,
            attempt_count: 0,
            scheduled_target: None,
            attempted_worker_units: HashSet::new(),
            native_wire: false,
            translate_response: None,
            prefix_hint: None,
            prefix_signature: None,
            prefix_lease: None,
            #[cfg(feature = "wire")]
            guardrail_restores: None,
        }
    }

    /// Serve upstream bodies unwrapped and ask OpenAI-compatible upstreams
    /// to report streamed usage.
    pub(crate) fn use_native_wire(&mut self) {
        self.native_wire = true;
        if self.request_identity.endpoint().protocol() == crate::config::InferenceProtocol::Openai {
            self.request.request_stream_usage();
        }
    }

    pub(crate) fn native_wire(&self) -> bool {
        self.native_wire
    }

    /// Usage attribution for signed-in callers; Cloud keys meter through the
    /// durable usage spool instead.
    pub(crate) fn usage_attribution(&self) -> Option<crate::inference::UsageAttribution> {
        let subject = self.authorizer.principal_subject(self.authenticated)?;
        Some(crate::inference::UsageAttribution {
            subject,
            model: self.model_alias.clone(),
            endpoint: self.request_identity.endpoint(),
            request_id: self.request_identity.request_id(),
        })
    }

    pub(crate) fn request_identity(&self) -> &InferenceRequestIdentity {
        &self.request_identity
    }

    pub(crate) fn model_alias(&self) -> &str {
        &self.model_alias
    }

    pub(crate) fn stream_requested(&self) -> bool {
        self.request.stream_requested()
    }

    /// Feed one first-response latency observation into the authorizer's
    /// bounded per-snapshot EWMA store (invariant 11).
    pub(crate) fn observe_first_response(&self, target_id: uuid::Uuid, latency_ms: u64) {
        self.authorizer
            .observe_target_latency(target_id, latency_ms);
    }

    pub(crate) fn take_prefix_lease(&mut self) -> Option<PrefixLease> {
        self.prefix_lease.take()
    }

    /// The grant's guardrails gate for the response leg, together with the
    /// placeholder restores captured on the request leg (wire feature).
    #[cfg(feature = "wire")]
    pub(crate) fn response_guardrails(
        &self,
    ) -> Option<(
        std::sync::Arc<crate::wire::WireGate>,
        Option<HashMap<String, String>>,
    )> {
        let name = self
            .authorizer
            .grant_guardrails(self.authenticated, chrono::Utc::now())
            .ok()
            .flatten()?;
        let gate = self.authorizer.wire_gate(&name)?;
        Some((gate, self.guardrail_restores.clone()))
    }

    /// Gate the dispatch body through the grant's guardrails policy:
    /// blocked bodies refuse naming the reason; masked bodies carry their
    /// placeholder restores for the response leg.
    #[cfg(feature = "wire")]
    fn apply_guardrails(&mut self, body: Bytes) -> Result<Bytes, InferenceAccessError> {
        let Some(name) = self
            .authorizer
            .grant_guardrails(self.authenticated, chrono::Utc::now())?
        else {
            return Ok(body);
        };
        let Some(gate) = self.authorizer.wire_gate(&name) else {
            return Ok(body);
        };
        let body_text = String::from_utf8_lossy(&body).into_owned();
        match gate.gate_request(&body_text) {
            crate::wire::Gated::Block { status: _, reason } => {
                tracing::warn!(
                    request_id = %self.request_identity.request_id(),
                    policy = %name,
                    reason,
                    "Guardrails blocked an inference request"
                );
                Err(InferenceAccessError::GuardrailBlocked { reason })
            }
            crate::wire::Gated::Forward {
                body: masked,
                restores,
                redacted,
            } => {
                if redacted > 0 {
                    tracing::info!(
                        request_id = %self.request_identity.request_id(),
                        policy = %name,
                        redacted,
                        "Guardrails masked secrets before dispatch"
                    );
                }
                self.guardrail_restores = (!restores.is_empty()).then_some(restores);
                Ok(Bytes::from(masked))
            }
        }
    }

    /// Back-translation direction for the currently dispatched attempt;
    /// `None` on same-protocol attempts.
    pub(crate) fn translate_response(&self) -> Option<ResponseTranslation> {
        self.translate_response
    }

    /// Prepare the first available target at or after the next priority.
    ///
    /// Callers invoke this again only when the preceding attempt failed before
    /// upstream response headers were received. Once a priority is attempted,
    /// the state advances past the complete priority group.
    /// HR3b/HR3c: resolves the task-routing window ONCE per request —
    /// the turn identity, the frozen-turn cache, the decision call
    /// (the turn's content against the plan's rubric, over the
    /// decision model's resolved backend), and the effective target
    /// list. Fail-closed throughout: any failure resolves primary.
    pub(crate) async fn resolve_routing(
        &mut self,
        state: &GatewayState,
        _req_headers: &http::HeaderMap,
    ) {
        if self.effective_targets.is_some() {
            return;
        }
        let now = chrono::Utc::now();
        let Ok(Some(plan)) =
            self.authorizer
                .model_routing_plan(self.authenticated, &self.model_alias.clone(), now)
        else {
            return;
        };
        let Ok(model_config) = self.authorizer.granted_model_config(
            self.authenticated,
            &self.model_alias.clone(),
            now,
        ) else {
            return;
        };
        let identity = crate::inference::turn_identity_from_document(
            &self.request.document,
            self.authorizer.credential_fingerprint(self.authenticated),
            &self.model_alias,
        );
        // HR3c: the turn's content against the plan's rubric; modes
        // that never decide build no request (and a body without a
        // user turn keeps the fail-closed path — None request ⇒
        // unreachable ⇒ primary).
        let request = if plan.needs_decision() {
            crate::inference::build_decision_request(&plan, &self.request.document).ok()
        } else {
            None
        };
        let transport = match (request.as_ref(), plan.decision_model.as_deref()) {
            (Some(_request), Some(alias)) => {
                self.resolve_decision_service(state, alias, plan.decision_protocol, now)
                    .await
            }
            _ => None,
        };
        let (decision, _frozen, frozen_hit) = state
            .decision_plane
            .decide(&model_config, &plan, &identity, request.as_ref(), transport)
            .await;
        self.effective_targets = Some(crate::inference::effective_targets(
            &model_config,
            &decision,
        ));
        // HR5: the prompt-free routing facts for the evidence lane.
        self.routing_facts = Some(crate::usage::RoutingEvidence {
            mode: crate::config::InferenceRoutingMode::wire_name(plan.mode).to_string(),
            group: decision.group.wire_name(),
            reason: decision.reason.wire_name().to_string(),
            frozen: frozen_hit,
        });
    }

    /// HR5: the routing facts for this request's usage lifecycle —
    /// absent when no plan applied (the wire stays unchanged).
    pub(crate) fn routing_facts(&self) -> Option<crate::usage::RoutingEvidence> {
        self.routing_facts.clone()
    }

    /// HR3c: resolves the configured local or upstream decision service.
    /// No Cloud calls; a failure to resolve returns None (the plane fails
    /// closed).
    async fn resolve_decision_service(
        &self,
        state: &GatewayState,
        decision_alias: &str,
        protocol: crate::config::InferenceDecisionProtocol,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Option<Arc<dyn crate::inference::DecisionService>> {
        if protocol == crate::config::InferenceDecisionProtocol::Apofasi {
            return (decision_alias == crate::inference::PLATFORM_APOFASI_ALIAS)
                .then(|| state.decision_plane.apofasi())
                .flatten();
        }
        let target = self
            .authorizer
            .select_protocol_target_from_priority(
                self.authenticated,
                decision_alias,
                crate::config::InferenceEndpoint::ChatCompletions,
                0,
                now,
                |service, _target_id| inference_service_is_available(state, service),
            )
            .ok()?;
        let selected = select_backend(state, &target.service, &http::HeaderMap::new())?;
        let authorization = self
            .authorizer
            .upstream_authorization(target.target_id)
            .and_then(|value| value.to_str().ok().map(str::to_owned));
        let transport = match protocol {
            crate::config::InferenceDecisionProtocol::Decisions => {
                crate::inference::HttpDecisionTransport::new(
                    selected.backend.url.clone(),
                    authorization,
                )
            }
            crate::config::InferenceDecisionProtocol::OpenaiChat => {
                crate::inference::HttpDecisionTransport::new_chat(
                    selected.backend.url.clone(),
                    authorization,
                    target.upstream_model.clone(),
                )
            }
            crate::config::InferenceDecisionProtocol::Apofasi => return None,
        };
        Some(Arc::new(transport))
    }

    pub(crate) fn prepare_next(
        &mut self,
        state: &GatewayState,
        headers: &mut http::HeaderMap,
        access_log: Option<&mut RequestAccessLog>,
    ) -> Result<PreparedInferenceAttempt, InferenceAccessError> {
        let now = chrono::Utc::now();
        let model =
            self.authorizer
                .granted_model_config(self.authenticated, &self.model_alias, now)?;
        if model.prefix_affinity
            && self.prefix_hint.is_none()
            && self.request_identity.endpoint() == crate::config::InferenceEndpoint::ChatCompletions
        {
            self.prefix_hint = self.authorizer.prefix_hint(
                self.authenticated,
                model.model_id,
                self.request_identity.endpoint(),
                &self.request,
            );
            if let Some(hint) = self.prefix_hint.as_mut() {
                hint.parallelism = model.prefix_parallelism;
            }
        }
        self.prefix_signature = None;
        self.prefix_lease = None;
        let mut prefix_matched_bytes = 0;
        let mut prefix_estimated_ms = None;
        let scheduled = self.authorizer.model_uses_worker_scheduling(
            self.authenticated,
            &self.model_alias,
            now,
        )?;
        let distributed = self.authorizer.distributed_serving_config(
            self.authenticated,
            &self.model_alias,
            now,
        )?;
        if distributed.is_some()
            && !matches!(
                self.request_identity.endpoint(),
                crate::config::InferenceEndpoint::ChatCompletions
                    | crate::config::InferenceEndpoint::Completions
            )
        {
            return Err(InferenceAccessError::Denied);
        }
        let reusable_target = self.scheduled_target.clone().filter(|target| {
            self.scheduled_service_is_available(
                state,
                &target.service,
                target.target_id,
                distributed.is_some(),
                now,
            )
        });
        let (target, reused_target) = if let Some(target) = reusable_target {
            (target, true)
        } else {
            self.scheduled_target = None;
            self.attempted_worker_units.clear();
            // HR3b: the task-routing window was resolved before the
            // walk (resolve_routing, at the entrypoint) — nothing to
            // compute here.
            let minimum_priority = self
                .next_priority
                .ok_or(InferenceAccessError::Unavailable)?;
            let target = self.authorizer.select_target_with_preference(
                self.authenticated,
                &self.model_alias,
                self.request_identity.endpoint(),
                self.effective_targets.as_deref(),
                minimum_priority,
                now,
                |service, target_id| {
                    if scheduled {
                        self.scheduled_service_is_available(
                            state,
                            service,
                            target_id,
                            distributed.is_some(),
                            now,
                        )
                    } else {
                        inference_service_is_available(state, service)
                    }
                },
                |available| {
                    let Some(hint) = self.prefix_hint.as_ref() else {
                        return;
                    };
                    let candidates = available
                        .iter()
                        .filter_map(|target| prefix_target(state, target))
                        .collect::<Vec<_>>();
                    // Unmeasured failover/revision pools retain their existing
                    // selector. A missing observation cannot authorize a pool.
                    if candidates.len() != available.len() {
                        return;
                    }
                    let selection = self.authorizer.prefix_affinity.select(
                        hint,
                        &candidates,
                        std::time::Instant::now(),
                    );
                    prefix_matched_bytes = selection.matched_bytes;
                    prefix_estimated_ms = selection.estimated_ms;
                    available.retain(|target| match selection.preferred {
                        Some(preferred) => target.target_id == preferred,
                        None => selection.least_loaded.contains(&target.target_id),
                    });
                },
            )?;
            (target, false)
        };
        if !reused_target {
            self.next_priority = target.priority.checked_add(1);
            if scheduled {
                self.scheduled_target = Some(target.clone());
            }
        }
        self.request_identity.set_model_id(target.model_id);

        let scoped_prompt_cache_key = self.request.scoped_prompt_cache_key(
            self.authenticated.credential_id(),
            target.model_id,
            self.request_identity.endpoint(),
        );
        let selected = if let Some(config) = distributed {
            self.select_distributed_backends(
                state,
                &target.service,
                target.target_id,
                scoped_prompt_cache_key.as_deref(),
                config,
                now,
            )?
        } else if scheduled {
            self.select_scheduled_backend(
                state,
                &target.service,
                target.target_id,
                scoped_prompt_cache_key.as_deref(),
                now,
            )?
        } else {
            select_backend(state, &target.service, headers)
                .ok_or(InferenceAccessError::Unavailable)?
        };
        if self.prefix_hint.is_some() {
            let descriptor = self
                .effective_targets
                .as_deref()
                .unwrap_or(&model.targets)
                .iter()
                .find(|descriptor| descriptor.target_id == target.target_id);
            if let Some(descriptor) = descriptor {
                // A service-level failover is not evidence about the primary
                // pool. Record only an actual member of this descriptor's pool.
                let belongs = state
                    .service_registry
                    .get(&target.service)
                    .is_some_and(|service| {
                        service
                            .backends()
                            .iter()
                            .any(|backend| backend.metric_id() == selected.backend.metric_id())
                    });
                if belongs {
                    self.prefix_signature =
                        prefix_target(state, descriptor).map(|target| target.signature);
                }
            }
        }
        if let (Some(hint), Some(signature)) = (&self.prefix_hint, self.prefix_signature) {
            self.prefix_lease = self.authorizer.prefix_affinity.reserve(
                hint,
                signature,
                prefix_matched_bytes,
                std::time::Instant::now(),
            );
        }
        if scheduled {
            if let Some(worker) = selected.backend.managed_target() {
                self.attempted_worker_units.insert(worker.unit_id.clone());
            }
            if let Some(distributed) = selected.distributed.as_ref() {
                self.attempted_worker_units
                    .insert(distributed.workers.prefill.unit_id.clone());
            }
        }
        let body = self
            .request
            .routed_body(&target.upstream_model, scoped_prompt_cache_key.as_deref())
            .map_err(|_| InferenceAccessError::Unavailable)?;
        let (body, translate_response) = self.translate_body_for_target(&target, body)?;
        self.translate_response = translate_response;
        #[cfg(feature = "wire")]
        let body = self.apply_guardrails(body)?;
        let content_length = http::HeaderValue::from_str(&body.len().to_string())
            .map_err(|_| InferenceAccessError::Unavailable)?;
        headers.insert(CONTENT_LENGTH, content_length);
        apply_upstream_credential(headers, &target, &selected.backend);

        let identity = self.request_identity.begin_attempt(target.target_id);
        identity.prepare_upstream_headers(headers);
        self.attempt_count = self.attempt_count.saturating_add(1);

        if let Some(access_log) = access_log {
            access_log.set_inference_attempt(&identity);
            access_log.set_backend(selected.backend.url.clone());
        }
        if state.metrics_enabled {
            state.metrics.record_service_request(&target.service);
            state
                .metrics
                .record_backend_request_id(selected.backend.metric_id());
            if let Some(distributed) = selected.distributed.as_ref() {
                state
                    .metrics
                    .record_backend_request_id(distributed.prefill_backend.metric_id());
            }
        }

        tracing::info!(
            request_id = %identity.request().request_id(),
            attempt_id = %identity.attempt_id(),
            target_id = %identity.target_id(),
            model_id = %target.model_id,
            priority = target.priority,
            attempt_count = self.attempt_count,
            service = %target.service,
            backend = %selected.backend.url,
            is_fallback = self.attempt_count > 1,
            prefix_matched_bytes,
            prefix_estimated_ms,
            "Prepared managed inference upstream attempt"
        );
        if let Some(scheduling) = selected.scheduling {
            tracing::info!(
                request_id = %identity.request().request_id(),
                attempt_id = %identity.attempt_id(),
                backend_id = %selected.backend.metric_id(),
                observation_generation = scheduling.observation_generation,
                worker_active = scheduling.active,
                worker_waiting = scheduling.waiting,
                worker_pressure_basis_points = scheduling.pressure_basis_points,
                cache_affinity_applied = scheduling.cache_affinity_applied,
                "Selected managed inference worker"
            );
        }
        if let Some(distributed) = selected.distributed.as_ref() {
            tracing::info!(
                request_id = %identity.request().request_id(),
                attempt_id = %identity.attempt_id(),
                prefill_backend_id = %distributed.prefill_backend.metric_id(),
                prefill_observation_generation = distributed.workers.prefill.observation_generation,
                decode_observation_generation = distributed.workers.decode.observation_generation,
                "Selected managed inference prefill/decode pair"
            );
        }

        Ok(PreparedInferenceAttempt {
            service_name: target.service,
            backend: selected.backend,
            body,
            timeouts: selected.timeouts,
            sticky_new_session: selected.sticky_new_session,
            identity,
            distributed: selected.distributed,
        })
    }

    /// Translate a routed request body when the selected target speaks a
    /// different dialect than the endpoint and opted in. Translation runs
    /// per attempt, before any upstream response starts, so fallback
    /// semantics are untouched. Streaming over a cross-protocol target
    /// refuses until the SSE lane ships; an untranslatable body refuses
    /// naming the field in the log.
    fn translate_body_for_target(
        &self,
        target: &InferenceDispatchTarget,
        body: Bytes,
    ) -> Result<(Bytes, Option<ResponseTranslation>), InferenceAccessError> {
        // Responses-dialect streaming runs on the translated lane (chat
        // chunks feed the machine, which observes usage); native streaming
        // still refuses until a native usage-observation path exists.
        if self.request_identity.endpoint() == crate::config::InferenceEndpoint::Responses
            && self.request.stream_requested()
            && !target.translates
        {
            return Err(InferenceAccessError::CrossProtocolStreaming);
        }
        if !target.translates {
            return Ok((body, None));
        }
        if self.request_identity.endpoint() == crate::config::InferenceEndpoint::Responses {
            self.translate_responses_body(target, body)
        } else {
            self.translate_cross_protocol_body(target, body)
        }
    }

    fn translate_responses_body(
        &self,
        target: &InferenceDispatchTarget,
        body: Bytes,
    ) -> Result<(Bytes, Option<ResponseTranslation>), InferenceAccessError> {
        {
            // Stateless responses→chat-completions translation rides
            // OpenAI-protocol targets only; chaining onto an Anthropic
            // target would compose two translators and refuse.
            if target.protocol != InferenceProtocol::Openai {
                tracing::warn!(
                    target_protocol = ?target.protocol,
                    "Responses translation requires an OpenAI-protocol target"
                );
                return Err(InferenceAccessError::TranslationRefused);
            }
            let value = serde_json::from_slice(&body)
                .map_err(|error| translation_refused(&error.to_string()))?;
            let translated = crate::translation::responses_request_to_openai(&value)
                .map_err(|error| translation_refused(&error.0))?;
            let translated_bytes = serde_json::to_vec(&translated)
                .map_err(|error| translation_refused(&error.to_string()))?;
            tracing::info!(
                target_id = %target.target_id,
                request_id = %self.request_identity.request_id(),
                "Translated responses-dialect inference request onto chat-completions"
            );
            Ok((
                Bytes::from(translated_bytes),
                Some(ResponseTranslation::ResponsesFromChat),
            ))
        }
    }

    fn translate_cross_protocol_body(
        &self,
        target: &InferenceDispatchTarget,
        body: Bytes,
    ) -> Result<(Bytes, Option<ResponseTranslation>), InferenceAccessError> {
        let endpoint_protocol = self.request_identity.endpoint().protocol();
        type RequestTranslator =
            fn(
                &serde_json::Value,
            ) -> Result<serde_json::Value, crate::translation::TranslationError>;
        let direction = match (endpoint_protocol, target.protocol) {
            (InferenceProtocol::Openai, InferenceProtocol::Anthropic) => (
                translate_openai_request as RequestTranslator,
                ResponseTranslation::ToOpenai,
            ),
            (InferenceProtocol::Anthropic, InferenceProtocol::Openai) => (
                translate_anthropic_request as RequestTranslator,
                ResponseTranslation::ToAnthropic,
            ),
            _ => {
                tracing::warn!(
                    endpoint = ?self.request_identity.endpoint(),
                    target_protocol = ?target.protocol,
                    "Cross-protocol target pair has no translator"
                );
                return Err(InferenceAccessError::TranslationRefused);
            }
        };
        let value = serde_json::from_slice(&body)
            .map_err(|error| translation_refused(&error.to_string()))?;
        let translated = direction.0(&value).map_err(|error| translation_refused(&error.0))?;
        let translated_bytes = serde_json::to_vec(&translated)
            .map_err(|error| translation_refused(&error.to_string()))?;
        tracing::info!(
            target_id = %target.target_id,
            request_id = %self.request_identity.request_id(),
            "Translated cross-protocol inference request"
        );
        Ok((Bytes::from(translated_bytes), Some(direction.1)))
    }

    fn scheduled_service_is_available(
        &self,
        state: &GatewayState,
        service: &str,
        target_id: uuid::Uuid,
        distributed: bool,
        now: chrono::DateTime<chrono::Utc>,
    ) -> bool {
        let Some(load_balancer) = state.service_registry.get(service) else {
            return false;
        };
        let backends = load_balancer.backends();
        let candidates = worker_candidates(&backends, &self.attempted_worker_units);
        if distributed {
            self.authorizer
                .has_eligible_worker_pair(
                    self.authenticated,
                    &self.model_alias,
                    target_id,
                    &candidates,
                    now,
                    self.request_identity.request_id().as_bytes(),
                )
                .unwrap_or(false)
        } else {
            self.authorizer
                .has_eligible_worker(
                    self.authenticated,
                    &self.model_alias,
                    target_id,
                    &candidates,
                    now,
                )
                .unwrap_or(false)
        }
    }

    fn select_scheduled_backend(
        &self,
        state: &GatewayState,
        service: &str,
        target_id: uuid::Uuid,
        cache_affinity_key: Option<&str>,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Result<SelectedBackend, InferenceAccessError> {
        let load_balancer = state
            .service_registry
            .get(service)
            .ok_or(InferenceAccessError::Unavailable)?;
        let backends = load_balancer.backends();
        let candidates = worker_candidates(&backends, &self.attempted_worker_units);
        let scheduling = self.authorizer.select_worker(
            self.authenticated,
            &self.model_alias,
            InferenceWorkerSelectionRequest {
                target_id,
                candidates: &candidates,
                now,
                routing_key: self.request_identity.request_id().as_bytes(),
                cache_affinity_key,
            },
        )?;
        let backend = backends
            .get(scheduling.index)
            .cloned()
            .ok_or(InferenceAccessError::Unavailable)?;
        Ok(SelectedBackend {
            backend,
            timeouts: load_balancer.timeouts(),
            sticky_new_session: None,
            scheduling: Some(scheduling),
            distributed: None,
        })
    }

    fn select_distributed_backends(
        &self,
        state: &GatewayState,
        service: &str,
        target_id: uuid::Uuid,
        cache_affinity_key: Option<&str>,
        config: crate::config::InferenceDistributedServingConfig,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Result<SelectedBackend, InferenceAccessError> {
        let load_balancer = state
            .service_registry
            .get(service)
            .ok_or(InferenceAccessError::Unavailable)?;
        let backends = load_balancer.backends();
        let candidates = worker_candidates(&backends, &self.attempted_worker_units);
        let workers = self.authorizer.select_worker_pair(
            self.authenticated,
            &self.model_alias,
            InferenceWorkerPairSelectionRequest {
                target_id,
                candidates: &candidates,
                now,
                routing_key: self.request_identity.request_id().as_bytes(),
                cache_affinity_key,
            },
        )?;
        let prefill_backend = backends
            .get(workers.prefill.index)
            .cloned()
            .ok_or(InferenceAccessError::Unavailable)?;
        let decode_backend = backends
            .get(workers.decode.index)
            .cloned()
            .ok_or(InferenceAccessError::Unavailable)?;
        Ok(SelectedBackend {
            backend: decode_backend,
            timeouts: load_balancer.timeouts(),
            sticky_new_session: None,
            scheduling: None,
            distributed: Some(PreparedDistributedAttempt {
                prefill_backend,
                workers,
                config,
            }),
        })
    }
}

fn prefix_target(
    state: &GatewayState,
    target: &crate::config::InferenceTargetConfig,
) -> Option<PrefixTarget> {
    let service = state.service_registry.get(&target.service)?;
    // A revision router, dynamic scaler, sticky manager, or service failover
    // owns its own selection semantics. Do not pretend its root pool is a
    // complete observation of the eventual execution target.
    if state.failovers.contains_key(&target.service)
        || state.sticky_managers.contains_key(&target.service)
        || state
            .scaling
            .as_ref()
            .is_some_and(|scaling| scaling.revision_routers.contains_key(&target.service))
    {
        return None;
    }
    let backends = service.backends();
    let healthy = backends
        .iter()
        .filter(|backend| backend.is_healthy())
        .collect::<Vec<_>>();
    if healthy.is_empty() {
        return None;
    }
    let outstanding = healthy.iter().fold(0u64, |total, backend| {
        total.saturating_add(backend.concurrency_connections() as u64)
    });
    let load = outstanding
        .saturating_mul(1000)
        .div_ceil(healthy.len() as u64);
    Some(PrefixTarget {
        target_id: target.target_id,
        signature: crate::inference::target_signature(
            target,
            backends
                .iter()
                .map(|backend| format!("{}:{:?}", backend.metric_id(), backend.managed_target()))
                .collect(),
        )?,
        load,
    })
}

struct SelectedBackend {
    backend: Arc<Backend>,
    timeouts: ServiceTimeouts,
    sticky_new_session: Option<String>,
    scheduling: Option<InferenceWorkerSelection>,
    distributed: Option<PreparedDistributedAttempt>,
}

fn worker_candidates<'a>(
    backends: &'a [Arc<Backend>],
    excluded_worker_units: &HashSet<String>,
) -> Vec<InferenceWorkerCandidate<'a>> {
    backends
        .iter()
        .map(|backend| InferenceWorkerCandidate {
            target: backend.managed_target(),
            healthy: backend.is_healthy()
                && backend
                    .managed_target()
                    .is_some_and(|target| !excluded_worker_units.contains(&target.unit_id)),
            local_connections: u64::try_from(backend.connections()).unwrap_or(u64::MAX),
        })
        .collect()
}

/// Inject the provider key of an external target and address its virtual
/// host. Every attempt resets these headers so a fallback to a self-hosted
/// target never inherits another provider's credential.
fn apply_upstream_credential(
    headers: &mut http::HeaderMap,
    target: &InferenceDispatchTarget,
    backend: &Backend,
) {
    headers.remove(http::header::AUTHORIZATION);
    headers.remove("x-api-key");
    let Some(key) = target.upstream_key.clone() else {
        return;
    };
    match target.protocol {
        InferenceProtocol::Openai | InferenceProtocol::Ocr | InferenceProtocol::SystemOne => {
            headers.insert(http::header::AUTHORIZATION, key);
        }
        InferenceProtocol::Anthropic => {
            headers.insert("x-api-key", key);
            if !headers.contains_key("anthropic-version") {
                headers.insert(
                    "anthropic-version",
                    http::HeaderValue::from_static("2023-06-01"),
                );
            }
        }
    }
    if let Some(authority) = backend
        .http_base_uri()
        .and_then(|uri| uri.authority())
        .and_then(|authority| http::HeaderValue::from_str(authority.as_str()).ok())
    {
        headers.insert(http::header::HOST, authority);
    }
}

/// Select one concrete backend without entering Gateway-owned scaling loops.
///
/// Cloud-managed mode may use static revisions, capacity-aware selection,
/// sticky affinity, and a configured service failover pool. Scale-from-zero
/// buffering remains a standalone-only control loop.
fn select_backend(
    state: &GatewayState,
    service: &str,
    headers: &http::HeaderMap,
) -> Option<SelectedBackend> {
    let load_balancer = state.service_registry.get(service)?;
    let timeouts = load_balancer.timeouts();
    let scaling = state.scaling.as_ref();
    let concurrency_limiter = scaling.and_then(|scaling| scaling.limiters.get(service));
    let mut sticky_new_session = None;
    let sticky_backend = state
        .sticky_managers
        .get(service)
        .and_then(|manager| {
            let session_id = headers
                .get("cookie")
                .and_then(|value| value.to_str().ok())
                .and_then(|cookie| manager.extract_session_id(cookie));
            manager
                .select_backend(session_id, load_balancer.backends().as_slice())
                .map(|(backend, new_session)| {
                    sticky_new_session = new_session;
                    backend
                })
        })
        .filter(|backend| {
            let allowed = concurrency_limiter.is_none_or(|limiter| {
                matches!(
                    limiter.check(backend),
                    crate::scaling::concurrency::ConcurrencyCheckResult::Allowed
                )
            });
            if !allowed {
                sticky_new_session = None;
            }
            allowed
        });

    let backend = if sticky_backend.is_some() {
        sticky_backend
    } else if let Some(router) = scaling.and_then(|scaling| scaling.revision_routers.get(service)) {
        concurrency_limiter
            .map(|limiter| router.next_backend_with_capacity(limiter))
            .unwrap_or_else(|| router.next_backend())
            .map(|(backend, _revision)| backend)
    } else if let Some(limiter) = scaling.and_then(|scaling| scaling.limiters.get(service)) {
        limiter.select_with_capacity(load_balancer.backends().as_slice())
    } else {
        load_balancer.next_backend()
    }
    .or_else(|| {
        state
            .failovers
            .get(service)
            .and_then(|selector| selector.next_backend().map(|(backend, _)| backend))
    })?;

    Some(SelectedBackend {
        backend,
        timeouts,
        sticky_new_session,
        scheduling: None,
        distributed: None,
    })
}

fn translate_openai_request(
    value: &serde_json::Value,
) -> Result<serde_json::Value, crate::translation::TranslationError> {
    crate::translation::openai_request_to_anthropic(value)
}

fn translate_anthropic_request(
    value: &serde_json::Value,
) -> Result<serde_json::Value, crate::translation::TranslationError> {
    crate::translation::anthropic_request_to_openai(value)
}

fn translation_refused(reason: &str) -> InferenceAccessError {
    tracing::warn!(reason, "Cross-protocol translation refused");
    InferenceAccessError::TranslationRefused
}
