//! Snapshot-backed local admission limits for managed inference grants.

use super::access_error::InferenceAccessError;
use super::observed_tokens::ObservedTokens;
use crate::config::{InferenceConfig, InferenceLimitsConfig, InferenceModelPricing};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};
use uuid::Uuid;

const NANOS_PER_SECOND: u128 = 1_000_000_000;
const NANOS_PER_MINUTE: u128 = 60 * NANOS_PER_SECOND;

/// Estimated token reservation split by price lane. The input lane prices at
/// the model's input rate, the completion lane at its output rate.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct TokenReservation {
    pub(crate) input: u64,
    pub(crate) completion: u64,
}

impl TokenReservation {
    pub(crate) const fn total(self) -> u64 {
        self.input.saturating_add(self.completion)
    }
}

/// Immutable identity of one credential grant's local counters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct InferenceGrantIdentity {
    pub(super) route_id: Uuid,
    pub(super) policy_revision: u64,
    pub(super) credential_id: Uuid,
    pub(super) credential_generation: u64,
}

/// Upper bound on lazily created per-subject limiters.
const MAX_PRINCIPAL_LIMITERS: usize = 100_000;

/// Local limit state for every grant in one active inference projection.
pub(super) struct InferenceLimitStore {
    states: HashMap<InferenceGrantIdentity, Arc<InferenceGrantLimiter>>,
    /// Principal route limits keyed by `(route_id, policy_revision)`.
    principal_limits: HashMap<(Uuid, u64), InferenceLimitsConfig>,
    /// Per-subject limiters created on first admission.
    principal_states: Mutex<HashMap<InferenceGrantIdentity, Arc<InferenceGrantLimiter>>>,
}

impl InferenceLimitStore {
    pub(super) fn new(policy: &InferenceConfig, previous: Option<&Self>) -> Self {
        let principal_limits = policy
            .routes
            .values()
            .filter_map(|route| {
                route.principal.as_ref().map(|principal| {
                    (
                        (route.route_id, route.policy_revision),
                        principal.limits.clone(),
                    )
                })
            })
            .collect::<HashMap<_, _>>();
        let principal_states = previous
            .map(|previous| {
                previous
                    .principal_states
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .iter()
                    .filter(|(identity, state)| {
                        principal_limits
                            .get(&(identity.route_id, identity.policy_revision))
                            .is_some_and(|limits| *limits == state.limits)
                    })
                    .map(|(identity, state)| (*identity, state.clone()))
                    .collect()
            })
            .unwrap_or_default();
        Self {
            states: Self::grant_states(policy, previous),
            principal_limits,
            principal_states: Mutex::new(principal_states),
        }
    }

    fn grant_states(
        policy: &InferenceConfig,
        previous: Option<&Self>,
    ) -> HashMap<InferenceGrantIdentity, Arc<InferenceGrantLimiter>> {
        let mut states = HashMap::new();
        for route in policy.routes.values() {
            for (credential_id, grant) in &route.grants {
                let identity = InferenceGrantIdentity {
                    route_id: route.route_id,
                    policy_revision: route.policy_revision,
                    credential_id: *credential_id,
                    credential_generation: grant.credential_generation,
                };
                let state = previous
                    .and_then(|previous| previous.states.get(&identity))
                    .filter(|state| state.limits == grant.limits)
                    .cloned()
                    .unwrap_or_else(|| Arc::new(InferenceGrantLimiter::new(grant.limits.clone())));
                states.insert(identity, state);
            }
        }
        states
    }

    fn principal_state(
        &self,
        identity: InferenceGrantIdentity,
    ) -> Option<Arc<InferenceGrantLimiter>> {
        if identity.credential_generation != 0 {
            return None;
        }
        let limits = self
            .principal_limits
            .get(&(identity.route_id, identity.policy_revision))?;
        let mut states = self
            .principal_states
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(state) = states.get(&identity) {
            return Some(state.clone());
        }
        if states.len() >= MAX_PRINCIPAL_LIMITERS {
            // Idle limiters only carry refillable budget; in-flight ones are
            // still referenced by admission guards and must survive.
            states.retain(|_, state| Arc::strong_count(state) > 1);
            if states.len() >= MAX_PRINCIPAL_LIMITERS {
                return None;
            }
        }
        let state = Arc::new(InferenceGrantLimiter::new(limits.clone()));
        states.insert(identity, state.clone());
        Some(state)
    }

    pub(super) fn try_admit(
        &self,
        identity: InferenceGrantIdentity,
        reservation: TokenReservation,
        pricing: Option<InferenceModelPricing>,
    ) -> Result<InferenceGrantAdmissionGuard, InferenceAccessError> {
        self.try_admit_at(identity, reservation, pricing, Instant::now())
    }

    fn try_admit_at(
        &self,
        identity: InferenceGrantIdentity,
        reservation: TokenReservation,
        pricing: Option<InferenceModelPricing>,
        now: Instant,
    ) -> Result<InferenceGrantAdmissionGuard, InferenceAccessError> {
        let state = match self.states.get(&identity) {
            Some(state) => state.clone(),
            None => self
                .principal_state(identity)
                .ok_or(InferenceAccessError::Unavailable)?,
        };
        state.try_admit(now, reservation, pricing)
    }
}

struct InferenceGrantLimiter {
    limits: InferenceLimitsConfig,
    requests: Mutex<RequestTokenBucket>,
    tokens: Mutex<TokenBudgetBucket>,
    spend: Option<Mutex<SpendBudgetBucket>>,
    in_flight: AtomicU64,
}

impl InferenceGrantLimiter {
    fn new(limits: InferenceLimitsConfig) -> Self {
        let now = Instant::now();
        let spend = match (limits.max_spend_micros, limits.spend_window) {
            (Some(max_spend_micros), Some(window)) => {
                Some(Mutex::new(SpendBudgetBucket::with_baseline(
                    max_spend_micros,
                    window.window_nanos(),
                    limits.spend_baseline_micros.unwrap_or(0),
                    now,
                )))
            }
            _ => None,
        };
        Self {
            requests: Mutex::new(RequestTokenBucket::new(
                limits.requests_per_minute,
                limits.request_burst,
                now,
            )),
            tokens: Mutex::new(TokenBudgetBucket::new(limits.tokens_per_minute, now)),
            spend,
            limits,
            in_flight: AtomicU64::new(0),
        }
    }

    fn try_admit(
        self: Arc<Self>,
        now: Instant,
        reservation: TokenReservation,
        pricing: Option<InferenceModelPricing>,
    ) -> Result<InferenceGrantAdmissionGuard, InferenceAccessError> {
        let reserved_tokens = reservation.total();
        let retry_after_secs = {
            let mut requests = self.requests.lock().unwrap_or_else(PoisonError::into_inner);
            requests.try_acquire(now)
        };
        if let Err(retry_after_secs) = retry_after_secs {
            return Err(InferenceAccessError::RateLimited { retry_after_secs });
        }

        let token_retry_after = {
            let mut tokens = self.tokens.lock().unwrap_or_else(PoisonError::into_inner);
            tokens.try_reserve(now, reserved_tokens)
        };
        if let Err(retry_after_secs) = token_retry_after {
            return Err(InferenceAccessError::RateLimited { retry_after_secs });
        }

        let reserved_spend_micros = match &self.spend {
            None => 0,
            Some(_) => {
                let Some(pricing) = pricing else {
                    // Snapshot validation refuses budgeted grants over
                    // unpriced models; this is the runtime backstop.
                    if reserved_tokens > 0 {
                        let mut tokens = self.tokens.lock().unwrap_or_else(PoisonError::into_inner);
                        tokens.refund(Instant::now(), reserved_tokens);
                    }
                    return Err(InferenceAccessError::Unavailable);
                };
                pricing.price_micros(reservation.input, reservation.completion)
            }
        };
        if let Some(spend) = &self.spend {
            let spend_retry_after = {
                let mut spend = spend.lock().unwrap_or_else(PoisonError::into_inner);
                spend.try_reserve(now, reserved_spend_micros)
            };
            if let Err(retry_after_secs) = spend_retry_after {
                if reserved_tokens > 0 {
                    let mut tokens = self.tokens.lock().unwrap_or_else(PoisonError::into_inner);
                    tokens.refund(Instant::now(), reserved_tokens);
                }
                return Err(InferenceAccessError::SpendLimited { retry_after_secs });
            }
        }

        let mut current = self.in_flight.load(Ordering::Acquire);
        loop {
            if current >= self.limits.max_concurrent_requests {
                if reserved_tokens > 0 {
                    let mut tokens = self.tokens.lock().unwrap_or_else(PoisonError::into_inner);
                    tokens.refund(Instant::now(), reserved_tokens);
                }
                if reserved_spend_micros > 0 {
                    if let Some(spend) = &self.spend {
                        let mut spend = spend.lock().unwrap_or_else(PoisonError::into_inner);
                        spend.refund(Instant::now(), reserved_spend_micros);
                    }
                }
                return Err(InferenceAccessError::ConcurrencyLimited);
            }
            match self.in_flight.compare_exchange_weak(
                current,
                current + 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    return Ok(InferenceGrantAdmissionGuard {
                        state: self,
                        reserved_tokens: AtomicU64::new(reserved_tokens),
                        reserved_spend_micros: AtomicU64::new(reserved_spend_micros),
                        pricing,
                        reconciled: AtomicBool::new(false),
                    })
                }
                Err(observed) => current = observed,
            }
        }
    }
}

/// Drop guard for one request admitted against a credential grant.
///
/// The guard must live until the request or response stream reaches its
/// terminal boundary. It deliberately cannot be cloned.
///
/// Token budget: `reserved_tokens` are deducted at admit time. Call
/// [`reconcile_usage`] when trusted usage is known; otherwise the reservation
/// remains charged (fail-closed under-billing protection). The spend budget
/// reconciles the same way, priced at the admitted model's snapshot pricing.
pub(super) struct InferenceGrantAdmissionGuard {
    state: Arc<InferenceGrantLimiter>,
    reserved_tokens: AtomicU64,
    reserved_spend_micros: AtomicU64,
    pricing: Option<InferenceModelPricing>,
    reconciled: AtomicBool,
}

impl InferenceGrantAdmissionGuard {
    /// Adjust the token and spend budgets from reserved estimates to observed
    /// upstream usage. A total that cannot be resolved keeps both
    /// reservations charged.
    pub(super) fn reconcile_usage(&self, observed: &ObservedTokens) {
        if self
            .reconciled
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return;
        }
        let Some(actual_total) = observed.resolved_total() else {
            return;
        };
        let reserved = self.reserved_tokens.swap(actual_total, Ordering::AcqRel);
        {
            let mut tokens = self
                .state
                .tokens
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            tokens.reconcile(Instant::now(), reserved, actual_total);
        }
        let Some(spend) = &self.state.spend else {
            return;
        };
        let Some(pricing) = self.pricing else {
            return;
        };
        let actual_spend_micros = match (observed.prompt, observed.completion) {
            (Some(prompt), Some(completion)) => pricing.price_micros(prompt, completion),
            // Split unknown: the dearer lane prices the whole total.
            _ => pricing.price_conservative_micros(actual_total),
        };
        let reserved_spend = self
            .reserved_spend_micros
            .swap(actual_spend_micros, Ordering::AcqRel);
        let mut spend = spend.lock().unwrap_or_else(PoisonError::into_inner);
        spend.reconcile(Instant::now(), reserved_spend, actual_spend_micros);
    }
}

impl Drop for InferenceGrantAdmissionGuard {
    fn drop(&mut self) {
        let result =
            self.state
                .in_flight
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                    current.checked_sub(1)
                });
        debug_assert!(result.is_ok(), "inference admission guard underflow");
    }
}

/// Exact rational token bucket for request admissions.
///
/// One request costs `NANOS_PER_MINUTE` units. A nanosecond of elapsed time
/// earns `requests_per_minute` units, avoiding floating-point drift.
struct RequestTokenBucket {
    requests_per_minute: u64,
    capacity: u128,
    available: u128,
    last_refill: Instant,
}

impl RequestTokenBucket {
    fn new(requests_per_minute: u64, burst: u64, now: Instant) -> Self {
        let capacity = u128::from(burst) * NANOS_PER_MINUTE;
        Self {
            requests_per_minute,
            capacity,
            available: capacity,
            last_refill: now,
        }
    }

    fn try_acquire(&mut self, now: Instant) -> Result<(), u64> {
        self.refill(now);
        if self.available >= NANOS_PER_MINUTE {
            self.available -= NANOS_PER_MINUTE;
            return Ok(());
        }
        if self.requests_per_minute == 0 {
            return Err(u64::MAX);
        }

        let missing = NANOS_PER_MINUTE - self.available;
        let units_per_second =
            u128::from(self.requests_per_minute).saturating_mul(NANOS_PER_SECOND);
        let seconds = ceil_div(missing, units_per_second).max(1);
        Err(u64::try_from(seconds).unwrap_or(u64::MAX))
    }

    fn refill(&mut self, now: Instant) {
        let elapsed = now
            .checked_duration_since(self.last_refill)
            .unwrap_or(Duration::ZERO);
        let earned = elapsed
            .as_nanos()
            .saturating_mul(u128::from(self.requests_per_minute));
        self.available = self.available.saturating_add(earned).min(self.capacity);
        if now > self.last_refill {
            self.last_refill = now;
        }
    }
}

/// Exact rational token-budget bucket for model-token reservations.
///
/// Capacity equals one minute of `tokens_per_minute` (no separate burst knob).
/// One model token costs `NANOS_PER_MINUTE` units so refill math matches the
/// request bucket.
struct TokenBudgetBucket {
    tokens_per_minute: u64,
    capacity: u128,
    available: u128,
    last_refill: Instant,
}

impl TokenBudgetBucket {
    fn new(tokens_per_minute: u64, now: Instant) -> Self {
        let capacity = u128::from(tokens_per_minute).saturating_mul(NANOS_PER_MINUTE);
        Self {
            tokens_per_minute,
            capacity,
            available: capacity,
            last_refill: now,
        }
    }

    fn try_reserve(&mut self, now: Instant, tokens: u64) -> Result<(), u64> {
        if tokens == 0 {
            return Ok(());
        }
        self.refill(now);
        let cost = u128::from(tokens).saturating_mul(NANOS_PER_MINUTE);
        if self.available >= cost {
            self.available -= cost;
            return Ok(());
        }
        if self.tokens_per_minute == 0 {
            return Err(u64::MAX);
        }

        let missing = cost - self.available;
        let units_per_second = u128::from(self.tokens_per_minute).saturating_mul(NANOS_PER_SECOND);
        let seconds = ceil_div(missing, units_per_second).max(1);
        Err(u64::try_from(seconds).unwrap_or(u64::MAX))
    }

    fn refund(&mut self, now: Instant, tokens: u64) {
        if tokens == 0 {
            return;
        }
        self.refill(now);
        let credit = u128::from(tokens).saturating_mul(NANOS_PER_MINUTE);
        self.available = self.available.saturating_add(credit).min(self.capacity);
    }

    fn reconcile(&mut self, now: Instant, reserved: u64, actual: u64) {
        if actual == reserved {
            return;
        }
        if actual < reserved {
            self.refund(now, reserved - actual);
            return;
        }
        self.refill(now);
        let extra = actual - reserved;
        let cost = u128::from(extra).saturating_mul(NANOS_PER_MINUTE);
        self.available = self.available.saturating_sub(cost);
    }

    fn refill(&mut self, now: Instant) {
        let elapsed = now
            .checked_duration_since(self.last_refill)
            .unwrap_or(Duration::ZERO);
        let earned = elapsed
            .as_nanos()
            .saturating_mul(u128::from(self.tokens_per_minute));
        self.available = self.available.saturating_add(earned).min(self.capacity);
        if now > self.last_refill {
            self.last_refill = now;
        }
    }
}

/// Exact rational rolling budget for spend, mirroring
/// [`TokenBudgetBucket`] with a configurable window. One micro-USD costs
/// `window_nanos` units; `max_spend_micros` refills per window.
struct SpendBudgetBucket {
    max_spend_micros: u64,
    window_nanos: u128,
    capacity: u128,
    available: u128,
    last_refill: Instant,
}

impl SpendBudgetBucket {
    fn new(max_spend_micros: u64, window_nanos: u128, now: Instant) -> Self {
        let capacity = u128::from(max_spend_micros).saturating_mul(window_nanos);
        Self {
            max_spend_micros,
            window_nanos,
            capacity,
            available: capacity,
            last_refill: now,
        }
    }

    /// A bucket seeded by a projected ledger baseline: the local window
    /// starts at `capacity − baseline` (the `spend_baseline` design
    /// record — advisory correction, asynchronous only; the parse layer
    /// already refuses a baseline above the budget).
    fn with_baseline(
        max_spend_micros: u64,
        window_nanos: u128,
        baseline_micros: u64,
        now: Instant,
    ) -> Self {
        let mut bucket = Self::new(max_spend_micros, window_nanos, now);
        let consumed = u128::from(baseline_micros).saturating_mul(window_nanos);
        bucket.available = bucket.available.saturating_sub(consumed);
        bucket
    }

    fn try_reserve(&mut self, now: Instant, micros: u64) -> Result<(), u64> {
        if micros == 0 {
            return Ok(());
        }
        self.refill(now);
        let cost = u128::from(micros).saturating_mul(self.window_nanos);
        if self.available >= cost {
            self.available -= cost;
            return Ok(());
        }
        if self.max_spend_micros == 0 {
            return Err(u64::MAX);
        }

        let missing = cost - self.available;
        let units_per_second = u128::from(self.max_spend_micros).saturating_mul(NANOS_PER_SECOND);
        let seconds = ceil_div(missing, units_per_second).max(1);
        Err(u64::try_from(seconds).unwrap_or(u64::MAX))
    }

    fn refund(&mut self, now: Instant, micros: u64) {
        if micros == 0 {
            return;
        }
        self.refill(now);
        let credit = u128::from(micros).saturating_mul(self.window_nanos);
        self.available = self.available.saturating_add(credit).min(self.capacity);
    }

    fn reconcile(&mut self, now: Instant, reserved: u64, actual: u64) {
        if actual == reserved {
            return;
        }
        if actual < reserved {
            self.refund(now, reserved - actual);
            return;
        }
        self.refill(now);
        let extra = actual - reserved;
        let cost = u128::from(extra).saturating_mul(self.window_nanos);
        self.available = self.available.saturating_sub(cost);
    }

    fn refill(&mut self, now: Instant) {
        let elapsed = now
            .checked_duration_since(self.last_refill)
            .unwrap_or(Duration::ZERO);
        let earned = elapsed
            .as_nanos()
            .saturating_mul(u128::from(self.max_spend_micros));
        self.available = self.available.saturating_add(earned).min(self.capacity);
        if now > self.last_refill {
            self.last_refill = now;
        }
    }
}

fn ceil_div(numerator: u128, denominator: u128) -> u128 {
    numerator.div_ceil(denominator)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        InferenceCredentialConfig, InferenceEndpoint, InferenceGrantConfig, InferenceModelConfig,
        InferenceRouteConfig, InferenceTargetConfig,
    };
    use chrono::Utc;

    fn limits(
        max_concurrent_requests: u64,
        requests_per_minute: u64,
        burst: u64,
    ) -> InferenceLimitsConfig {
        InferenceLimitsConfig {
            max_concurrent_requests,
            requests_per_minute,
            request_burst: burst,
            tokens_per_minute: 10_000,
            max_spend_micros: None,
            spend_window: None,
            spend_baseline_micros: None,
            baseline_epoch: None,
        }
    }

    fn limits_with_tpm(
        max_concurrent_requests: u64,
        requests_per_minute: u64,
        burst: u64,
        tokens_per_minute: u64,
    ) -> InferenceLimitsConfig {
        InferenceLimitsConfig {
            max_concurrent_requests,
            requests_per_minute,
            request_burst: burst,
            tokens_per_minute,
            max_spend_micros: None,
            spend_window: None,
            spend_baseline_micros: None,
            baseline_epoch: None,
        }
    }

    fn policy(limits: InferenceLimitsConfig) -> (InferenceConfig, InferenceGrantIdentity) {
        let route_id = Uuid::new_v4();
        let credential_id = Uuid::new_v4();
        let environment_id = Uuid::new_v4();
        let identity = InferenceGrantIdentity {
            route_id,
            policy_revision: 7,
            credential_id,
            credential_generation: 3,
        };
        let policy = InferenceConfig {
            expires_at: Utc::now() + chrono::Duration::hours(1),
            tokenizer_revision: crate::config::INFERENCE_TOKENIZER_REVISION.into(),
            credentials: HashMap::from([(
                credential_id,
                InferenceCredentialConfig {
                    credential_id,
                    environment_id,
                    audience: "cloud-inference".into(),
                    prefix: "a3s_inf_abc12345".into(),
                    verifier_hash: String::new(),
                    generation: 3,
                    expires_at: Utc::now() + chrono::Duration::hours(1),
                    revoked: false,
                },
            )]),
            routes: HashMap::from([(
                route_id,
                InferenceRouteConfig {
                    engine_profiles: Vec::new(),
                    route_id,
                    router: "inference".into(),
                    environment_id,
                    policy_revision: 7,
                    console_catalog: None,
                    models: HashMap::from([(
                        "model".into(),
                        InferenceModelConfig {
                            engine_profile: None,
                            routing: None,
                            failover: Default::default(),
                            pricing: None,
                            latency_bias: false,
                            prefix_affinity: false,
                            prefix_parallelism: 1,
                            model_id: Uuid::new_v4(),
                            targets: vec![InferenceTargetConfig {
                                target_id: Uuid::new_v4(),
                                service: "model".into(),
                                upstream_model: "model".into(),
                                priority: 0,
                                weight: 1,
                                protocol: Default::default(),
                                api_key_env: None,
                                translate: false,
                            }],
                            scheduling: None,
                        },
                    )]),
                    grants: HashMap::from([(
                        credential_id,
                        InferenceGrantConfig {
                            credential_generation: 3,
                            models: vec!["model".into()],
                            endpoints: vec![InferenceEndpoint::Models],
                            limits,
                            listings: Vec::new(),
                            guardrails: None,
                        },
                    )]),
                    listings: HashMap::new(),
                    wire: Default::default(),
                    principal: None,
                    ocr_supplier: None,
                    decision_supplier: None,
                    embedding_supplier: None,
                    rerank_supplier: None,
                },
            )]),
            guardrails: HashMap::new(),
            workers: HashMap::new(),
        };
        (policy, identity)
    }

    #[test]
    fn request_bucket_enforces_burst_and_exact_refill() {
        let start = Instant::now();
        let mut bucket = RequestTokenBucket::new(60, 2, start);

        assert_eq!(bucket.try_acquire(start), Ok(()));
        assert_eq!(bucket.try_acquire(start), Ok(()));
        assert_eq!(bucket.try_acquire(start), Err(1));
        assert_eq!(
            bucket.try_acquire(start + Duration::from_millis(999)),
            Err(1)
        );
        assert_eq!(bucket.try_acquire(start + Duration::from_secs(1)), Ok(()));
    }

    #[test]
    fn request_bucket_reports_ceiling_retry_after() {
        let start = Instant::now();
        let mut bucket = RequestTokenBucket::new(10, 1, start);
        assert_eq!(bucket.try_acquire(start), Ok(()));
        assert_eq!(bucket.try_acquire(start), Err(6));
        assert_eq!(
            bucket.try_acquire(start + Duration::from_millis(1_500)),
            Err(5)
        );
    }

    #[test]
    fn request_bucket_defensively_rejects_a_zero_rate() {
        let start = Instant::now();
        let mut bucket = RequestTokenBucket::new(0, 0, start);

        assert_eq!(bucket.try_acquire(start), Err(u64::MAX));
    }

    #[test]
    fn token_budget_enforces_reservation_and_exact_refill() {
        let start = Instant::now();
        let mut bucket = TokenBudgetBucket::new(60, start);

        assert_eq!(bucket.try_reserve(start, 40), Ok(()));
        assert_eq!(bucket.try_reserve(start, 20), Ok(()));
        assert_eq!(bucket.try_reserve(start, 1), Err(1));
        assert_eq!(
            bucket.try_reserve(start + Duration::from_millis(999), 1),
            Err(1)
        );
        assert_eq!(
            bucket.try_reserve(start + Duration::from_secs(1), 1),
            Ok(())
        );
    }

    #[test]
    fn token_budget_reconcile_refunds_unused_reservation() {
        let start = Instant::now();
        let mut bucket = TokenBudgetBucket::new(100, start);
        assert_eq!(bucket.try_reserve(start, 80), Ok(()));
        bucket.reconcile(start, 80, 30);
        assert_eq!(bucket.try_reserve(start, 70), Ok(()));
    }

    #[test]
    fn token_budget_reconcile_charges_extra_tokens() {
        let start = Instant::now();
        let mut bucket = TokenBudgetBucket::new(100, start);
        assert_eq!(bucket.try_reserve(start, 40), Ok(()));
        bucket.reconcile(start, 40, 100);
        assert_eq!(bucket.try_reserve(start, 1), Err(1));
    }

    #[test]
    fn concurrency_is_held_until_each_guard_drops() {
        let (policy, identity) = policy(limits(2, 60, 60));
        let store = InferenceLimitStore::new(&policy, None);
        let first = store
            .try_admit(
                identity,
                TokenReservation {
                    input: 0,
                    completion: 0,
                },
                None,
            )
            .unwrap();
        let second = store
            .try_admit(
                identity,
                TokenReservation {
                    input: 0,
                    completion: 0,
                },
                None,
            )
            .unwrap();

        assert!(matches!(
            store.try_admit(
                identity,
                TokenReservation {
                    input: 0,
                    completion: 0
                },
                None
            ),
            Err(InferenceAccessError::ConcurrencyLimited)
        ));
        drop(first);
        assert!(store
            .try_admit(
                identity,
                TokenReservation {
                    input: 0,
                    completion: 0
                },
                None
            )
            .is_ok());
        drop(second);
    }

    #[test]
    fn concurrency_rejection_still_consumes_an_authorized_request_token() {
        let (policy, identity) = policy(limits(1, 2, 2));
        let store = InferenceLimitStore::new(&policy, None);
        let active = store
            .try_admit(
                identity,
                TokenReservation {
                    input: 0,
                    completion: 0,
                },
                None,
            )
            .unwrap();

        assert!(matches!(
            store.try_admit(
                identity,
                TokenReservation {
                    input: 0,
                    completion: 0
                },
                None
            ),
            Err(InferenceAccessError::ConcurrencyLimited)
        ));
        drop(active);
        assert!(matches!(
            store.try_admit(
                identity,
                TokenReservation {
                    input: 0,
                    completion: 0
                },
                None
            ),
            Err(InferenceAccessError::RateLimited { .. })
        ));
    }

    #[test]
    fn concurrency_rejection_refunds_token_reservation() {
        let (policy, identity) = policy(limits_with_tpm(1, 60, 60, 50));
        let store = InferenceLimitStore::new(&policy, None);
        let active = store
            .try_admit(
                identity,
                TokenReservation {
                    input: 10,
                    completion: 0,
                },
                None,
            )
            .unwrap();

        assert!(matches!(
            store.try_admit(
                identity,
                TokenReservation {
                    input: 40,
                    completion: 0
                },
                None
            ),
            Err(InferenceAccessError::ConcurrencyLimited)
        ));
        drop(active);
        assert!(store
            .try_admit(
                identity,
                TokenReservation {
                    input: 40,
                    completion: 0
                },
                None
            )
            .is_ok());
    }

    #[test]
    fn token_budget_blocks_admission_when_exhausted() {
        let (policy, identity) = policy(limits_with_tpm(8, 60, 60, 100));
        let store = InferenceLimitStore::new(&policy, None);
        let _first = store
            .try_admit(
                identity,
                TokenReservation {
                    input: 100,
                    completion: 0,
                },
                None,
            )
            .unwrap();
        assert!(matches!(
            store.try_admit(
                identity,
                TokenReservation {
                    input: 1,
                    completion: 0
                },
                None
            ),
            Err(InferenceAccessError::RateLimited { .. })
        ));
    }

    #[test]
    fn unreconciled_reservation_remains_charged_after_drop() {
        let (policy, identity) = policy(limits_with_tpm(8, 60, 60, 100));
        let store = InferenceLimitStore::new(&policy, None);
        let guard = store
            .try_admit(
                identity,
                TokenReservation {
                    input: 100,
                    completion: 0,
                },
                None,
            )
            .unwrap();
        drop(guard);
        assert!(matches!(
            store.try_admit(
                identity,
                TokenReservation {
                    input: 1,
                    completion: 0
                },
                None
            ),
            Err(InferenceAccessError::RateLimited { .. })
        ));
    }

    #[test]
    fn reconcile_after_admit_refunds_unused_tokens_for_later_requests() {
        let (policy, identity) = policy(limits_with_tpm(8, 60, 60, 100));
        let store = InferenceLimitStore::new(&policy, None);
        let guard = store
            .try_admit(
                identity,
                TokenReservation {
                    input: 100,
                    completion: 0,
                },
                None,
            )
            .unwrap();
        guard.reconcile_usage(&ObservedTokens {
            prompt: Some(10),
            completion: Some(0),
            ..ObservedTokens::default()
        });
        drop(guard);
        assert!(store
            .try_admit(
                identity,
                TokenReservation {
                    input: 90,
                    completion: 0
                },
                None
            )
            .is_ok());
    }

    #[test]
    fn identical_policy_refresh_reuses_rate_state() {
        let (mut policy, identity) = policy(limits(1, 1, 1));
        let previous = InferenceLimitStore::new(&policy, None);
        let active = previous
            .try_admit(
                identity,
                TokenReservation {
                    input: 0,
                    completion: 0,
                },
                None,
            )
            .unwrap();

        policy.expires_at += chrono::Duration::minutes(5);
        let refreshed = InferenceLimitStore::new(&policy, Some(&previous));
        assert!(matches!(
            refreshed.try_admit(
                identity,
                TokenReservation {
                    input: 0,
                    completion: 0
                },
                None
            ),
            Err(InferenceAccessError::RateLimited { .. })
        ));

        drop(active);
        assert!(matches!(
            refreshed.try_admit(
                identity,
                TokenReservation {
                    input: 0,
                    completion: 0
                },
                None
            ),
            Err(InferenceAccessError::RateLimited { .. })
        ));
    }

    #[test]
    fn identical_policy_refresh_preserves_active_concurrency() {
        let (mut policy, identity) = policy(limits(1, 60, 60));
        let previous = InferenceLimitStore::new(&policy, None);
        let active = previous
            .try_admit(
                identity,
                TokenReservation {
                    input: 0,
                    completion: 0,
                },
                None,
            )
            .unwrap();

        policy.expires_at += chrono::Duration::minutes(5);
        let refreshed = InferenceLimitStore::new(&policy, Some(&previous));
        assert!(matches!(
            refreshed.try_admit(
                identity,
                TokenReservation {
                    input: 0,
                    completion: 0
                },
                None
            ),
            Err(InferenceAccessError::ConcurrencyLimited)
        ));

        drop(active);
        assert!(refreshed
            .try_admit(
                identity,
                TokenReservation {
                    input: 0,
                    completion: 0
                },
                None
            )
            .is_ok());
    }

    #[test]
    fn changed_policy_identity_starts_new_limit_state() {
        let (mut policy, identity) = policy(limits(1, 1, 1));
        let previous = InferenceLimitStore::new(&policy, None);
        let _active = previous
            .try_admit(
                identity,
                TokenReservation {
                    input: 0,
                    completion: 0,
                },
                None,
            )
            .unwrap();

        let route = policy.routes.get_mut(&identity.route_id).unwrap();
        route.policy_revision += 1;
        let changed_identity = InferenceGrantIdentity {
            policy_revision: route.policy_revision,
            ..identity
        };
        let changed = InferenceLimitStore::new(&policy, Some(&previous));

        assert!(changed
            .try_admit(
                changed_identity,
                TokenReservation {
                    input: 0,
                    completion: 0
                },
                None
            )
            .is_ok());
    }

    #[test]
    fn admission_guard_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<InferenceGrantAdmissionGuard>();
        assert_send_sync::<InferenceLimitStore>();
    }

    const TEST_PRICING: crate::config::InferenceModelPricing =
        crate::config::InferenceModelPricing {
            input_micros_per_million: 1_000,
            output_micros_per_million: 3_000,
        };

    fn budgeted_limits(max_spend_micros: u64) -> InferenceLimitsConfig {
        baselined_limits(max_spend_micros, None)
    }

    fn baselined_limits(max_spend_micros: u64, baseline: Option<u64>) -> InferenceLimitsConfig {
        InferenceLimitsConfig {
            max_concurrent_requests: 8,
            requests_per_minute: 60,
            request_burst: 10,
            tokens_per_minute: 1_000_000,
            max_spend_micros: Some(max_spend_micros),
            spend_window: Some(crate::config::InferenceSpendWindow::Minute),
            spend_baseline_micros: baseline,
            baseline_epoch: baseline.map(|_| 7),
        }
    }

    #[test]
    fn a_projected_baseline_seeds_the_spend_bucket_at_the_delta() {
        // Budget 60 micros with a ledger-projected baseline of 40: the
        // local window starts with 20 micros of headroom. One 20-micro
        // reservation fits; the next is refused with the honest wait.
        let (policy, identity) = policy(baselined_limits(60, Some(40)));
        let store = InferenceLimitStore::new(&policy, None);
        let guard = store
            .try_admit(
                identity,
                TokenReservation {
                    input: 5_000,
                    completion: 5_000,
                },
                Some(TEST_PRICING),
            )
            .expect("the baseline leaves exactly the delta");
        assert!(matches!(
            store.try_admit(
                identity,
                TokenReservation {
                    input: 5_000,
                    completion: 5_000,
                },
                Some(TEST_PRICING),
            ),
            Err(InferenceAccessError::SpendLimited {
                retry_after_secs: 20
            })
        ));
        drop(guard);
    }

    fn observed(prompt: u64, completion: u64) -> ObservedTokens {
        ObservedTokens {
            prompt: Some(prompt),
            completion: Some(completion),
            total: Some(prompt + completion),
            ..ObservedTokens::default()
        }
    }

    #[test]
    fn spend_reservation_charges_the_budget_and_rejects_past_it() {
        // Budget 60 micros per minute; each reservation prices at 40 micros
        // (10k input at 1k/million + 10k completion at 3k/million), so the
        // second admission is 20 micros short.
        let (policy, identity) = policy(budgeted_limits(60));
        let store = InferenceLimitStore::new(&policy, None);
        let guard = store
            .try_admit(
                identity,
                TokenReservation {
                    input: 10_000,
                    completion: 10_000,
                },
                Some(TEST_PRICING),
            )
            .unwrap();
        assert!(matches!(
            store.try_admit(
                identity,
                TokenReservation {
                    input: 10_000,
                    completion: 10_000,
                },
                Some(TEST_PRICING),
            ),
            Err(InferenceAccessError::SpendLimited {
                retry_after_secs: 20
            })
        ));
        drop(guard);
    }

    #[test]
    fn spend_reconcile_refunds_the_unused_reservation() {
        let (policy, identity) = policy(budgeted_limits(100));
        let store = InferenceLimitStore::new(&policy, None);
        let guard = store
            .try_admit(
                identity,
                TokenReservation {
                    input: 10_000,
                    completion: 10_000,
                },
                Some(TEST_PRICING),
            )
            .unwrap();
        // Actual usage prices at 3 + 3 = 6 micros.
        guard.reconcile_usage(&observed(3_000, 1_000));
        drop(guard);
        assert!(store
            .try_admit(
                identity,
                TokenReservation {
                    input: 10_000,
                    completion: 10_000,
                },
                Some(TEST_PRICING),
            )
            .is_ok());
    }

    #[test]
    fn unreconciled_spend_reservation_stays_charged() {
        let (policy, identity) = policy(budgeted_limits(40));
        let store = InferenceLimitStore::new(&policy, None);
        let guard = store
            .try_admit(
                identity,
                TokenReservation {
                    input: 10_000,
                    completion: 10_000,
                },
                Some(TEST_PRICING),
            )
            .unwrap();
        drop(guard);
        assert!(matches!(
            store.try_admit(
                identity,
                TokenReservation {
                    input: 1,
                    completion: 0,
                },
                Some(TEST_PRICING),
            ),
            Err(InferenceAccessError::SpendLimited { .. })
        ));
    }

    #[test]
    fn split_unknown_spend_reconcile_prices_conservatively() {
        let (policy, identity) = policy(budgeted_limits(100));
        let store = InferenceLimitStore::new(&policy, None);
        let guard = store
            .try_admit(
                identity,
                TokenReservation {
                    input: 10_000,
                    completion: 0,
                },
                Some(TEST_PRICING),
            )
            .unwrap();
        // Total-only observation prices every token at the dearer output
        // lane: 20k tokens at 3k/million = 60 micros, above the 10-micro
        // reservation — the budget moves to 60 charged.
        guard.reconcile_usage(&ObservedTokens {
            total: Some(20_000),
            ..ObservedTokens::default()
        });
        drop(guard);
        assert!(matches!(
            store.try_admit(
                identity,
                TokenReservation {
                    input: 20_000,
                    completion: 20_000,
                },
                Some(TEST_PRICING),
            ),
            Err(InferenceAccessError::SpendLimited { .. })
        ));
    }

    #[test]
    fn budgeted_grant_refuses_unpriced_model_at_runtime() {
        let (policy, identity) = policy(budgeted_limits(100));
        let store = InferenceLimitStore::new(&policy, None);
        assert!(matches!(
            store.try_admit(
                identity,
                TokenReservation {
                    input: 1_000,
                    completion: 0,
                },
                None,
            ),
            Err(InferenceAccessError::Unavailable)
        ));
    }

    #[test]
    fn unbudgeted_grant_ignores_pricing_entirely() {
        let (policy, identity) = policy(limits_with_tpm(8, 60, 60, 1_000_000_000));
        let store = InferenceLimitStore::new(&policy, None);
        assert!(store
            .try_admit(
                identity,
                TokenReservation {
                    input: 500_000_000,
                    completion: 500_000_000,
                },
                None,
            )
            .is_ok());
    }
}
