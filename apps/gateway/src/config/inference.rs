//! Cloud-managed inference policy configuration.

mod validation;

#[cfg(test)]
mod tests;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

/// Audience accepted by the native inference data plane.
pub const INFERENCE_CREDENTIAL_AUDIENCE: &str = "cloud-inference";

/// Power worker observation schema accepted by this Gateway revision.
pub const POWER_WORKER_OBSERVATION_SCHEMA: &str = "a3s.power.worker-observation.v1";

/// Frozen provisional tokenizer revision this Gateway accepts on managed policy.
///
/// Cloud's policy compiler must emit this exact string. It identifies the local
/// grant-reservation estimator (`a3s.gateway.tokenizer.v1`), not a billing
/// tokenizer.
pub const INFERENCE_TOKENIZER_REVISION: &str = "a3s.gateway.tokenizer.v1";

/// Complete, expiring inference policy projected by A3S Cloud.
/// One named guardrails policy: an inline a3s-sentry ACL source. The
/// gateway compiles it into a wire gate at activation; bodies over
/// grants referencing this policy pass the gate before dispatch and the
/// buffered response passes it again before release.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InferenceGuardrailsConfig {
    /// Inline a3s-sentry ACL source (empty is the rules-only posture:
    /// masking with fail-open escalation).
    pub sentry_acl: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InferenceConfig {
    /// Exclusive end of the policy validity window.
    pub expires_at: DateTime<Utc>,
    /// Tokenizer revision the policy was compiled against.
    ///
    /// Must equal [`INFERENCE_TOKENIZER_REVISION`]. Fail-closed on mismatch so
    /// Cloud cannot silently change reservation semantics under Edge.
    pub tokenizer_revision: String,
    /// Credential verifier projections keyed by stable credential ID.
    #[serde(default)]
    pub credentials: HashMap<Uuid, InferenceCredentialConfig>,
    /// Inference routes keyed by stable route ID.
    #[serde(default)]
    pub routes: HashMap<Uuid, InferenceRouteConfig>,
    /// Named guardrails policies (inline a3s-sentry ACL sources) that
    /// grants reference by name. Gated on the `wire` cargo feature;
    /// without it, parsing a guardrails block fails closed.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub guardrails: HashMap<String, InferenceGuardrailsConfig>,
    /// Cloud-certified Power observations keyed by exact Runtime unit ID.
    #[serde(default)]
    pub workers: HashMap<String, InferenceWorkerConfig>,
}

/// One inference-key verifier projection.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InferenceCredentialConfig {
    /// Stable Identity-owned credential ID.
    pub credential_id: Uuid,
    /// Environment that owns the credential.
    pub environment_id: Uuid,
    /// Credential audience. Must be `cloud-inference`.
    pub audience: String,
    /// Stable, non-secret lookup prefix.
    pub prefix: String,
    /// Memory-hard Argon2id PHC verifier.
    ///
    /// This value is deliberately omitted from every serialized configuration
    /// view so local configuration output cannot expose it.
    #[serde(skip_serializing)]
    pub(crate) verifier_hash: String,
    /// Positive issuance generation.
    pub generation: u64,
    /// Credential expiry evaluated by the request authorization stage.
    pub expires_at: DateTime<Utc>,
    /// Explicit revocation state.
    pub revoked: bool,
}

impl InferenceCredentialConfig {
    pub(crate) fn verifier_hash(&self) -> &str {
        &self.verifier_hash
    }
}

impl fmt::Debug for InferenceCredentialConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InferenceCredentialConfig")
            .field("credential_id", &self.credential_id)
            .field("environment_id", &self.environment_id)
            .field("audience", &self.audience)
            .field("prefix", &self.prefix)
            .field("verifier_hash", &"<redacted>")
            .field("generation", &self.generation)
            .field("expires_at", &self.expires_at)
            .field("revoked", &self.revoked)
            .finish()
    }
}

/// One environment-scoped inference route bound to an ordinary HTTP router.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InferenceRouteConfig {
    /// Stable Inference-owned route ID.
    pub route_id: Uuid,
    /// Existing Gateway router that establishes hostname and path ownership.
    pub router: String,
    /// Environment that owns the route and all of its grants.
    pub environment_id: Uuid,
    /// Positive immutable access-policy revision.
    pub policy_revision: u64,
    /// External model aliases available on this route.
    #[serde(default)]
    pub models: HashMap<String, InferenceModelConfig>,
    /// Credential grants keyed by stable credential ID.
    #[serde(default)]
    pub grants: HashMap<Uuid, InferenceGrantConfig>,
    /// Tool listings keyed by SKU. Empty keeps `/v1/tools/{sku}/invoke` denied.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub listings: HashMap<String, InferenceListingConfig>,
    /// Client-facing response shape.
    #[serde(default, skip_serializing_if = "InferenceWire::is_envelope")]
    pub wire: InferenceWire,
    /// Signed-in user access verified by the OS auth service instead of
    /// Cloud-issued inference keys.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub principal: Option<InferencePrincipalConfig>,
    /// Optional supplier for `POST /v1/ocr`. Absent means the platform OCR model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ocr_supplier: Option<InferenceTargetConfig>,
    /// Optional supplier for `POST /v1/systemone`. Absent means platform Apofasi.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision_supplier: Option<InferenceTargetConfig>,
    /// Optional supplier for platform-alias embeddings. Absent means the
    /// local bge-m3 engine.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embedding_supplier: Option<InferenceTargetConfig>,
    /// Optional supplier for platform-alias rerank. Absent means the local
    /// bge-reranker-v2-m3 engine.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rerank_supplier: Option<InferenceTargetConfig>,
    /// Console-managed catalog source; mutually exclusive with `models`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub console_catalog: Option<ConsoleCatalogConfig>,
    /// GE0: declarative inference-engine deployment profiles carried on
    /// the snapshot (skip-when-empty — pre-GE0 wires stay stable).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub engine_profiles: Vec<EngineProfileConfig>,
}

/// Client-facing response shape of one inference route.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InferenceWire {
    /// A3S work envelope (`{success, code, data, ...}`).
    #[default]
    Envelope,
    /// Native OpenAI / Anthropic wire bodies, as SDKs expect.
    Openai,
}

impl InferenceWire {
    pub const fn is_envelope(&self) -> bool {
        matches!(self, Self::Envelope)
    }
}

impl FromStr for InferenceWire {
    type Err = String;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        match value {
            "envelope" => Ok(Self::Envelope),
            "openai" => Ok(Self::Openai),
            other => Err(format!(
                "unknown inference wire '{other}'; expected envelope or openai"
            )),
        }
    }
}

/// Session or personal-key access for signed-in OS users.
///
/// Gateway forwards the caller credential to `verify_url` (a forward-auth
/// style endpoint) and trusts the subject header of a 2xx answer. Successful
/// verifications are cached by credential digest for `cache_ttl_secs`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InferencePrincipalConfig {
    /// Absolute http(s) URL answering 2xx for an accepted credential.
    pub verify_url: String,
    /// Response header carrying the stable subject ID.
    pub subject_header: String,
    /// Optional response header carrying the subject's organization.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub org_header: Option<String>,
    /// Positive verification cache lifetime.
    pub cache_ttl_secs: u64,
    /// Model aliases visible to every principal; `*` grants all route models.
    pub models: Vec<String>,
    /// Closed protocol endpoints allowed for principals.
    pub endpoints: Vec<InferenceEndpoint>,
    /// Limits applied independently to each subject.
    pub limits: InferenceLimitsConfig,
}

impl InferencePrincipalConfig {
    pub(crate) fn allows_model(&self, alias: &str) -> bool {
        self.models
            .iter()
            .any(|model| model == "*" || model == alias)
    }
}

/// Upstream wire protocol spoken by one inference target.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InferenceProtocol {
    /// OpenAI-compatible `/v1/chat/completions`, `/v1/embeddings`, ...
    #[default]
    Openai,
    /// Anthropic Messages `/v1/messages`.
    Anthropic,
    /// Image OCR. The wire name stays `ocr`.
    #[serde(rename = "ocr")]
    Ocr,
    /// System One decisions. The wire name stays `systemone`.
    #[serde(rename = "systemone")]
    SystemOne,
}

impl InferenceProtocol {
    pub const fn is_openai(&self) -> bool {
        matches!(self, Self::Openai)
    }

    /// Header value for an upstream secret. Anthropic sends the raw key.
    /// Every other protocol sends a bearer token.
    pub(crate) fn authorization_value(self, secret: &str) -> String {
        match self {
            Self::Anthropic => secret.trim().to_string(),
            Self::Openai | Self::Ocr | Self::SystemOne => format!("Bearer {}", secret.trim()),
        }
    }
}

impl FromStr for InferenceProtocol {
    type Err = String;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        match value {
            "openai" => Ok(Self::Openai),
            "anthropic" => Ok(Self::Anthropic),
            "ocr" => Ok(Self::Ocr),
            "systemone" => Ok(Self::SystemOne),
            other => Err(format!(
                "unknown inference protocol '{other}'; expected openai, anthropic, ocr, or systemone"
            )),
        }
    }
}

/// One snapshot-published tool listing invoked at `/v1/tools/{sku}/invoke`.
///
/// Gateway forwards to `service` and records `listing_id` as the usage model id.
/// It does not implement the upstream adapter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InferenceListingConfig {
    /// Stable listing identity stored in the existing usage `model_id` field.
    pub listing_id: Uuid,
    /// Existing Gateway service that owns the upstream servers.
    pub service: String,
}

/// One externally visible model alias and its ordered target groups.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InferenceModelConfig {
    /// Stable logical model identity retained for usage correlation.
    pub model_id: Uuid,
    /// Targets ordered by ascending priority, then weighted within a priority.
    pub targets: Vec<InferenceTargetConfig>,
    /// Declared failure-transfer policy; console-sourced catalogs carry it
    /// from the offering, ACL-declared models default to pass-through.
    #[serde(default)]
    pub failover: FailoverPolicy,
    /// Optional per-model serving price projected from Cloud. A grant with a
    /// spend budget may only cover priced models.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pricing: Option<InferenceModelPricing>,
    /// Opt-in latency bias inside priority groups: warm first-response
    /// EWMA observations adjust (never replace) the configured weights,
    /// clamped to half/double. A bounded per-snapshot observation in the
    /// sense of invariant 11.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub latency_bias: bool,
    /// Opt-in private prefix locality across authorized targets, bounded by
    /// local outstanding-request pressure. This is a routing hint, not a KV
    /// residency claim or permission to share prompts between credentials.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub prefix_affinity: bool,
    /// Operator-declared slots per replica; estimates never grant admission.
    #[serde(default = "default_prefix_parallelism")]
    pub prefix_parallelism: u32,
    /// Optional closed pool-defense and endpoint-scheduling policy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scheduling: Option<InferenceSchedulingConfig>,
    /// HR1: optional task-aware routing plan (the HiRoute alignment).
    /// Absent = mode `fixed`, exactly today's behavior — the snapshot
    /// digest stays stable for existing models. Cloud-projected only;
    /// never hand-authored in cloud mode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub routing: Option<InferenceRoutingPlan>,
    /// GE1: the route engine profile this model deploys through.
    /// Absent = an externally-managed target (today's behavior).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine_profile: Option<String>,
}

/// GE0: one declarative inference-engine deployment profile (the
/// GPU-platform plan) — a digest-pinned engine image with its launch
/// template and accelerator shape. The dispatch consumption (GE1)
/// turns a profile into a serving workload; the config layer carries
/// and validates it fail-closed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineProfileConfig {
    pub name: String,
    pub engine: String,
    pub image: String,
    pub args_template: Vec<String>,
    pub accelerator: EngineAcceleratorShapeConfig,
    pub health_path: String,
    pub port: u16,
    pub environment_keys: Vec<String>,
}

/// GE0: the accelerator claim shape a profile demands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineAcceleratorShapeConfig {
    pub count: u32,
    pub vram_mib: u32,
    pub kind: String,
}

impl EngineProfileConfig {
    /// Fail-closed validation — the same rules the Cloud contract
    /// enforces at publish (digest-pinned oci://, bounded printable
    /// identities, positive port, accelerator ranges).
    pub fn validate(&self) -> Result<(), String> {
        if self.name.is_empty()
            || self.name.len() > 128
            || !self.name.bytes().all(|b| b.is_ascii_graphic())
        {
            return Err("engine profile name is invalid".into());
        }
        if self.engine.is_empty() || self.engine.len() > 64 {
            return Err("engine profile engine identity is invalid".into());
        }
        if !self.image.starts_with("oci://") || !self.image.contains('@') {
            return Err("engine profile image must be a digest-pinned oci:// reference".into());
        }
        if self.args_template.len() > 64 || self.args_template.iter().any(|a| a.len() > 1024) {
            return Err("engine profile args template is unbounded".into());
        }
        if self.port == 0 {
            return Err("engine profile port is invalid".into());
        }
        if self.accelerator.count == 0
            || self.accelerator.count > 1024
            || self.accelerator.vram_mib == 0
        {
            return Err("engine profile accelerator shape is out of range".into());
        }
        if self.accelerator.kind.is_empty()
            || self.accelerator.kind.len() > 32
            || !self
                .accelerator
                .kind
                .bytes()
                .all(|b| b.is_ascii_graphic() || b == b'-')
        {
            return Err("engine profile accelerator kind is invalid".into());
        }
        if self.environment_keys.len() > 32 {
            return Err("engine profile environment keys are unbounded".into());
        }
        Ok(())
    }
}

/// HR1: how a model's candidate groups are chosen for a turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum InferenceRoutingMode {
    /// Today's behavior: the declared target order, no decision.
    Fixed,
    /// Economy group when the turn's simple-task probability meets the
    /// threshold and no fresh assessment falls below the competence
    /// floor; primary otherwise. Competence blocks risky cost-cutting.
    SmartSaving,
    /// Category first (the decision's categorical choice), then
    /// optional degree within the category; per-branch regular and
    /// optional primary groups.
    Branches,
    /// Prefer unpriced (free) targets first; priced targets follow in
    /// declared order.
    FreeFirst,
}

/// HR1: one task category's candidate groups.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InferenceRoutingBranch {
    /// The categorical option id the decision service may choose.
    pub category: String,
    /// The branch's regular (economy) candidate target ids, ordered.
    pub regular_targets: Vec<uuid::Uuid>,
    /// Optional primary candidate target ids for the category;
    /// absent = no upgrade path within the branch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary_targets: Option<Vec<uuid::Uuid>>,
}

/// Explicit wire contract of the configured decision-model target.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InferenceDecisionProtocol {
    #[default]
    Decisions,
    OpenaiChat,
    Apofasi,
}
impl InferenceDecisionProtocol {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "decisions" => Ok(Self::Decisions),
            "openai_chat" => Ok(Self::OpenaiChat),
            "apofasi" => Ok(Self::Apofasi),
            _ => Err(format!(
                "unknown decision protocol '{value}'; expected decisions, openai_chat, or apofasi"
            )),
        }
    }
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Decisions => "decisions",
            Self::OpenaiChat => "openai_chat",
            Self::Apofasi => "apofasi",
        }
    }
    pub const fn is_default(&self) -> bool {
        matches!(self, Self::Decisions)
    }
}

/// HR1: the task-aware routing plan over the model's declared targets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InferenceRoutingPlan {
    pub mode: InferenceRoutingMode,
    /// The model alias serving decisions (HR0's contract); required
    /// for `smart_saving` and `branches`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision_model: Option<String>,
    #[serde(default, skip_serializing_if = "InferenceDecisionProtocol::is_default")]
    pub decision_protocol: InferenceDecisionProtocol,
    /// Economy candidate target ids (smart saving), ordered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub economy_targets: Option<Vec<uuid::Uuid>>,
    /// Primary candidate target ids; absent = the model's full
    /// declared target order (fail-closed upgrade ceiling).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary_targets: Option<Vec<uuid::Uuid>>,
    /// Simple-task probability threshold in basis points (HiRoute
    /// default 8000 = 0.80).
    #[serde(default = "default_simple_probability_threshold")]
    pub simple_probability_threshold_bp: u32,
    /// Competence floor in basis points (HiRoute default 5000 = 0.50;
    /// floors below 5000 refuse — competence blocks cost-cutting).
    #[serde(default = "default_competence_floor")]
    pub competence_floor_bp: u32,
    /// Category branches (mode `branches`), 2 to 16.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub branches: Vec<InferenceRoutingBranch>,
}

const fn default_simple_probability_threshold() -> u32 {
    8000
}

const fn default_competence_floor() -> u32 {
    5000
}

impl InferenceRoutingPlan {
    /// HR3c: whether this mode consults a decision service at all.
    pub fn needs_decision(&self) -> bool {
        matches!(
            self.mode,
            InferenceRoutingMode::SmartSaving | InferenceRoutingMode::Branches
        )
    }
}

impl InferenceRoutingMode {
    /// HR5: the wire name for evidence lanes (snake_case, the serde
    /// naming).
    pub fn wire_name(self) -> &'static str {
        match self {
            Self::Fixed => "fixed",
            Self::SmartSaving => "smart_saving",
            Self::Branches => "branches",
            Self::FreeFirst => "free_first",
        }
    }
}

impl InferenceRoutingPlan {
    /// Fail-closed validation against the model's declared targets:
    /// groups may only name declared target ids (never add one —
    /// invariant 4), thresholds live in (0, 1], and the decision model
    /// is required exactly when a decision is needed.
    pub fn validate(&self, declared_target_ids: &[uuid::Uuid]) -> Result<(), String> {
        let contains = |id: &uuid::Uuid, group: &str| {
            if !declared_target_ids.contains(id) {
                return Err(format!("routing plan {group} names undeclared target {id}"));
            }
            Ok(())
        };
        let check_group = |ids: &Option<Vec<uuid::Uuid>>, group: &str| -> Result<(), String> {
            if let Some(list) = ids {
                if list.is_empty() {
                    return Err(format!("routing plan {group} is empty"));
                }
                for id in list {
                    contains(id, group)?;
                }
            }
            Ok(())
        };
        if self.simple_probability_threshold_bp == 0
            || self.simple_probability_threshold_bp > 10_000
        {
            return Err("routing simple-probability threshold is out of range".into());
        }
        if self.competence_floor_bp < 5_000 || self.competence_floor_bp > 10_000 {
            return Err("routing competence floor is out of range".into());
        }
        match self.mode {
            InferenceRoutingMode::Fixed => Ok(()),
            InferenceRoutingMode::FreeFirst => Ok(()),
            InferenceRoutingMode::SmartSaving => {
                if self
                    .decision_model
                    .as_deref()
                    .unwrap_or("")
                    .trim()
                    .is_empty()
                {
                    return Err("smart-saving routing requires a decision model".into());
                }
                check_group(&self.economy_targets, "economy group")?;
                check_group(&self.primary_targets, "primary group")
            }
            InferenceRoutingMode::Branches => {
                if self
                    .decision_model
                    .as_deref()
                    .unwrap_or("")
                    .trim()
                    .is_empty()
                {
                    return Err("branch routing requires a decision model".into());
                }
                if self.branches.len() < 2 || self.branches.len() > 16 {
                    return Err("branch routing needs 2 to 16 categories".into());
                }
                let mut seen = std::collections::BTreeSet::new();
                for branch in &self.branches {
                    if !seen.insert(branch.category.as_str()) {
                        return Err(format!(
                            "branch category {} is declared twice",
                            branch.category
                        ));
                    }
                    check_group(
                        &Some(branch.regular_targets.clone()),
                        "branch regular group",
                    )?;
                    check_group(&branch.primary_targets, "branch primary group")?;
                }
                Ok(())
            }
        }
    }
}

/// Execution role required from a Power endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InferencePhaseRole {
    Aggregated,
    Prefill,
    Decode,
}

impl InferencePhaseRole {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Aggregated => "aggregated",
            Self::Prefill => "prefill",
            Self::Decode => "decode",
        }
    }
}

impl fmt::Display for InferencePhaseRole {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for InferencePhaseRole {
    type Err = String;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        match value {
            "aggregated" => Ok(Self::Aggregated),
            "prefill" => Ok(Self::Prefill),
            "decode" => Ok(Self::Decode),
            other => Err(format!(
                "unknown inference phase '{other}'; expected aggregated, prefill, or decode"
            )),
        }
    }
}

/// Power's model-neutral state-transfer health projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InferenceTransferHealth {
    Unsupported,
    Ready,
    Degraded,
    Unavailable,
}

impl InferenceTransferHealth {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unsupported => "unsupported",
            Self::Ready => "ready",
            Self::Degraded => "degraded",
            Self::Unavailable => "unavailable",
        }
    }
}

impl fmt::Display for InferenceTransferHealth {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for InferenceTransferHealth {
    type Err = String;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        match value {
            "unsupported" => Ok(Self::Unsupported),
            "ready" => Ok(Self::Ready),
            "degraded" => Ok(Self::Degraded),
            "unavailable" => Ok(Self::Unavailable),
            other => Err(format!(
                "unknown inference transfer health '{other}'; expected unsupported, ready, degraded, or unavailable"
            )),
        }
    }
}

/// One complete Cloud-bound projection of a Power worker observation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InferenceWorkerConfig {
    /// Exact managed endpoint identity certified by Cloud.
    pub target: crate::config::ManagedTargetConfig,
    pub schema: String,
    pub worker_epoch: Uuid,
    /// SHA-256 of the immutable Power execution profile bound to this epoch.
    ///
    /// Aggregated scheduling does not call Power's internal distributed API,
    /// so older aggregated projections may omit this field. Prefill/decode
    /// scheduling requires an exact lowercase digest for every selected
    /// worker and sends it back to Power on every phase operation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_profile_sha256: Option<String>,
    pub observation_generation: u64,
    pub observed_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub phases: Vec<InferencePhaseRole>,
    pub prompt_cache_capable: bool,
    pub state_transfer_capable: bool,
    pub ready_phases: Vec<InferencePhaseRole>,
    pub active_limit: Option<u64>,
    pub active: u64,
    pub waiting: u64,
    pub prompt_cache_supported: bool,
    pub prompt_cache_entries: u64,
    pub prompt_cache_capacity: u64,
    pub prompt_cache_pressure_basis_points: u16,
    pub transfer_health: InferenceTransferHealth,
    /// Optional Cloud-certified latency signal; never sampled by Gateway.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub certified_latency_ms: Option<u64>,
}

/// Pool-level request defense and worker selection policy for one model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InferenceSchedulingConfig {
    pub phase: InferencePhaseRole,
    pub max_concurrent_requests: u64,
    pub max_queued_requests: u64,
    pub queue_timeout_ms: u64,
    pub prompt_cache_affinity: bool,
    /// Closed runtime settings used only by Gateway-owned prefill/decode
    /// orchestration. The secret itself is never accepted in ACL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distributed_serving: Option<InferenceDistributedServingConfig>,
}

/// Infrastructure settings for the authenticated Gateway-to-Power protocol.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InferenceDistributedServingConfig {
    /// Environment variable containing the Power internal API key.
    pub api_key_env: String,
    /// End-to-end deadline for decode prepare, prefill, transfer, and decode.
    pub execution_timeout_ms: u64,
}

/// One local target selected by native model dispatch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InferenceTargetConfig {
    /// Stable target identity retained for attempt correlation.
    pub target_id: Uuid,
    /// Existing Gateway service containing the complete allowed endpoint set.
    pub service: String,
    /// Model identifier forwarded to this target.
    pub upstream_model: String,
    /// Zero-based fallback group. Lower priorities are attempted first.
    pub priority: u32,
    /// Positive selection weight within one priority group.
    pub weight: u32,
    /// Wire protocol the upstream speaks.
    #[serde(default, skip_serializing_if = "InferenceProtocol::is_openai")]
    pub protocol: InferenceProtocol,
    /// Environment variable holding the upstream provider key. When set,
    /// Gateway injects it in the protocol's auth header and rewrites `Host`
    /// to the backend authority. The secret itself is never accepted in ACL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_env: Option<String>,
    /// Explicit opt-in to cross-protocol translation: a target whose
    /// protocol differs from the endpoint dialect stays selectable and each
    /// attempt translates the request and the non-streaming response
    /// through the frozen corpus translator. Streaming requests over a
    /// cross-protocol target refuse until the SSE lane ships.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub translate: bool,
}

/// One credential's exact grant on an inference route.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InferenceGrantConfig {
    /// Credential generation accepted by this immutable policy revision.
    pub credential_generation: u64,
    /// External model aliases visible to and invokable by the credential.
    pub models: Vec<String>,
    /// Closed protocol endpoints allowed by the credential.
    pub endpoints: Vec<InferenceEndpoint>,
    /// Named guardrails policy applied to this grant's inference bodies
    /// (pre-call mask/block on the request, inspect-before-release on the
    /// buffered response). The named policy must exist in the inference
    /// block's guardrails registry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guardrails: Option<String>,
    /// Tool SKUs this credential may invoke. Empty denies every tool path.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub listings: Vec<String>,
    /// Explicit local enforcement limits.
    pub limits: InferenceLimitsConfig,
}

/// Closed native inference endpoint identifiers.
///
/// `ocr` and `systemone` keep those exact wire strings. Kebab-case would turn
/// the decision endpoint into `system-one`, which the usage ledger rejects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InferenceEndpoint {
    Models,
    ChatCompletions,
    Completions,
    Embeddings,
    Tools,
    /// Anthropic Messages, including `count_tokens`.
    Messages,
    #[serde(rename = "ocr")]
    Ocr,
    #[serde(rename = "systemone")]
    SystemOne,
    #[serde(rename = "rerank")]
    Rerank,
    /// OpenAI Responses API (`/v1/responses`).
    #[serde(rename = "responses")]
    Responses,
}

impl InferenceEndpoint {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Models => "models",
            Self::ChatCompletions => "chat-completions",
            Self::Completions => "completions",
            Self::Embeddings => "embeddings",
            Self::Tools => "tools",
            Self::Messages => "messages",
            Self::Ocr => "ocr",
            Self::SystemOne => "systemone",
            Self::Rerank => "rerank",
            Self::Responses => "responses",
        }
    }

    /// Upstream protocol a target must speak to serve this endpoint.
    pub const fn protocol(self) -> InferenceProtocol {
        match self {
            Self::Messages => InferenceProtocol::Anthropic,
            Self::Ocr => InferenceProtocol::Ocr,
            Self::SystemOne => InferenceProtocol::SystemOne,
            _ => InferenceProtocol::Openai,
        }
    }
}

impl fmt::Display for InferenceEndpoint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for InferenceEndpoint {
    type Err = String;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        match value {
            "models" => Ok(Self::Models),
            "chat-completions" => Ok(Self::ChatCompletions),
            "completions" => Ok(Self::Completions),
            "embeddings" => Ok(Self::Embeddings),
            "tools" => Ok(Self::Tools),
            "messages" => Ok(Self::Messages),
            "ocr" => Ok(Self::Ocr),
            "systemone" => Ok(Self::SystemOne),
            "rerank" => Ok(Self::Rerank),
            "responses" => Ok(Self::Responses),
            other => Err(format!(
                "unknown inference endpoint '{other}'; expected models, chat-completions, completions, embeddings, tools, messages, ocr, systemone, rerank, or responses"
            )),
        }
    }
}

/// Declared failure-transfer policy for one model. A closed set: the parse
/// error names both values. `None` preserves today's pass-through — an
/// upstream error response reaches the caller untouched.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FailoverPolicy {
    #[default]
    None,
    RetryableStatus,
}

impl FailoverPolicy {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::RetryableStatus => "retryable_status",
        }
    }

    /// Upstream response statuses that trigger a transfer when the policy is
    /// `RetryableStatus`. 401/403 are deliberately absent: a failed upstream
    /// credential is a configuration incident that must surface to the
    /// operator, not be buried by switching suppliers.
    pub const RETRYABLE_STATUSES: [u16; 3] = [408, 429, 500];

    pub fn transfers_on(self, status: u16) -> bool {
        self == Self::RetryableStatus
            && (Self::RETRYABLE_STATUSES.contains(&status) || (500..=599).contains(&status))
    }

    pub fn parse(value: &str) -> std::result::Result<Self, String> {
        match value.trim() {
            "none" => Ok(Self::None),
            "retryable_status" => Ok(Self::RetryableStatus),
            other => Err(format!(
                "unknown failover policy '{other}'; expected none or retryable_status"
            )),
        }
    }
}

impl fmt::Display for FailoverPolicy {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Console-managed model catalog source for one inference route. The ACL
/// keeps policy (principal, limits); the console supplies which models and
/// targets the route serves. Declaring `models` on the same route refuses
/// activation — two sources for one router is a configuration error.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConsoleCatalogConfig {
    /// Gateway service whose URL reaches a3s-api (the console).
    pub api_service: String,
    /// Poll interval in seconds; the default matches the spec's 30 s.
    #[serde(default = "default_catalog_poll_interval")]
    pub poll_interval_secs: u64,
    /// Apply target prefix locality to models projected from this catalog.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub prefix_affinity: bool,
    #[serde(default = "default_prefix_parallelism")]
    pub prefix_parallelism: u32,
    /// Exact opaque supplier IDs withheld locally during maintenance. This
    /// only narrows a catalog; it never grants a model or edits Cloud state.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub excluded_supplier_ids: Vec<String>,
}

pub const fn default_catalog_poll_interval() -> u64 {
    30
}

pub const fn default_prefix_parallelism() -> u32 {
    1
}

/// Explicit per-Gateway limits attached to one credential grant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InferenceLimitsConfig {
    /// Maximum concurrent requests for this grant on one Gateway.
    pub max_concurrent_requests: u64,
    /// Sustained request allowance per minute on one Gateway.
    pub requests_per_minute: u64,
    /// Maximum immediate request burst.
    pub request_burst: u64,
    /// Token budget per minute on one Gateway.
    pub tokens_per_minute: u64,
    /// Rolling spend budget in micro-USD on one Gateway. Absence means the
    /// grant carries no local spend limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_spend_micros: Option<u64>,
    /// Window for [`Self::max_spend_micros`]; required when a spend budget
    /// is set. Absence means no spend limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spend_window: Option<InferenceSpendWindow>,
    /// Ledger-committed spend for the current window across every replica,
    /// projected by Cloud at credential rotation (the `spend_baseline`
    /// design record). The local bucket starts at
    /// `max_spend_micros − spend_baseline_micros`; absence means full
    /// capacity. Advisory state: correctness never depends on freshness —
    /// the ledger stays the billing truth.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spend_baseline_micros: Option<u64>,
    /// Epoch of the projection that carried the baseline. Rides with
    /// [`Self::spend_baseline_micros`] only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline_epoch: Option<u64>,
}

/// Closed set of rolling windows for grant spend budgets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InferenceSpendWindow {
    Minute,
    Hour,
    Day,
    Month,
}

impl InferenceSpendWindow {
    pub const fn window_nanos(self) -> u128 {
        const SECOND: u128 = 1_000_000_000;
        const MINUTE: u128 = 60 * SECOND;
        match self {
            Self::Minute => MINUTE,
            Self::Hour => 60 * MINUTE,
            Self::Day => 24 * 60 * MINUTE,
            Self::Month => 30 * 24 * 60 * MINUTE,
        }
    }
}

/// Per-model serving price in micro-USD per one million tokens. Integer
/// arithmetic only; prices are projected facts from Cloud, never guessed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InferenceModelPricing {
    /// Price of one million prompt tokens in micro-USD.
    pub input_micros_per_million: u64,
    /// Price of one million completion tokens in micro-USD.
    pub output_micros_per_million: u64,
}

impl InferenceModelPricing {
    /// Exact integer price of a prompt/completion split, rounded up so a
    /// partial micro never under-charges.
    pub const fn price_micros(&self, input_tokens: u64, output_tokens: u64) -> u64 {
        let input = ceil_div_u64(
            input_tokens.saturating_mul(self.input_micros_per_million),
            1_000_000,
        );
        let output = ceil_div_u64(
            output_tokens.saturating_mul(self.output_micros_per_million),
            1_000_000,
        );
        input.saturating_add(output)
    }

    /// Conservative price for a token total whose split is unknown: the
    /// dearer lane prices every token. Over-charging is the fail-closed
    /// direction for budgets.
    pub const fn price_conservative_micros(&self, total_tokens: u64) -> u64 {
        let input = self.input_micros_per_million;
        let output = self.output_micros_per_million;
        let dearer = if input > output { input } else { output };
        ceil_div_u64(total_tokens.saturating_mul(dearer), 1_000_000)
    }
}

const fn ceil_div_u64(numerator: u64, denominator: u64) -> u64 {
    if denominator == 0 {
        return numerator;
    }
    let quotient = numerator / denominator;
    if numerator % denominator == 0 {
        quotient
    } else {
        quotient + 1
    }
}
