//! HR3b: the gateway's decision plane — where task-aware routing
//! consults the frozen-turn cache and the configured decision service.
//! The plane is process-local and evictable; with no transport
//! configured every decision mode resolves fail-closed to primary
//! (the stub below), so a snapshot carrying a routing plan never
//! strands a request.

use crate::config::{InferenceModelConfig, InferenceRoutingPlan};
use crate::inference::decision::{DecisionRequest, DecisionResponse, DecisionService};
use crate::inference::decision_cache::{
    DecisionReason, FrozenDecision, TurnDecisionCache, TurnIdentity,
};
use crate::inference::routing::{evaluate_routing, DecisionInputs, RouteDecision};
use std::sync::{Arc, Mutex};

/// HR4: one recorded competence assessment — a FACT, never a policy
/// write. Scores are basis points for exact comparison against the
/// plan's floor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CompetenceFact {
    score_bp: u32,
    assessed_at: std::time::Instant,
}

/// HR4: the bounded, process-local competence store. Keyed by
/// (credential fingerprint, category scope) — smart saving uses the
/// empty scope, branches use their category. A FRESH, COMPLETE score
/// below the plan's floor protects the next decision in the same
/// scope; stale or partial assessments never lock behavior (HiRoute's
/// fresh-scores-only rule). Eviction is bounded; a lost fact merely
/// stops protecting.
#[derive(Debug, Default)]
pub(crate) struct CompetenceStore {
    facts: Mutex<std::collections::HashMap<(u64, String), CompetenceFact>>,
}

/// How long a recorded assessment stays fresh enough to protect.
const COMPETENCE_FRESHNESS: std::time::Duration = std::time::Duration::from_secs(15 * 60);
/// The retained-fact bound.
const COMPETENCE_CAPACITY: usize = 4096;

impl CompetenceStore {
    fn record(&self, key: (u64, String), score: f64, now: std::time::Instant) {
        let Ok(mut facts) = self.facts.lock() else {
            return;
        };
        if facts.len() >= COMPETENCE_CAPACITY && !facts.contains_key(&key) {
            // Bound: drop the oldest entry (HashMap order is
            // arbitrary; correctness never depends on which).
            if let Some(oldest) = facts
                .iter()
                .min_by_key(|(_, fact)| fact.assessed_at)
                .map(|(key, _)| key.clone())
            {
                facts.remove(&oldest);
            }
        }
        facts.insert(
            key,
            CompetenceFact {
                score_bp: (score * 10_000.0).round() as u32,
                assessed_at: now,
            },
        );
    }

    fn below_floor(&self, key: &(u64, String), floor_bp: u32, now: std::time::Instant) -> bool {
        let Ok(facts) = self.facts.lock() else {
            return false;
        };
        facts.get(key).is_some_and(|fact| {
            now.duration_since(fact.assessed_at) <= COMPETENCE_FRESHNESS && fact.score_bp < floor_bp
        })
    }
}

pub(crate) struct DecisionPlane {
    service: Option<Arc<dyn DecisionService>>,
    #[cfg(feature = "neural")]
    apofasi: Option<Arc<super::ApofasiDecisionService>>,
    cache: Mutex<TurnDecisionCache>,
    flights: super::decision_flights::DecisionFlights,
    competence: CompetenceStore,
    /// The clock — real time in production; injectable in tests so
    /// TTL and freshness cases run without wall clocks.
    clock: Arc<dyn Fn() -> std::time::Instant + Send + Sync>,
}

impl DecisionPlane {
    pub(crate) fn new(service: Option<Arc<dyn DecisionService>>) -> Self {
        Self::with_clock(service, Arc::new(std::time::Instant::now))
    }

    /// Test seam: a controllable clock.
    pub(crate) fn with_clock(
        service: Option<Arc<dyn DecisionService>>,
        clock: Arc<dyn Fn() -> std::time::Instant + Send + Sync>,
    ) -> Self {
        Self {
            service,
            #[cfg(feature = "neural")]
            apofasi: None,
            cache: Mutex::new(TurnDecisionCache::new()),
            flights: Default::default(),
            competence: CompetenceStore::default(),
            clock,
        }
    }

    #[cfg(feature = "neural")]
    pub(crate) fn from_environment(required: bool) -> Result<Self, super::DecisionError> {
        let checkpoint = std::env::var_os("A3S_APOFASI_CHECKPOINT");
        if required && checkpoint.is_none() {
            return Err(super::DecisionError::Invalid(
                "apofasi routing requires A3S_APOFASI_CHECKPOINT".into(),
            ));
        }
        let mut plane = Self::dormant();
        if let Some(checkpoint) = checkpoint {
            let concurrency = std::env::var("A3S_APOFASI_DECISION_CONCURRENCY")
                .ok()
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(2);
            plane.apofasi = Some(Arc::new(super::ApofasiDecisionService::load(
                checkpoint,
                concurrency,
            )?));
        }
        Ok(plane)
    }

    #[cfg(not(feature = "neural"))]
    pub(crate) fn from_environment(required: bool) -> Result<Self, super::DecisionError> {
        if required {
            return Err(super::DecisionError::Invalid(
                "apofasi routing requires a gateway built with the neural feature".into(),
            ));
        }
        Ok(Self::dormant())
    }

    #[cfg(feature = "neural")]
    pub(crate) fn apofasi(&self) -> Option<Arc<dyn DecisionService>> {
        self.apofasi
            .as_ref()
            .map(|service| service.clone() as Arc<dyn DecisionService>)
    }

    #[cfg(not(feature = "neural"))]
    pub(crate) fn apofasi(&self) -> Option<Arc<dyn DecisionService>> {
        None
    }

    fn unavailable(
        &self,
        model: &InferenceModelConfig,
        plan: &InferenceRoutingPlan,
    ) -> (RouteDecision, FrozenDecision, bool) {
        let decision = evaluate_routing(model, Some(plan), None, true);
        let frozen = FrozenDecision {
            group: decision.group.clone(),
            reason: DecisionReason::DecisionUnreachable,
            frozen_at: (self.clock)(),
        };
        (decision, frozen, false)
    }

    /// The fail-closed default: no transport, every decision mode
    /// resolves primary (competence blocks risky cost-cutting).
    pub(crate) fn dormant() -> Self {
        Self::new(None)
    }

    /// Evaluates one turn against the model's plan: a frozen
    /// continuation reuses its facts; a new turn asks the service (or
    /// fails closed). Returns the frozen facts for the evidence lane.
    pub(crate) async fn decide(
        &self,
        model: &InferenceModelConfig,
        plan: &InferenceRoutingPlan,
        identity: &TurnIdentity,
        request: Option<&DecisionRequest>,
        transport: Option<std::sync::Arc<dyn DecisionService>>,
    ) -> (RouteDecision, FrozenDecision, bool) {
        // Bind both the frozen decision and in-flight work to this exact
        // published policy. A snapshot successor cannot reuse an older choice.
        use std::hash::Hasher;
        let policy = serde_json::to_vec(&(model, plan));
        let Ok(policy) = policy else {
            return self.unavailable(model, plan);
        };
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        hasher.write_u64(identity.model_alias_fingerprint);
        hasher.write(&policy);
        hasher.write_u8(u8::from(request.is_some()));
        let identity = TurnIdentity {
            model_alias_fingerprint: hasher.finish(),
            ..*identity
        };
        let Some(_flight) = self.flights.acquire(identity).await else {
            return self.unavailable(model, plan);
        };
        let identity = &identity;
        let now = (self.clock)();
        if let Ok(cache) = self.cache.lock() {
            if let Some(frozen) = cache.lookup(identity, now) {
                // A frozen continuation replays its group: the frozen
                // facts map back onto the inputs that produced them,
                // so the window recomputes identically.
                let inputs = replay_inputs(&frozen.group);
                return (
                    evaluate_routing(model, Some(plan), Some(&inputs), false),
                    frozen.clone(),
                    true,
                );
            }
        }
        // The dispatch layer builds the request (the turn's content
        // against the plan's rubric); a None request (no transport
        // context) is an unreachable decision — fail closed.
        // The configured service (the composition's double seam) wins;
        // otherwise the dispatch-provided HTTP transport serves the
        // call. Neither — fail closed.
        let service = self.service.clone().or(transport);
        let (decision, reason) = match (service, request) {
            (Some(service), Some(request)) => match service.decide(request).await {
                Ok(response) => {
                    let inputs = extract_inputs(&response, plan);
                    match inputs {
                        Ok(mut inputs) => {
                            // HR4: the competence overlay — a recorded
                            // FRESH below-floor assessment in this
                            // scope protects this decision, and a
                            // complete assessment in this answer
                            // records for the next one.
                            let scope = inputs.branch_choice.clone().unwrap_or_default();
                            let key = (identity.credential_fingerprint, scope);
                            if self
                                .competence
                                .below_floor(&key, plan.competence_floor_bp, now)
                            {
                                inputs.fresh_competence_below_floor = true;
                            }
                            if let Some(assessment) = &response.assessment {
                                if !assessment.partial {
                                    self.competence.record(key, assessment.score, now);
                                }
                            }
                            (
                                evaluate_routing(model, Some(plan), Some(&inputs), false),
                                None,
                            )
                        }
                        Err(_) => (
                            evaluate_routing(model, Some(plan), None, false),
                            Some(DecisionReason::DecisionInvalid),
                        ),
                    }
                }
                Err(_) => (
                    evaluate_routing(model, Some(plan), None, true),
                    Some(DecisionReason::DecisionUnreachable),
                ),
            },
            _ => (
                evaluate_routing(model, Some(plan), None, true),
                Some(DecisionReason::DecisionUnreachable),
            ),
        };
        let frozen = FrozenDecision {
            group: decision.group.clone(),
            reason: reason.unwrap_or(decision.reason),
            frozen_at: now,
        };
        if let Ok(mut cache) = self.cache.lock() {
            cache.freeze(*identity, frozen.clone(), now);
        }
        (decision, frozen, false)
    }
}

/// Reconstructs the decision inputs a frozen group implies — the
/// inverse of the group choice, so a replayed window equals the
/// frozen one.
fn replay_inputs(group: &crate::inference::decision_cache::ChosenGroup) -> DecisionInputs {
    use crate::inference::decision_cache::ChosenGroup;
    match group {
        ChosenGroup::Economy => DecisionInputs {
            simple_probability: Some(1.0),
            ..DecisionInputs::default()
        },
        ChosenGroup::Primary => DecisionInputs {
            simple_probability: Some(0.0),
            ..DecisionInputs::default()
        },
        ChosenGroup::BranchRegular { category } => DecisionInputs {
            branch_choice: Some(category.clone()),
            degree_simple_probability: Some(1.0),
            ..DecisionInputs::default()
        },
        ChosenGroup::BranchPrimary { category } => DecisionInputs {
            branch_choice: Some(category.clone()),
            degree_simple_probability: Some(0.0),
            ..DecisionInputs::default()
        },
    }
}

fn extract_inputs(
    response: &DecisionResponse,
    plan: &InferenceRoutingPlan,
) -> Result<DecisionInputs, crate::inference::decision::DecisionError> {
    use crate::config::InferenceRoutingMode;
    match plan.mode {
        InferenceRoutingMode::SmartSaving => crate::inference::decision::from_ordinal_simple(
            response,
            "simple",
            plan.competence_floor_bp,
        ),
        InferenceRoutingMode::Branches => {
            let ids: Vec<&str> = plan.branches.iter().map(|b| b.category.as_str()).collect();
            crate::inference::decision::from_categorical(
                response,
                &ids,
                "light",
                plan.competence_floor_bp,
            )
        }
        InferenceRoutingMode::Fixed | InferenceRoutingMode::FreeFirst => {
            Ok(DecisionInputs::default())
        }
    }
}
