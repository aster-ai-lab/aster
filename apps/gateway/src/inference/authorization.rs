//! Snapshot-backed inference-key authentication and grant authorization.

use super::access_error::InferenceAccessError;
use super::limits::{
    InferenceGrantAdmissionGuard, InferenceGrantIdentity, InferenceLimitStore, TokenReservation,
};
use super::principal::{PrincipalSubject, PrincipalVerifier};
use super::scheduling::{
    has_eligible_worker, select_worker, select_worker_pair, InferencePoolAdmissionGuard,
    InferencePoolIdentity, InferenceScheduler, InferenceWorkerPairSelection,
    InferenceWorkerPairSelectionRequest, InferenceWorkerSelection, InferenceWorkerSelectionRequest,
};
use super::{InferenceRequestIdentity, InferenceWorkerCandidate, OpenAiRequestProfile};
use crate::config::{
    InferenceConfig, InferenceCredentialConfig, InferenceDistributedServingConfig,
    InferenceEndpoint, InferenceGrantConfig, InferenceModelConfig, InferenceModelPricing,
    InferencePrincipalConfig, InferenceProtocol, InferenceRouteConfig,
};
use argon2::{Argon2, PasswordHash, PasswordVerifier};
use chrono::{DateTime, Utc};
use http::header::AUTHORIZATION;
use http::{HeaderMap, HeaderValue};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};
use tokio::sync::Semaphore;
use uuid::Uuid;

const MAX_INFERENCE_KEY_BYTES: usize = 512;
const MAX_PARALLEL_ARGON2_VERIFICATIONS: usize = 2;
const INFERENCE_KEY_PREFIX: &str = "a3s_inf_";

#[cfg(test)]
#[derive(Clone)]
struct VerificationCompletionGate {
    completed: std::sync::mpsc::Sender<()>,
    release: Arc<std::sync::Barrier>,
}

/// Runtime view of one complete inference authorization snapshot.
///
/// The authorizer owns no plaintext credentials. Successful verification is
/// cached by a SHA-256 digest for the lifetime of this exact runtime snapshot;
/// reload replaces the authorizer and therefore invalidates the cache.
/// Integer EWMA of one target's first-response latency, in milliseconds.
/// Rebuilt with every snapshot (the authorizer owns it), so the map is
/// bounded by the active snapshot's target set — a bounded, per-snapshot
/// scheduling observation in the sense of invariant 11.
#[derive(Debug, Default, Clone, Copy)]
struct TargetLatencyEwma {
    /// Smoothed latency in milliseconds; `None` until warm.
    ewma_ms: Option<u64>,
    samples: u32,
}

impl TargetLatencyEwma {
    /// Smooth with an integer alpha of 1/8: the EWMA converges after a
    /// handful of samples without floating point.
    fn observe(&mut self, latency_ms: u64) {
        self.ewma_ms = Some(match self.ewma_ms {
            Some(previous) => (previous * 7 + latency_ms) / 8,
            None => latency_ms,
        });
        self.samples = self.samples.saturating_add(1);
    }
}

/// Per-mille weight multiplier a latency-biased selection applies:
/// 1000 is neutral; the factor is clamped to 500..=2000 so bias adjusts
/// — never replaces — the configured weights. Targets need at least two
/// samples before their latency counts.
const LATENCY_BIAS_CLAMP_PERMILLE: (u64, u64) = (500, 2_000);
const LATENCY_BIAS_MIN_SAMPLES: u32 = 2;

pub(crate) struct InferenceAuthorizer {
    policy: InferenceConfig,
    routes_by_router: HashMap<String, Uuid>,
    /// HR3b: model_id → route_id (routing windows re-level priorities,
    /// so target counters key by the model's owning route).
    route_by_model: HashMap<Uuid, Uuid>,
    credentials_by_prefix: HashMap<String, Uuid>,
    prefix_lengths: Vec<usize>,
    verified: Mutex<HashMap<[u8; 32], CachedCredential>>,
    verification_permits: Arc<Semaphore>,
    #[cfg(test)]
    verification_completion_gate: Option<VerificationCompletionGate>,
    target_counters: Mutex<HashMap<TargetCounterKey, u64>>,
    latency_observations: Mutex<HashMap<Uuid, TargetLatencyEwma>>,
    pub(crate) prefix_affinity: super::prefix_affinity::PrefixAffinity,
    /// Guardrails gates compiled from the policy's named sentry ACLs at
    /// authorizer construction (wire feature only; the config parser
    /// rejects guardrails blocks without the feature).
    #[cfg(feature = "wire")]
    wire_gates: HashMap<String, std::sync::Arc<crate::wire::WireGate>>,
    limits: InferenceLimitStore,
    scheduler: InferenceScheduler,
    principals: PrincipalVerifier,
    upstream_keys: HashMap<Uuid, HeaderValue>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct CachedCredential {
    credential_id: Uuid,
    generation: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct TargetCounterKey {
    route_id: Uuid,
    model_id: Uuid,
    priority: u32,
}

/// Authenticated route and credential identity retained for grant checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AuthenticatedInference {
    route_id: Uuid,
    environment_id: Uuid,
    credential_id: Uuid,
    credential_generation: u64,
    principal: bool,
}

/// The grant that applies to one authenticated caller.
#[derive(Clone, Copy)]
enum GrantView<'a> {
    Credential(&'a InferenceGrantConfig),
    Principal(&'a InferencePrincipalConfig),
}

impl GrantView<'_> {
    fn allows_endpoint(self, endpoint: InferenceEndpoint) -> bool {
        match self {
            Self::Credential(grant) => grant.endpoints.contains(&endpoint),
            Self::Principal(principal) => principal.endpoints.contains(&endpoint),
        }
    }

    fn allows_model(self, alias: &str) -> bool {
        match self {
            Self::Credential(grant) => grant.models.iter().any(|model| model == alias),
            Self::Principal(principal) => principal.allows_model(alias),
        }
    }

    fn credential_generation(self) -> u64 {
        match self {
            Self::Credential(grant) => grant.credential_generation,
            Self::Principal(_) => 0,
        }
    }
}

/// One granted tool listing and the identity used to meter it.
pub(crate) struct AuthorizedListing {
    pub(crate) service: String,
    pub(crate) sku: String,
    pub(crate) listing_id: Uuid,
    pub(crate) authenticated: AuthenticatedInference,
    pub(crate) identity: InferenceRequestIdentity,
}

/// Drop guard retaining both credential-grant and model-pool admission.
pub(crate) struct InferenceAdmissionGuard {
    grant: InferenceGrantAdmissionGuard,
    _pool: Option<InferencePoolAdmissionGuard>,
}

impl InferenceAdmissionGuard {
    /// Adjust the grant token and spend budgets once trusted usage is known.
    pub(crate) fn reconcile_usage(&self, observed: &super::observed_tokens::ObservedTokens) {
        self.grant.reconcile_usage(observed);
    }
}

impl AuthenticatedInference {
    pub(crate) fn environment_id(self) -> Uuid {
        self.environment_id
    }

    pub(crate) fn credential_id(self) -> Uuid {
        self.credential_id
    }

    pub(crate) fn credential_generation(self) -> u64 {
        self.credential_generation
    }

    /// Whether the caller is a signed-in OS user rather than a Cloud key.
    pub(crate) fn is_principal(self) -> bool {
        self.principal
    }
}

/// One authorized model target selected from the active snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InferenceDispatchTarget {
    pub(crate) model_id: Uuid,
    pub(crate) target_id: Uuid,
    pub(crate) priority: u32,
    pub(crate) service: String,
    pub(crate) upstream_model: String,
    pub(crate) protocol: InferenceProtocol,
    /// True when this target speaks a different dialect than the endpoint
    /// and opted into translation; dispatch translates per attempt.
    pub(crate) translates: bool,
    /// Sensitive provider credential resolved from `api_key_env`.
    pub(crate) upstream_key: Option<HeaderValue>,
}

impl InferenceAuthorizer {
    /// The grant's guardrails policy name, if any. Principal grants carry
    /// no guardrails (policies bind credentials).
    pub(crate) fn grant_guardrails(
        &self,
        authenticated: AuthenticatedInference,
        now: DateTime<Utc>,
    ) -> Result<Option<String>, InferenceAccessError> {
        let (_, grant) = self.grant(authenticated, now)?;
        Ok(match grant {
            GrantView::Credential(grant) => grant.guardrails.clone(),
            GrantView::Principal(_) => None,
        })
    }

    /// The compiled wire gate for a named guardrails policy (wire feature).
    #[cfg(feature = "wire")]
    pub(crate) fn wire_gate(&self, name: &str) -> Option<std::sync::Arc<crate::wire::WireGate>> {
        self.wire_gates.get(name).cloned()
    }

    /// Record one first-response latency observation for a target. The
    /// observation map dies with this authorizer (per-snapshot), keeping
    /// it bounded by the active target set.
    pub(crate) fn observe_target_latency(&self, target_id: Uuid, latency_ms: u64) {
        let mut observations = self
            .latency_observations
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        observations
            .entry(target_id)
            .or_default()
            .observe(latency_ms);
    }

    /// Per-mille weight multiplier for latency-biased selection. The
    /// reference is the fastest warm target among `candidates`; targets
    /// without warm observations stay neutral.
    pub(crate) fn latency_bias_permille(&self, candidates: &[Uuid]) -> HashMap<Uuid, u64> {
        let observations = self
            .latency_observations
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let reference = candidates
            .iter()
            .filter_map(|target| observations.get(target))
            .filter(|ewma| ewma.samples >= LATENCY_BIAS_MIN_SAMPLES)
            .filter_map(|ewma| ewma.ewma_ms)
            .min()
            .unwrap_or(0);
        if reference == 0 {
            return HashMap::new();
        }
        candidates
            .iter()
            .map(|target| {
                let factor = observations
                    .get(target)
                    .filter(|ewma| ewma.samples >= LATENCY_BIAS_MIN_SAMPLES)
                    .and_then(|ewma| ewma.ewma_ms)
                    .map(|ewma_ms| reference.saturating_mul(1_000) / ewma_ms.max(1))
                    .unwrap_or(1_000)
                    .clamp(LATENCY_BIAS_CLAMP_PERMILLE.0, LATENCY_BIAS_CLAMP_PERMILLE.1);
                (*target, factor)
            })
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn new(policy: &InferenceConfig) -> Self {
        Self::with_previous(policy, None)
    }

    /// Build a new exact-snapshot authorizer while retaining counters for
    /// unchanged immutable grant identities.
    pub(crate) fn with_previous(policy: &InferenceConfig, previous: Option<&Self>) -> Self {
        Self::with_upstream_keys(policy, previous, resolve_upstream_keys(policy))
    }

    /// Production snapshot. Keyed targets use console supplier secrets matched
    /// by service name. The `api_key_env` name is only the marker that a key
    /// is required.
    pub(crate) fn with_supplier_keys(
        policy: &InferenceConfig,
        previous: Option<&Self>,
        suppliers: &[super::supplier_keys::SupplierCredential],
    ) -> Self {
        Self::with_upstream_keys(
            policy,
            previous,
            super::supplier_keys::resolve_supplier_keys(policy, suppliers),
        )
    }

    fn with_upstream_keys(
        policy: &InferenceConfig,
        previous: Option<&Self>,
        upstream_keys: HashMap<Uuid, HeaderValue>,
    ) -> Self {
        let route_by_model = policy
            .routes
            .values()
            .flat_map(|route| {
                route
                    .models
                    .values()
                    .map(move |model| (model.model_id, route.route_id))
            })
            .collect::<HashMap<Uuid, Uuid>>();
        let routes_by_router = policy
            .routes
            .values()
            .map(|route| (route.router.clone(), route.route_id))
            .collect();
        let credentials_by_prefix = policy
            .credentials
            .values()
            .map(|credential| (credential.prefix.clone(), credential.credential_id))
            .collect::<HashMap<_, _>>();
        let mut prefix_lengths = credentials_by_prefix
            .keys()
            .map(String::len)
            .collect::<Vec<_>>();
        prefix_lengths.sort_unstable();
        prefix_lengths.dedup();
        prefix_lengths.reverse();

        Self {
            policy: policy.clone(),
            routes_by_router,
            route_by_model,
            credentials_by_prefix,
            prefix_lengths,
            verified: Mutex::new(HashMap::new()),
            verification_permits: Arc::new(Semaphore::new(MAX_PARALLEL_ARGON2_VERIFICATIONS)),
            #[cfg(test)]
            verification_completion_gate: None,
            target_counters: Mutex::new(HashMap::new()),
            latency_observations: Mutex::new(HashMap::new()),
            // Hints survive a policy refresh, but lookup still requires fresh
            // authorization and exact credential/model/target identities.
            prefix_affinity: previous
                .map(|previous| previous.prefix_affinity.clone())
                .unwrap_or_default(),
            #[cfg(feature = "wire")]
            wire_gates: policy
                .guardrails
                .iter()
                .map(|(name, config)| {
                    // Infallible by construction: the ACL parser already
                    // compiled this sentry source once (fail-closed at
                    // parse), so the second compile cannot fail.
                    let gate = crate::wire::WireGate::from_acl(&config.sentry_acl)
                        .expect("sentry ACL validated at parse");
                    (name.clone(), std::sync::Arc::new(gate))
                })
                .collect(),
            limits: InferenceLimitStore::new(policy, previous.map(|previous| &previous.limits)),
            scheduler: InferenceScheduler::new(
                policy,
                previous.map(|previous| &previous.scheduler),
            ),
            principals: PrincipalVerifier::new(previous.map(|previous| &previous.principals)),
            upstream_keys,
        }
    }

    /// Cached identity of one principal caller, for usage attribution.
    pub(crate) fn principal_subject(
        &self,
        authenticated: AuthenticatedInference,
    ) -> Option<PrincipalSubject> {
        authenticated
            .principal
            .then(|| self.principals.subject(authenticated.credential_id))
            .flatten()
    }

    /// Client wire shape of the route that authenticated this caller.
    pub(crate) fn route_wire(&self, router: &str) -> crate::config::InferenceWire {
        self.route(router)
            .map(|route| route.wire)
            .unwrap_or_default()
    }

    /// Whether this exact router is owned by the native inference policy.
    pub(crate) fn owns_router(&self, router: &str) -> bool {
        self.routes_by_router.contains_key(router)
    }

    /// Create one Gateway-owned identity after endpoint authorization.
    pub(crate) fn request_identity(
        &self,
        authenticated: AuthenticatedInference,
        profile: OpenAiRequestProfile,
        correlation_id: String,
        now: DateTime<Utc>,
    ) -> Result<InferenceRequestIdentity, InferenceAccessError> {
        let (route, grant) = self.grant(authenticated, now)?;
        let endpoint = endpoint(profile);
        if !grant.allows_endpoint(endpoint) {
            return Err(InferenceAccessError::Denied);
        }
        Ok(InferenceRequestIdentity::new(
            correlation_id,
            route.route_id,
            route.policy_revision,
            endpoint,
        ))
    }

    /// Authenticate an inference key and enforce its route and endpoint grant.
    pub(crate) async fn authenticate(
        &self,
        router: &str,
        profile: OpenAiRequestProfile,
        headers: &HeaderMap,
        now: DateTime<Utc>,
    ) -> Result<AuthenticatedInference, InferenceAccessError> {
        if self.policy.expires_at <= now {
            return Err(InferenceAccessError::Unavailable);
        }
        let route = self.route(router)?;
        if let Some(principal) = &route.principal {
            let inference_key = bearer_token(headers)
                .ok()
                .filter(|token| token.starts_with(INFERENCE_KEY_PREFIX));
            if inference_key.is_none() {
                return self
                    .authenticate_principal(route, principal, profile, headers)
                    .await;
            }
        }
        let token = bearer_token(headers)?;
        let credential = self.credential_for_token(token)?;
        if credential.revoked {
            return Err(InferenceAccessError::Revoked);
        }
        if credential.expires_at <= now {
            return Err(InferenceAccessError::Unauthorized);
        }

        self.verify_token(token, credential).await?;
        let verified_at = Utc::now();
        if self.policy.expires_at <= verified_at {
            return Err(InferenceAccessError::Unavailable);
        }
        if credential.revoked {
            return Err(InferenceAccessError::Revoked);
        }
        if credential.expires_at <= verified_at {
            return Err(InferenceAccessError::Unauthorized);
        }
        let grant = route
            .grants
            .get(&credential.credential_id)
            .ok_or(InferenceAccessError::Denied)?;
        if credential.environment_id != route.environment_id
            || grant.credential_generation != credential.generation
            || !grant.endpoints.contains(&endpoint(profile))
        {
            return Err(InferenceAccessError::Denied);
        }

        Ok(AuthenticatedInference {
            route_id: route.route_id,
            environment_id: route.environment_id,
            credential_id: credential.credential_id,
            credential_generation: credential.generation,
            principal: false,
        })
    }

    async fn authenticate_principal(
        &self,
        route: &InferenceRouteConfig,
        principal: &InferencePrincipalConfig,
        profile: OpenAiRequestProfile,
        headers: &HeaderMap,
    ) -> Result<AuthenticatedInference, InferenceAccessError> {
        let subject = self
            .principals
            .verify(route.route_id, principal, headers)
            .await?;
        if !principal.endpoints.contains(&endpoint(profile)) {
            return Err(InferenceAccessError::Denied);
        }
        Ok(AuthenticatedInference {
            route_id: route.route_id,
            environment_id: route.environment_id,
            credential_id: subject.credential_id,
            credential_generation: 0,
            principal: true,
        })
    }

    /// Authenticate a Capability Key and enforce a snapshot-published tool SKU.
    pub(crate) async fn authenticate_listing(
        &self,
        router: &str,
        sku: &str,
        headers: &HeaderMap,
        now: DateTime<Utc>,
        correlation_id: String,
    ) -> Result<AuthorizedListing, InferenceAccessError> {
        if self.policy.expires_at <= now {
            return Err(InferenceAccessError::Unavailable);
        }
        let route = self.route(router)?;
        let token = bearer_token(headers)?;
        let credential = self.credential_for_token(token)?;
        if credential.revoked {
            return Err(InferenceAccessError::Revoked);
        }
        if credential.expires_at <= now {
            return Err(InferenceAccessError::Unauthorized);
        }

        self.verify_token(token, credential).await?;
        let verified_at = Utc::now();
        if self.policy.expires_at <= verified_at {
            return Err(InferenceAccessError::Unavailable);
        }
        if credential.revoked {
            return Err(InferenceAccessError::Revoked);
        }
        if credential.expires_at <= verified_at {
            return Err(InferenceAccessError::Unauthorized);
        }
        let grant = route
            .grants
            .get(&credential.credential_id)
            .ok_or(InferenceAccessError::Denied)?;
        let listing = route
            .listings
            .get(sku)
            .ok_or(InferenceAccessError::Denied)?;
        if credential.environment_id != route.environment_id
            || grant.credential_generation != credential.generation
            || !grant.endpoints.contains(&InferenceEndpoint::Tools)
            || !grant.listings.iter().any(|granted| granted == sku)
        {
            return Err(InferenceAccessError::Denied);
        }

        let authenticated = AuthenticatedInference {
            route_id: route.route_id,
            environment_id: route.environment_id,
            credential_id: credential.credential_id,
            credential_generation: credential.generation,
            principal: false,
        };
        let mut identity = InferenceRequestIdentity::new(
            correlation_id,
            route.route_id,
            route.policy_revision,
            InferenceEndpoint::Tools,
        );
        identity.set_model_id(listing.listing_id);
        Ok(AuthorizedListing {
            service: listing.service.clone(),
            sku: sku.to_string(),
            listing_id: listing.listing_id,
            authenticated,
            identity,
        })
    }

    /// Return the deterministic model aliases visible to one credential.
    pub(crate) fn allowed_models(
        &self,
        authenticated: AuthenticatedInference,
        now: DateTime<Utc>,
    ) -> Result<Vec<String>, InferenceAccessError> {
        let (route, grant) = self.grant(authenticated, now)?;
        let mut models = match grant {
            GrantView::Credential(grant) => grant
                .models
                .iter()
                .filter(|alias| route.models.contains_key(alias.as_str()))
                .cloned()
                .collect::<Vec<_>>(),
            GrantView::Principal(principal) => route
                .models
                .keys()
                .filter(|alias| principal.allows_model(alias))
                .cloned()
                .collect(),
        };
        // Platform models are visible exactly when the grant allows their
        // endpoint; they carry no route-model entry of their own.
        for (endpoint, alias) in [
            (InferenceEndpoint::Ocr, crate::inference::PLATFORM_OCR_ALIAS),
            (
                InferenceEndpoint::SystemOne,
                crate::inference::PLATFORM_APOFASI_ALIAS,
            ),
            (
                InferenceEndpoint::Embeddings,
                crate::inference::PLATFORM_EMBEDDING_ALIAS,
            ),
            (
                InferenceEndpoint::Rerank,
                crate::inference::PLATFORM_RERANK_ALIAS,
            ),
        ] {
            if grant.allows_endpoint(endpoint) && !models.iter().any(|existing| existing == alias) {
                models.push(alias.to_string());
            }
        }
        models.sort_unstable();
        models.dedup();
        Ok(models)
    }

    /// Admit one granted endpoint request against its local RPM and
    /// concurrency limits. Token budget is not charged for zero-body endpoints
    /// such as `/v1/models`.
    pub(crate) fn admit_request(
        &self,
        authenticated: AuthenticatedInference,
        now: DateTime<Utc>,
    ) -> Result<InferenceAdmissionGuard, InferenceAccessError> {
        let grant =
            self.admit_grant_request(authenticated, now, TokenReservation::default(), None)?;
        Ok(InferenceAdmissionGuard { grant, _pool: None })
    }

    fn admit_grant_request(
        &self,
        authenticated: AuthenticatedInference,
        now: DateTime<Utc>,
        reservation: TokenReservation,
        pricing: Option<InferenceModelPricing>,
    ) -> Result<InferenceGrantAdmissionGuard, InferenceAccessError> {
        let (route, grant) = self.grant(authenticated, now)?;
        self.limits.try_admit(
            InferenceGrantIdentity {
                route_id: route.route_id,
                policy_revision: route.policy_revision,
                credential_id: authenticated.credential_id,
                credential_generation: grant.credential_generation(),
            },
            reservation,
            pricing,
        )
    }

    /// Enforce a model grant before charging and admitting an invocation.
    ///
    /// `reservation` is the fail-closed estimate charged against
    /// `tokens_per_minute` (and the spend budget at the model's snapshot
    /// pricing) until [`InferenceAdmissionGuard::reconcile_usage`] observes
    /// trusted usage.
    pub(crate) async fn admit_model(
        &self,
        authenticated: AuthenticatedInference,
        alias: &str,
        reservation: TokenReservation,
        now: DateTime<Utc>,
    ) -> Result<InferenceAdmissionGuard, InferenceAccessError> {
        let (route, model) = self.granted_model(authenticated, alias, now)?;
        let pool = self
            .scheduler
            .admit(InferencePoolIdentity {
                route_id: route.route_id,
                model_id: model.model_id,
            })
            .await?;
        let grant =
            self.admit_grant_request(authenticated, Utc::now(), reservation, model.pricing)?;
        Ok(InferenceAdmissionGuard { grant, _pool: pool })
    }

    pub(crate) fn model_uses_worker_scheduling(
        &self,
        authenticated: AuthenticatedInference,
        alias: &str,
        now: DateTime<Utc>,
    ) -> Result<bool, InferenceAccessError> {
        let (_, model) = self.granted_model(authenticated, alias, now)?;
        Ok(model.scheduling.is_some())
    }

    pub(crate) fn distributed_serving_config(
        &self,
        authenticated: AuthenticatedInference,
        alias: &str,
        now: DateTime<Utc>,
    ) -> Result<Option<InferenceDistributedServingConfig>, InferenceAccessError> {
        let (_, model) = self.granted_model(authenticated, alias, now)?;
        Ok(model
            .scheduling
            .as_ref()
            .and_then(|scheduling| scheduling.distributed_serving.clone()))
    }

    pub(crate) fn has_eligible_worker(
        &self,
        authenticated: AuthenticatedInference,
        alias: &str,
        target_id: Uuid,
        candidates: &[InferenceWorkerCandidate<'_>],
        now: DateTime<Utc>,
    ) -> Result<bool, InferenceAccessError> {
        let (_, model) = self.granted_model(authenticated, alias, now)?;
        let Some(scheduling) = &model.scheduling else {
            return Ok(false);
        };
        Ok(has_eligible_worker(
            &self.policy.workers,
            scheduling,
            target_id,
            candidates,
            now,
        ))
    }

    pub(crate) fn has_eligible_worker_pair(
        &self,
        authenticated: AuthenticatedInference,
        alias: &str,
        target_id: Uuid,
        candidates: &[InferenceWorkerCandidate<'_>],
        now: DateTime<Utc>,
        routing_key: &[u8],
    ) -> Result<bool, InferenceAccessError> {
        let (_, model) = self.granted_model(authenticated, alias, now)?;
        let Some(scheduling) = &model.scheduling else {
            return Ok(false);
        };
        Ok(select_worker_pair(
            &self.policy.workers,
            scheduling,
            target_id,
            candidates,
            now,
            routing_key,
            None,
        )
        .is_some())
    }

    pub(crate) fn select_worker(
        &self,
        authenticated: AuthenticatedInference,
        alias: &str,
        request: InferenceWorkerSelectionRequest<'_, '_>,
    ) -> Result<InferenceWorkerSelection, InferenceAccessError> {
        let (_, model) = self.granted_model(authenticated, alias, request.now)?;
        let scheduling = model
            .scheduling
            .as_ref()
            .ok_or(InferenceAccessError::Unavailable)?;
        select_worker(
            &self.policy.workers,
            scheduling,
            request.target_id,
            request.candidates,
            request.now,
            request.routing_key,
            request.cache_affinity_key,
        )
        .ok_or(InferenceAccessError::Unavailable)
    }

    pub(crate) fn select_worker_pair(
        &self,
        authenticated: AuthenticatedInference,
        alias: &str,
        request: InferenceWorkerPairSelectionRequest<'_, '_>,
    ) -> Result<InferenceWorkerPairSelection, InferenceAccessError> {
        let (_, model) = self.granted_model(authenticated, alias, request.now)?;
        let scheduling = model
            .scheduling
            .as_ref()
            .ok_or(InferenceAccessError::Unavailable)?;
        select_worker_pair(
            &self.policy.workers,
            scheduling,
            request.target_id,
            request.candidates,
            request.now,
            request.routing_key,
            request.cache_affinity_key,
        )
        .ok_or(InferenceAccessError::Unavailable)
    }

    /// Select one available target at or after the requested priority.
    ///
    /// A caller advances `minimum_priority` only after a concrete upstream
    /// attempt fails before any client response. Targets in the failed
    /// priority group are therefore never retried implicitly.
    #[cfg(test)]
    pub(crate) fn select_target_from_priority<F>(
        &self,
        authenticated: AuthenticatedInference,
        alias: &str,
        minimum_priority: u32,
        now: DateTime<Utc>,
        service_is_available: F,
    ) -> Result<InferenceDispatchTarget, InferenceAccessError>
    where
        F: FnMut(&str, Uuid) -> bool,
    {
        self.select_protocol_target_from_priority(
            authenticated,
            alias,
            crate::config::InferenceEndpoint::ChatCompletions,
            minimum_priority,
            now,
            service_is_available,
        )
    }

    /// [`Self::select_target_from_priority`] restricted to targets speaking
    /// `protocol`.
    pub(crate) fn select_protocol_target_from_priority<F>(
        &self,
        authenticated: AuthenticatedInference,
        alias: &str,
        endpoint: crate::config::InferenceEndpoint,
        minimum_priority: u32,
        now: DateTime<Utc>,
        service_is_available: F,
    ) -> Result<InferenceDispatchTarget, InferenceAccessError>
    where
        F: FnMut(&str, Uuid) -> bool,
    {
        let (_, model) = self.granted_model(authenticated, alias, now)?;
        self.walk_targets(
            model,
            endpoint,
            minimum_priority,
            service_is_available,
            |_| {},
        )
    }

    /// HR3b: selection over an EFFECTIVE target list (the routing
    /// window's re-leveled candidates) — the same walk, the same
    /// grant, a different list.
    pub(crate) fn select_from_effective_targets<F>(
        &self,
        authenticated: AuthenticatedInference,
        alias: &str,
        endpoint: crate::config::InferenceEndpoint,
        effective: &[crate::config::InferenceTargetConfig],
        minimum_priority: u32,
        now: DateTime<Utc>,
        mut service_is_available: F,
    ) -> Result<InferenceDispatchTarget, InferenceAccessError>
    where
        F: FnMut(&str, Uuid) -> bool,
    {
        let (_, model) = self.granted_model(authenticated, alias, now)?;
        // The effective list stands in for the model's; counters and
        // bias stay keyed by target id, so cross-snapshot behavior is
        // unchanged.
        let shim = InferenceModelConfig {
            targets: effective.to_vec(),
            ..model.clone()
        };
        self.walk_targets(
            &shim,
            endpoint,
            minimum_priority,
            service_is_available,
            |_| {},
        )
    }

    /// Locality may narrow only the already granted, available protocol and
    /// priority group. Effective task-routing candidates retain their authority.
    pub(crate) fn select_target_with_preference<F, P>(
        &self,
        authenticated: AuthenticatedInference,
        alias: &str,
        endpoint: InferenceEndpoint,
        effective: Option<&[crate::config::InferenceTargetConfig]>,
        minimum_priority: u32,
        now: DateTime<Utc>,
        service_is_available: F,
        prefer: P,
    ) -> Result<InferenceDispatchTarget, InferenceAccessError>
    where
        F: FnMut(&str, Uuid) -> bool,
        P: FnMut(&mut Vec<&crate::config::InferenceTargetConfig>),
    {
        let (_, model) = self.granted_model(authenticated, alias, now)?;
        if let Some(effective) = effective {
            let shim = InferenceModelConfig {
                targets: effective.to_vec(),
                ..model.clone()
            };
            self.walk_targets(
                &shim,
                endpoint,
                minimum_priority,
                service_is_available,
                prefer,
            )
        } else {
            self.walk_targets(
                model,
                endpoint,
                minimum_priority,
                service_is_available,
                prefer,
            )
        }
    }

    pub(crate) fn prefix_hint(
        &self,
        authenticated: AuthenticatedInference,
        model_id: Uuid,
        endpoint: InferenceEndpoint,
        request: &super::OpenAiJsonRequest,
    ) -> Option<super::prefix_affinity::PrefixHint> {
        self.prefix_affinity.hint(
            (
                authenticated.route_id,
                authenticated.environment_id,
                authenticated.credential_id,
                authenticated.credential_generation,
                model_id,
                endpoint.as_str(),
            ),
            &request.document,
        )
    }

    fn walk_targets<F, P>(
        &self,
        model: &InferenceModelConfig,
        endpoint: crate::config::InferenceEndpoint,
        minimum_priority: u32,
        mut service_is_available: F,
        mut prefer: P,
    ) -> Result<InferenceDispatchTarget, InferenceAccessError>
    where
        F: FnMut(&str, Uuid) -> bool,
        P: FnMut(&mut Vec<&crate::config::InferenceTargetConfig>),
    {
        let protocol = endpoint.protocol();
        let mut offset = 0;
        while offset < model.targets.len() {
            let priority = model.targets[offset].priority;
            let end = model.targets[offset..]
                .iter()
                .position(|target| target.priority != priority)
                .map_or(model.targets.len(), |relative| offset + relative);
            if priority < minimum_priority {
                offset = end;
                continue;
            }
            let mut available = model.targets[offset..end]
                .iter()
                // Cross-protocol selection is explicit: only targets that
                // opted in via `translate` may serve a different dialect.
                .filter(|target| target.protocol == protocol || target.translate)
                .filter(|target| {
                    target.api_key_env.is_none()
                        || self.upstream_keys.contains_key(&target.target_id)
                })
                .filter(|target| service_is_available(&target.service, target.target_id))
                .collect::<Vec<_>>();
            prefer(&mut available);
            if !available.is_empty() {
                // Opt-in latency bias: warm EWMA observations adjust —
                // never replace — the configured weights, clamped so a
                // slow target keeps at least half its weight and a fast
                // one gains at most double.
                let bias = if model.latency_bias {
                    self.latency_bias_permille(
                        &available
                            .iter()
                            .map(|target| target.target_id)
                            .collect::<Vec<_>>(),
                    )
                } else {
                    HashMap::new()
                };
                let total_weight = available
                    .iter()
                    .map(|target| {
                        let factor = bias.get(&target.target_id).copied().unwrap_or(1_000);
                        u64::from(target.weight).saturating_mul(factor) / 1_000
                    })
                    .sum::<u64>();
                if total_weight == 0 {
                    return Err(InferenceAccessError::Unavailable);
                }
                let key = TargetCounterKey {
                    route_id: self.route_id_for_model(model),
                    model_id: model.model_id,
                    priority,
                };
                let selected_weight = {
                    let mut counters = self
                        .target_counters
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner);
                    let counter = counters.entry(key).or_default();
                    let selected = *counter % total_weight;
                    *counter = counter.wrapping_add(1);
                    selected
                };
                let mut cumulative = 0_u64;
                for target in available {
                    let factor = bias.get(&target.target_id).copied().unwrap_or(1_000);
                    cumulative += u64::from(target.weight).saturating_mul(factor) / 1_000;
                    if selected_weight < cumulative {
                        return Ok(InferenceDispatchTarget {
                            model_id: model.model_id,
                            target_id: target.target_id,
                            priority,
                            service: target.service.clone(),
                            upstream_model: target.upstream_model.clone(),
                            protocol: target.protocol,
                            // The responses dialect translates onto
                            // chat-completions within the OpenAI family.
                            translates: (target.protocol != protocol
                                || (endpoint == crate::config::InferenceEndpoint::Responses
                                    && target.protocol == InferenceProtocol::Openai))
                                && target.translate,
                            upstream_key: self.upstream_keys.get(&target.target_id).cloned(),
                        });
                    }
                }
                return Err(InferenceAccessError::Unavailable);
            }
            offset = end;
        }

        Err(InferenceAccessError::Unavailable)
    }

    /// HR3b: the granted model's routing plan, when it carries one.
    pub(crate) fn model_routing_plan(
        &self,
        authenticated: AuthenticatedInference,
        alias: &str,
        now: DateTime<Utc>,
    ) -> Result<Option<crate::config::InferenceRoutingPlan>, InferenceAccessError> {
        let (_, model) = self.granted_model(authenticated, alias, now)?;
        Ok(model.routing.clone())
    }

    /// HR3b: the granted model's full config (for the routing window
    /// composition).
    pub(crate) fn granted_model_config(
        &self,
        authenticated: AuthenticatedInference,
        alias: &str,
        now: DateTime<Utc>,
    ) -> Result<InferenceModelConfig, InferenceAccessError> {
        let (_, model) = self.granted_model(authenticated, alias, now)?;
        Ok(model.clone())
    }

    /// HR2: a bounded fingerprint of the authenticated credential for
    /// turn identity (never key material).
    pub(crate) fn credential_fingerprint(&self, authenticated: AuthenticatedInference) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        authenticated.credential_id.hash(&mut hasher);
        authenticated.credential_generation.hash(&mut hasher);
        hasher.finish()
    }

    /// The route id owning a model — routing windows re-level
    /// priorities, so the counter key routes by the model's owner.
    fn route_id_for_model(&self, model: &InferenceModelConfig) -> Uuid {
        self.route_by_model
            .get(&model.model_id)
            .copied()
            .unwrap_or(Uuid::nil())
    }

    fn granted_model(
        &self,
        authenticated: AuthenticatedInference,
        alias: &str,
        now: DateTime<Utc>,
    ) -> Result<(&InferenceRouteConfig, &InferenceModelConfig), InferenceAccessError> {
        let (route, grant) = self.grant(authenticated, now)?;
        if !grant.allows_model(alias) {
            return Err(InferenceAccessError::Denied);
        }
        let model = route
            .models
            .get(alias)
            .ok_or(InferenceAccessError::Denied)?;
        Ok((route, model))
    }

    /// Supplier bound to OCR or System One on this router.
    ///
    /// `Ok(None)` means the platform model serves the call. A present target
    /// is the only upstream; the caller does not fall back when it is down.
    pub(crate) fn platform_supplier(
        &self,
        router: &str,
        endpoint: InferenceEndpoint,
    ) -> Result<Option<crate::config::InferenceTargetConfig>, InferenceAccessError> {
        let route = self.route(router)?;
        Ok(match endpoint {
            InferenceEndpoint::Ocr => route.ocr_supplier.clone(),
            InferenceEndpoint::SystemOne => route.decision_supplier.clone(),
            InferenceEndpoint::Embeddings => route.embedding_supplier.clone(),
            InferenceEndpoint::Rerank => route.rerank_supplier.clone(),
            _ => None,
        })
    }

    /// Resolved upstream authorization header for one target, when a key exists.
    pub(crate) fn upstream_authorization(&self, target_id: Uuid) -> Option<HeaderValue> {
        self.upstream_keys.get(&target_id).cloned()
    }

    fn route(&self, router: &str) -> Result<&InferenceRouteConfig, InferenceAccessError> {
        let route_id = self
            .routes_by_router
            .get(router)
            .ok_or(InferenceAccessError::Unavailable)?;
        self.policy
            .routes
            .get(route_id)
            .ok_or(InferenceAccessError::Unavailable)
    }

    fn grant(
        &self,
        authenticated: AuthenticatedInference,
        now: DateTime<Utc>,
    ) -> Result<(&InferenceRouteConfig, GrantView<'_>), InferenceAccessError> {
        if self.policy.expires_at <= now {
            return Err(InferenceAccessError::Unavailable);
        }
        let route = self
            .policy
            .routes
            .get(&authenticated.route_id)
            .ok_or(InferenceAccessError::Unavailable)?;
        if authenticated.principal {
            let principal = route
                .principal
                .as_ref()
                .ok_or(InferenceAccessError::Unavailable)?;
            return Ok((route, GrantView::Principal(principal)));
        }
        let credential = self
            .policy
            .credentials
            .get(&authenticated.credential_id)
            .ok_or(InferenceAccessError::Unavailable)?;
        let grant = route
            .grants
            .get(&authenticated.credential_id)
            .ok_or(InferenceAccessError::Denied)?;
        if credential.revoked {
            return Err(InferenceAccessError::Revoked);
        }
        if credential.expires_at <= now {
            return Err(InferenceAccessError::Unauthorized);
        }
        if credential.environment_id != route.environment_id
            || credential.generation != authenticated.credential_generation
            || grant.credential_generation != authenticated.credential_generation
        {
            return Err(InferenceAccessError::Denied);
        }
        Ok((route, GrantView::Credential(grant)))
    }

    fn credential_for_token(
        &self,
        token: &str,
    ) -> Result<&InferenceCredentialConfig, InferenceAccessError> {
        if !valid_inference_key(token) {
            return Err(InferenceAccessError::Unauthorized);
        }
        for length in &self.prefix_lengths {
            if token.len() <= *length {
                continue;
            }
            let Some(prefix) = token.get(..*length) else {
                return Err(InferenceAccessError::Unauthorized);
            };
            let Some(credential_id) = self.credentials_by_prefix.get(prefix) else {
                continue;
            };
            return self
                .policy
                .credentials
                .get(credential_id)
                .ok_or(InferenceAccessError::Unavailable);
        }
        Err(InferenceAccessError::Unauthorized)
    }

    async fn verify_token(
        &self,
        token: &str,
        credential: &InferenceCredentialConfig,
    ) -> Result<(), InferenceAccessError> {
        let digest: [u8; 32] = Sha256::digest(token.as_bytes()).into();
        {
            let cache = self.verified.lock().unwrap_or_else(PoisonError::into_inner);
            if cache.get(&digest)
                == Some(&CachedCredential {
                    credential_id: credential.credential_id,
                    generation: credential.generation,
                })
            {
                return Ok(());
            }
        }

        let permit = self
            .verification_permits
            .clone()
            .try_acquire_owned()
            .map_err(|_| InferenceAccessError::Unavailable)?;
        #[cfg(test)]
        let completion_gate = self.verification_completion_gate.clone();
        let candidate = token.to_owned();
        let verifier_hash = credential.verifier_hash().to_owned();
        let verified = tokio::task::spawn_blocking(move || {
            // Blocking tasks cannot be canceled after they start. Keep the
            // permit here so a disconnected caller cannot release capacity
            // while its Argon2 work is still consuming memory.
            let _permit = permit;
            let parsed =
                PasswordHash::new(&verifier_hash).map_err(|_| InferenceAccessError::Unavailable)?;
            let verified = Argon2::default()
                .verify_password(candidate.as_bytes(), &parsed)
                .is_ok();
            #[cfg(test)]
            if let Some(gate) = completion_gate {
                gate.completed
                    .send(())
                    .map_err(|_| InferenceAccessError::Unavailable)?;
                gate.release.wait();
            }
            Ok::<_, InferenceAccessError>(verified)
        })
        .await
        .map_err(|_| InferenceAccessError::Unavailable)??;
        if !verified {
            return Err(InferenceAccessError::Unauthorized);
        }

        let mut cache = self.verified.lock().unwrap_or_else(PoisonError::into_inner);
        if cache.len() < self.policy.credentials.len() {
            cache.insert(
                digest,
                CachedCredential {
                    credential_id: credential.credential_id,
                    generation: credential.generation,
                },
            );
        }
        Ok(())
    }
}

fn endpoint(profile: OpenAiRequestProfile) -> InferenceEndpoint {
    match profile {
        OpenAiRequestProfile::Models => InferenceEndpoint::Models,
        OpenAiRequestProfile::ChatCompletions => InferenceEndpoint::ChatCompletions,
        OpenAiRequestProfile::Completions => InferenceEndpoint::Completions,
        OpenAiRequestProfile::Embeddings => InferenceEndpoint::Embeddings,
        OpenAiRequestProfile::Messages | OpenAiRequestProfile::CountTokens => {
            InferenceEndpoint::Messages
        }
        OpenAiRequestProfile::Ocr => InferenceEndpoint::Ocr,
        OpenAiRequestProfile::SystemOne => InferenceEndpoint::SystemOne,
        OpenAiRequestProfile::Rerank => InferenceEndpoint::Rerank,
        OpenAiRequestProfile::Responses => InferenceEndpoint::Responses,
    }
}

/// Resolve every target `api_key_env` once per snapshot. A missing variable
/// leaves the target unavailable instead of forwarding an unauthenticated
/// request to a paid provider.
fn resolve_upstream_keys(policy: &InferenceConfig) -> HashMap<Uuid, HeaderValue> {
    let mut keys = HashMap::new();
    for route in policy.routes.values() {
        for model in route.models.values() {
            for target in &model.targets {
                remember_upstream_key(&mut keys, target);
            }
        }
        if let Some(target) = &route.ocr_supplier {
            remember_upstream_key(&mut keys, target);
        }
        if let Some(target) = &route.decision_supplier {
            remember_upstream_key(&mut keys, target);
        }
    }
    keys
}

fn remember_upstream_key(
    keys: &mut HashMap<Uuid, HeaderValue>,
    target: &crate::config::InferenceTargetConfig,
) {
    let Some(name) = &target.api_key_env else {
        return;
    };
    let secret = std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty());
    let value = secret.and_then(|secret| {
        HeaderValue::from_str(&target.protocol.authorization_value(&secret)).ok()
    });
    match value {
        Some(mut value) => {
            value.set_sensitive(true);
            keys.insert(target.target_id, value);
        }
        None => tracing::warn!(
            target_id = %target.target_id,
            api_key_env = %name,
            "Inference target disabled: upstream key environment variable is unset or invalid"
        ),
    }
}

fn bearer_token(headers: &HeaderMap) -> Result<&str, InferenceAccessError> {
    let mut values = headers.get_all(AUTHORIZATION).iter();
    let value = values.next().ok_or(InferenceAccessError::Unauthorized)?;
    if values.next().is_some() {
        return Err(InferenceAccessError::Unauthorized);
    }
    let value = value
        .to_str()
        .map_err(|_| InferenceAccessError::Unauthorized)?;
    let (scheme, token) = value
        .split_once(' ')
        .ok_or(InferenceAccessError::Unauthorized)?;
    if !scheme.eq_ignore_ascii_case("bearer")
        || token.is_empty()
        || token.contains(char::is_whitespace)
    {
        return Err(InferenceAccessError::Unauthorized);
    }
    Ok(token)
}

fn valid_inference_key(token: &str) -> bool {
    token.len() <= MAX_INFERENCE_KEY_BYTES
        && token.starts_with("a3s_inf_")
        && token.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(byte, b'-' | b'.' | b'_' | b'~' | b'+' | b'/' | b'=')
        })
}
#[cfg(test)]
#[path = "authorization_tests.rs"]
mod tests;
