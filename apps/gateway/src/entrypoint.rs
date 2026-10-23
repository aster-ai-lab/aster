//! Entrypoint — network listeners for HTTP/HTTPS/TCP
//!
//! Manages the lifecycle of network listeners that accept incoming
//! connections and dispatch them to the router. Supports HTTP, WebSocket,
//! gRPC, SSE/streaming, TCP, and UDP protocols.

pub(crate) mod agent_billing;
mod http_listener;
mod inference_dispatch;
#[cfg(test)]
mod inference_distributed_fallback_tests;
#[cfg(test)]
mod inference_distributed_rollover_tests;
#[cfg(test)]
mod inference_distributed_tests;
#[cfg(test)]
mod inference_fallback_tests;
#[cfg(test)]
mod inference_identity_tests;
#[cfg(test)]
mod inference_scheduling_tests;
#[cfg(test)]
mod inference_tests;
#[cfg(test)]
mod inference_usage_tests;
mod listener;
mod native_response;
#[cfg(test)]
mod platform_model_tests;
#[cfg(test)]
mod principal_inference_tests;
pub(crate) mod protocol;
#[cfg(test)]
mod tests;
mod udp_listener;
mod websocket_dispatch;

pub(crate) use listener::{
    start_entrypoints, validate_entrypoints, EntryPointHandles, PreparedEntrypointReconfigure,
};
#[cfg(test)]
use listener::{start_http_entrypoint, start_tcp_entrypoint};

use inference_dispatch::{InferenceDispatchState, PreparedInferenceAttempt};
use native_response::{
    error_bytes_response, error_response, finish_access_log, finish_native_response, full_body,
    inference_access_error_response, openai_request_error_response, BufferedResponsePipeline,
};
use protocol::ProtocolContext;

use crate::inference::{
    collect_json_body, collect_proxy_json_body, models_response, serve_platform_model,
    tool_invoke_sku, AuthenticatedInference, InferenceAccessError, InferenceAdmissionGuard,
    InferenceAuthorizer, InferenceRequestIdentity, OpenAiRequestError, OpenAiRequestProfile,
    ServedPlatformModel, OPENAI_REQUEST_BODY_LIMIT,
};
use crate::middleware::{Pipeline, RequestContext};
use crate::observability::access_log::RequestAccessLog;
use crate::proxy::acme::ChallengeStore;
use crate::proxy::{
    BackendOperationTracking, ForwardOptions, ForwardedContext, ForwardedProto, HttpProxy,
    HttpTimeouts, OwnedBufferedRequest, OwnedStreamingRequest, PreparedForwardedContext,
};
use crate::response_body::ResponseBody;
use crate::router::RouterTable;
use crate::scaling::buffer::RequestBuffer;
use crate::scaling::concurrency::ConcurrencyLimiter;
use crate::scaling::revision::RevisionRouter;
use crate::service::passive_health::PassiveHealthCheck;
use crate::service::sticky::StickySessionManager;
use crate::service::{Backend, LoadBalancer, ServiceRegistry};
use crate::usage::{track_usage_response, UsageRequestLifecycle};
use arc_swap::{ArcSwap, ArcSwapOption};
use bytes::Bytes;
use http_body_util::{BodyExt, LengthLimitError, Limited};
use hyper::body::{Body as _, Incoming};
use std::collections::HashMap;
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

type UpgradedSession = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;
type UpgradedSessionSender = tokio::sync::mpsc::UnboundedSender<UpgradedSession>;

/// Safety bound for protocol paths that must buffer a request body but do not
/// have a route-specific body-limit middleware.
const DEFAULT_BUFFERED_REQUEST_BODY_LIMIT: usize = 8 * 1024 * 1024;

pub(super) struct HttpConnectionContext {
    remote_addr: Option<SocketAddr>,
    entrypoint: Arc<str>,
    forwarded: ForwardedContext,
    prepared_forwarded: Option<Arc<PreparedForwardedContext>>,
    trust_forwarded_headers: Arc<AtomicBool>,
    upgraded_sessions: UpgradedSessionSender,
}

impl HttpConnectionContext {
    #[cfg(test)]
    pub(super) fn new(
        remote_addr: SocketAddr,
        entrypoint: Arc<str>,
        forwarded_proto: ForwardedProto,
        local_port: u16,
        trust_forwarded_headers: Arc<AtomicBool>,
        upgraded_sessions: UpgradedSessionSender,
    ) -> Self {
        Self::from_peer(
            Some(remote_addr),
            entrypoint,
            forwarded_proto,
            local_port,
            trust_forwarded_headers,
            upgraded_sessions,
        )
    }

    fn from_peer(
        remote_addr: Option<SocketAddr>,
        entrypoint: Arc<str>,
        forwarded_proto: ForwardedProto,
        local_port: u16,
        trust_forwarded_headers: Arc<AtomicBool>,
        upgraded_sessions: UpgradedSessionSender,
    ) -> Self {
        let trusted = trust_forwarded_headers.load(Ordering::Acquire);
        let forwarded =
            ForwardedContext::from_peer(remote_addr, forwarded_proto).with_trusted_headers(trusted);
        Self {
            remote_addr,
            entrypoint,
            forwarded,
            prepared_forwarded: remote_addr.and_then(|_| {
                PreparedForwardedContext::with_trust_flag(
                    forwarded,
                    local_port,
                    trust_forwarded_headers.clone(),
                )
                .map(Arc::new)
                .ok()
            }),
            trust_forwarded_headers,
            upgraded_sessions,
        }
    }

    fn forwarded(&self) -> ForwardedContext {
        self.forwarded
            .with_trusted_headers(self.trust_forwarded_headers.load(Ordering::Acquire))
    }
}

fn inference_service_is_available(state: &GatewayState, service: &str) -> bool {
    let primary_is_available = if let Some(revision_router) = state
        .scaling
        .as_ref()
        .and_then(|scaling| scaling.revision_routers.get(service))
    {
        revision_router.has_healthy_backend()
    } else {
        state
            .service_registry
            .get(service)
            .is_some_and(|load_balancer| load_balancer.healthy_count() > 0)
    };
    primary_is_available
        || state
            .failovers
            .get(service)
            .is_some_and(|failover| failover.has_healthy_backend())
}

/// Select one backend for protocol listeners that cannot wait for a
/// scale-from-zero buffer.  Keeping this policy in the entrypoint module makes
/// TCP, UDP, and WebSocket dispatch obey the same revision, capacity, and
/// failover rules as ordinary HTTP selection.
pub(crate) fn select_backend_for_service(
    state: &GatewayState,
    service: &str,
) -> Option<Arc<Backend>> {
    select_backend_for_service_request(state, service, None).map(|selected| selected.backend)
}

/// Backend selection for request-bearing protocols (HTTP, WebSocket) that may
/// honour sticky cookies and emit a new affinity session.
pub(crate) struct ServiceBackendSelection {
    pub(crate) backend: Arc<Backend>,
    pub(crate) sticky_new_session: Option<String>,
}

/// Select a backend with optional sticky affinity from request cookies.
///
/// When `headers` is `None`, sticky managers are skipped (TCP/UDP session
/// affinity uses a different mechanism). When sticky is configured and
/// headers are present, a missing/unknown cookie may allocate a new session
/// id for the caller to attach via `Set-Cookie`.
pub(crate) fn select_backend_for_service_request(
    state: &GatewayState,
    service: &str,
    headers: Option<&http::HeaderMap>,
) -> Option<ServiceBackendSelection> {
    let load_balancer = state.service_registry.get(service)?;
    let limiter = state
        .scaling
        .as_ref()
        .and_then(|scaling| scaling.limiters.get(service));
    let mut sticky_new_session = None;
    let sticky_backend = headers.and_then(|headers| {
        state.sticky_managers.get(service).and_then(|manager| {
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
                .filter(|backend| {
                    let allowed = limiter.is_none_or(|limiter| {
                        matches!(
                            limiter.check(backend),
                            crate::scaling::concurrency::ConcurrencyCheckResult::Allowed
                        )
                    });
                    if !allowed {
                        sticky_new_session = None;
                    }
                    allowed
                })
        })
    });

    let primary = if let Some(backend) = sticky_backend {
        Some(backend)
    } else if let Some(revision_router) = state
        .scaling
        .as_ref()
        .and_then(|scaling| scaling.revision_routers.get(service))
    {
        sticky_new_session = None;
        limiter
            .map(|limiter| revision_router.next_backend_with_capacity(limiter))
            .unwrap_or_else(|| revision_router.next_backend())
            .map(|(backend, _revision)| backend)
    } else {
        sticky_new_session = None;
        limiter
            .map(|limiter| limiter.select_with_capacity(load_balancer.backends().as_slice()))
            .unwrap_or_else(|| load_balancer.next_backend())
    };

    let backend = primary.or_else(|| {
        sticky_new_session = None;
        state
            .failovers
            .get(service)
            .and_then(|selector| selector.next_backend().map(|(backend, _)| backend))
    })?;

    Some(ServiceBackendSelection {
        backend,
        sticky_new_session,
    })
}

/// Return the backend slots that may own an already-established protocol
/// session.  UDP uses this to preserve session affinity across revision pools
/// and failover without accepting a stale endpoint after a health transition.
pub(crate) fn backend_candidates_for_service(
    state: &GatewayState,
    service: &str,
) -> Vec<Arc<Backend>> {
    let mut candidates = Vec::new();
    if let Some(revision_router) = state
        .scaling
        .as_ref()
        .and_then(|scaling| scaling.revision_routers.get(service))
    {
        for revision in revision_router.revisions() {
            candidates.extend(revision.load_balancer().backends().iter().cloned());
        }
    } else if let Some(load_balancer) = state.service_registry.get(service) {
        candidates.extend(load_balancer.backends().iter().cloned());
    }

    if let Some(selector) = state.failovers.get(service) {
        if let Some(load_balancer) = state.service_registry.get(selector.failover_name()) {
            candidates.extend(load_balancer.backends().iter().cloned());
        }
    }
    candidates
}

/// Pick a different healthy backend for a replayable ordinary request.
///
/// This is deliberately separate from managed inference fallback. Managed
/// inference retains an exact target/generation (and, for distributed
/// serving, an exact worker pair) until its dispatch state advances. Ordinary
/// retries may instead move within the already compiled service policy, while
/// still respecting revisions, capacity limits, and configured failover.
pub(crate) fn select_retry_backend(
    state: &GatewayState,
    service: &str,
    excluded: &Backend,
) -> Option<Arc<Backend>> {
    let load_balancer = state.service_registry.get(service);

    let primary = if let Some(revision_router) = state
        .scaling
        .as_ref()
        .and_then(|scaling| scaling.revision_routers.get(service))
    {
        let limiter = state
            .scaling
            .as_ref()
            .and_then(|scaling| scaling.limiters.get(service));
        limiter
            .map(|limiter| revision_router.next_backend_with_capacity_excluding(limiter, excluded))
            .unwrap_or_else(|| revision_router.next_backend_excluding(excluded))
            .map(|(backend, _revision)| backend)
    } else if let (Some(load_balancer), Some(limiter)) = (
        load_balancer.as_ref(),
        state
            .scaling
            .as_ref()
            .and_then(|scaling| scaling.limiters.get(service)),
    ) {
        limiter.select_with_capacity_excluding(load_balancer.backends().as_slice(), Some(excluded))
    } else {
        load_balancer
            .as_ref()
            .and_then(|load_balancer| load_balancer.next_backend_excluding(excluded))
    };

    primary.or_else(|| {
        state
            .failovers
            .get(service)
            .and_then(|selector| selector.next_backend_excluding(excluded).map(|(b, _)| b))
    })
}

/// Scaling-related state for services with autoscaling enabled
pub struct ScalingState {
    /// Per-service request buffers (for scale-from-zero)
    pub buffers: HashMap<String, Arc<RequestBuffer>>,
    /// Per-service concurrency limiters
    pub limiters: HashMap<String, Arc<ConcurrencyLimiter>>,
    /// Per-service revision routers
    pub revision_routers: HashMap<String, Arc<RevisionRouter>>,
}

/// Startup-bound single-backend inputs for a direct ordinary HTTP route.
pub(crate) struct DirectHttpBinding {
    pub backend: Arc<Backend>,
    pub timeouts: HttpTimeouts,
}

/// Startup-bound middleware and service objects for one compiled HTTP route.
pub(crate) struct RoutePlan {
    pub pipeline: Arc<Pipeline>,
    pub load_balancer: Option<Arc<LoadBalancer>>,
    pub passive_health: Option<Arc<PassiveHealthCheck>>,
    /// This route has no middleware or service features that require the
    /// general request dispatcher. Protocol and observability checks remain
    /// request-scoped before the direct HTTP path is selected.
    pub direct_http_eligible: bool,
    /// Preselected single backend and timeout policy for the common direct
    /// route shape. Multi-backend routes retain request-time selection.
    pub direct_http_binding: Option<DirectHttpBinding>,
    /// Read-only static bundle target (`WEB0.4`). When set, proxy backends are
    /// absent and the request path serves digest-bound objects.
    pub static_bundle: Option<Arc<crate::static_object::StaticBundleRuntime>>,
}

/// Shared state for request handling
/// The AR0.6 request-path plane: the snapshot's policy table, the
/// demand registry, and the bounded emission queue in one shared handle.
pub(crate) struct AgentWakePlane {
    pub(crate) policies: crate::agent_wake::AgentIdlePolicyTable,
    pub(crate) demand: crate::agent_wake::AgentWakeDemandRegistry,
    pub(crate) emitter: crate::agent_wake::AgentWakeEmitter,
}

impl AgentWakePlane {
    /// The dormant posture (no projected policies, no endpoint): every
    /// lookup misses and normal dispatch proceeds.
    pub(crate) fn dormant() -> Self {
        Self {
            policies: crate::agent_wake::AgentIdlePolicyTable::default(),
            demand: crate::agent_wake::AgentWakeDemandRegistry::new(),
            emitter: crate::agent_wake::AgentWakeEmitter::disabled(),
        }
    }
}

pub struct GatewayState {
    pub router_table: Arc<RouterTable>,
    /// SNI (`HostSNI`) routes used by TCP entrypoints.
    pub tcp_router_table: Arc<crate::router::TcpRouterTable>,
    /// Route plans in the same order as `router_table` runtime indices.
    pub route_plans: Box<[RoutePlan]>,
    pub service_registry: Arc<ServiceRegistry>,
    /// Optional exact-snapshot inference authorization runtime.
    pub inference_authorizer: Option<Arc<InferenceAuthorizer>>,
    /// Gateway-owned application service for authenticated Power P/D calls.
    pub(crate) distributed_serving: Arc<crate::inference::DistributedServingOrchestrator>,
    /// Optional node-local durable lifecycle spool for managed inference.
    pub usage_spool: Option<Arc<crate::usage::UsageSpool>>,
    pub http_proxy: Arc<HttpProxy>,
    /// Per-service HTTP proxies that use a private upstream CA (`tls_ca_file`).
    /// Services absent from this map use [`Self::http_proxy`].
    pub service_http_proxies: HashMap<String, Arc<HttpProxy>>,
    /// gRPC proxy (HTTP/2 with h2c support)
    pub grpc_proxy: Arc<crate::proxy::grpc::GrpcProxy>,
    /// Per-service gRPC proxies that use a private upstream CA (`tls_ca_file`).
    /// Services absent from this map use [`Self::grpc_proxy`].
    pub service_grpc_proxies: HashMap<String, Arc<crate::proxy::grpc::GrpcProxy>>,
    /// Per-service WebSocket TLS configs that trust only `tls_ca_file`.
    /// Services absent from this map use the public webpki roots.
    pub service_ws_tls: HashMap<String, Arc<rustls::ClientConfig>>,
    /// Scaling state (None if no service has scaling config)
    pub scaling: Option<Arc<ScalingState>>,
    /// Traffic mirrors: service_name → TrafficMirror
    pub mirrors: HashMap<String, Arc<crate::service::TrafficMirror>>,
    /// Failover selectors: service_name → FailoverSelector
    pub failovers: HashMap<String, Arc<crate::service::FailoverSelector>>,
    /// Structured access log (counter + background task target)
    pub access_log: Arc<crate::observability::access_log::AccessLog>,
    /// Channel for fire-and-forget log entries — background task does JSON + tracing
    pub log_tx: crate::observability::access_log::AccessLogSender,
    /// Sticky session managers (only for services with sticky config)
    pub sticky_managers: HashMap<String, Arc<StickySessionManager>>,
    /// Passive health checkers for all services
    pub passive_health: HashMap<String, Arc<PassiveHealthCheck>>,
    /// Gateway-wide metrics collector
    pub metrics: Arc<crate::observability::metrics::GatewayMetrics>,
    /// Current process-wide graceful-drain deadline.
    pub shutdown_timeout: Duration,
    /// H0.5-C3 activation targets for unavailable hosted Function routes, by router.
    pub(crate) hosted_function_activations:
        Arc<HashMap<String, crate::hosted_function_activation::HostedFunctionActivation>>,
    /// AR0.6/ED1: wake-eligible agent idle policies projected by Cloud,
    /// the shared demand registry, and the emission queue. Inference
    /// dispatch consults the table; telemetry never creates desired state.
    pub(crate) agent_wake: Arc<AgentWakePlane>,
    /// HR3b: the task-routing decision plane (frozen-turn cache + the
    /// configured decision service). Dormant by default: decision
    /// routing modes resolve fail-closed to primary until a transport
    /// is configured.
    pub(crate) decision_plane: Arc<crate::inference::DecisionPlane>,
    /// Whether metrics recording is enabled (hot-path flag)
    pub metrics_enabled: bool,
    /// Whether access logging is enabled (hot-path flag)
    pub access_log_enabled: bool,
    /// Whether distributed tracing is enabled (hot-path flag)
    pub tracing_enabled: bool,
    /// Search and parse charges. Unset when billing credentials are absent.
    pub agent_billing: Option<agent_billing::AgentBilling>,
    /// Anonymous Chatbot consumer. Absent fails `POST /v1/chatbot` closed.
    pub anonymous_chatbot: Option<crate::config::AnonymousChatbotUpstream>,
}

impl GatewayState {
    /// HTTP proxy for `service`, including a private CA when configured.
    pub(crate) fn http_proxy_for(&self, service: &str) -> &Arc<HttpProxy> {
        self.service_http_proxies
            .get(service)
            .unwrap_or(&self.http_proxy)
    }

    /// gRPC proxy for `service`, including a private CA when configured.
    pub(crate) fn grpc_proxy_for(&self, service: &str) -> &Arc<crate::proxy::grpc::GrpcProxy> {
        self.service_grpc_proxies
            .get(service)
            .unwrap_or(&self.grpc_proxy)
    }

    /// Service whose upstream TLS config must be used for `backend`.
    ///
    /// Failover returns a backend from the backup pool. That pool's
    /// `tls_ca_file` is not the primary route's.
    pub(crate) fn upstream_service_for<'a>(
        &'a self,
        route_service: &'a str,
        backend: &Backend,
    ) -> &'a str {
        let Some(selector) = self.failovers.get(route_service) else {
            return route_service;
        };
        let failover_name = selector.failover_name();
        let owned_by_failover =
            self.service_registry
                .get(failover_name)
                .is_some_and(|load_balancer| {
                    load_balancer
                        .backends()
                        .iter()
                        .any(|candidate| std::ptr::eq(candidate.as_ref(), backend))
                });
        if owned_by_failover {
            failover_name
        } else {
            route_service
        }
    }

    pub(crate) fn http_proxy_for_backend(
        &self,
        route_service: &str,
        backend: &Backend,
    ) -> &Arc<HttpProxy> {
        self.http_proxy_for(self.upstream_service_for(route_service, backend))
    }

    pub(crate) fn grpc_proxy_for_backend(
        &self,
        route_service: &str,
        backend: &Backend,
    ) -> &Arc<crate::proxy::grpc::GrpcProxy> {
        self.grpc_proxy_for(self.upstream_service_for(route_service, backend))
    }

    pub(crate) fn ws_tls_for_backend(
        &self,
        route_service: &str,
        backend: &Backend,
    ) -> Option<Arc<rustls::ClientConfig>> {
        self.ws_tls_for(self.upstream_service_for(route_service, backend))
    }

    /// WebSocket upstream TLS for `service` when `tls_ca_file` replaces public roots.
    pub(crate) fn ws_tls_for(&self, service: &str) -> Option<Arc<rustls::ClientConfig>> {
        self.service_ws_tls.get(service).cloned()
    }
}

/// Shared runtime snapshot used by entrypoints.
///
/// Listeners keep this handle for their lifetime and clone the current
/// `GatewayState` for each new request/connection. Reload can replace the
/// snapshot without rebinding unchanged traffic ports.
#[derive(Clone)]
pub struct GatewayRuntime {
    current: Arc<ArcSwap<GatewayState>>,
    managed_snapshot_store: Option<Arc<crate::managed_snapshot::ManagedSnapshotStore>>,
    /// Shared ACME HTTP-01 challenge store (same Arc as `AcmeManager`).
    acme_challenges: Arc<ArcSwapOption<ChallengeStore>>,
    /// Process-wide hosted Function activation demand; survives snapshot swaps.
    activation_demand: Arc<crate::hosted_function_activation::ActivationDemandRegistry>,
    /// Wakes requests held for activation when a new snapshot is applied.
    swapped: Arc<tokio::sync::Notify>,
}

impl GatewayRuntime {
    pub fn new(state: Arc<GatewayState>) -> Self {
        Self {
            current: Arc::new(ArcSwap::from(state)),
            managed_snapshot_store: None,
            acme_challenges: Arc::new(ArcSwapOption::empty()),
            activation_demand: Arc::new(
                crate::hosted_function_activation::ActivationDemandRegistry::new(),
            ),
            swapped: Arc::new(tokio::sync::Notify::new()),
        }
    }

    /// Share the Gateway-owned activation demand registry with this runtime.
    pub(crate) fn with_activation_demand(
        mut self,
        registry: Arc<crate::hosted_function_activation::ActivationDemandRegistry>,
    ) -> Self {
        self.activation_demand = registry;
        self
    }

    /// Attach the managed snapshot store used to enforce its validity window
    /// at the listener boundary.
    pub(crate) fn with_managed_snapshot_store(
        mut self,
        store: Arc<crate::managed_snapshot::ManagedSnapshotStore>,
    ) -> Self {
        self.managed_snapshot_store = Some(store);
        self
    }

    /// Attach (or clear) the ACME HTTP-01 challenge store served on the data plane.
    pub(crate) fn set_acme_challenges(&self, challenges: Option<Arc<ChallengeStore>>) {
        self.acme_challenges.store(challenges);
    }

    pub(crate) fn acme_challenges(&self) -> Option<Arc<ChallengeStore>> {
        self.acme_challenges.load_full()
    }

    pub(crate) fn allows_traffic(&self) -> bool {
        self.managed_snapshot_store
            .as_ref()
            .is_none_or(|store| store.allows_traffic(chrono::Utc::now()))
    }

    pub fn load(&self) -> Arc<GatewayState> {
        self.current.load_full()
    }

    pub fn replace(&self, state: Arc<GatewayState>) {
        let previous = self.current.load_full();
        retire_absent_managed_target_generations(&previous, &state);
        self.current.store(state);
        self.swapped.notify_waiters();
    }
}

/// Close admission on managed generations present in `previous` but absent from
/// `next`. In-flight guards keep those backends alive until responses finish;
/// new admits must not land on a retired exact generation (H0.3).
fn retire_absent_managed_target_generations(previous: &GatewayState, next: &GatewayState) {
    let next_generations: std::collections::HashSet<crate::config::ManagedTargetConfig> = next
        .service_registry
        .iter()
        .flat_map(|(_, load_balancer)| {
            load_balancer
                .backends()
                .iter()
                .filter_map(|backend| backend.managed_target().cloned())
                .collect::<Vec<_>>()
        })
        .collect();

    for (_, load_balancer) in previous.service_registry.iter() {
        for backend in load_balancer.backends().iter() {
            let Some(target) = backend.managed_target() else {
                continue;
            };
            if !next_generations.contains(target) {
                backend.close_managed_admission();
            }
        }
    }
}

fn request_trace_context(
    headers: &http::HeaderMap,
    tracing_enabled: bool,
) -> Option<crate::observability::tracing::TraceContext> {
    tracing_enabled.then(|| {
        crate::observability::tracing::extract_trace_context(headers)
            .unwrap_or_else(crate::observability::tracing::TraceContext::new_root)
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RequestBodyCollectionError {
    TooLarge,
    ReadFailed,
}

/// Collect a request body with a hard byte bound.
///
/// This is used only when the incoming framing does not expose a safe upper
/// bound or when another feature already requires buffering. The limit is
/// enforced by the body adapter, so a chunked request cannot bypass the
/// route's body-limit middleware.
fn declared_body_exceeds_limit(headers: &http::HeaderMap, limit: usize) -> bool {
    headers
        .get(http::header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .is_some_and(|length| length > limit as u64)
}

async fn serve_signed_in_platform_model(
    state: &GatewayState,
    route: &crate::router::ResolvedRoute,
    profile: OpenAiRequestProfile,
    authenticated_inference: &Option<(Arc<InferenceAuthorizer>, AuthenticatedInference)>,
    identity: Option<&mut InferenceRequestIdentity>,
    req_parts: &http::request::Parts,
    body: Incoming,
) -> ServedPlatformModel {
    let Some((authorizer, authenticated)) = authenticated_inference else {
        return ServedPlatformModel {
            response: inference_access_error_response(InferenceAccessError::Unavailable, None),
            lifecycle: None,
        };
    };
    let Some(identity) = identity else {
        return ServedPlatformModel {
            response: inference_access_error_response(InferenceAccessError::Unavailable, None),
            lifecycle: None,
        };
    };
    if declared_body_exceeds_limit(&req_parts.headers, OPENAI_REQUEST_BODY_LIMIT) {
        return ServedPlatformModel {
            response: openai_request_error_response(
                OpenAiRequestError::BodyTooLarge,
                Some(identity),
            ),
            lifecycle: None,
        };
    }
    let body = match collect_request_body_up_to(body, OPENAI_REQUEST_BODY_LIMIT).await {
        Ok(body) => body,
        Err(RequestBodyCollectionError::TooLarge) => {
            return ServedPlatformModel {
                response: openai_request_error_response(
                    OpenAiRequestError::BodyTooLarge,
                    Some(identity),
                ),
                lifecycle: None,
            };
        }
        Err(RequestBodyCollectionError::ReadFailed) => {
            return ServedPlatformModel {
                response: openai_request_error_response(
                    OpenAiRequestError::BodyReadFailed,
                    Some(identity),
                ),
                lifecycle: None,
            };
        }
    };
    serve_platform_model(
        &state.service_registry,
        state.usage_spool.clone(),
        authorizer,
        *authenticated,
        identity,
        &route.router_name,
        profile,
        req_parts.uri.path(),
        &req_parts.headers,
        body,
    )
    .await
}

async fn collect_request_body_up_to(
    body: Incoming,
    limit: usize,
) -> std::result::Result<Bytes, RequestBodyCollectionError> {
    collect_limited_body(body, limit).await
}

async fn collect_limited_body<B>(
    body: B,
    limit: usize,
) -> std::result::Result<Bytes, RequestBodyCollectionError>
where
    B: hyper::body::Body,
    B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    Limited::new(body, limit)
        .collect()
        .await
        .map_err(|error| {
            if error.downcast_ref::<LengthLimitError>().is_some() {
                RequestBodyCollectionError::TooLarge
            } else {
                let mut rendered = error.to_string();
                rendered.truncate(180);
                if rendered.contains("a3s_") || rendered.contains("Bearer") {
                    rendered = "redacted".to_string();
                }
                tracing::warn!(error = %rendered, "request body could not be read");
                RequestBodyCollectionError::ReadFailed
            }
        })
        .map(|collected| collected.to_bytes())
}

/// Forward a feature-free HTTP route without constructing the general
/// protocol-dispatch state. Streaming responses already pass through the
/// shared HTTP body relay; native OpenAI requests retain bounded validation
/// before entering the same sharded upstream pool.
async fn handle_direct_http_request(
    req: hyper::Request<Incoming>,
    state: &GatewayState,
    route_plan: &RoutePlan,
    service_name: &str,
    forwarded: ForwardedContext,
    prepared_forwarded: Option<&PreparedForwardedContext>,
    openai_profile: Option<OpenAiRequestProfile>,
) -> hyper::Response<ResponseBody> {
    let (mut parts, incoming_body) = req.into_parts();
    // Native OpenAI validation remains ahead of backend selection. The body is
    // retained as immutable Bytes so the upstream sees the original payload.
    let (buffered_body, streaming_body) =
        if openai_profile.is_some_and(OpenAiRequestProfile::requires_json_body) {
            let request = match collect_proxy_json_body(&parts.headers, incoming_body).await {
                Ok(request) => request,
                Err(error) => {
                    let (parts, body) = error.into_response().into_parts();
                    return hyper::Response::from_parts(parts, full_body(body));
                }
            };
            let body = request.into_body();
            let content_length = match http::HeaderValue::from_str(&body.len().to_string()) {
                Ok(content_length) => content_length,
                Err(_) => return error_response(500, "Internal server error"),
            };
            parts
                .headers
                .insert(http::header::CONTENT_LENGTH, content_length);
            (Some(body), None)
        } else {
            (None, Some(incoming_body))
        };

    let load_balancer = route_plan
        .load_balancer
        .as_ref()
        .expect("direct HTTP routes always bind a load balancer")
        .as_ref();
    let bound_backend = route_plan.direct_http_binding.as_ref();
    let backend_tracking = if bound_backend.is_some() {
        BackendOperationTracking::Untracked
    } else {
        BackendOperationTracking::Tracked
    };
    let selected_backend = if bound_backend.is_none() {
        load_balancer.next_backend()
    } else {
        None
    };
    let (backend, timeouts) = if let Some(binding) = bound_backend {
        if !binding.backend.is_healthy() {
            return error_response(503, "No healthy backends");
        }
        (&binding.backend, binding.timeouts)
    } else {
        let Some(backend) = selected_backend.as_ref() else {
            return error_response(503, "No healthy backends");
        };
        let timeouts = load_balancer.timeouts();
        (
            backend,
            HttpTimeouts::new(
                timeouts.request_timeout(),
                timeouts.stream_idle_timeout(),
                timeouts.stream_total_timeout(),
            ),
        )
    };
    let bill_path = parts.uri.path().to_string();
    let bill_headers = parts.headers.clone();
    let forward_options = ForwardOptions {
        context: Some(forwarded),
        timeouts: Some(timeouts),
    };
    let result = if let Some(body) = buffered_body {
        state
            .http_proxy_for_backend(service_name, backend)
            .forward_buffered_exchange_owned(
                backend,
                OwnedBufferedRequest {
                    method: parts.method,
                    uri: parts.uri,
                    headers: parts.headers,
                    body,
                },
                forward_options,
                prepared_forwarded,
                backend_tracking,
            )
            .await
    } else {
        let Some(incoming_body) = streaming_body else {
            return error_response(500, "Internal server error");
        };
        state
            .http_proxy_for_backend(service_name, backend)
            .forward_streaming_exchange_owned(
                backend,
                OwnedStreamingRequest {
                    method: parts.method,
                    uri: parts.uri,
                    headers: parts.headers,
                    body: incoming_body,
                },
                forward_options,
                prepared_forwarded,
                backend_tracking,
            )
            .await
    };

    match result {
        Ok(mut proxy_response) => {
            route_plan
                .pipeline
                .observe_upstream_response(proxy_response.status);
            route_plan
                .passive_health
                .as_ref()
                .expect("proxy routes bind passive health")
                .record_response(backend, proxy_response.status.as_u16());
            if let Err(error) = agent_billing::settle(
                state.agent_billing.as_ref(),
                service_name,
                &bill_path,
                proxy_response.status,
                &mut proxy_response.headers,
                &bill_headers,
                None,
            )
            .await
            {
                tracing::error!(error = %error, service = service_name, "agent usage was not charged");
                return error_response(503, "usage could not be recorded");
            }
            let mut response = hyper::Response::new(ResponseBody::proxy(proxy_response.body));
            *response.status_mut() = proxy_response.status;
            *response.headers_mut() = proxy_response.headers;
            response
        }
        Err(error) => {
            let status = protocol::proxy_error_status(&error);
            // Only failures that happen before upstream response headers can
            // be attributed to the selected backend. Local admission and
            // response-body errors use `ServiceUnavailable` and must not trip
            // circuit/passive health state.
            if error.permits_pre_response_fallback() {
                route_plan.pipeline.observe_upstream_failure();
                route_plan
                    .passive_health
                    .as_ref()
                    .expect("proxy routes bind passive health")
                    .record_error(backend, status);
            }
            tracing::error!(error = %error, backend = backend.url, "Proxy error");
            let mut response = hyper::Response::new(ResponseBody::full(Bytes::from(
                crate::error::json_error_body(error.to_string()),
            )));
            *response.status_mut() =
                http::StatusCode::from_u16(status).unwrap_or(http::StatusCode::BAD_GATEWAY);
            response
        }
    }
}

/// Resolve the request authority used for `Host()` routing and access logs.
///
/// `req.uri().authority()` wins whenever present — HTTP/2 `:authority` and
/// HTTP/1.1 absolute-form name the authority the client actually targeted,
/// and a conflicting `Host` header is attacker-controllable. Virtual-routing
/// clients that connect by address send HTTP/1.1 origin-form, which has no
/// URI authority and falls back to `Host`.
fn request_host_authority<B>(req: &hyper::Request<B>) -> Option<&str> {
    let host_header = req
        .headers()
        .get(http::header::HOST)
        .and_then(|value| value.to_str().ok());
    let uri_authority = req.uri().authority().map(|authority| authority.as_str());

    match (host_header, uri_authority) {
        (_, Some(authority)) => Some(authority),
        (Some(host), None) => Some(host),
        (None, None) => None,
    }
}

/// Serve an ACME HTTP-01 key authorization from the shared challenge store.
fn serve_acme_http01_challenge(
    store: &ChallengeStore,
    path: &str,
) -> hyper::Response<ResponseBody> {
    let Some(token) = ChallengeStore::extract_token(path) else {
        return error_response(404, "Not found");
    };
    match store.get(token) {
        Some(key_authorization) => {
            let mut response = hyper::Response::new(full_body(key_authorization));
            *response.status_mut() = http::StatusCode::OK;
            response.headers_mut().insert(
                http::header::CONTENT_TYPE,
                http::HeaderValue::from_static("text/plain"),
            );
            response
        }
        None => error_response(404, "Not found"),
    }
}

/// Handle an individual HTTP request, dispatching to the correct protocol proxy.
///
/// Protocol detection order:
/// 1. ACME HTTP-01 (`/.well-known/acme-challenge/*`) when a challenge store is attached
/// 2. WebSocket upgrade (Upgrade: websocket) → bidirectional relay
/// 3. gRPC (Content-Type: application/grpc) → HTTP/2 h2c proxy
/// 4. SSE (`Accept: text/event-stream` or native OpenAI `stream: true`) →
///    streaming passthrough
/// 5. Plain HTTP → buffered reverse proxy
/// Only requests without a body can be held and re-routed: nothing has been read
/// from the client, so replaying them after activation cannot duplicate input.
fn request_is_holdable<B>(req: &hyper::Request<B>) -> bool {
    let headers = req.headers();
    if headers.contains_key(http::header::TRANSFER_ENCODING) {
        return false;
    }
    match headers.get(http::header::CONTENT_LENGTH) {
        None => true,
        Some(value) => {
            value
                .to_str()
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
                == Some(0)
        }
    }
}

/// True when the request matches no activation-pending route after a newer
/// snapshot was applied before `hold` elapsed.
async fn wait_for_activation<B>(
    runtime: &GatewayRuntime,
    held_state: &Arc<GatewayState>,
    req: &hyper::Request<B>,
    entrypoint: &str,
    hold: std::time::Duration,
) -> bool {
    let deadline = tokio::time::Instant::now() + hold;
    loop {
        let notified = runtime.swapped.notified();
        tokio::pin!(notified);
        notified.as_mut().enable();
        let current = runtime.load();
        if !Arc::ptr_eq(&current, held_state) && !activation_pending(&current, req, entrypoint) {
            return true;
        }
        if tokio::time::timeout_at(deadline, notified).await.is_err() {
            return false;
        }
    }
}

/// Whether the route's service pool currently exposes a healthy backend.
fn route_service_has_healthy_backend(state: &GatewayState, service: &str) -> bool {
    state
        .service_registry
        .get(service)
        .map(|load_balancer| {
            load_balancers_backends(load_balancer.as_ref())
                .iter()
                .any(|backend| backend.is_healthy())
        })
        .unwrap_or(false)
}

/// Reuse the snapshot-swap wait: parking ends when a new state lands and
/// the service gains a healthy backend (the activation variant's
/// router-map check generalized to service health).
async fn wait_for_parking<B>(
    runtime: &GatewayRuntime,
    held_state: &Arc<GatewayState>,
    req: &hyper::Request<B>,
    entrypoint: &str,
    hold: std::time::Duration,
) -> bool {
    let _ = (req, entrypoint);
    let deadline = tokio::time::Instant::now() + hold;
    loop {
        let notified = runtime.swapped.notified();
        tokio::pin!(notified);
        notified.as_mut().enable();
        let current = runtime.load();
        if !Arc::ptr_eq(&current, held_state) {
            return true;
        }
        if tokio::time::timeout_at(deadline, notified).await.is_err() {
            return false;
        }
    }
}

fn parking_response(retry_after_secs: u64) -> hyper::Response<ResponseBody> {
    let mut response = hyper::Response::new(full_body(r#"{"code":"target_activating"}"#));
    *response.status_mut() = http::StatusCode::SERVICE_UNAVAILABLE;
    if let Ok(value) = http::HeaderValue::from_str(&retry_after_secs.to_string()) {
        response
            .headers_mut()
            .insert(http::header::RETRY_AFTER, value);
    }
    response
}

fn load_balancers_backends(
    load_balancer: &crate::service::LoadBalancer,
) -> std::sync::Arc<Vec<std::sync::Arc<crate::service::Backend>>> {
    load_balancer.backends()
}

fn activation_pending<B>(state: &GatewayState, req: &hyper::Request<B>, entrypoint: &str) -> bool {
    state
        .router_table
        .match_request_ref(
            request_host_authority(req),
            req.uri().path(),
            req.method().as_str(),
            req.headers(),
            entrypoint,
        )
        .is_some_and(|(route, _)| {
            state
                .hosted_function_activations
                .contains_key(&route.router_name)
        })
}

fn activating_response(retry_after_secs: u64) -> hyper::Response<ResponseBody> {
    let mut response = hyper::Response::new(full_body(r#"{"code":"hosted_function_activating"}"#));
    *response.status_mut() = http::StatusCode::SERVICE_UNAVAILABLE;
    response.headers_mut().insert(
        http::header::CONTENT_TYPE,
        http::HeaderValue::from_static("application/json"),
    );
    response.headers_mut().insert(
        http::header::RETRY_AFTER,
        http::HeaderValue::from(retry_after_secs),
    );
    response
}

const IDEMPOTENT_REQUEST_LIMIT: usize = 32 * 1024 * 1024;
const IDEMPOTENT_RESPONSE_LIMIT: usize = 8 * 1024 * 1024;

fn idempotent_bytes_response(status: u16, message: &str) -> hyper::Response<Bytes> {
    error_bytes_response(status, message)
}

async fn finish_idempotent(
    pipeline: &crate::middleware::Pipeline,
    state: &GatewayState,
    route: &crate::router::ResolvedRoute,
    request_headers: &http::HeaderMap,
    request_start: std::time::Instant,
    access_log: Option<crate::observability::access_log::RequestAccessLog>,
    response: hyper::Response<Bytes>,
) -> hyper::Response<ResponseBody> {
    finish_native_response(
        BufferedResponsePipeline::new(pipeline, request_headers),
        state,
        route,
        request_start,
        access_log,
        None,
        response,
    )
    .await
}

async fn run_idempotent_agent_request(
    state: &GatewayState,
    route: &Arc<crate::router::ResolvedRoute>,
    pipeline: &crate::middleware::Pipeline,
    req_parts: &http::request::Parts,
    body: Incoming,
    forwarded: crate::proxy::ForwardedContext,
    request_start: std::time::Instant,
    access_log: Option<crate::observability::access_log::RequestAccessLog>,
) -> hyper::Response<ResponseBody> {
    let fail = |status: u16, message: &str, access_log| {
        let response = idempotent_bytes_response(status, message);
        finish_idempotent(
            pipeline,
            state,
            route,
            &req_parts.headers,
            request_start,
            access_log,
            response,
        )
    };
    let Some(billing) = state.agent_billing.as_ref() else {
        return fail(503, "usage could not be recorded", access_log).await;
    };
    let key = req_parts
        .headers
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .unwrap_or("");
    if uuid::Uuid::parse_str(key).is_err() {
        return fail(400, "idempotency key must be a UUID", access_log).await;
    }
    let Some(org_id) = req_parts
        .headers
        .get("x-a3s-org")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| uuid::Uuid::parse_str(value).is_ok())
    else {
        return fail(403, "search and parse require an organization", access_log).await;
    };
    let Some((code, _)) = agent_billing::meter(&route.service_name, req_parts.uri.path()) else {
        return fail(503, "usage could not be recorded", access_log).await;
    };
    let collected = match collect_request_body_up_to(body, IDEMPOTENT_REQUEST_LIMIT).await {
        Ok(body) => body,
        Err(RequestBodyCollectionError::TooLarge) => {
            return fail(413, "request body exceeds limit", access_log).await;
        }
        Err(RequestBodyCollectionError::ReadFailed) => {
            return fail(400, "request body could not be read", access_log).await;
        }
    };
    let target = req_parts
        .uri
        .path_and_query()
        .map(|value| value.as_str())
        .unwrap_or(req_parts.uri.path());
    let digest = agent_billing::request_digest(req_parts.method.as_str(), target, &collected);
    let decision = billing.begin_idempotency(
        org_id,
        req_parts.uri.path(),
        code,
        key,
        &digest,
        std::time::SystemTime::now(),
    );
    let transaction_id = match decision {
        agent_billing::Idempotency::Replay(stored) => {
            let mut response = hyper::Response::new(Bytes::from(stored.body));
            *response.status_mut() = http::StatusCode::from_u16(stored.status)
                .unwrap_or(http::StatusCode::INTERNAL_SERVER_ERROR);
            if let Some(content_type) = stored.content_type {
                if let Ok(value) = http::HeaderValue::from_str(&content_type) {
                    response
                        .headers_mut()
                        .insert(http::header::CONTENT_TYPE, value);
                }
            }
            return finish_idempotent(
                pipeline,
                state,
                route,
                &req_parts.headers,
                request_start,
                access_log,
                response,
            )
            .await;
        }
        agent_billing::Idempotency::Rejected(message) => {
            let status = if message == "idempotency store is full" {
                503
            } else {
                409
            };
            return fail(status, message, access_log).await;
        }
        agent_billing::Idempotency::Proceed { transaction_id } => transaction_id,
    };
    let mut lease = billing.lease(org_id, req_parts.uri.path(), key);
    let Some(service) = state.service_registry.get(&route.service_name) else {
        return fail(503, "service has no backends", access_log).await;
    };
    let Some(backend) = service.next_backend() else {
        return fail(503, "service has no backends", access_log).await;
    };
    let service_timeouts = service.timeouts();
    let proxy_result = state
        .http_proxy_for_backend(&route.service_name, &backend)
        .forward_streaming_response_with_options(
            &backend,
            &req_parts.method,
            &req_parts.uri,
            &req_parts.headers,
            collected,
            ForwardOptions {
                context: Some(forwarded),
                timeouts: Some(HttpTimeouts::new(
                    service_timeouts.request_timeout(),
                    service_timeouts.stream_idle_timeout(),
                    service_timeouts.stream_total_timeout(),
                )),
            },
        )
        .await;
    let proxy_response = match proxy_result {
        Ok(response) => response,
        Err(error) => {
            let status = protocol::proxy_error_status(&error);
            return fail(status, "upstream request failed", access_log).await;
        }
    };
    if let Some(health) = state.passive_health.get(&route.service_name) {
        health.record_response(&backend, proxy_response.status.as_u16());
    }
    let status = proxy_response.status;
    let mut headers = proxy_response.headers;
    let response_body =
        match collect_limited_body(proxy_response.body, IDEMPOTENT_RESPONSE_LIMIT).await {
            Ok(body) => body,
            Err(_) => {
                return fail(503, "idempotency response is too large", access_log).await;
            }
        };
    if !agent_billing::remember_idempotent_response(status) {
        let _ = agent_billing::settle(
            Some(billing),
            &route.service_name,
            req_parts.uri.path(),
            status,
            &mut headers,
            &req_parts.headers,
            Some(&transaction_id),
        )
        .await;
        let mut response = hyper::Response::new(response_body);
        *response.status_mut() = status;
        if let Some(value) = headers.get(http::header::CONTENT_TYPE) {
            response
                .headers_mut()
                .insert(http::header::CONTENT_TYPE, value.clone());
        }
        return finish_idempotent(
            pipeline,
            state,
            route,
            &req_parts.headers,
            request_start,
            access_log,
            response,
        )
        .await;
    }
    if let Err(error) = billing.accept_stored_response(response_body.len()) {
        return fail(503, error, access_log).await;
    }
    if let Err(error) = agent_billing::settle(
        Some(billing),
        &route.service_name,
        req_parts.uri.path(),
        status,
        &mut headers,
        &req_parts.headers,
        Some(&transaction_id),
    )
    .await
    {
        tracing::error!(error = %error, "agent usage was not charged");
        lease.release_for_retry();
        return fail(503, "usage could not be recorded", access_log).await;
    }
    let content_type = headers
        .get(http::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    if let Err(error) = billing.complete_idempotency(
        org_id,
        req_parts.uri.path(),
        key,
        &digest,
        agent_billing::StoredResponse {
            status: status.as_u16(),
            content_type,
            body: response_body.to_vec(),
        },
    ) {
        tracing::error!(error, "idempotency response was charged but not stored");
        agent_billing::retain_idempotency_after_charge(&mut lease, false);
        let mut response = hyper::Response::new(response_body);
        *response.status_mut() = status;
        if let Some(value) = headers.get(http::header::CONTENT_TYPE) {
            response
                .headers_mut()
                .insert(http::header::CONTENT_TYPE, value.clone());
        }
        return finish_idempotent(
            pipeline,
            state,
            route,
            &req_parts.headers,
            request_start,
            access_log,
            response,
        )
        .await;
    }
    agent_billing::retain_idempotency_after_charge(&mut lease, true);
    let mut response = hyper::Response::new(response_body);
    *response.status_mut() = status;
    if let Some(value) = headers.get(http::header::CONTENT_TYPE) {
        response
            .headers_mut()
            .insert(http::header::CONTENT_TYPE, value.clone());
    }
    finish_idempotent(
        pipeline,
        state,
        route,
        &req_parts.headers,
        request_start,
        access_log,
        response,
    )
    .await
}

async fn handle_http_request(
    req: hyper::Request<Incoming>,
    runtime: GatewayRuntime,
    connection: Arc<HttpConnectionContext>,
) -> std::result::Result<hyper::Response<ResponseBody>, hyper::Error> {
    let (response, wire) = crate::inference::with_native_wire_scope(Box::pin(
        handle_http_request_inner(req, runtime, connection),
    ))
    .await;
    let response = response?;
    Ok(match wire {
        Some(wire) => native_response::render_native_wire_error(response, wire),
        None => response,
    })
}

async fn handle_http_request_inner(
    mut req: hyper::Request<Incoming>,
    runtime: GatewayRuntime,
    connection: Arc<HttpConnectionContext>,
) -> std::result::Result<hyper::Response<ResponseBody>, hyper::Error> {
    let state = runtime.load();
    let remote_addr = connection.remote_addr;
    let entrypoint = connection.entrypoint.as_ref();
    // Capture one trust-policy snapshot for the whole request. Listener
    // reconfiguration may update the shared flag concurrently, but routing,
    // policy identity, and outbound forwarding must agree on one generation.
    let forwarded = connection.forwarded();
    let effective_client_ip = remote_addr.map_or_else(
        || "unix".to_owned(),
        |remote_addr| {
            crate::proxy::http_proxy::effective_client_ip(
                req.headers(),
                remote_addr,
                forwarded.trust_inbound_headers,
            )
        },
    );

    // ACME HTTP-01 must be answered from the same ChallengeStore the manager
    // writes — before route match so issuance cannot soft-open a forever-warn
    // loop while /.well-known/acme-challenge/* only 404s.
    if let Some(challenges) = runtime.acme_challenges() {
        if ChallengeStore::is_challenge_path(req.uri().path()) {
            return Ok(serve_acme_http01_challenge(
                challenges.as_ref(),
                req.uri().path(),
            ));
        }
    }

    // Anonymous Chatbot owns this path before route match. An absent block
    // fails closed and must not fall through to a snapshot catch-all.
    if crate::anonymous_chatbot::owns_path(req.uri().path()) {
        return Ok(crate::anonymous_chatbot::handle(req, state.anonymous_chatbot.clone()).await);
    }

    // WebSocket and gRPC require protocol-specific dispatch. SSE and ordinary
    // HTTP can share the zero-copy HTTP relay on a feature-free route.
    let is_ws = crate::proxy::websocket::is_websocket_upgrade(req.headers());
    let is_grpc = crate::proxy::grpc::is_grpc_request(req.headers());
    let initial_openai_profile =
        OpenAiRequestProfile::match_request(req.method(), req.uri().path());

    let request_host = request_host_authority(&req);
    let mut access_log = if state.access_log_enabled {
        let host = request_host.map(str::to_owned);
        let user_agent = req
            .headers()
            .get("user-agent")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        Some(RequestAccessLog::new(
            state.access_log.start_request(),
            state.log_tx.clone(),
            effective_client_ip.clone(),
            req.method().as_str().to_owned(),
            req.uri().path().to_owned(),
            host,
            entrypoint.to_owned(),
            user_agent,
        ))
    } else {
        None
    };

    // Build a trace only when propagation is enabled. Managed inference can
    // still create one lazily below when it needs a stable request identity.
    let mut trace_ctx = request_trace_context(req.headers(), state.tracing_enabled);

    // Reserved catalog paths never fall through to an ordinary snapshot route.
    if OpenAiRequestProfile::is_reserved_route(req.method(), req.uri().path()) {
        if state.metrics_enabled {
            state.metrics.record_request(404, 0);
        }
        return Ok(finish_access_log(
            access_log,
            error_response(404, "standard_not_admitted"),
        ));
    }

    // Route the request.
    let (route, route_plan_index) = match state.router_table.match_request_ref(
        request_host,
        req.uri().path(),
        req.method().as_str(),
        req.headers(),
        entrypoint,
    ) {
        Some(route) => route,
        None => {
            if state.metrics_enabled {
                state.metrics.record_request(404, 0);
            }
            return Ok(finish_access_log(
                access_log,
                error_response(404, "No route matched"),
            ));
        }
    };
    if let Some(access_log) = access_log.as_mut() {
        access_log.set_router(route.router_name.clone());
    }
    // ED2: generalized request parking — a route declaring a parking
    // policy holds holdable requests while its service pool has no
    // healthy backend, re-routing after the next applied snapshot;
    // otherwise 503 with the bounded Retry-After. The hosted-function
    // activation check below remains the scale-from-zero special case.
    if let Some(parking) = route.parking.as_ref() {
        if !route_service_has_healthy_backend(&state, &route.service_name) {
            if request_is_holdable(&req) {
                if wait_for_parking(&runtime, &state, &req, entrypoint, parking.hold()).await {
                    drop(access_log);
                    return Box::pin(handle_http_request(req, runtime, connection)).await;
                }
            }
            return Ok(finish_access_log(
                access_log,
                parking_response(parking.retry_after_secs),
            ));
        }
    }
    if let Some(activation) = state.hosted_function_activations.get(&route.router_name) {
        let activation = activation.clone();
        runtime
            .activation_demand
            .record(&activation, chrono::Utc::now());
        if let Some(hold) = activation.hold.filter(|_| request_is_holdable(&req)) {
            if wait_for_activation(&runtime, &state, &req, entrypoint, hold).await {
                drop(access_log);
                return Box::pin(handle_http_request(req, runtime, connection)).await;
            }
        }
        return Ok(finish_access_log(
            access_log,
            activating_response(activation.retry_after_secs),
        ));
    }
    // AR0.6 duty 1 + telemetry: a router carrying wake-eligible agent
    // executions records ingress demand (the observation half). The
    // emission (duty 2) fires on the dedup window; the hold (duty 3)
    // rides the agent-execution dispatch binding; duty 4 is the
    // precheck the dispatch path already enforces via invariant 5.
    // Recording never blocks the request and never creates desired
    // state.
    if !state.agent_wake.policies.is_empty() {
        if let Some((organization_id, execution_id)) = state
            .agent_wake
            .policies
            .entries()
            .first()
            .map(|entry| (entry.organization_id, entry.agent_execution_id))
        {
            let now = chrono::Utc::now();
            if let Some(entry) = state
                .agent_wake
                .policies
                .lookup(organization_id, execution_id)
            {
                let decision = state.agent_wake.demand.observe(
                    entry.organization_id,
                    entry.agent_execution_id,
                    &entry.idle_policy_digest,
                    now,
                );
                if matches!(decision, crate::agent_wake::WakeDecision::Emit) {
                    let emission = crate::agent_wake::AgentWakeEmission {
                        organization_id: entry.organization_id,
                        agent_execution_id: entry.agent_execution_id,
                        idle_policy_digest: entry.idle_policy_digest.clone(),
                        demand_observed_at: now,
                        expires_at: now + crate::agent_wake::SIGNAL_VALIDITY,
                    };
                    state.agent_wake.emitter.enqueue(emission);
                }
            }
        }
    }
    let inference_authorizer = state
        .inference_authorizer
        .as_ref()
        .filter(|authorizer| authorizer.owns_router(&route.router_name))
        .cloned();
    // Middleware and service objects are bound to the sorted route at startup,
    // so the request path uses one checked array lookup instead of name hashes.
    let Some(route_plan) = state.route_plans.get(route_plan_index) else {
        tracing::error!(
            router = %route.router_name,
            "Pre-compiled route plan is missing"
        );
        return Ok(finish_access_log(
            access_log,
            error_response(500, "Internal server error"),
        ));
    };
    let static_bundle = route_plan.static_bundle.clone();
    if static_bundle.is_some() && (is_ws || is_grpc) {
        return Ok(finish_access_log(
            access_log,
            error_response(404, "Not found"),
        ));
    }
    let direct_http = route_plan.direct_http_eligible
        && static_bundle.is_none()
        && inference_authorizer.is_none()
        && !state.metrics_enabled
        && !state.access_log_enabled
        && !state.tracing_enabled
        && !is_ws
        && !is_grpc;
    if direct_http {
        return Ok(handle_direct_http_request(
            req,
            state.as_ref(),
            route_plan,
            &route.service_name,
            forwarded,
            connection.prepared_forwarded.as_deref(),
            initial_openai_profile,
        )
        .await);
    }

    let mut route = Arc::clone(route);
    let mut is_sse = crate::proxy::streaming::is_streaming_request(req.headers());

    let request_start = std::time::Instant::now();

    // Ordinary routes retain their existing route-time service accounting.
    // Managed inference routes select a service only after authorization and
    // model resolution, so their service count is recorded at that point.
    if state.metrics_enabled {
        state.metrics.record_router_request(&route.router_name);
        if inference_authorizer.is_none() {
            state.metrics.record_service_request(&route.service_name);
        }
    }
    let mut service_request = if state.metrics_enabled && inference_authorizer.is_none() {
        state
            .metrics
            .track_service_request(&route.service_name, request_start)
    } else {
        None
    };
    let pipeline = route_plan.pipeline.clone();

    let request_context = (!pipeline.is_empty()).then(|| RequestContext {
        client_ip: effective_client_ip,
        entrypoint: entrypoint.to_owned(),
        router: route.router_name.clone(),
    });

    // A router present in the managed inference policy is a closed native
    // surface. Authenticate before request middleware or body collection, and
    // remove the client credential before any later upstream dispatch.
    let mut managed_openai_profile = None;
    let mut authenticated_inference: Option<(Arc<InferenceAuthorizer>, AuthenticatedInference)> =
        None;
    let mut inference_admission: Option<InferenceAdmissionGuard> = None;
    let mut inference_request_identity: Option<InferenceRequestIdentity> = None;
    let mut inference_dispatch: Option<InferenceDispatchState> = None;
    let mut prepared_inference_attempt: Option<PreparedInferenceAttempt> = None;
    let mut usage_lifecycle: Option<UsageRequestLifecycle> = None;
    let mut listing_forward = None;
    if let Some(authorizer) = inference_authorizer {
        if let Some(sku) = tool_invoke_sku(req.method(), req.uri().path()) {
            if is_ws || is_grpc {
                return Ok(finish_native_response(
                    BufferedResponsePipeline::new(&pipeline, req.headers()),
                    &state,
                    &route,
                    request_start,
                    access_log,
                    None,
                    inference_access_error_response(
                        InferenceAccessError::UnsupportedEndpoint,
                        None,
                    ),
                )
                .await);
            }
            let trace_id = trace_ctx
                .get_or_insert_with(crate::observability::tracing::TraceContext::new_root)
                .trace_id
                .clone();
            let listing = match authorizer
                .authenticate_listing(
                    &route.router_name,
                    sku,
                    req.headers(),
                    chrono::Utc::now(),
                    trace_id,
                )
                .await
            {
                Ok(listing) => listing,
                Err(error) => {
                    return Ok(finish_native_response(
                        BufferedResponsePipeline::new(&pipeline, req.headers()),
                        &state,
                        &route,
                        request_start,
                        access_log,
                        None,
                        inference_access_error_response(error, None),
                    )
                    .await);
                }
            };
            req.headers_mut().remove(http::header::AUTHORIZATION);
            listing.identity.prepare_request_headers(req.headers_mut());
            if let Some(access_log) = access_log.as_mut() {
                access_log.set_inference_request(&listing.identity);
            }
            let attempt = listing.identity.begin_attempt(listing.listing_id);
            if let Some(spool) = state.usage_spool.clone() {
                let mut lifecycle = match UsageRequestLifecycle::begin(
                    spool,
                    &listing.identity,
                    listing.authenticated,
                    &listing.sku,
                    None,
                )
                .await
                {
                    Ok(lifecycle) => lifecycle,
                    Err(error) => {
                        tracing::error!(
                            error = %error,
                            request_id = %listing.identity.request_id(),
                            "Tool listing rejected because durable usage could not start"
                        );
                        return Ok(finish_native_response(
                            BufferedResponsePipeline::new(&pipeline, req.headers()),
                            &state,
                            &route,
                            request_start,
                            access_log,
                            Some(&listing.identity),
                            inference_access_error_response(
                                InferenceAccessError::UsageUnavailable,
                                Some(&listing.identity),
                            ),
                        )
                        .await);
                    }
                };
                if let Err(error) = lifecycle.begin_attempt(&attempt).await {
                    tracing::error!(
                        error = %error,
                        request_id = %listing.identity.request_id(),
                        "Tool listing rejected because durable attempt usage could not start"
                    );
                    let response = finish_native_response(
                        BufferedResponsePipeline::new(&pipeline, req.headers()),
                        &state,
                        &route,
                        request_start,
                        access_log,
                        Some(&listing.identity),
                        inference_access_error_response(
                            InferenceAccessError::UsageUnavailable,
                            Some(&listing.identity),
                        ),
                    )
                    .await;
                    return Ok(track_usage_response(response, Some(lifecycle)));
                }
                usage_lifecycle = Some(lifecycle);
            }
            let admission =
                match authorizer.admit_request(listing.authenticated, chrono::Utc::now()) {
                    Ok(admission) => admission,
                    Err(error) => {
                        let response = finish_native_response(
                            BufferedResponsePipeline::new(&pipeline, req.headers()),
                            &state,
                            &route,
                            request_start,
                            access_log,
                            Some(&listing.identity),
                            inference_access_error_response(error, Some(&listing.identity)),
                        )
                        .await;
                        return Ok(track_usage_response(response, usage_lifecycle.take()));
                    }
                };
            inference_admission = Some(admission);
            authenticated_inference = Some((authorizer, listing.authenticated));
            inference_request_identity = Some(listing.identity);
            listing_forward = Some((listing.service, attempt));
        } else {
            if authorizer.route_wire(&route.router_name) == crate::config::InferenceWire::Openai {
                crate::inference::mark_native_wire(
                    if initial_openai_profile.is_some_and(OpenAiRequestProfile::is_anthropic) {
                        crate::inference::NativeWire::Anthropic
                    } else {
                        crate::inference::NativeWire::OpenAi
                    },
                );
            }
            let Some(profile) = initial_openai_profile else {
                return Ok(finish_native_response(
                    BufferedResponsePipeline::new(&pipeline, req.headers()),
                    &state,
                    &route,
                    request_start,
                    access_log,
                    None,
                    inference_access_error_response(
                        InferenceAccessError::UnsupportedEndpoint,
                        None,
                    ),
                )
                .await);
            };
            if is_ws || is_grpc {
                return Ok(finish_native_response(
                    BufferedResponsePipeline::new(&pipeline, req.headers()),
                    &state,
                    &route,
                    request_start,
                    access_log,
                    None,
                    inference_access_error_response(
                        InferenceAccessError::UnsupportedEndpoint,
                        None,
                    ),
                )
                .await);
            }
            let authenticated = match authorizer
                .authenticate(
                    &route.router_name,
                    profile,
                    req.headers(),
                    chrono::Utc::now(),
                )
                .await
            {
                Ok(authenticated) => authenticated,
                Err(error) => {
                    return Ok(finish_native_response(
                        BufferedResponsePipeline::new(&pipeline, req.headers()),
                        &state,
                        &route,
                        request_start,
                        access_log,
                        None,
                        inference_access_error_response(error, None),
                    )
                    .await);
                }
            };
            let trace_id = trace_ctx
                .get_or_insert_with(crate::observability::tracing::TraceContext::new_root)
                .trace_id
                .clone();
            let identity = match authorizer.request_identity(
                authenticated,
                profile,
                trace_id,
                chrono::Utc::now(),
            ) {
                Ok(identity) => identity,
                Err(error) => {
                    return Ok(finish_native_response(
                        BufferedResponsePipeline::new(&pipeline, req.headers()),
                        &state,
                        &route,
                        request_start,
                        access_log,
                        None,
                        inference_access_error_response(error, None),
                    )
                    .await);
                }
            };
            req.headers_mut().remove(http::header::AUTHORIZATION);
            req.headers_mut().remove("x-api-key");
            if authenticated.is_principal() {
                req.headers_mut().remove(http::header::COOKIE);
            }
            identity.prepare_request_headers(req.headers_mut());
            if let Some(access_log) = access_log.as_mut() {
                access_log.set_inference_request(&identity);
            }
            managed_openai_profile = Some(profile);
            authenticated_inference = Some((authorizer, authenticated));
            inference_request_identity = Some(identity);
        }
    }

    // ── WebSocket upgrade path ───────────────────────────────────────────────
    // Must be handled before req.into_parts() since hyper::upgrade::on() needs
    // the full Request<Incoming>.
    if is_ws {
        return Ok(websocket_dispatch::dispatch(
            req,
            websocket_dispatch::WebSocketDispatchContext {
                route,
                state,
                pipeline,
                request_context,
                trace_context: trace_ctx,
                remote_addr,
                forwarded,
                access_log,
                request_start,
                service_request,
                upgraded_sessions: connection.upgraded_sessions.clone(),
            },
        )
        .await);
    }

    // ── Non-WebSocket path: consume request body ─────────────────────────────
    let (mut req_parts, body) = req.into_parts();

    // Run request-phase middleware.
    if let Some(request_context) = request_context.as_ref() {
        match pipeline
            .process_request(&mut req_parts, request_context)
            .await
        {
            Ok(Some(response)) => {
                // A short-circuit is still a route response. Run the
                // response phase so CORS, security headers, compression, and
                // bounded response transforms apply consistently to local
                // policy responses as they do to upstream responses.
                return Ok(finish_native_response(
                    BufferedResponsePipeline::new(&pipeline, &req_parts.headers),
                    &state,
                    &route,
                    request_start,
                    access_log,
                    inference_request_identity.as_ref(),
                    response.map(Bytes::from),
                )
                .await);
            }
            Ok(None) => {}
            Err(e) => {
                tracing::error!(error = %e, "Middleware error");
                return Ok(finish_native_response(
                    BufferedResponsePipeline::new(&pipeline, &req_parts.headers),
                    &state,
                    &route,
                    request_start,
                    access_log,
                    inference_request_identity.as_ref(),
                    error_bytes_response(500, "Middleware error"),
                )
                .await);
            }
        }
    }

    if let Some(message) = agent_billing::refusal(
        state.agent_billing.as_ref(),
        &route.service_name,
        req_parts.uri.path(),
        &req_parts.headers,
    ) {
        return Ok(finish_native_response(
            BufferedResponsePipeline::new(&pipeline, &req_parts.headers),
            &state,
            &route,
            request_start,
            access_log,
            inference_request_identity.as_ref(),
            error_bytes_response(403, message),
        )
        .await);
    }

    if agent_billing::idempotent_agent_request(
        state.agent_billing.as_ref(),
        &route.service_name,
        req_parts.uri.path(),
        &req_parts.headers,
    ) {
        return Ok(run_idempotent_agent_request(
            state.as_ref(),
            &route,
            &pipeline,
            &req_parts,
            body,
            forwarded,
            request_start,
            access_log,
        )
        .await);
    }

    if let Some(static_bundle) = static_bundle.as_ref() {
        let accept = req_parts
            .headers
            .get(http::header::ACCEPT)
            .and_then(|value| value.to_str().ok());
        let if_none_match = req_parts
            .headers
            .get(http::header::IF_NONE_MATCH)
            .and_then(|value| value.to_str().ok());
        let range = req_parts
            .headers
            .get(http::header::RANGE)
            .and_then(|value| value.to_str().ok());
        let if_range = req_parts
            .headers
            .get(http::header::IF_RANGE)
            .and_then(|value| value.to_str().ok());
        let response = match crate::static_object::serve_static_bundle(
            static_bundle,
            &req_parts.method,
            req_parts.uri.path(),
            crate::static_object::StaticServeHeaders {
                accept,
                if_none_match,
                range,
                if_range,
            },
        )
        .await
        {
            Ok(response) => response,
            Err(error) => {
                let (status, message) = crate::static_object::static_serve_error_response(error);
                error_bytes_response(status, message)
            }
        };
        return Ok(finish_native_response(
            BufferedResponsePipeline::new(&pipeline, &req_parts.headers),
            &state,
            &route,
            request_start,
            access_log,
            None,
            response,
        )
        .await);
    }

    // Standalone and ordinary managed routes retain the post-middleware
    // request profile. A policy-bound inference router uses the exact
    // pre-middleware profile authenticated above.
    let openai_profile = managed_openai_profile
        .or_else(|| OpenAiRequestProfile::match_request(&req_parts.method, req_parts.uri.path()));

    if managed_openai_profile == Some(OpenAiRequestProfile::Models) {
        let response_and_admission = authenticated_inference
            .as_ref()
            .ok_or(InferenceAccessError::Unavailable)
            .and_then(|(authorizer, authenticated)| {
                let models = authorizer.allowed_models(*authenticated, chrono::Utc::now())?;
                let request_id = inference_request_identity
                    .as_ref()
                    .map(|identity| identity.request_id())
                    .unwrap_or_else(uuid::Uuid::new_v4);
                let response = if authorizer.route_wire(&route.router_name)
                    == crate::config::InferenceWire::Openai
                {
                    native_response::native_json_response(
                        crate::inference::native_models_body(&models),
                        request_id,
                    )
                } else {
                    models_response(&models, request_id)
                        .map_err(|_| InferenceAccessError::Unavailable)?
                };
                let admission = authorizer.admit_request(*authenticated, chrono::Utc::now())?;
                Ok((response, admission))
            });
        let (response, admission) = match response_and_admission {
            Ok(response_and_admission) => response_and_admission,
            Err(error) => {
                return Ok(finish_native_response(
                    BufferedResponsePipeline::new(&pipeline, &req_parts.headers),
                    &state,
                    &route,
                    request_start,
                    access_log,
                    inference_request_identity.as_ref(),
                    inference_access_error_response(error, inference_request_identity.as_ref()),
                )
                .await);
            }
        };
        let response = finish_native_response(
            BufferedResponsePipeline::new(&pipeline, &req_parts.headers),
            &state,
            &route,
            request_start,
            access_log,
            inference_request_identity.as_ref(),
            response,
        )
        .await;
        drop(admission);
        return Ok(response);
    }

    // OCR and System One carry no model alias. Serve them here so the image
    // bytes and the decision body never enter chat or embeddings dispatch.
    if let Some(profile) = managed_openai_profile.filter(|profile| profile.is_platform_model()) {
        let served = serve_signed_in_platform_model(
            &state,
            &route,
            profile,
            &authenticated_inference,
            inference_request_identity.as_mut(),
            &req_parts,
            body,
        )
        .await;
        let response = finish_native_response(
            BufferedResponsePipeline::new(&pipeline, &req_parts.headers),
            &state,
            &route,
            request_start,
            access_log,
            inference_request_identity.as_ref(),
            served.response,
        )
        .await;
        return Ok(track_usage_response(response, served.lifecycle));
    }

    // Sample a mirror before optional body collection so an unsampled request
    // keeps its streaming path. Requests that already require buffering defer
    // sampling until service resolution; managed inference can replace the
    // route's placeholder service after parsing the OpenAI body.
    let buffers_without_mirror =
        is_sse || openai_profile.is_some_and(OpenAiRequestProfile::requires_json_body);
    let mut selected_mirror = if buffers_without_mirror {
        None
    } else {
        state
            .mirrors
            .get(&route.service_name)
            .filter(|mirror| mirror.should_mirror())
            // A mirror requires a replayable buffered body. If the transport
            // does not advertise a finite size, keep the primary request on
            // its streaming path and omit the auxiliary copy.
            .filter(|_| {
                body.size_hint()
                    .upper()
                    .is_some_and(|upper| upper <= crate::service::MAX_MIRROR_BODY_BYTES as u64)
            })
            .cloned()
    };
    // A retry policy needs an immutable body that can be replayed.  Without
    // this gate the common empty-body GET path stays streaming and silently
    // bypasses retry execution because `Incoming` cannot be cloned safely.
    let retry_requires_buffered_body = pipeline.retry_policy().is_some()
        && protocol::request_is_replayable(&req_parts.method, &req_parts.headers);
    let needs_buffered_body =
        buffers_without_mirror || selected_mirror.is_some() || retry_requires_buffered_body;
    let request_body_limit = pipeline.request_body_limit();
    let body_requires_bounded_collection = request_body_limit.is_some_and(|limit| {
        body.size_hint()
            .upper()
            .is_none_or(|upper| upper > limit as u64)
    });
    let buffered_body_limit =
        request_body_limit.or(needs_buffered_body.then_some(DEFAULT_BUFFERED_REQUEST_BODY_LIMIT));

    let (body_bytes, streaming_body) = if openai_profile
        .is_some_and(OpenAiRequestProfile::requires_json_body)
    {
        let openai_limit = OPENAI_REQUEST_BODY_LIMIT;
        let collection_limit = request_body_limit.unwrap_or(openai_limit).min(openai_limit);
        if declared_body_exceeds_limit(&req_parts.headers, collection_limit) {
            return Ok(finish_native_response(
                BufferedResponsePipeline::new(&pipeline, &req_parts.headers),
                &state,
                &route,
                request_start,
                access_log,
                inference_request_identity.as_ref(),
                openai_request_error_response(
                    OpenAiRequestError::BodyTooLarge,
                    inference_request_identity.as_ref(),
                ),
            )
            .await);
        }
        let collected = match collect_request_body_up_to(body, collection_limit).await {
            Ok(body) => {
                collect_json_body(&req_parts.headers, http_body_util::Full::new(body)).await
            }
            Err(RequestBodyCollectionError::TooLarge) => {
                return Ok(finish_native_response(
                    BufferedResponsePipeline::new(&pipeline, &req_parts.headers),
                    &state,
                    &route,
                    request_start,
                    access_log,
                    inference_request_identity.as_ref(),
                    openai_request_error_response(
                        OpenAiRequestError::BodyTooLarge,
                        inference_request_identity.as_ref(),
                    ),
                )
                .await);
            }
            Err(RequestBodyCollectionError::ReadFailed) => {
                tracing::warn!(
                    peer = ?remote_addr,
                    version = ?req_parts.version,
                    content_length = req_parts
                        .headers
                        .get(http::header::CONTENT_LENGTH)
                        .and_then(|value| value.to_str().ok())
                        .unwrap_or("-"),
                    transfer_encoding = req_parts
                        .headers
                        .get(http::header::TRANSFER_ENCODING)
                        .and_then(|value| value.to_str().ok())
                        .unwrap_or("-"),
                    expect = req_parts
                        .headers
                        .get(http::header::EXPECT)
                        .and_then(|value| value.to_str().ok())
                        .unwrap_or("-"),
                    connection = req_parts
                        .headers
                        .get(http::header::CONNECTION)
                        .and_then(|value| value.to_str().ok())
                        .unwrap_or("-"),
                    "request body read failed"
                );
                return Ok(finish_native_response(
                    BufferedResponsePipeline::new(&pipeline, &req_parts.headers),
                    &state,
                    &route,
                    request_start,
                    access_log,
                    inference_request_identity.as_ref(),
                    openai_request_error_response(
                        OpenAiRequestError::BodyReadFailed,
                        inference_request_identity.as_ref(),
                    ),
                )
                .await);
            }
        };
        match collected {
            Ok(request) => {
                // The responses dialect streams through its own SSE lane
                // (translated); native responses streaming refuses at
                // admission before this point selects a handler.
                is_sse |= openai_profile.is_some_and(OpenAiRequestProfile::supports_streaming)
                    && request.stream_requested();
                is_sse |= openai_profile == Some(OpenAiRequestProfile::Responses)
                    && request.stream_requested();
                // Reserved platform aliases on embeddings and rerank serve
                // through the platform engines or a bound supplier before any
                // managed-model admission runs.
                if let Some((authorizer, authenticated)) = &authenticated_inference {
                    if let Some(identity) = inference_request_identity.as_mut() {
                        let alias = request.model_alias();
                        if matches!(
                            openai_profile,
                            Some(OpenAiRequestProfile::Embeddings)
                                | Some(OpenAiRequestProfile::Rerank)
                        ) && crate::inference::RESERVED_PLATFORM_ALIASES.contains(&alias)
                        {
                            let served = crate::inference::serve_platform_alias_model(
                                &state.service_registry,
                                state.usage_spool.clone(),
                                authorizer,
                                *authenticated,
                                identity,
                                &route.router_name,
                                openai_profile.unwrap_or(OpenAiRequestProfile::Embeddings),
                                &req_parts.headers,
                                request.body_bytes().clone(),
                            )
                            .await;
                            let response = finish_native_response(
                                BufferedResponsePipeline::new(&pipeline, &req_parts.headers),
                                &state,
                                &route,
                                request_start,
                                access_log,
                                inference_request_identity.as_ref(),
                                served.response,
                            )
                            .await;
                            return Ok(track_usage_response(response, served.lifecycle));
                        }
                    }
                }
                let body = if let Some((authorizer, authenticated)) = &authenticated_inference {
                    let alias = request.model_alias().to_string();
                    let reservation = openai_profile
                        .map(|profile| request.estimate_token_reservation_split(profile))
                        .unwrap_or(crate::inference::TokenReservation {
                            input: 1,
                            completion: 0,
                        });
                    let admission = match authorizer
                        .admit_model(*authenticated, &alias, reservation, chrono::Utc::now())
                        .await
                    {
                        Ok(admission) => admission,
                        Err(error) => {
                            return Ok(finish_native_response(
                                BufferedResponsePipeline::new(&pipeline, &req_parts.headers),
                                &state,
                                &route,
                                request_start,
                                access_log,
                                inference_request_identity.as_ref(),
                                inference_access_error_response(
                                    error,
                                    inference_request_identity.as_ref(),
                                ),
                            )
                            .await);
                        }
                    };
                    let identity = match inference_request_identity.take() {
                        Some(identity) => identity,
                        None => {
                            return Ok(finish_native_response(
                                BufferedResponsePipeline::new(&pipeline, &req_parts.headers),
                                &state,
                                &route,
                                request_start,
                                access_log,
                                None,
                                inference_access_error_response(
                                    InferenceAccessError::Unavailable,
                                    None,
                                ),
                            )
                            .await);
                        }
                    };
                    let mut dispatch = InferenceDispatchState::new(
                        authorizer.clone(),
                        *authenticated,
                        alias,
                        request,
                        identity,
                    );
                    if authorizer.route_wire(&route.router_name)
                        == crate::config::InferenceWire::Openai
                    {
                        dispatch.use_native_wire();
                    }
                    // HR3b/HR3c: resolve the task-routing window before
                    // the first attempt (the turn identity, the frozen
                    // cache, and — when a transport resolves — the
                    // decision call carrying the turn's content).
                    dispatch.resolve_routing(&state).await;
                    let prepared = match dispatch.prepare_next(
                        &state,
                        &mut req_parts.headers,
                        access_log.as_mut(),
                    ) {
                        Ok(prepared) => prepared,
                        Err(error) => {
                            return Ok(finish_native_response(
                                BufferedResponsePipeline::new(&pipeline, &req_parts.headers),
                                &state,
                                &route,
                                request_start,
                                access_log,
                                Some(dispatch.request_identity()),
                                inference_access_error_response(
                                    error,
                                    Some(dispatch.request_identity()),
                                ),
                            )
                            .await);
                        }
                    };
                    if let Some(spool) = state.usage_spool.clone() {
                        let lifecycle = match UsageRequestLifecycle::begin(
                            spool,
                            dispatch.request_identity(),
                            *authenticated,
                            dispatch.model_alias(),
                            dispatch.routing_facts(),
                        )
                        .await
                        {
                            Ok(lifecycle) => lifecycle,
                            Err(error) => {
                                tracing::error!(
                                    error = %error,
                                    request_id = %dispatch.request_identity().request_id(),
                                    "Managed inference rejected because durable usage could not start"
                                );
                                return Ok(finish_native_response(
                                    BufferedResponsePipeline::new(&pipeline, &req_parts.headers),
                                    &state,
                                    &route,
                                    request_start,
                                    access_log,
                                    Some(dispatch.request_identity()),
                                    inference_access_error_response(
                                        InferenceAccessError::UsageUnavailable,
                                        Some(dispatch.request_identity()),
                                    ),
                                )
                                .await);
                            }
                        };
                        usage_lifecycle = Some(lifecycle);
                        if let Some(lifecycle) = usage_lifecycle.as_mut() {
                            if let Err(error) = lifecycle.begin_attempt(&prepared.identity).await {
                                tracing::error!(
                                    error = %error,
                                    request_id = %dispatch.request_identity().request_id(),
                                    attempt_id = %prepared.identity.attempt_id(),
                                    "Managed inference rejected because durable attempt usage could not start"
                                );
                                let response = finish_native_response(
                                    BufferedResponsePipeline::new(&pipeline, &req_parts.headers),
                                    &state,
                                    &route,
                                    request_start,
                                    access_log,
                                    Some(dispatch.request_identity()),
                                    inference_access_error_response(
                                        InferenceAccessError::UsageUnavailable,
                                        Some(dispatch.request_identity()),
                                    ),
                                )
                                .await;
                                return Ok(track_usage_response(response, usage_lifecycle.take()));
                            }
                        }
                    }
                    let body = prepared.body.clone();
                    inference_request_identity = Some(dispatch.request_identity().clone());
                    prepared_inference_attempt = Some(prepared);
                    inference_dispatch = Some(dispatch);
                    inference_admission = Some(admission);
                    body
                } else {
                    request.into_body()
                };
                let content_length = match http::HeaderValue::from_str(&body.len().to_string()) {
                    Ok(content_length) => content_length,
                    Err(_) => {
                        return Ok(finish_native_response(
                            BufferedResponsePipeline::new(&pipeline, &req_parts.headers),
                            &state,
                            &route,
                            request_start,
                            access_log,
                            inference_request_identity.as_ref(),
                            inference_access_error_response(
                                InferenceAccessError::Unavailable,
                                inference_request_identity.as_ref(),
                            ),
                        )
                        .await);
                    }
                };
                req_parts
                    .headers
                    .insert(http::header::CONTENT_LENGTH, content_length);
                (body, None)
            }
            Err(error) => {
                return Ok(finish_native_response(
                    BufferedResponsePipeline::new(&pipeline, &req_parts.headers),
                    &state,
                    &route,
                    request_start,
                    access_log,
                    inference_request_identity.as_ref(),
                    openai_request_error_response(error, inference_request_identity.as_ref()),
                )
                .await);
            }
        }
    } else if needs_buffered_body || body_requires_bounded_collection {
        let Some(limit) = buffered_body_limit else {
            return Ok(finish_native_response(
                BufferedResponsePipeline::new(&pipeline, &req_parts.headers),
                &state,
                &route,
                request_start,
                access_log,
                inference_request_identity.as_ref(),
                error_bytes_response(500, "Internal server error"),
            )
            .await);
        };
        let collected = match collect_request_body_up_to(body, limit).await {
            Ok(body) => body,
            Err(RequestBodyCollectionError::TooLarge) => {
                return Ok(finish_native_response(
                    BufferedResponsePipeline::new(&pipeline, &req_parts.headers),
                    &state,
                    &route,
                    request_start,
                    access_log,
                    inference_request_identity.as_ref(),
                    error_bytes_response(413, "Request body too large"),
                )
                .await);
            }
            Err(RequestBodyCollectionError::ReadFailed) => {
                return Ok(finish_native_response(
                    BufferedResponsePipeline::new(&pipeline, &req_parts.headers),
                    &state,
                    &route,
                    request_start,
                    access_log,
                    inference_request_identity.as_ref(),
                    error_bytes_response(400, "Could not read request body"),
                )
                .await);
            }
        };
        (collected, None)
    } else {
        (Bytes::new(), Some(body))
    };

    // ── Backend selection ─────────────────────────────────────────────────────
    let (backend, service_timeouts, sticky_new_session, mut inference_attempt, distributed_attempt) =
        if let Some(prepared) = prepared_inference_attempt.take() {
            Arc::make_mut(&mut route).service_name = prepared.service_name;
            if state.metrics_enabled {
                service_request = state
                    .metrics
                    .track_service_request(&route.service_name, request_start);
            }
            (
                prepared.backend,
                prepared.timeouts,
                prepared.sticky_new_session,
                Some(prepared.identity),
                prepared.distributed,
            )
        } else if let Some((service_name, attempt)) = listing_forward.take() {
            Arc::make_mut(&mut route).service_name = service_name.clone();
            if state.metrics_enabled {
                service_request = state
                    .metrics
                    .track_service_request(&route.service_name, request_start);
            }
            let Some(load_balancer) = state.service_registry.get(&service_name) else {
                return Ok(finish_native_response(
                    BufferedResponsePipeline::new(&pipeline, &req_parts.headers),
                    &state,
                    &route,
                    request_start,
                    access_log,
                    inference_request_identity.as_ref(),
                    inference_access_error_response(
                        InferenceAccessError::Unavailable,
                        inference_request_identity.as_ref(),
                    ),
                )
                .await);
            };
            let backend = state
                .scaling
                .as_ref()
                .and_then(|scaling| scaling.revision_routers.get(&service_name))
                .and_then(|router| router.next_backend())
                .map(|(backend, _revision)| backend)
                .or_else(|| load_balancer.next_backend());
            let Some(backend) = backend else {
                return Ok(finish_native_response(
                    BufferedResponsePipeline::new(&pipeline, &req_parts.headers),
                    &state,
                    &route,
                    request_start,
                    access_log,
                    inference_request_identity.as_ref(),
                    inference_access_error_response(
                        InferenceAccessError::Unavailable,
                        inference_request_identity.as_ref(),
                    ),
                )
                .await);
            };
            (backend, load_balancer.timeouts(), None, Some(attempt), None)
        } else {
            let lb = route_plan
                .load_balancer
                .as_ref()
                .expect("proxy routes bind a load balancer")
                .as_ref();
            let service_timeouts = lb.timeouts();

            let scaling = state.scaling.as_ref();

            // Step 1: Sticky session — try to honour an existing affinity cookie.
            let mut sticky_new_session: Option<String> = None;
            let concurrency_limiter = scaling.and_then(|s| s.limiters.get(&route.service_name));
            let backend_from_sticky = state
                .sticky_managers
                .get(&route.service_name)
                .and_then(|mgr| {
                    let session_id = req_parts
                        .headers
                        .get("cookie")
                        .and_then(|v| v.to_str().ok())
                        .and_then(|cookie| mgr.extract_session_id(cookie))
                        .map(|s| s.to_string());
                    match mgr.select_backend(session_id.as_deref(), lb.backends().as_slice()) {
                        Some((backend, new_id)) => {
                            sticky_new_session = new_id;
                            Some(backend)
                        }
                        None => None,
                    }
                })
                .filter(|backend| {
                    let allowed = concurrency_limiter.is_none_or(|limiter| {
                        matches!(
                            limiter.check(backend),
                            crate::scaling::concurrency::ConcurrencyCheckResult::Allowed
                        )
                    });
                    if !allowed {
                        // The sticky manager may have generated a cookie
                        // before the capacity check. Do not attach that
                        // cookie if normal selection has to choose a
                        // different backend.
                        sticky_new_session = None;
                    }
                    allowed
                });

            // Step 2: Normal selection (revision router → concurrency limiter → standard LB).
            let backend = if let Some(b) = backend_from_sticky {
                Some(b)
            } else if let Some(rev_router) = state
                .scaling
                .as_ref()
                .and_then(|s| s.revision_routers.get(&route.service_name))
            {
                concurrency_limiter
                    .map(|limiter| rev_router.next_backend_with_capacity(limiter))
                    .unwrap_or_else(|| rev_router.next_backend())
                    .map(|(b, _rev_name)| b)
            } else if let Some(limiter) = concurrency_limiter {
                limiter.select_with_capacity(lb.backends().as_slice())
            } else {
                lb.next_backend()
            };

            let backend = match backend {
                Some(b) => b,
                None => {
                    // Step 3: Scale-from-zero buffer or failover.
                    if let Some(buffer) = scaling.and_then(|s| s.buffers.get(&route.service_name)) {
                        if buffer.needs_scale_up() {
                            tracing::info!(
                                service = %route.service_name,
                                "Scale-from-zero triggered, buffering request"
                            );
                        }

                        match buffer.wait_for_backend().await {
                            crate::scaling::buffer::BufferResult::Ready => {
                                let selected = if let Some(rev_router) = scaling
                                    .and_then(|s| s.revision_routers.get(&route.service_name))
                                {
                                    scaling
                                        .and_then(|s| s.limiters.get(&route.service_name))
                                        .map(|limiter| {
                                            rev_router.next_backend_with_capacity(limiter)
                                        })
                                        .unwrap_or_else(|| rev_router.next_backend())
                                        .map(|(backend, _revision)| backend)
                                } else if let Some(limiter) =
                                    scaling.and_then(|s| s.limiters.get(&route.service_name))
                                {
                                    limiter.select_with_capacity(lb.backends().as_slice())
                                } else {
                                    lb.next_backend()
                                };
                                match selected {
                                    Some(b) => b,
                                    None => {
                                        return Ok(finish_native_response(
                                            BufferedResponsePipeline::new(
                                                &pipeline,
                                                &req_parts.headers,
                                            ),
                                            &state,
                                            &route,
                                            request_start,
                                            access_log,
                                            inference_request_identity.as_ref(),
                                            error_bytes_response(
                                                503,
                                                "No healthy backends after scale-up",
                                            ),
                                        )
                                        .await);
                                    }
                                }
                            }
                            crate::scaling::buffer::BufferResult::Timeout => {
                                return Ok(finish_native_response(
                                    BufferedResponsePipeline::new(&pipeline, &req_parts.headers),
                                    &state,
                                    &route,
                                    request_start,
                                    access_log,
                                    inference_request_identity.as_ref(),
                                    error_bytes_response(504, "Backend scale-up timed out"),
                                )
                                .await);
                            }
                            crate::scaling::buffer::BufferResult::Overflow => {
                                return Ok(finish_native_response(
                                    BufferedResponsePipeline::new(&pipeline, &req_parts.headers),
                                    &state,
                                    &route,
                                    request_start,
                                    access_log,
                                    inference_request_identity.as_ref(),
                                    error_bytes_response(503, "Request buffer full"),
                                )
                                .await);
                            }
                            crate::scaling::buffer::BufferResult::Shutdown => {
                                return Ok(finish_native_response(
                                    BufferedResponsePipeline::new(&pipeline, &req_parts.headers),
                                    &state,
                                    &route,
                                    request_start,
                                    access_log,
                                    inference_request_identity.as_ref(),
                                    error_bytes_response(503, "Gateway shutting down"),
                                )
                                .await);
                            }
                        }
                    } else if let Some(failover) = state.failovers.get(&route.service_name) {
                        match failover.next_backend() {
                            Some((b, _is_failover)) => b,
                            None => {
                                return Ok(finish_native_response(
                                    BufferedResponsePipeline::new(&pipeline, &req_parts.headers),
                                    &state,
                                    &route,
                                    request_start,
                                    access_log,
                                    inference_request_identity.as_ref(),
                                    error_bytes_response(
                                        503,
                                        "No healthy backends (primary + failover)",
                                    ),
                                )
                                .await);
                            }
                        }
                    } else {
                        return Ok(finish_native_response(
                            BufferedResponsePipeline::new(&pipeline, &req_parts.headers),
                            &state,
                            &route,
                            request_start,
                            access_log,
                            inference_request_identity.as_ref(),
                            error_bytes_response(503, "No healthy backends"),
                        )
                        .await);
                    }
                }
            };
            (backend, service_timeouts, sticky_new_session, None, None)
        };
    if let Some(identity) = inference_attempt.as_ref() {
        identity.prepare_upstream_headers(&mut req_parts.headers);
        if let Some(access_log) = access_log.as_mut() {
            access_log.set_inference_attempt(identity);
        }
    }
    if let Some(access_log) = access_log.as_mut() {
        access_log.set_backend(backend.url.clone());
    }

    // Record per-backend request.
    if state.metrics_enabled && inference_dispatch.is_none() {
        state.metrics.record_backend_request_id(backend.metric_id());
    }

    if let Some(distributed) = distributed_attempt {
        let ctx = ProtocolContext {
            route,
            backend,
            req_parts,
            body_bytes,
            streaming_body: None,
            pipeline,
            state,
            forwarded,
            prepared_forwarded: None,
            timeouts: service_timeouts,
            access_log,
            sticky_new_session,
            request_start,
            inference_admission,
            inference_attempt,
            usage_lifecycle,
            inference_dispatch,
            service_request,
        };
        return Ok(protocol::handle_distributed_dispatch(ctx, distributed).await);
    }

    // Already-buffered traffic is sampled against the final resolved service.
    if buffers_without_mirror {
        selected_mirror = state
            .mirrors
            .get(&route.service_name)
            .filter(|mirror| mirror.should_mirror())
            .cloned();
    }

    // Mirror traffic if selected (fire-and-forget, before primary forward).
    if let Some(mirror) = selected_mirror {
        mirror.mirror_selected_request(
            req_parts.method.clone(),
            req_parts.uri.clone(),
            req_parts.headers.clone(),
            body_bytes.clone(),
        );
    }

    // Inject outbound trace context (W3C traceparent).
    if let Some(trace_ctx) = trace_ctx.as_ref().filter(|_| state.tracing_enabled) {
        let traceparent = trace_ctx.to_traceparent();
        if let Ok(hval) = hyper::header::HeaderValue::from_str(&traceparent) {
            req_parts
                .headers
                .insert(hyper::header::HeaderName::from_static("traceparent"), hval);
        }
    }

    // ── gRPC dispatch ─────────────────────────────────────────────────────────
    if is_grpc {
        let grpc_proxy = state
            .grpc_proxy_for_backend(&route.service_name, &backend)
            .clone();
        let ctx = ProtocolContext {
            route,
            backend,
            req_parts,
            body_bytes,
            streaming_body,
            pipeline,
            state: state.clone(),
            forwarded,
            prepared_forwarded: None,
            timeouts: service_timeouts,
            access_log,
            sticky_new_session,
            request_start,
            inference_admission: inference_admission.take(),
            inference_attempt: inference_attempt.take(),
            usage_lifecycle: usage_lifecycle.take(),
            inference_dispatch: inference_dispatch.take(),
            service_request,
        };
        return Ok(protocol::handle_grpc_dispatch(ctx, grpc_proxy).await);
    }

    // ── SSE / streaming dispatch ──────────────────────────────────────────────
    if is_sse {
        let ctx = ProtocolContext {
            route,
            backend,
            req_parts,
            body_bytes,
            streaming_body: None,
            pipeline,
            state: state.clone(),
            forwarded,
            prepared_forwarded: None,
            timeouts: service_timeouts,
            access_log,
            sticky_new_session,
            request_start,
            inference_admission: inference_admission.take(),
            inference_attempt: inference_attempt.take(),
            usage_lifecycle: usage_lifecycle.take(),
            inference_dispatch: inference_dispatch.take(),
            service_request,
        };
        return Ok(protocol::handle_sse_dispatch(ctx).await);
    }

    // ── Plain HTTP dispatch ───────────────────────────────────────────────────
    {
        let prepared_forwarded =
            if streaming_body.is_some() && pipeline.is_empty() && inference_dispatch.is_none() {
                connection.prepared_forwarded.clone()
            } else {
                None
            };
        let ctx = ProtocolContext {
            route,
            backend,
            req_parts,
            body_bytes,
            streaming_body,
            pipeline,
            state,
            forwarded,
            prepared_forwarded,
            timeouts: service_timeouts,
            access_log,
            sticky_new_session,
            request_start,
            inference_admission,
            inference_attempt,
            usage_lifecycle,
            inference_dispatch,
            service_request,
        };
        Ok(protocol::handle_http_dispatch(ctx).await)
    }
}
