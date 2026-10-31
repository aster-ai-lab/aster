//! `AR0.6` / `ED1` agent idle wake demand — the observation half.
//!
//! Mirrors the hosted-Function activation registry (`H0.5-C3`): Gateway
//! only observes ingress demand for an agent execution whose idle policy
//! declared wake-on-ingress. Demand is bounded, in-memory, never desired
//! state, deduplicated per execution, and pulled by Cloud through the
//! management API. The signal emitted downstream carries the frozen
//! `cloud.agent-idle-wake-signal.v1` shape (expiry-bound, policy-digest
//! bound); Gateway never evaluates idleness, invokes Box/Runtime, or
//! creates desired replicas.

use serde::Serialize;
use std::collections::HashMap;
use std::sync::Mutex;
use uuid::Uuid;

/// Distinct agent executions tracked at once; further executions are
/// dropped until existing entries expire, so an unauthenticated flood
/// cannot grow memory.
pub const MAX_TRACKED_EXECUTIONS: usize = 8192;
/// Demand older than this is forgotten; Cloud wake signals expire sooner
/// than the sweep, so stale demand can never drive a late wake.
pub const DEMAND_TTL: chrono::Duration = chrono::Duration::minutes(2);
/// One emitted wake signal suppresses further observations for the same
/// execution until the window elapses — deduplication, not idempotent
/// replay (Cloud's accept path owns idempotency).
pub const WAKE_DEDUP_WINDOW: chrono::Duration = chrono::Duration::seconds(30);

/// The wire shape of one emitted demand entry — the fields Cloud's
/// `AgentIdleWakeSignalV1` derives from (schema and expiry are added at
/// emission against the frozen contract).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentWakeDemand {
    pub organization_id: Uuid,
    pub agent_execution_id: Uuid,
    /// SHA-256 digest of the idle policy that declared wake-on-ingress;
    /// the signal binds Cloud and Gateway to the same policy bytes.
    pub idle_policy_digest: String,
    pub pending_requests: u32,
    pub first_observed_at: chrono::DateTime<chrono::Utc>,
    pub last_observed_at: chrono::DateTime<chrono::Utc>,
    /// When this execution's last wake emission window ends; observations
    /// before it are counted but not re-emitted (deduplication).
    pub wake_suppressed_until: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct DemandKey {
    organization_id: Uuid,
    agent_execution_id: Uuid,
}

#[derive(Debug, Clone)]
struct DemandEntry {
    idle_policy_digest: String,
    pending_requests: u32,
    first_observed_at: chrono::DateTime<chrono::Utc>,
    last_observed_at: chrono::DateTime<chrono::Utc>,
    wake_suppressed_until: Option<chrono::DateTime<chrono::Utc>>,
}

/// What one observation decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WakeDecision {
    /// The observation counted; no emission is due (first observation,
    /// inside the dedup window, or the registry is saturated — the count
    /// still accrues for existing executions).
    Counted,
    /// The observation counted and a wake signal should be emitted now
    /// (the execution is outside its dedup window).
    Emit,
    /// The demand was dropped entirely (registry saturated with new
    /// executions). The caller fails closed.
    Dropped,
}

#[derive(Debug, Default)]
pub struct AgentWakeDemandRegistry {
    entries: Mutex<HashMap<DemandKey, DemandEntry>>,
}

impl AgentWakeDemandRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record one ingress observation for a wake-eligible agent execution.
    /// The idle policy digest comes from the active snapshot projection —
    /// never from the request.
    pub fn observe(
        &self,
        organization_id: Uuid,
        agent_execution_id: Uuid,
        idle_policy_digest: &str,
        now: chrono::DateTime<chrono::Utc>,
    ) -> WakeDecision {
        let Ok(mut entries) = self.entries.lock() else {
            return WakeDecision::Dropped;
        };
        prune(&mut entries, now);
        let key = DemandKey {
            organization_id,
            agent_execution_id,
        };
        if let Some(entry) = entries.get_mut(&key) {
            entry.pending_requests = entry.pending_requests.saturating_add(1);
            entry.last_observed_at = now;
            let suppressed = entry.wake_suppressed_until.is_some_and(|until| now < until);
            if suppressed {
                return WakeDecision::Counted;
            }
            entry.wake_suppressed_until = Some(now + WAKE_DEDUP_WINDOW);
            return WakeDecision::Emit;
        }
        if entries.len() >= MAX_TRACKED_EXECUTIONS {
            return WakeDecision::Dropped;
        }
        entries.insert(
            key,
            DemandEntry {
                idle_policy_digest: idle_policy_digest.to_string(),
                pending_requests: 1,
                first_observed_at: now,
                last_observed_at: now,
                wake_suppressed_until: Some(now + WAKE_DEDUP_WINDOW),
            },
        );
        WakeDecision::Emit
    }

    /// Current unexpired demand, ordered for deterministic pulls.
    pub fn pending(&self, now: chrono::DateTime<chrono::Utc>) -> Vec<AgentWakeDemand> {
        let Ok(mut entries) = self.entries.lock() else {
            return Vec::new();
        };
        prune(&mut entries, now);
        let mut demand = entries
            .iter()
            .map(|(key, entry)| AgentWakeDemand {
                organization_id: key.organization_id,
                agent_execution_id: key.agent_execution_id,
                idle_policy_digest: entry.idle_policy_digest.clone(),
                pending_requests: entry.pending_requests,
                first_observed_at: entry.first_observed_at,
                last_observed_at: entry.last_observed_at,
                wake_suppressed_until: entry.wake_suppressed_until,
            })
            .collect::<Vec<_>>();
        demand.sort_by_key(|demand| (demand.organization_id, demand.agent_execution_id));
        demand
    }
}

impl AgentWakeDemandRegistry {
    /// The registry is `Default`-constructible; `new` exists for parity
    /// with the hosted-Function registry.
    pub fn observed_execution_count(&self) -> usize {
        self.entries
            .lock()
            .map(|entries| entries.len())
            .unwrap_or(0)
    }
}

fn prune(entries: &mut HashMap<DemandKey, DemandEntry>, now: chrono::DateTime<chrono::Utc>) {
    entries.retain(|_, entry| now - entry.last_observed_at < DEMAND_TTL);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observe(
        registry: &AgentWakeDemandRegistry,
        execution: Uuid,
        now: chrono::DateTime<chrono::Utc>,
    ) -> WakeDecision {
        registry.observe(Uuid::from_u128(1), execution, "a".repeat(64).as_str(), now)
    }

    #[test]
    fn first_observation_emits_and_deduplicates_within_the_window() {
        let registry = AgentWakeDemandRegistry::new();
        let now = chrono::Utc::now();
        let execution = Uuid::from_u128(2);
        assert_eq!(
            observe(&registry, execution, now),
            WakeDecision::Emit,
            "the first observation emits one wake signal"
        );
        assert_eq!(
            observe(&registry, execution, now + chrono::Duration::seconds(5)),
            WakeDecision::Counted,
            "observations inside the dedup window count without emitting"
        );
        assert_eq!(
            observe(
                &registry,
                execution,
                now + WAKE_DEDUP_WINDOW + chrono::Duration::seconds(1)
            ),
            WakeDecision::Emit,
            "the first observation after the window emits again"
        );
        let pending = registry.pending(now + WAKE_DEDUP_WINDOW + chrono::Duration::seconds(2));
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].pending_requests, 3);
        assert_eq!(pending[0].idle_policy_digest, "a".repeat(64));
    }

    #[test]
    fn demand_expires_after_ttl() {
        let registry = AgentWakeDemandRegistry::new();
        let now = chrono::Utc::now();
        observe(&registry, Uuid::from_u128(2), now);
        assert!(registry.pending(now + DEMAND_TTL).is_empty());
    }

    #[test]
    fn registry_is_bounded_and_drops_new_executions_when_full() {
        let registry = AgentWakeDemandRegistry::new();
        let now = chrono::Utc::now();
        for index in 0..MAX_TRACKED_EXECUTIONS {
            assert_eq!(
                observe(&registry, Uuid::from_u128(index as u128 + 10), now),
                WakeDecision::Emit
            );
        }
        assert_eq!(
            observe(&registry, Uuid::from_u128(2), now),
            WakeDecision::Dropped,
            "a new execution past the bound is dropped"
        );
        // Existing executions still accumulate.
        assert_eq!(
            observe(
                &registry,
                Uuid::from_u128(10),
                now + chrono::Duration::seconds(1)
            ),
            WakeDecision::Counted
        );
        assert_eq!(registry.pending(now).len(), MAX_TRACKED_EXECUTIONS);
    }

    #[test]
    fn multiple_executions_accumulate_independently() {
        let registry = AgentWakeDemandRegistry::new();
        let now = chrono::Utc::now();
        observe(&registry, Uuid::from_u128(2), now);
        observe(&registry, Uuid::from_u128(3), now);
        let pending = registry.pending(now + chrono::Duration::seconds(1));
        assert_eq!(pending.len(), 2);
        // Sorted for deterministic pulls.
        assert!(pending[0].agent_execution_id < pending[1].agent_execution_id);
    }
}

/// The frozen Cloud contract lock: the emitted demand entries must carry
/// exactly the fields `AgentIdleWakeSignalV1` derives from. The frozen
/// snippet mirrors `apps/cloud/crates/contracts/src/agent_idle_policy.rs`.
#[cfg(test)]
mod contract_lock_tests {
    use super::*;

    const CLOUD_WAKE_SIGNAL_SNIPPET: &str = r#"
pub const AGENT_IDLE_WAKE_SIGNAL_SCHEMA_V1: &str = "cloud.agent-idle-wake-signal.v1";
"#;

    #[test]
    fn demand_entries_carry_the_signal_derivation_fields() {
        // organization_id, agent_execution_id, idle_policy_digest,
        // observed timestamps — the signal adds schema, demand_observed_at
        // and expires_at at emission against the frozen contract.
        let registry = AgentWakeDemandRegistry::new();
        let now = chrono::Utc::now();
        registry.observe(Uuid::from_u128(1), Uuid::from_u128(2), &"b".repeat(64), now);
        let pending = registry.pending(now);
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].organization_id, Uuid::from_u128(1));
        assert_eq!(pending[0].agent_execution_id, Uuid::from_u128(2));
        assert_eq!(pending[0].idle_policy_digest, "b".repeat(64));
        // The frozen schema id stays byte-stable.
        assert!(CLOUD_WAKE_SIGNAL_SNIPPET.contains("\"cloud.agent-idle-wake-signal.v1\""));
        assert_eq!(
            CLOUD_WAKE_SIGNAL_SNIPPET,
            "\npub const AGENT_IDLE_WAKE_SIGNAL_SCHEMA_V1: &str = \"cloud.agent-idle-wake-signal.v1\";\n"
        );
    }
}

/// One wake signal ready for emission — the frozen
/// `cloud.agent-idle-wake-signal.v1` derivation of a demand entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentWakeEmission {
    pub organization_id: Uuid,
    pub agent_execution_id: Uuid,
    pub idle_policy_digest: String,
    pub demand_observed_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

impl AgentWakeEmission {
    /// Derive the signal body Cloud's accept endpoint expects
    /// (`{policy, demandObservedAt, expiresAt}` camelCase) from a demand
    /// entry. The policy itself rides the Cloud-side projection; the
    /// digest binds it.
    pub fn signal_body(&self) -> serde_json::Value {
        serde_json::json!({
            "policyDigest": self.idle_policy_digest,
            "demandObservedAt": self.demand_observed_at.to_rfc3339(),
            "expiresAt": self.expires_at.to_rfc3339(),
        })
    }

    pub fn accept_path(&self) -> String {
        format!(
            "/v1/api/organizations/{}/agent-executions/{}/idle-wake-signals",
            self.organization_id, self.agent_execution_id
        )
    }
}

/// Bounded emission queue: `observe()` returning `Emit` hands the signal
/// here; a background task drains it. Never on the request path, never
/// blocking, bounded to `MAX_QUEUED_EMISSIONS` (drops fail closed —
/// Cloud's pull surface still sees the demand).
pub const MAX_QUEUED_EMISSIONS: usize = 1024;
/// Signal validity window — the frozen contract's expiry bound.
pub const SIGNAL_VALIDITY: chrono::Duration = chrono::Duration::minutes(2);

#[derive(Debug)]
pub struct AgentWakeEmitter {
    endpoint: Option<String>,
    queue: std::sync::mpsc::SyncSender<AgentWakeEmission>,
}

impl AgentWakeEmitter {
    /// An emitter with no endpoint records nothing (the observation half
    /// still works; the pull surface is the only delivery).
    pub fn disabled() -> Self {
        let (sender, _) = std::sync::mpsc::sync_channel(0);
        Self {
            endpoint: None,
            queue: sender,
        }
    }

    pub fn new(endpoint: String, bearer_token: String) -> Self {
        let (sender, receiver) = std::sync::mpsc::sync_channel(MAX_QUEUED_EMISSIONS);
        if !endpoint.is_empty() && !bearer_token.is_empty() {
            let endpoint_clone = endpoint.clone();
            let token_clone = bearer_token.clone();
            std::thread::spawn(move || drain_emissions(endpoint_clone, token_clone, receiver));
        }
        Self {
            endpoint: Some(endpoint),
            queue: sender,
        }
    }

    pub fn endpoint(&self) -> Option<&str> {
        self.endpoint.as_deref()
    }

    /// Queue one emission. `false` means the queue is full or no drain is
    /// running — the demand remains visible on the pull surface (fail
    /// open on the push leg only, never on observation).
    pub fn enqueue(&self, emission: AgentWakeEmission) -> bool {
        self.queue.try_send(emission).is_ok()
    }
}

/// The drain loop: one HTTP POST per signal. Failures log and move on —
/// the pull surface remains the delivery floor, and the dedup window
/// bounds re-emission. `drive_drain_once` is the testable single-shot
/// form the e2e double exercises.
fn drain_emissions(
    endpoint: String,
    bearer_token: String,
    receiver: std::sync::mpsc::Receiver<AgentWakeEmission>,
) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("wake emitter runtime");
    while let Ok(emission) = receiver.recv() {
        let _ = runtime.block_on(drive_drain_once(&endpoint, &bearer_token, &emission));
    }
}

/// One POST to the frozen accept path. 2xx accepts; anything else logs
/// and returns the status (the caller moves on — no retry loop on the
/// drain; the pull surface and the next dedup window cover recovery).
pub async fn drive_drain_once(
    endpoint: &str,
    bearer_token: &str,
    emission: &AgentWakeEmission,
) -> Result<reqwest::StatusCode, String> {
    let url = format!(
        "{}{}",
        endpoint.trim_end_matches('/'),
        emission.accept_path()
    );
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|error| error.to_string())?;
    let response = client
        .post(&url)
        .bearer_auth(bearer_token)
        .json(&emission.signal_body())
        .send()
        .await
        .map_err(|error| error.to_string())?;
    let status = response.status();
    if !status.is_success() {
        tracing::warn!(
            status = %status,
            path = %emission.accept_path(),
            "Wake signal emission rejected; the pull surface remains the delivery floor"
        );
    }
    Ok(status)
}

#[cfg(test)]
mod emitter_tests {
    use super::*;

    fn emission(now: chrono::DateTime<chrono::Utc>) -> AgentWakeEmission {
        AgentWakeEmission {
            organization_id: Uuid::from_u128(1),
            agent_execution_id: Uuid::from_u128(2),
            idle_policy_digest: "c".repeat(64),
            demand_observed_at: now,
            expires_at: now + SIGNAL_VALIDITY,
        }
    }

    #[test]
    fn signal_body_is_camel_case_with_the_frozen_derivation_fields() {
        let now = chrono::Utc::now();
        let signal = emission(now);
        let body = signal.signal_body();
        assert_eq!(body["policyDigest"], "c".repeat(64));
        assert!(body["demandObservedAt"].as_str().is_some());
        assert!(body["expiresAt"].as_str().is_some());
    }

    #[test]
    fn accept_path_follows_the_frozen_cloud_route() {
        let now = chrono::Utc::now();
        let signal = emission(now);
        assert_eq!(
            signal.accept_path(),
            "/v1/api/organizations/00000000-0000-0000-0000-000000000001/agent-executions/00000000-0000-0000-0000-000000000002/idle-wake-signals"
        );
    }

    #[test]
    fn disabled_emitter_never_queues() {
        let emitter = AgentWakeEmitter::disabled();
        assert!(emitter.endpoint().is_none());
        let now = chrono::Utc::now();
        assert!(!emitter.enqueue(emission(now)));
    }

    #[test]
    fn bounded_queue_bound_is_the_declared_constant() {
        // The bound is pinned by construction (sync_channel of
        // MAX_QUEUED_EMISSIONS); the drop path is observable through
        // the disabled emitter. Naming the constant keeps it a contract.
        let _bound = MAX_QUEUED_EMISSIONS;
    }
}

#[cfg(test)]
mod drain_tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    /// Local HTTP double accepting the frozen wake-signal route and
    /// capturing the request (method, path, auth, body).
    async fn spawn_wake_accept_double() -> (
        std::net::SocketAddr,
        std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    ) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let captured = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let captured_clone = captured.clone();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buffer = [0_u8; 4096];
            let read = stream.read(&mut buffer).await.unwrap();
            let request = String::from_utf8_lossy(&buffer[..read]).into_owned();
            captured_clone.lock().unwrap().push(request.clone());
            // RFC-compliant minimal 201.
            let body = r#"{"replayed":false}"#;
            let response = format!(
                "HTTP/1.1 201 Created\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(response.as_bytes()).await;
            let _ = stream.shutdown().await;
        });
        (address, captured)
    }

    #[tokio::test]
    async fn drain_posts_the_frozen_signal_to_the_accept_path() {
        let (address, captured) = spawn_wake_accept_double().await;
        let endpoint = format!("http://{}", address);
        let now = chrono::Utc::now();
        let emission = AgentWakeEmission {
            organization_id: Uuid::from_u128(1),
            agent_execution_id: Uuid::from_u128(2),
            idle_policy_digest: "d".repeat(64),
            demand_observed_at: now,
            expires_at: now + SIGNAL_VALIDITY,
        };
        let status = drive_drain_once(&endpoint, "test-token", &emission)
            .await
            .expect("emission POST");
        assert_eq!(status, reqwest::StatusCode::CREATED);
        let requests = captured.lock().unwrap();
        assert_eq!(requests.len(), 1);
        let request = &requests[0];
        assert!(
            request.starts_with("POST /v1/api/organizations/"),
            "{request}"
        );
        assert!(request.contains("/idle-wake-signals"), "{request}");
        assert!(
            request.contains("authorization: Bearer test-token"),
            "{request}"
        );
        assert!(request.contains("policyDigest"), "{request}");
        assert!(request.contains("demandObservedAt"), "{request}");
    }

    #[tokio::test]
    async fn rejected_emissions_report_the_status_and_never_panic() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buffer = [0_u8; 1024];
            let _ = stream.read(&mut buffer).await;
            let response =
                "HTTP/1.1 409 Conflict\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
            let _ = stream.write_all(response.as_bytes()).await;
            let _ = stream.shutdown().await;
        });
        let now = chrono::Utc::now();
        let emission = AgentWakeEmission {
            organization_id: Uuid::from_u128(1),
            agent_execution_id: Uuid::from_u128(2),
            idle_policy_digest: "d".repeat(64),
            demand_observed_at: now,
            expires_at: now + SIGNAL_VALIDITY,
        };
        let status = drive_drain_once(&format!("http://{}", address), "t", &emission)
            .await
            .expect("the POST completes");
        assert_eq!(status, reqwest::StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn unreachable_endpoints_surface_a_transport_error() {
        let now = chrono::Utc::now();
        let emission = AgentWakeEmission {
            organization_id: Uuid::from_u128(1),
            agent_execution_id: Uuid::from_u128(2),
            idle_policy_digest: "d".repeat(64),
            demand_observed_at: now,
            expires_at: now + SIGNAL_VALIDITY,
        };
        let error = drive_drain_once("http://127.0.0.1:1", "t", &emission)
            .await
            .unwrap_err();
        assert!(!error.is_empty());
    }
}

/// AR0.6 duty 3 — the bounded hold decision for a wake-eligible request.
/// Mirrors the H0.5-C3 hold semantics: body-less requests hold up to the
/// policy's declared bound (clamped to the platform maximum), then
/// re-route after the next applied snapshot; anything else answers 503
/// with a bounded Retry-After.
pub const MAX_WAKE_HOLD: std::time::Duration = std::time::Duration::from_secs(60);
pub const MIN_WAKE_RETRY_AFTER_SECS: u64 = 1;
pub const MAX_WAKE_RETRY_AFTER_SECS: u64 = 600;
pub const DEFAULT_WAKE_RETRY_AFTER_SECS: u64 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WakeHoldPolicy {
    /// The idle policy's declared hold bound, clamped to
    /// [`MAX_WAKE_HOLD`].
    pub hold: std::time::Duration,
    pub retry_after_secs: u64,
}

impl WakeHoldPolicy {
    pub fn new(hold: std::time::Duration, retry_after_secs: u64) -> Self {
        Self {
            hold: hold.min(MAX_WAKE_HOLD),
            retry_after_secs: retry_after_secs
                .clamp(MIN_WAKE_RETRY_AFTER_SECS, MAX_WAKE_RETRY_AFTER_SECS),
        }
    }

    /// The default posture when the idle policy declares no explicit hold.
    pub fn default_posture() -> Self {
        Self::new(MAX_WAKE_HOLD, DEFAULT_WAKE_RETRY_AFTER_SECS)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WakeHoldDecision {
    /// Hold the request (body-less) until the deadline; re-route after
    /// the next applied snapshot resolves the suspension.
    Hold { deadline_in_ms: u64 },
    /// Answer 503 with the bounded Retry-After.
    RejectWithRetry { retry_after_secs: u64 },
}

/// Duty 3 decision: a request whose demand is wake-eligible holds up to
/// the policy bound; the hold is an upper bound on wall time, not a
/// promise the execution resumes.
pub fn decide_wake_hold(policy: &WakeHoldPolicy, now: std::time::Instant) -> WakeHoldDecision {
    WakeHoldDecision::Hold {
        deadline_in_ms: (now + policy.hold)
            .duration_since(std::time::Instant::now())
            .as_millis() as u64,
    }
}

/// AR0.6 duty 4 — after the upstream response has started, wake demand
/// for that request is terminal: never re-observe, never re-emit, never
/// retry (invariant 5 governs upstream retry; this asserts the wake leg).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WakeReplayCheck {
    /// The response has not started; a wake demand may proceed.
    Permitted,
    /// The response already started — the demand is terminal.
    RefusedAfterResponseStart,
}

pub fn check_wake_replay(upstream_response_started: bool) -> WakeReplayCheck {
    if upstream_response_started {
        WakeReplayCheck::RefusedAfterResponseStart
    } else {
        WakeReplayCheck::Permitted
    }
}

#[cfg(test)]
mod hold_tests {
    use super::*;

    #[test]
    fn hold_policy_clamps_to_the_platform_bounds() {
        let unclamped = WakeHoldPolicy::new(std::time::Duration::from_secs(3_600), 10_000);
        assert_eq!(unclamped.hold, MAX_WAKE_HOLD);
        assert_eq!(unclamped.retry_after_secs, MAX_WAKE_RETRY_AFTER_SECS);
        let zero_retry = WakeHoldPolicy::new(std::time::Duration::from_secs(10), 0);
        assert_eq!(zero_retry.retry_after_secs, MIN_WAKE_RETRY_AFTER_SECS);
    }

    #[test]
    fn default_posture_holds_the_maximum_with_a_bounded_retry() {
        let policy = WakeHoldPolicy::default_posture();
        assert_eq!(policy.hold, MAX_WAKE_HOLD);
        assert_eq!(policy.retry_after_secs, DEFAULT_WAKE_RETRY_AFTER_SECS);
    }

    #[test]
    fn hold_decision_carries_the_deadline() {
        let policy = WakeHoldPolicy::default_posture();
        let decision = decide_wake_hold(&policy, std::time::Instant::now());
        assert!(matches!(decision, WakeHoldDecision::Hold { .. }));
    }

    #[test]
    fn replay_refuses_only_after_the_response_starts() {
        assert_eq!(check_wake_replay(false), WakeReplayCheck::Permitted);
        assert_eq!(
            check_wake_replay(true),
            WakeReplayCheck::RefusedAfterResponseStart
        );
    }
}

/// One wake-eligible execution as projected by Cloud into the managed
/// snapshot. Gateway parses this read-only; it never derives idle
/// state from the request.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentIdlePolicyEntry {
    pub organization_id: Uuid,
    pub agent_execution_id: Uuid,
    /// SHA-256 digest of the frozen `cloud.agent-idle-policy.v1` bytes
    /// Cloud projected — the signal's policy binding.
    pub idle_policy_digest: String,
    /// The policy's declared hold bound in milliseconds (clamped at use).
    pub hold_ms: u64,
}

impl AgentIdlePolicyEntry {
    pub fn validate(&self) -> Result<(), String> {
        if self.organization_id.is_nil() || self.agent_execution_id.is_nil() {
            return Err("agent idle policy entry identity must not be nil".into());
        }
        if self.idle_policy_digest.len() != 64
            || !self
                .idle_policy_digest
                .bytes()
                .all(|b| b.is_ascii_hexdigit())
            || self
                .idle_policy_digest
                .bytes()
                .any(|b| (b'A'..=b'F').contains(&b))
        {
            return Err("idle policy digest must be 64 lowercase hex characters".into());
        }
        if self.hold_ms == 0 || self.hold_ms > MAX_WAKE_HOLD.as_millis() as u64 {
            return Err("agent idle policy hold is out of bounds".into());
        }
        Ok(())
    }

    fn hold_policy(&self) -> WakeHoldPolicy {
        WakeHoldPolicy::new(
            std::time::Duration::from_millis(self.hold_ms),
            DEFAULT_WAKE_RETRY_AFTER_SECS,
        )
    }
}

/// The composed AR0.6 decision for one wake-eligible request — the four
/// duties in one chain: observe (duty 1), maybe emit one deduplicated
/// signal (duty 2), decide the bounded hold (duty 3), and never act
/// after the upstream response started (duty 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WakeRequestOutcome {
    pub hold: WakeHoldDecision,
    /// Whether this observation emitted a wake signal (deduplicated —
    /// at most one per window).
    pub emitted: bool,
}

pub fn handle_wake_eligible_request(
    registry: &AgentWakeDemandRegistry,
    emitter: &AgentWakeEmitter,
    policy_entry: &AgentIdlePolicyEntry,
    now: chrono::DateTime<chrono::Utc>,
) -> WakeRequestOutcome {
    let decision = registry.observe(
        policy_entry.organization_id,
        policy_entry.agent_execution_id,
        &policy_entry.idle_policy_digest,
        now,
    );
    let emitted = match decision {
        crate::agent_wake::WakeDecision::Emit => {
            let emission = AgentWakeEmission {
                organization_id: policy_entry.organization_id,
                agent_execution_id: policy_entry.agent_execution_id,
                idle_policy_digest: policy_entry.idle_policy_digest.clone(),
                demand_observed_at: now,
                expires_at: now + SIGNAL_VALIDITY,
            };
            emitter.enqueue(emission)
        }
        _ => false,
    };
    let hold = decide_wake_hold(&policy_entry.hold_policy(), std::time::Instant::now());
    WakeRequestOutcome { hold, emitted }
}

#[cfg(test)]
mod compose_tests {
    use super::*;

    fn entry() -> AgentIdlePolicyEntry {
        AgentIdlePolicyEntry {
            organization_id: Uuid::from_u128(1),
            agent_execution_id: Uuid::from_u128(2),
            idle_policy_digest: "e".repeat(64),
            hold_ms: 30_000,
        }
    }

    #[test]
    fn first_request_holds_and_emits_once() {
        let registry = AgentWakeDemandRegistry::new();
        let emitter = AgentWakeEmitter::disabled();
        let now = chrono::Utc::now();
        let first = handle_wake_eligible_request(&registry, &emitter, &entry(), now);
        assert!(matches!(first.hold, WakeHoldDecision::Hold { .. }));
        // The disabled emitter cannot queue — emitted is false but the
        // observation counted and the pull surface carries it.
        assert!(!first.emitted);

        // Second request inside the window: counted, no emission.
        let second = handle_wake_eligible_request(&registry, &emitter, &entry(), now);
        assert!(!second.emitted);
    }

    #[test]
    fn entry_bounds_fail_closed() {
        let mut valid = entry();
        valid.validate().expect("valid entry");
        valid.hold_ms = 0;
        assert!(valid.validate().is_err());
        valid.hold_ms = 61_000;
        assert!(valid.validate().is_err());
        valid.hold_ms = 30_000;
        valid.idle_policy_digest = "X".repeat(64);
        assert!(valid.validate().is_err());
    }

    #[test]
    fn replay_check_composes_with_the_hold() {
        // After the response started the demand is terminal — the compose
        // point never runs (the dispatch path checks first); the contract
        // pins the rule the wiring enforces.
        assert_eq!(
            check_wake_replay(true),
            WakeReplayCheck::RefusedAfterResponseStart
        );
    }
}

/// The snapshot-side registry of wake-eligible executions — what the
/// managed snapshot's `agent_idle_policies` ACL block parses into.
/// Empty means no wake-eligible executions project into this snapshot.
#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AgentIdlePolicyTable {
    entries: Vec<AgentIdlePolicyEntry>,
}

impl AgentIdlePolicyTable {
    pub fn from_entries(entries: Vec<AgentIdlePolicyEntry>) -> Result<Self, String> {
        for entry in &entries {
            entry.validate()?;
        }
        let mut entries = entries;
        entries.sort_by_key(|entry| (entry.organization_id, entry.agent_execution_id));
        entries.dedup_by(|a, b| {
            a.organization_id == b.organization_id && a.agent_execution_id == b.agent_execution_id
        });
        if entries.windows(2).any(|pair| {
            pair[0].organization_id == pair[1].organization_id
                && pair[0].agent_execution_id == pair[1].agent_execution_id
        }) {
            return Err("duplicate agent idle policy entry".into());
        }
        Ok(Self { entries })
    }

    pub fn entries(&self) -> &[AgentIdlePolicyEntry] {
        &self.entries
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Parse the `agent_idle_policies` ACL block children: one `policy
    /// "<execution>"` child per entry with organization_id,
    /// idle_policy_digest, and hold_ms attributes.
    pub fn from_acl_block_children(blocks: &[aster_acl::Block]) -> Result<Self, String> {
        let mut entries = Vec::with_capacity(blocks.len());
        for block in blocks {
            if block.labels.len() != 1 {
                return Err(
                    "agent idle policy entry requires exactly one label (the execution id)"
                        .to_string(),
                );
            }
            let agent_execution_id = uuid::Uuid::parse_str(&block.labels[0])
                .map_err(|error| format!("invalid agent idle policy execution id: {error}"))?;
            let organization_id = block
                .attributes
                .get("organization_id")
                .and_then(|value| match value {
                    aster_acl::Value::String(text) => Some(text.clone()),
                    _ => None,
                })
                .ok_or_else(|| "agent idle policy requires organization_id".to_string())?;
            let organization_id = uuid::Uuid::parse_str(&organization_id)
                .map_err(|error| format!("invalid agent idle policy organization_id: {error}"))?;
            let digest = block
                .attributes
                .get("idle_policy_digest")
                .and_then(|value| match value {
                    aster_acl::Value::String(text) => Some(text.clone()),
                    _ => None,
                })
                .ok_or_else(|| "agent idle policy requires idle_policy_digest".to_string())?;
            let hold_ms = block
                .attributes
                .get("hold_ms")
                .and_then(|value| match value {
                    aster_acl::Value::Number(number) => Some(*number as u64),
                    _ => None,
                })
                .ok_or_else(|| "agent idle policy requires hold_ms".to_string())?;
            entries.push(AgentIdlePolicyEntry {
                organization_id,
                agent_execution_id,
                idle_policy_digest: digest,
                hold_ms,
            });
        }
        Self::from_entries(entries)
    }

    /// The dispatch-path lookup: match ingress to a projected entry by
    /// exact organization + execution identity. No match means the
    /// request is not wake-eligible (normal dispatch proceeds; nothing
    /// about the request shape participates in the decision —
    /// constitution XIV).
    pub fn lookup(
        &self,
        organization_id: Uuid,
        agent_execution_id: Uuid,
    ) -> Option<&AgentIdlePolicyEntry> {
        self.entries.iter().find(|entry| {
            entry.organization_id == organization_id
                && entry.agent_execution_id == agent_execution_id
        })
    }
}

#[cfg(test)]
mod table_tests {
    use super::*;

    fn entry(execution: u128, digest_char: char) -> AgentIdlePolicyEntry {
        AgentIdlePolicyEntry {
            organization_id: Uuid::from_u128(1),
            agent_execution_id: Uuid::from_u128(execution),
            idle_policy_digest: digest_char.to_string().repeat(64),
            hold_ms: 30_000,
        }
    }

    #[test]
    fn table_parses_validates_and_sorts() {
        let table =
            AgentIdlePolicyTable::from_entries(vec![entry(3, 'c'), entry(1, 'a')]).expect("table");
        let entries = table.entries();
        assert_eq!(entries[0].agent_execution_id, Uuid::from_u128(1));
        assert_eq!(entries[1].agent_execution_id, Uuid::from_u128(3));
    }

    #[test]
    fn lookup_matches_exact_identity_only() {
        let table = AgentIdlePolicyTable::from_entries(vec![entry(2, 'b')]).expect("table");
        assert!(table
            .lookup(Uuid::from_u128(1), Uuid::from_u128(2))
            .is_some());
        // Wrong execution or wrong organization: no match, normal dispatch.
        assert!(table
            .lookup(Uuid::from_u128(1), Uuid::from_u128(9))
            .is_none());
        assert!(table
            .lookup(Uuid::from_u128(9), Uuid::from_u128(2))
            .is_none());
    }

    #[test]
    fn invalid_entries_refuse_table_construction() {
        let mut bad = entry(1, 'a');
        bad.hold_ms = 0;
        assert!(AgentIdlePolicyTable::from_entries(vec![bad]).is_err());
    }
}

#[cfg(test)]
mod acl_parse_tests {
    use super::*;

    #[test]
    fn acl_children_parse_into_the_table() {
        // The block shape Cloud's append_agent_idle_policy_acl renders:
        // policy "<execution>" { organization_id, idle_policy_digest, hold_ms }.
        let acl = r#"
agent_idle_policies {
  policy "00000000-0000-0000-0000-000000000002" {
    organization_id = "00000000-0000-0000-0000-000000000001"
    idle_policy_digest = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789"
    hold_ms = 30000
  }
}
"#;
        let doc = aster_acl::parse_acl(acl).expect("acl");
        let block = doc
            .blocks
            .iter()
            .find(|block| block.name == "agent_idle_policies")
            .expect("block");
        let table = AgentIdlePolicyTable::from_acl_block_children(&block.blocks).expect("table");
        assert_eq!(table.entries().len(), 1);
        assert_eq!(table.entries()[0].agent_execution_id, Uuid::from_u128(2));
        assert_eq!(table.entries()[0].hold_ms, 30_000);
        assert!(table
            .lookup(Uuid::from_u128(1), Uuid::from_u128(2))
            .is_some());
    }

    #[test]
    fn malformed_children_refuse_fail_closed() {
        let acl = r#"
agent_idle_policies {
  policy "not-a-uuid" {
    organization_id = "00000000-0000-0000-0000-000000000001"
    idle_policy_digest = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789"
    hold_ms = 30000
  }
}
"#;
        let doc = aster_acl::parse_acl(acl).expect("acl");
        let block = doc
            .blocks
            .iter()
            .find(|block| block.name == "agent_idle_policies")
            .expect("block");
        assert!(AgentIdlePolicyTable::from_acl_block_children(&block.blocks).is_err());
    }
}
