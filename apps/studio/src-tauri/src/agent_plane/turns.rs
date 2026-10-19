//! Agent turns: persist the user row and assistant placeholder, drive a3s-code over ACP,
//! publish renderer stream events, and settle the assistant row.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine;
use serde_json::{Map, Value, json};
#[cfg(test)]
use tokio::sync::broadcast;
use tokio::sync::{mpsc, oneshot, watch};

pub use super::events::EventHub;

#[path = "message_delete.rs"]
mod message_delete;

use super::PlaneConfig;
use super::acp::{AcpClient, Inbound};
use super::catalog::{AgentCatalog, SessionCatalog};
use super::domain::{
    Agent, BUILTIN_CODE_AGENT_ASSET_ID, BUILTIN_CODE_AGENT_ID, BUILTIN_CODE_AGENT_RELEASE_ID,
    BUILTIN_CODE_PROVIDER_KIND, BUILTIN_TOBOSS_SESSION_ID, BUILTIN_TOBOT_SESSION_ID, Message,
    Session, is_resident_session, is_toboss_room_session, is_toboss_session, is_tobot_session,
    member_toboss_room_session_id, member_toboss_session_id, member_tobot_session_id,
    member_uuid_ok, new_id, now_iso, private_memory, searchable_text, split_model_id,
    toboss_room_parts,
};
use super::error::PlaneError;
use super::forms::{FormBinding, FormLease, FormOwner, FormRegistry, FormRun, FormValues};
use super::launcher;
use super::question::Question;
use super::review_prompt::{
    accepted_finding_fences, finding_fence_blocks, is_review_skill_prompt, review_document_path,
    review_document_paths, review_follow_up_prompt, review_model_prompt, review_reading,
    review_segment_prompt, review_text_reading,
};
use super::session_query::{ExecutionStatus, SessionExecution};
use super::store::PlaneStore;
use super::stream::{TurnStream, finish_reason};

#[path = "workbench_turns.rs"]
mod workbench;

#[path = "reasoning_effort.rs"]
mod reasoning_effort;
use reasoning_effort::{ReasoningEffort, apply_turn_settings};

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(30);
const TITLE_CHARS: usize = 40;
const METHOD_NOT_FOUND: i64 = -32601;
/// Prose a held turn keeps on screen. Stopping the turn must not delete it.
const HOLD_PROSE: &str = "回合正文还在";
/// OpenUI Lang for the messages catalog. The required field is `when`.
const CATALOG_FORM: &str = "\
root = Card(\"期\", [when, submit])
when = Textarea(\"when\", \"日期\", \"\", true)
submit = Button(\"submit\", \"提交\")";

#[cfg(test)]
#[path = "scheduled_recovery_tests.rs"]
mod scheduled_recovery_tests;

#[cfg(test)]
#[path = "execution_status_tests.rs"]
mod execution_status_tests;

#[cfg(test)]
#[path = "cloud_agent_tests.rs"]
mod cloud_agent_tests;

#[cfg(test)]
#[path = "turns_form_tests.rs"]
mod turns_form_tests;

#[cfg(test)]
#[path = "question_recovery_tests.rs"]
mod question_recovery_tests;

#[cfg(test)]
#[path = "reasoning_transport_tests.rs"]
mod reasoning_transport_tests;

#[cfg(test)]
#[path = "encyclopedia_hit_tests.rs"]
mod encyclopedia_hit_tests;

#[cfg(test)]
#[path = "delivery_contract_tests.rs"]
mod delivery_contract_tests;

#[derive(Clone)]
struct Execution {
    session_id: String,
    topic_id: String,
    execution_id: String,
    attempt_id: u32,
    anchor_id: String,
}

impl Execution {
    fn summary(&self, status: ExecutionStatus) -> SessionExecution {
        SessionExecution {
            status,
            execution_id: self.execution_id.clone(),
            anchor_message_id: self.anchor_id.clone(),
            observed_at: now_iso(),
        }
    }

    fn active(&self) -> Value {
        json!({
            "executionId": self.execution_id,
            "attemptId": self.attempt_id,
            "anchorMessageId": self.anchor_id,
        })
    }

    fn envelope(&self) -> serde_json::Map<String, Value> {
        let mut map = serde_json::Map::new();
        map.insert("topicId".into(), json!(self.topic_id));
        map.insert("executionId".into(), json!(self.execution_id));
        map.insert("attemptId".into(), json!(self.attempt_id));
        map.insert("anchorMessageId".into(), json!(self.anchor_id));
        map
    }
}

/// One sentence the user inserted while this turn was still running.
struct PendingSteer {
    text: String,
    done: oneshot::Sender<Result<(), String>>,
}

struct LiveTurn {
    owner: String,
    execution_id: String,
    status: String,
    observed: Option<super::execution_observation::Snapshot>,
    buffered: Vec<Value>,
    abort: watch::Sender<bool>,
    approvals: HashMap<String, oneshot::Sender<bool>>,
    /// Accepts an inserted sentence and delivers it on the running prompt.
    steer_tx: mpsc::UnboundedSender<PendingSteer>,
}

struct SessionRuntime {
    client: Arc<AcpClient>,
    acp_session: String,
    model: String,
    cwd: String,
    /// ACP servers handed to this process on `session/new`. A later install
    /// changes the list and starts a new process so the next turn can use it.
    mcp_servers: Vec<Value>,
    /// Installed plugin tool entries handed to this process on `session/new`.
    /// A later install starts a new process so the next turn receives the path.
    tools: Vec<Value>,
    plugin_selection: Option<super::plugin_selection::Binding>,
    document_reader: Option<Arc<super::document_read::DocumentReader>>,
    /// App-workspace wiki directory admitted on `session/new`. A different
    /// directory starts a new process so the index follows the notes.
    wiki: String,
    /// Identity line admitted on `session/new`. ToBot replaces "You are A3S Code".
    /// A task session leaves this empty so that process keeps the product role.
    role: String,
    /// Admitted assemble list for this process. A different list starts a new one.
    components: Vec<String>,
    reasoning_effort: tokio::sync::Mutex<Option<ReasoningEffort>>,
    mode: tokio::sync::Mutex<String>,
    inbound: tokio::sync::Mutex<mpsc::UnboundedReceiver<Inbound>>,
}

enum Outcome {
    Finished(String),
    Cancelled,
    Failed(String),
}

enum CloudOutcome {
    Success(String),
    Failed(String),
}

enum PromptEnd {
    Finished(String),
    Cancelled,
    Failed(String),
}

#[derive(Clone)]
struct ReviewMemory {
    document: String,
    findings: String,
    document_revision: Option<String>,
    text_revision: Option<String>,
}

pub struct TurnService {
    proof_artifacts: Arc<super::workbench_artifacts::ArtifactStore>,
    store: Arc<PlaneStore>,
    agents: Arc<AgentCatalog>,
    sessions: Arc<SessionCatalog>,
    config: Arc<PlaneConfig>,
    hub: Arc<EventHub>,
    live: Mutex<HashMap<String, LiveTurn>>,
    forms: Arc<FormRegistry>,
    runtimes: tokio::sync::Mutex<HashMap<String, Arc<SessionRuntime>>>,
    /// Session ids whose a3s-code process is starting.
    warming: Mutex<HashSet<String>>,
    /// Review dossier for every later turn. The contract text stays out of this map.
    review_memory: Mutex<HashMap<String, ReviewMemory>>,
    /// Bumped by reset. A turn that started earlier does not write its row back.
    turn_epoch: AtomicU64,
    /// Held while a turn saves, and while reset deletes the transcript.
    settle_gate: Mutex<()>,
    /// Unit tests set this on one service. The process env is only for the held fork.
    hold_turns: AtomicBool,
}

impl TurnService {
    pub fn new(
        store: Arc<PlaneStore>,
        agents: Arc<AgentCatalog>,
        sessions: Arc<SessionCatalog>,
        config: Arc<PlaneConfig>,
        hub: Arc<EventHub>,
    ) -> Self {
        Self {
            proof_artifacts: Arc::new(super::workbench_artifacts::ArtifactStore::new(
                config.root.clone(),
            )),
            store,
            agents,
            sessions,
            config,
            hub,
            live: Mutex::new(HashMap::new()),
            forms: Arc::new(FormRegistry::default()),
            runtimes: tokio::sync::Mutex::new(HashMap::new()),
            warming: Mutex::new(HashSet::new()),
            review_memory: Mutex::new(HashMap::new()),
            turn_epoch: AtomicU64::new(0),
            settle_gate: Mutex::new(()),
            hold_turns: AtomicBool::new(false),
        }
    }

    /// The held fork sets `A3S_STUDIO_HOLD_TURN=1`. Tests flip the atomic instead,
    /// because `open` runs in parallel and must not race on the process environment.
    fn should_hold(&self) -> bool {
        self.hold_turns.load(Ordering::SeqCst)
            || std::env::var("A3S_STUDIO_HOLD_TURN").ok().as_deref() == Some("1")
    }

    #[cfg(test)]
    fn hold_turns_for_test(&self) {
        self.hold_turns.store(true, Ordering::SeqCst);
    }

    pub fn capability_root(&self) -> Option<std::path::PathBuf> {
        crate::providers::tenant_data_dir(&self.config.slug).map(|dir| dir.join("capabilities"))
    }

    /// Keep one a3s-code process for the ToBot session. `warm` is true when that
    /// process is already running for this model.
    pub async fn ensure_tobot(self: &Arc<Self>, body: &Value) -> Result<Value, PlaneError> {
        self.ensure_resident(body, false, None).await
    }

    /// Keep one a3s-code process for the private ToBoss session. Does not stop ToBot.
    /// A group process is a different session and stays up.
    pub async fn ensure_toboss(self: &Arc<Self>, body: &Value) -> Result<Value, PlaneError> {
        self.ensure_resident(body, true, None).await
    }

    /// One process for this group. The private ToBoss process is left running.
    pub async fn ensure_toboss_room(self: &Arc<Self>, body: &Value) -> Result<Value, PlaneError> {
        let room = body
            .get("conversationId")
            .and_then(Value::as_str)
            .unwrap_or("");
        let session_id = self.own_toboss_room_id(room)?;
        self.ensure_resident(body, true, Some(session_id)).await
    }

    async fn ensure_resident(
        self: &Arc<Self>,
        body: &Value,
        boss: bool,
        room_session: Option<String>,
    ) -> Result<Value, PlaneError> {
        let label = "ToBot";
        let session_id = if let Some(session_id) = room_session {
            self.release_other_runtimes(&session_id, is_toboss_room_session)
                .await;
            session_id
        } else {
            let resident: fn(&str) -> bool = if boss {
                is_toboss_session
            } else {
                is_tobot_session
            };
            match self.resident_session_id(boss) {
                Ok(session_id) => {
                    self.release_other_runtimes(&session_id, resident).await;
                    session_id
                }
                Err(error) => {
                    self.release_other_runtimes("", resident).await;
                    return Err(error);
                }
            }
        };
        let model = tobot_model(body)?;
        let agent = self
            .agents
            .ensure_builtin_code_agent(Some(model.as_str()))?;
        let session = if boss {
            self.sessions.ensure_toboss_session(&session_id)?
        } else {
            self.sessions.ensure_tobot_session(&session_id)?
        };
        let assets = session_assets(&self.config, &session.id, body);
        let host = acp_host(body, assets.as_ref());
        let components =
            match super::harness_assemble::admitted_components(body.get("harnessComponents")) {
                Ok(components) => components,
                Err(error) => return Err(error),
            };
        if self
            .runtime_is_current(&session, &model, host.as_ref(), &components)
            .await
        {
            return Ok(tobot_ready(&session.id, &model, true));
        }
        if !self.mark_warming(&session.id) {
            return Ok(tobot_ready(&session.id, &model, false));
        }
        let service = Arc::clone(self);
        let model_for_task = model.clone();
        let session_id = session.id.clone();
        let warming_id = session_id.clone();
        let host_for_task = host.clone();
        let components_for_task = components.clone();
        tokio::spawn(async move {
            let result = service
                .runtime(
                    &session,
                    &agent,
                    &model_for_task,
                    host_for_task.as_ref(),
                    &components_for_task,
                )
                .await;
            service.clear_warming(&warming_id);
            if let Err(error) = result {
                eprintln!("studio: {label} a3s-code warm: {error}");
            }
        });
        Ok(tobot_ready(&session_id, &model, false))
    }

    /// Path `tobot` or `toboss` is the signed-in member. Another member's id is absent.
    pub fn resolve_session_id(&self, requested: &str) -> Result<String, PlaneError> {
        if is_tobot_session(requested) {
            let own = self.tobot_session_id()?;
            if requested == own || requested == BUILTIN_TOBOT_SESSION_ID {
                return Ok(own);
            }
            return Err(PlaneError::NotFound(format!("session {requested}")));
        }
        if toboss_room_parts(requested).is_some() {
            let own = self.own_toboss_room_id(
                toboss_room_parts(requested)
                    .map(|(_, room)| room)
                    .unwrap_or(""),
            )?;
            if requested == own {
                return Ok(own);
            }
            return Err(PlaneError::NotFound(format!("session {requested}")));
        }
        if is_toboss_session(requested) {
            let own = self.toboss_session_id()?;
            if requested == own || requested == BUILTIN_TOBOSS_SESSION_ID {
                return Ok(own);
            }
            return Err(PlaneError::NotFound(format!("session {requested}")));
        }
        Ok(requested.to_owned())
    }

    fn resident_session_id(&self, boss: bool) -> Result<String, PlaneError> {
        if boss {
            self.toboss_session_id()
        } else {
            self.tobot_session_id()
        }
    }

    fn tobot_session_id(&self) -> Result<String, PlaneError> {
        self.member_session_id(
            BUILTIN_TOBOT_SESSION_ID,
            member_tobot_session_id,
            "session tobot",
        )
    }

    fn toboss_session_id(&self) -> Result<String, PlaneError> {
        self.member_session_id(
            BUILTIN_TOBOSS_SESSION_ID,
            member_toboss_session_id,
            "session toboss",
        )
    }

    /// The signed-in member's session for one group. An empty slug uses `toboss-room-{uuid}`.
    fn own_toboss_room_id(&self, room_id: &str) -> Result<String, PlaneError> {
        if !member_uuid_ok(room_id) {
            return Err(PlaneError::bad("invalid session id"));
        }
        if self.config.slug.is_empty() {
            return member_toboss_room_session_id("", room_id)
                .ok_or_else(|| PlaneError::bad("invalid session id"));
        }
        let member = crate::member_session::MemberSessionStore::open(&self.config.slug).member_id();
        let Some(member) = member.as_deref() else {
            return Err(PlaneError::NotFound("session toboss".into()));
        };
        member_toboss_room_session_id(member, room_id)
            .ok_or_else(|| PlaneError::NotFound(format!("session toboss-room-{room_id}")))
    }

    fn member_session_id(
        &self,
        builtin: &str,
        member_id: fn(&str) -> Option<String>,
        missing: &str,
    ) -> Result<String, PlaneError> {
        if self.config.slug.is_empty() {
            return Ok(builtin.to_owned());
        }
        let member = crate::member_session::MemberSessionStore::open(&self.config.slug).member_id();
        member
            .as_deref()
            .and_then(member_id)
            .ok_or_else(|| PlaneError::NotFound(missing.into()))
    }

    async fn release_other_runtimes(&self, keep: &str, resident: fn(&str) -> bool) {
        let runtime_ids: Vec<String> = self
            .runtimes
            .lock()
            .await
            .keys()
            .filter(|id| resident(id) && id.as_str() != keep)
            .cloned()
            .collect();
        let live_ids: Vec<String> = self
            .lock_live()
            .keys()
            .filter(|id| resident(id) && id.as_str() != keep)
            .cloned()
            .collect();
        let mut ids = runtime_ids;
        for id in live_ids {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
        for id in ids {
            self.abort(&id);
            self.discard_runtime(&id).await;
            self.lock_live().remove(&id);
            self.clear_warming(&id);
        }
    }

    async fn runtime_is_current(
        &self,
        session: &Session,
        model: &str,
        host: Option<&Value>,
        components: &[String],
    ) -> bool {
        let mut tools = installed_session_tools(&self.config);
        let Ok(plugin_selection) = super::plugin_selection::current(&self.config, &tools).await
        else {
            return false;
        };
        if plugin_selection.is_some() {
            tools.clear();
        }
        let mcp_servers = installed_mcp_servers(
            &self.config,
            plugin_selection
                .as_ref()
                .map(|binding| binding.mcp_names.as_slice())
                .unwrap_or(&[]),
        );
        let role = host_for_session(&session.id, host)
            .as_ref()
            .and_then(|value| value.get("role"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        let wiki = self
            .config
            .resident_wiki()
            .map(|path| path.display().to_string())
            .unwrap_or_default();
        let runtimes = self.runtimes.lock().await;
        runtimes.get(&session.id).is_some_and(|runtime| {
            runtime.model == model
                && runtime.cwd == session.workspace.path
                && runtime.mcp_servers == mcp_servers
                && runtime.tools == tools
                && runtime.plugin_selection == plugin_selection
                && runtime.wiki == wiki
                && runtime.role == role
                && runtime.components == components
        })
    }

    fn mark_warming(&self, session_id: &str) -> bool {
        let mut warming = self
            .warming
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        warming.insert(session_id.to_owned())
    }

    fn clear_warming(&self, session_id: &str) {
        let mut warming = self
            .warming
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        warming.remove(session_id);
    }

    /// `ai.stream.open` for `agent-session:<id>`.
    pub fn open(self: &Arc<Self>, session_id: &str, body: &Value) -> Result<Value, PlaneError> {
        if let Some(mode) = body.get("permissionMode").and_then(Value::as_str) {
            let _ = self.store.set_session_permission_mode(session_id, mode);
        }
        let mut session = self.sessions.get(session_id)?;
        let agent_id = session
            .agent_id
            .clone()
            .ok_or_else(|| PlaneError::bad("session has no agent"))?;
        let agent = self.agents.get(&agent_id)?;
        let requested = body.get("model").and_then(Value::as_str);
        let model = match requested {
            Some(model) if split_model_id(model).is_some() => model.to_owned(),
            Some(_) => return Err(PlaneError::bad("model must be providerId::modelId")),
            None => agent
                .model
                .clone()
                .ok_or_else(|| PlaneError::bad("agent has no model"))?,
        };
        let model_name = if agent.model.as_deref() == Some(model.as_str()) {
            agent.model_name.clone().unwrap_or_else(|| model.clone())
        } else {
            split_model_id(&model)
                .map(|(_, name)| name.to_owned())
                .unwrap_or_else(|| model.clone())
        };
        let mut parts = body
            .get("userMessageParts")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let prepared = prompt_for_turn(&parts, Path::new(&session.workspace.path));
        if prepared.text.trim().is_empty() {
            return Err(PlaneError::bad("message is empty"));
        }
        let prompt = agent_parameter_prompt(&prepared.text, body)?;
        let agent_instructions = document_review_instructions(&self.config, body);
        let skill_body = if agent_instructions.is_some() {
            None
        } else {
            crate::review_plugin::published_review_skill_text()
        };
        let text_reading = body
            .pointer("/turnContext/reviewSource")
            .map(|value| {
                let source: super::ReviewTextSource = serde_json::from_value(value.clone())
                    .map_err(|_| PlaneError::bad("Invalid review text source."))?;
                review_text_reading(
                    &source,
                    &prompt,
                    agent_instructions.as_deref(),
                    skill_body.as_deref(),
                )
                .map(|reading| (source, reading))
                .map_err(PlaneError::bad)
            })
            .transpose()?;
        let review_candidates: Vec<String> = if text_reading.is_none()
            && (agent_instructions.is_some()
                || is_review_skill_prompt(&prompt) && skill_body.is_some())
        {
            review_document_paths(&prompt)
                .into_iter()
                .map(str::to_owned)
                .collect()
        } else {
            Vec::new()
        };
        if review_candidates.len() > 8 {
            return Err(PlaneError::bad(
                "Select at most eight Word documents for review.",
            ));
        }
        parts.retain(|part| part["type"] != "data-review-source");
        if let Some((source, reading)) = &text_reading {
            parts.push(json!({"type":"data-review-source","data":{"kind":"text","text":source.text().map_err(PlaneError::bad)?,"sha256":reading.text_revision}}));
        }
        let form_pending = parameter_form_pending(body)?;
        let catalog_memory = match catalog_room_id(body)? {
            Some(room_id) => Some(self.catalog_memory_report(&room_id)?),
            None => None,
        };
        // Refuse an unknown list before the live lock or any stored row exists.
        let components =
            super::harness_assemble::admitted_components(body.get("harnessComponents"))?;
        let policy = super::workbench_proof::Policy::admit_body(body).map_err(PlaneError::bad)?;
        let policy = body
            .pointer("/turnContext/workbench")
            .is_some()
            .then_some(policy);
        self.settle_orphan_pending(session_id);
        let cloud = super::cloud_runtime::admit(body)?;
        if (text_reading.is_some() || !review_candidates.is_empty())
            && (cloud.is_some()
                || form_pending
                || catalog_memory.is_some()
                || body.pointer("/turnContext/agentRuntime").is_some()
                || policy.as_ref().is_some_and(|policy| policy.enabled()))
        {
            return Err(PlaneError::bad(
                "Document and text review require the native review host.",
            ));
        }
        if policy.as_ref().is_some_and(|policy| policy.enabled())
            && (cloud.is_some()
                || form_pending
                || catalog_memory.is_some()
                || body.pointer("/turnContext/agentRuntime").is_some())
        {
            return Err(PlaneError::bad(
                "This executor cannot provide independent review and native byte provenance. Use the default local agent or disable these options.",
            ));
        }
        let cloud_call = if cloud.is_some() {
            let endpoint = self
                .config
                .cloud_agent_runtime
                .as_ref()
                .filter(|endpoint| !endpoint.base.is_empty())
                .ok_or_else(|| PlaneError::bad(super::cloud_runtime::ENDPOINT_MISSING))?;
            let bearer =
                super::cloud_runtime::resolve_bearer(&self.config.slug, endpoint.bearer.as_deref())
                    .map_err(PlaneError::bad)?;
            Some((endpoint.base.clone(), bearer))
        } else {
            None
        };
        let host_deltas = if cloud.is_none()
            && !form_pending
            && body
                .get("turnContext")
                .and_then(|turn| turn.get("agentRuntime"))
                .is_some()
        {
            let seal = body
                .get("aixHostSeal")
                .and_then(Value::as_str)
                .unwrap_or("");
            let deltas = super::controllers::take_host_deltas(seal)
                .ok_or_else(|| PlaneError::bad("agent runtime output is missing"))?;
            if deltas.is_empty() || deltas.iter().any(|delta| delta.is_empty()) {
                return Err(PlaneError::bad("agent runtime output is missing"));
            }
            Some(deltas)
        } else {
            None
        };
        // Other executors own their reasoning contract. Only local ACP uses this set.
        let reasoning_effort = if cloud.is_none()
            && !form_pending
            && host_deltas.is_none()
            && catalog_memory.is_none()
            && !self.should_hold()
        {
            ReasoningEffort::from_body(body)?
        } else {
            None
        };
        // Hold the live lock before the pending row exists, so a message load
        // cannot mark this turn failed while it is still starting.
        let (abort, abort_rx) = watch::channel(false);
        let (steer_tx, steer_rx) = mpsc::unbounded_channel();
        let mut live_turns = self.lock_live();
        if live_turns.contains_key(session_id) {
            return Err(PlaneError::Conflict("a turn is already running".into()));
        }
        live_turns.insert(
            session.id.clone(),
            LiveTurn {
                owner: String::new(),
                execution_id: String::new(),
                status: "pending".into(),
                observed: None,
                buffered: Vec::new(),
                abort,
                approvals: HashMap::new(),
                steer_tx,
            },
        );
        drop(live_turns);

        let now = now_iso();
        let user = Message {
            id: new_id(),
            session_id: session.id.clone(),
            role: "user".into(),
            searchable_text: searchable_text(&parts),
            data: json!({ "parts": parts }),
            status: "success".into(),
            model_id: None,
            message_snapshot: None,
            stats: None,
            delivery: None,
            runtime_resume_token: None,
            created_at: now.clone(),
            updated_at: now.clone(),
        };
        let provider = split_model_id(&model)
            .map(|(provider, _)| provider)
            .unwrap_or("");
        let mut assistant = Message {
            id: new_id(),
            session_id: session.id.clone(),
            role: "assistant".into(),
            data: json!({ "parts": [] }),
            status: "pending".into(),
            model_id: Some(model.clone()),
            message_snapshot: Some(json!({
                "id": agent.id,
                "name": agent.name,
                "model": {
                    "id": model,
                    "name": model_name,
                    "provider": provider,
                },
            })),
            stats: None,
            delivery: None,
            searchable_text: String::new(),
            runtime_resume_token: None,
            created_at: now.clone(),
            updated_at: now.clone(),
        };
        if let Some(policy) = &policy {
            let proof = super::workbench_proof::Proof::new(
                &session.id,
                &assistant.id,
                &assistant.id,
                &model,
                &prepared.text,
                policy.clone(),
            );
            assistant.data = json!({"parts":[{"type":"data-workbench-proof","id":"workbench-proof","data":proof}]});
        }
        if let Err(error) = self
            .store
            .save_message(user.clone())
            .and_then(|_| self.store.save_message(assistant.clone()))
        {
            self.lock_live().remove(&session.id);
            return Err(error.into());
        }

        session.last_activity_at = now.clone();
        session.updated_at = now;
        if session.name.is_empty() && !session.is_name_manually_edited {
            // The model prompt starts with the file scaffold. The sidebar uses the user's sentence.
            session.name = visible_title(&parts);
        }
        if let Err(error) = self.store.save_session(session.clone()) {
            self.lock_live().remove(&session.id);
            return Err(error.into());
        }

        let execution = Execution {
            session_id: session.id.clone(),
            topic_id: session.topic_id(),
            execution_id: assistant.id.clone(),
            attempt_id: 1,
            anchor_id: assistant.id.clone(),
        };
        if !is_resident_session(&session.id) {
            if let Err(error) = self.store.save_session_execution(
                &session.id,
                execution.summary(ExecutionStatus::Pending),
                true,
            ) {
                self.lock_live().remove(&session.id);
                self.changed(&session.id);
                return Err(error.into());
            }
        }
        let epoch = self.turn_epoch.load(Ordering::SeqCst);
        if let Some(live) = self.lock_live().get_mut(&session.id) {
            live.owner = execution.anchor_id.clone();
            live.execution_id = execution.execution_id.clone();
        }
        self.status(&execution, "pending", false);
        self.changed(&session.id);

        let ack = json!({
            "mode": "started",
            "activeExecutions": [execution.active()],
            "reservedMessages": [user.to_ui_message(), assistant.to_ui_message()],
        });
        // Deterministic messages catalog. The live lock stays. a3s-code does not start,
        // and this path does not call reflect or retain.
        if let Some(memory) = catalog_memory {
            let service = Arc::clone(self);
            tokio::spawn(async move {
                service
                    .play_messages_catalog(execution, assistant, abort_rx, epoch)
                    .await;
            });
            let mut ack = ack;
            ack["memory"] = memory;
            ack["liveModel"] = json!(false);
            return Ok(ack);
        }
        // Same ack as a real send. The live lock stays, and a3s-code does not start.
        if self.should_hold() && cloud.is_none() && !form_pending && host_deltas.is_none() {
            let ask = agent.permission_mode() == "plan";
            let service = Arc::clone(self);
            tokio::spawn(async move {
                service
                    .hold_without_agent(execution, assistant, abort_rx, steer_rx, epoch, ask)
                    .await;
            });
            return Ok(ack);
        }
        if form_pending {
            spawn_parameter_form(self, body.clone(), execution, abort_rx);
            return Ok(ack);
        }
        if cloud.is_some() {
            let (Some(admission), Some((base, bearer))) = (cloud, cloud_call) else {
                self.lock_live().remove(&session.id);
                return Err(PlaneError::bad(super::cloud_runtime::ENDPOINT_MISSING));
            };
            let service = Arc::clone(self);
            tokio::spawn(async move {
                service
                    .drive_cloud(execution, admission, base, bearer, abort_rx, epoch)
                    .await;
            });
            return Ok(ack);
        }
        if let Some(deltas) = host_deltas {
            let mut stream = TurnStream::new(assistant.id.clone());
            let chunks = stream_host_deltas(&mut stream, &deltas);
            self.emit(&execution, chunks);
            assistant.status = "success".into();
            assistant.searchable_text = searchable_text(&stream.parts);
            assistant.data = json!({ "parts": stream.parts });
            assistant.updated_at = now_iso();
            if let Err(error) = self.store.save_message(assistant) {
                self.lock_live().remove(&session.id);
                return Err(error.into());
            }
            self.touch_session(&session.id);
            self.lock_live().remove(&session.id);
            let mut payload = execution.envelope();
            payload.insert("isTopicDone".into(), json!(true));
            payload.insert("status".into(), json!("success"));
            self.hub.publish("ai.stream.done", Value::Object(payload));
            self.status(&execution, "done", false);
            self.changed(&session.id);
            return Ok(ack);
        }
        let service = Arc::clone(self);
        let assets = session_assets(&self.config, &session.id, body);
        let human = prompt.clone();
        let reading = text_reading.map(|(_, reading)| reading).or_else(|| {
            review_reading(&human, agent_instructions.as_deref(), skill_body.as_deref())
        });
        if is_review_skill_prompt(&human) || agent_instructions.is_some() {
            self.store_review_memory(&session.id, "", "");
        }
        let segments = reading
            .as_ref()
            .map(|reading| reading.segments.clone())
            .unwrap_or_default();
        let document = reading
            .as_ref()
            .map(|reading| reading.document.clone())
            .unwrap_or_default();
        let text_revision = reading
            .as_ref()
            .and_then(|reading| reading.text_revision.clone());
        let document_revision = reading
            .as_ref()
            .and_then(|reading| reading.document_revision.clone());
        let review_agent = agent_instructions.is_some();
        let review_instructions = reading
            .as_ref()
            .map(|reading| reading.instructions.clone())
            .unwrap_or_else(|| {
                agent_instructions
                    .as_deref()
                    .or(skill_body.as_deref())
                    .unwrap_or_default()
                    .to_owned()
            });
        // A segmented read stays out of this prompt. The driver walks the segments.
        let model_prompt = if !segments.is_empty() || !review_candidates.is_empty() {
            human.clone()
        } else if is_review_skill_prompt(&human) || agent_instructions.is_some() {
            review_model_prompt(&human, agent_instructions.as_deref(), skill_body.as_deref())
        } else if let Some(dossier) = self.review_dossier(&session.id) {
            let source = dossier.text_revision.as_ref().map(|revision| {
                format!("Pasted text (SHA-256 {revision}); no Word document is bound.")
            }).or_else(|| dossier.document_revision.as_ref().map(|revision| {
                format!("{} (reviewed snapshot SHA-256 {revision}). These findings belong to that snapshot; do not assume the current file is unchanged.", dossier.document)
            }));
            review_follow_up_prompt(
                source.as_deref().unwrap_or(&dossier.document),
                &dossier.findings,
                &human,
            )
        } else {
            review_model_prompt(&human, None, None)
        };
        let wire = TurnWire {
            reasoning_effort,
            retry: false,
            policy,
            prompt: model_prompt,
            media: prepared.media,
            human,
            host: acp_host(body, assets.as_ref()),
            turn: acp_turn(body),
            document,
            document_revision,
            text_revision,
            segments,
            review_candidates,
            review_agent,
            review_instructions,
            components,
            memory_key: user.id.clone(),
            org_id: body
                .get("orgId")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned(),
        };
        tokio::spawn(async move {
            service
                .run(
                    execution, session, agent, model, wire, assistant, abort_rx, steer_rx, epoch,
                )
                .await;
        });
        Ok(ack)
    }

    async fn drive_cloud(
        self: Arc<Self>,
        execution: Execution,
        admission: super::cloud_runtime::CloudAdmission,
        base: String,
        bearer: Option<String>,
        mut abort_rx: watch::Receiver<bool>,
        epoch: u64,
    ) {
        let created =
            match super::cloud_runtime::create_projection(&base, &admission, bearer.as_deref())
                .await
            {
                Ok(created) => created,
                Err(message) => {
                    let outcome = if *abort_rx.borrow() {
                        CloudOutcome::Failed(super::cloud_runtime::CANCELLED.to_owned())
                    } else {
                        CloudOutcome::Failed(message)
                    };
                    self.settle_cloud(&execution, epoch, outcome);
                    return;
                }
            };
        if *abort_rx.borrow() {
            self.stop_cloud_once(
                &base,
                &created.agent_execution_id,
                &admission.idempotency_key,
                bearer.as_deref(),
            )
            .await;
            self.settle_cloud(
                &execution,
                epoch,
                CloudOutcome::Failed(super::cloud_runtime::CANCELLED.to_owned()),
            );
            return;
        }
        if created.replayed {
            self.settle_cloud(
                &execution,
                epoch,
                CloudOutcome::Failed(super::cloud_runtime::REPLAYED.to_owned()),
            );
            return;
        }
        let logged = tokio::select! {
            biased;
            result = abort_rx.wait_for(|value| *value) => {
                let _ = result;
                None
            }
            result = super::cloud_runtime::read_terminal_text(&base, &created.agent_execution_id, bearer.as_deref()) => {
                Some(result)
            }
        };
        if *abort_rx.borrow() {
            self.stop_cloud_once(
                &base,
                &created.agent_execution_id,
                &admission.idempotency_key,
                bearer.as_deref(),
            )
            .await;
            self.settle_cloud(
                &execution,
                epoch,
                CloudOutcome::Failed(super::cloud_runtime::CANCELLED.to_owned()),
            );
            return;
        }
        match logged {
            Some(Ok(text)) if !text.is_empty() => {
                self.settle_cloud(&execution, epoch, CloudOutcome::Success(text));
            }
            Some(Ok(_)) => {
                self.settle_cloud(
                    &execution,
                    epoch,
                    CloudOutcome::Failed("The cloud agent runtime returned no text.".to_owned()),
                );
            }
            Some(Err(message)) => {
                self.settle_cloud(&execution, epoch, CloudOutcome::Failed(message));
            }
            None => {
                self.stop_cloud_once(
                    &base,
                    &created.agent_execution_id,
                    &admission.idempotency_key,
                    bearer.as_deref(),
                )
                .await;
                self.settle_cloud(
                    &execution,
                    epoch,
                    CloudOutcome::Failed(super::cloud_runtime::CANCELLED.to_owned()),
                );
            }
        }
    }

    async fn stop_cloud_once(
        &self,
        base: &str,
        execution_id: &str,
        idempotency_key: &str,
        bearer: Option<&str>,
    ) {
        let _ = super::cloud_runtime::stop_projection(
            base,
            execution_id,
            &super::cloud_runtime::stop_key(idempotency_key),
            bearer,
        )
        .await;
    }

    fn settle_cloud(&self, execution: &Execution, epoch: u64, outcome: CloudOutcome) {
        let _gate = self.lock_settle();
        if self.turn_epoch.load(Ordering::SeqCst) != epoch {
            self.lock_live().remove(&execution.session_id);
            return;
        }
        let mut stream = TurnStream::new(&execution.anchor_id);
        let failure = match &outcome {
            CloudOutcome::Success(text) => {
                let mut chunks = stream.update(&json!({
                    "sessionUpdate": "agent_message_chunk",
                    "content": { "type": "text", "text": text }
                }));
                chunks.extend(stream.finish("stop"));
                self.emit(execution, chunks);
                None
            }
            CloudOutcome::Failed(message) => {
                let chunks = stream.error(message);
                self.emit(execution, chunks);
                Some(message.clone())
            }
        };
        if let Some(mut assistant) = self
            .store
            .messages(&execution.session_id)
            .into_iter()
            .find(|message| message.id == execution.anchor_id)
        {
            assistant.status = if failure.is_none() {
                "success".to_owned()
            } else {
                "error".to_owned()
            };
            assistant.searchable_text = searchable_text(&stream.parts);
            assistant.data = json!({ "parts": stream.parts });
            assistant.updated_at = now_iso();
            if self.store.save_message(assistant).is_ok() {
                self.touch_session(&execution.session_id);
            }
        }
        self.lock_live().remove(&execution.session_id);
        let mut payload = execution.envelope();
        payload.insert("isTopicDone".into(), json!(true));
        match failure {
            None => {
                payload.insert("status".into(), json!("success"));
                self.hub.publish("ai.stream.done", Value::Object(payload));
                self.status(execution, "done", false);
            }
            Some(message) => {
                payload.insert(
                    "error".into(),
                    json!({ "name": "Error", "message": message, "stack": null }),
                );
                self.hub.publish("ai.stream.error", Value::Object(payload));
                self.status(execution, "error", false);
            }
        }
        self.changed(&execution.session_id);
    }

    pub fn is_live(&self, session_id: &str) -> bool {
        self.lock_live().contains_key(session_id)
    }

    /// Insert a user sentence into the turn that is already running.
    ///
    /// The sentence is handed to a3s-code as `x.ai/interject`. A second turn is
    /// not opened, and the running turn is not aborted.
    pub async fn insert_into_live(
        &self,
        session_id: &str,
        body: &Value,
    ) -> Result<Value, PlaneError> {
        let session = self.sessions.get(session_id)?;
        let parts = body
            .get("userMessageParts")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let prepared = prompt_for_turn(&parts, Path::new(&session.workspace.path));
        if prepared.text.trim().is_empty() {
            return Err(PlaneError::bad("message is empty"));
        }
        let (done_tx, done_rx) = oneshot::channel();
        {
            let live = self.lock_live();
            let Some(turn) = live.get(session_id) else {
                return Err(PlaneError::Conflict("the turn ended".into()));
            };
            if self
                .store
                .messages(session_id)
                .iter()
                .find(|message| message.id == turn.owner)
                .and_then(|message| super::workbench_proof::Proof::from_parts(message.parts()))
                .is_some_and(|proof| {
                    matches!(
                        proof.review.status,
                        super::workbench_proof::ReviewStatus::Running
                            | super::workbench_proof::ReviewStatus::Remediating
                    )
                })
            {
                return Err(PlaneError::Conflict(
                    "independent review is active; queue this message for the next turn".into(),
                ));
            }
            turn.steer_tx
                .send(PendingSteer {
                    text: prepared.text,
                    done: done_tx,
                })
                .map_err(|_| PlaneError::Conflict("the turn ended".into()))?;
        }
        match done_rx.await {
            Ok(Ok(())) => {}
            Ok(Err(message)) => return Err(PlaneError::bad(message)),
            Err(_) => return Err(PlaneError::Conflict("the turn ended".into())),
        }
        let now = now_iso();
        let user = Message {
            id: new_id(),
            session_id: session.id.clone(),
            role: "user".into(),
            searchable_text: searchable_text(&parts),
            data: json!({ "parts": parts }),
            status: "success".into(),
            model_id: None,
            message_snapshot: None,
            stats: None,
            delivery: None,
            runtime_resume_token: None,
            created_at: now.clone(),
            updated_at: now,
        };
        self.store.save_message(user.clone())?;
        self.touch_session(&session.id);
        self.changed(&session.id);
        Ok(json!({
            "mode": "injected",
            "preserveActiveNode": true,
            "reservedMessages": [user.to_ui_message()],
        }))
    }

    pub fn attach(&self, session_id: &str) -> Value {
        match self.lock_live().get(session_id) {
            Some(turn) => json!({ "status": "attached", "bufferedChunks": turn.buffered }),
            None => json!({ "status": "not-found" }),
        }
    }

    pub fn abort(&self, session_id: &str) {
        if let Some(turn) = self.lock_live().get(session_id) {
            turn.abort.send_replace(true);
            self.forms.withdraw_session(session_id);
        }
    }

    pub(super) fn submit_form(&self, binding: FormBinding, values: FormValues) -> bool {
        let live = self.lock_live();
        if !live.get(&binding.owner.session_id).is_some_and(|turn| {
            turn.owner == binding.owner.anchor_message_id && !*turn.abort.borrow()
        }) {
            return false;
        }
        self.forms.resolve(&binding, values)
    }

    fn register_form(
        &self,
        execution: &Execution,
        acp_session: &str,
        prompt_id: &str,
        spec: &str,
    ) -> Option<FormLease> {
        let live = self.lock_live();
        let turn = live.get(&execution.session_id)?;
        self.forms.register(
            Self::form_owner(execution, acp_session, prompt_id, turn)?,
            spec,
        )
    }

    fn form_owner(
        execution: &Execution,
        acp_session: &str,
        prompt_id: &str,
        turn: &LiveTurn,
    ) -> Option<FormOwner> {
        if turn.owner != execution.anchor_id || *turn.abort.borrow() {
            return None;
        }
        let run = match &turn.observed {
            Some(observed) if observed.prompt_id == prompt_id => Some(FormRun {
                core_session_id: observed.core_session_id.clone(),
                run_id: observed.run_id.clone(),
                model_turn: observed.model_turn,
            }),
            Some(_) => return None,
            None => None,
        };
        Some(FormOwner {
            session_id: execution.session_id.clone(),
            execution_id: execution.execution_id.clone(),
            attempt_id: execution.attempt_id,
            anchor_message_id: execution.anchor_id.clone(),
            acp_session_id: acp_session.to_owned(),
            prompt_id: prompt_id.to_owned(),
            run,
        })
    }

    pub fn respond_approval(&self, approval_id: &str, approved: bool) -> bool {
        let mut live = self.lock_live();
        let sender = live
            .values_mut()
            .find_map(|turn| turn.approvals.remove(approval_id));
        sender.is_some_and(|sender| sender.send(approved).is_ok())
    }

    /// A persisted pending row cannot resume without its owned live turn.
    /// Recover task and resident sessions alike while protecting live starts.
    pub fn settle_orphan_pending(&self, session_id: &str) {
        self.write_interrupt_note(session_id, INTERRUPTED_EXHAUSTED, false);
    }

    /// Pending assistant rows become an interrupt note. When `replace_existing`
    /// is set, a note that already says the process is gone is rewritten so the
    /// sentence matches whether another attempt will start.
    pub(super) fn write_interrupt_note(
        &self,
        session_id: &str,
        reason: &str,
        replace_existing: bool,
    ) {
        let live = self.lock_live();
        if live.contains_key(session_id) {
            return;
        }
        let now = now_iso();
        let mut changed = false;
        for mut message in self.store.messages(session_id) {
            if message.role != "assistant" {
                continue;
            }
            let review_interrupted = self.interrupt_workbench_proof(&mut message, reason);
            let forms_closed = message.parts().iter().any(|part| {
                part["type"] == "data-form" && part["data"]["state"] == "form-requested"
            });
            if forms_closed {
                let mut stream = TurnStream::new(&message.id);
                stream.parts = message.parts().to_vec();
                stream.close_forms();
                message.data = json!({ "parts": stream.parts });
            }
            if message.status == "pending" {
                let mut stream = TurnStream::new(&message.id);
                stream.parts = message.parts().to_vec();
                stream.error(reason);
                message.data = json!({ "parts": stream.parts });
                message.searchable_text = if message.searchable_text.is_empty() {
                    reason.to_owned()
                } else {
                    format!("{}\n{reason}", message.searchable_text)
                };
                message.status = "error".into();
                message.runtime_resume_token = None;
            } else if replace_existing
                && message.status == "error"
                && message.searchable_text.contains(INTERRUPT_PREFIX)
                && !message.searchable_text.contains(reason)
            {
                message.searchable_text =
                    replace_interrupt_sentence(&message.searchable_text, reason);
                replace_error_part(&mut message.data, reason);
            } else if !review_interrupted && !forms_closed {
                continue;
            }
            message.updated_at = now.clone();
            if let Err(error) = self.store.save_message(message) {
                eprintln!("studio: interrupted agent turn was not settled: {error}");
            } else {
                self.touch_session(session_id);
                changed = true;
            }
        }
        // Keep the original execution identity when recovering a lost runtime.
        // This also repairs a prior recovery that already settled its message.
        match self.store.fail_orphan_execution(session_id) {
            Ok(saved) => changed |= saved,
            Err(error) => {
                changed = true;
                eprintln!("studio: interrupted execution summary was not settled: {error}");
            }
        }
        drop(live);
        if changed {
            self.changed(session_id);
        }
    }

    /// The turn task observed a newer epoch and will not drive this reply.
    /// Record the interruption now so the scheduler can continue the same session.
    pub(super) fn record_process_interrupt(&self, session_id: &str, message_id: &str) {
        let runs = self.store.runs();
        let Some(index) = runs.iter().position(|run| {
            run.status == "running" && run.message_id.as_deref() == Some(message_id)
        }) else {
            self.write_interrupt_note(session_id, INTERRUPTED_EXHAUSTED, false);
            return;
        };
        let attempt =
            super::tasks::next_interrupt_attempt(&runs, &runs[index].task_id, &runs[index].id);
        let reason = super::tasks::interrupt_reason(attempt);
        self.write_interrupt_note(session_id, reason, true);
        let mut runs = runs;
        let task_id = runs[index].task_id.clone();
        runs[index].status = "interrupted".into();
        runs[index].attempt = attempt;
        runs[index].finished_at = Some(now_iso());
        runs[index].error = Some(reason.to_owned());
        if let Err(error) = self.store.save_runs(&runs) {
            eprintln!("studio: interrupted scheduled run was not saved: {error}");
            return;
        }
        let now = chrono::Utc::now().fixed_offset();
        let mut tasks = self.store.tasks();
        if super::tasks::queue_interrupt_retry(&mut tasks, &task_id, session_id, attempt, now) {
            if let Err(error) = self.store.save_tasks(&tasks) {
                eprintln!("studio: interrupted scheduled task was not requeued: {error}");
            }
        }
    }

    /// Stop the live ToBot turn and delete its transcript. The session id stays.
    pub async fn reset_tobot(self: &Arc<Self>) -> Result<Value, PlaneError> {
        let session_id = self.tobot_session_id()?;
        self.reset_session(&session_id).await
    }

    /// Stop the live ToBoss turn and delete its transcript. The session id stays.
    pub async fn reset_toboss(self: &Arc<Self>) -> Result<Value, PlaneError> {
        let session_id = self.toboss_session_id()?;
        self.reset_session(&session_id).await
    }

    async fn reset_session(self: &Arc<Self>, session_id: &str) -> Result<Value, PlaneError> {
        let session = self.sessions.get(session_id)?;
        self.turn_epoch.fetch_add(1, Ordering::SeqCst);
        self.abort(session_id);
        self.discard_runtime(session_id).await;
        {
            let _gate = self.lock_settle();
            self.lock_live().remove(session_id);
            self.store.remove_messages(session_id)?;
            self.clear_review_memory(session_id);
        }
        // A turn may have launched after the first kill. Drop that process too.
        self.discard_runtime(session_id).await;
        self.changed(session_id);
        let _ = session;
        Ok(json!({ "ok": true }))
    }

    /// Keep the live lock and the assistant row. Do not start a3s-code.
    async fn hold_without_agent(
        self: Arc<Self>,
        execution: Execution,
        mut assistant: Message,
        mut abort: watch::Receiver<bool>,
        mut steer_rx: mpsc::UnboundedReceiver<PendingSteer>,
        epoch: u64,
        ask: bool,
    ) {
        if self.turn_epoch.load(Ordering::SeqCst) != epoch {
            self.lock_live().remove(&execution.session_id);
            return;
        }
        let mut stream = TurnStream::new(assistant.id.clone());
        self.emit_text(&execution, &mut stream, HOLD_PROSE);
        self.checkpoint_pending(&execution, &stream);
        self.status(&execution, "streaming", false);
        // `open` samples the agent before the composer finishes saving the posture.
        let ask = ask || self.plan_saved_after_open(&execution.session_id).await;
        if ask && self.hold_asks(&execution, &mut stream, &mut abort).await {
            self.finish_held(&execution, &mut assistant, &mut stream, epoch);
            return;
        }
        loop {
            tokio::select! {
                biased;
                _ = wait_for_abort(&mut abort) => {
                    self.finish_held(&execution, &mut assistant, &mut stream, epoch);
                    return;
                }
                steer = steer_rx.recv() => {
                    let Some(steer) = steer else {
                        return;
                    };
                    let _ = steer.done.send(Ok(()));
                }
            }
        }
    }

    /// The posture write can land just after `open` clones the agent.
    async fn plan_saved_after_open(&self, session_id: &str) -> bool {
        let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_millis(1500);
        loop {
            if self.agent_is_plan(session_id) {
                return true;
            }
            if tokio::time::Instant::now() >= deadline {
                return false;
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(40)).await;
        }
    }

    fn agent_is_plan(&self, session_id: &str) -> bool {
        let Ok(session) = self.sessions.get(session_id) else {
            return false;
        };
        let Some(agent_id) = session.agent_id.as_deref() else {
            return false;
        };
        self.agents
            .get(agent_id)
            .is_ok_and(|agent| agent.permission_mode() == "plan")
    }

    /// Plan mode only. Each ask is decided by the user, then the turn stays live.
    async fn hold_asks(
        &self,
        execution: &Execution,
        stream: &mut TurnStream,
        abort: &mut watch::Receiver<bool>,
    ) -> bool {
        let asks = [
            ("hold-edit", "edit", json!({ "title": "计划里的修改" })),
            (
                "hold-read",
                "read",
                json!({ "title": "计划里的读取", "filePath": "/tmp/quarter/hold-read.txt" }),
            ),
        ];
        for (tool_id, name, input) in asks {
            if self
                .ask_held(execution, stream, abort, tool_id, name, input)
                .await
            {
                return true;
            }
        }
        if *abort.borrow() {
            return true;
        }
        self.status(execution, "streaming", false);
        false
    }

    async fn ask_held(
        &self,
        execution: &Execution,
        stream: &mut TurnStream,
        abort: &mut watch::Receiver<bool>,
        tool_id: &str,
        name: &str,
        input: Value,
    ) -> bool {
        let tool = json!({ "toolCallId": tool_id, "title": name, "rawInput": input });
        self.emit(execution, stream.ensure_tool(&tool));
        let approval_id = new_id();
        let (sender, receiver) = oneshot::channel();
        {
            let mut live = self.lock_live();
            let Some(turn) = live.get_mut(&execution.session_id) else {
                return true;
            };
            turn.approvals.insert(approval_id.clone(), sender);
        }
        let chunks = stream.approval_request(&approval_id, tool_id);
        self.checkpoint_pending(execution, stream);
        self.emit(execution, chunks);
        self.status(execution, "awaiting-approval", true);
        // The oneshot stays with this task. `select!` must not drop it when abort wins.
        let (report_tx, mut report_rx) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            let approved = receiver.await.unwrap_or(false);
            let _ = report_tx.send(approved);
        });
        let approved = tokio::select! {
            biased;
            _ = wait_for_abort(abort) => return true,
            approved = report_rx.recv() => approved.unwrap_or(false),
        };
        let chunks = stream.decide_tool(tool_id, approved);
        self.checkpoint_pending(execution, stream);
        self.emit(execution, chunks);
        false
    }

    /// Pending row stays empty long enough for the open thread to show the thinking chip.
    fn catalog_thinking_wait() -> Duration {
        if cfg!(test) {
            Duration::from_millis(40)
        } else {
            Duration::from_secs(12)
        }
    }

    /// Tool names, allow, deny, then the required OpenUI field. No model and no memory HTTP.
    async fn play_messages_catalog(
        self: Arc<Self>,
        execution: Execution,
        mut assistant: Message,
        mut abort: watch::Receiver<bool>,
        epoch: u64,
    ) {
        tokio::time::sleep(Self::catalog_thinking_wait()).await;
        if self.turn_epoch.load(Ordering::SeqCst) != epoch || *abort.borrow() {
            self.lock_live().remove(&execution.session_id);
            return;
        }
        let mut stream = TurnStream::new(assistant.id.clone());
        for (tool_id, name, input) in [
            ("call-read", "read", json!({ "path": "notes/meeting.md" })),
            ("call-minutes", "meeting-minutes", json!({})),
            ("call-calendar", "calendar", json!({})),
        ] {
            let tool = json!({ "toolCallId": tool_id, "title": name, "rawInput": input });
            self.emit(&execution, stream.ensure_tool(&tool));
        }
        self.checkpoint_pending(&execution, &stream);
        if self
            .ask_held(
                &execution,
                &mut stream,
                &mut abort,
                "call-minutes",
                "meeting-minutes",
                json!({}),
            )
            .await
            || self
                .ask_held(
                    &execution,
                    &mut stream,
                    &mut abort,
                    "call-calendar",
                    "calendar",
                    json!({}),
                )
                .await
        {
            self.finish_catalog(&execution, &mut assistant, &mut stream, epoch, false);
            return;
        }
        let Some(mut form) = self.register_form(
            &execution,
            &execution.anchor_id,
            "messages-catalog",
            CATALOG_FORM,
        ) else {
            self.finish_catalog(&execution, &mut assistant, &mut stream, epoch, false);
            return;
        };
        let binding = form.binding.clone();
        let chunks = stream.form_request(&binding, CATALOG_FORM);
        self.checkpoint_pending(&execution, &stream);
        self.emit(&execution, chunks);
        let values = tokio::select! {
            biased;
            _ = wait_for_abort(&mut abort) => None,
            values = form.receive() => values.ok(),
        };
        if let Some(values) = values {
            let chunks = stream.form_answered(&binding, &values);
            self.checkpoint_pending(&execution, &stream);
            self.emit(&execution, chunks);
            self.finish_catalog(&execution, &mut assistant, &mut stream, epoch, true);
            return;
        }
        self.finish_catalog(&execution, &mut assistant, &mut stream, epoch, false);
    }

    /// Settle the catalog row. Does not write the memory bank and does not add a user row.
    fn finish_catalog(
        &self,
        execution: &Execution,
        assistant: &mut Message,
        stream: &mut TurnStream,
        epoch: u64,
        success: bool,
    ) {
        let _gate = self.lock_settle();
        if self.turn_epoch.load(Ordering::SeqCst) != epoch {
            self.lock_live().remove(&execution.session_id);
            return;
        }
        let chunks = if success {
            stream.finish("stop")
        } else {
            stream.abort()
        };
        self.emit(execution, chunks);
        assistant.status = if success { "success" } else { "paused" }.into();
        assistant.searchable_text = searchable_text(stream.parts.as_slice());
        assistant.data = json!({ "parts": stream.parts });
        assistant.updated_at = now_iso();
        if self.store.save_message(assistant.clone()).is_ok() {
            self.touch_session(&execution.session_id);
        }
        self.lock_live().remove(&execution.session_id);
        let mut payload = execution.envelope();
        payload.insert("isTopicDone".into(), json!(true));
        payload.insert(
            "status".into(),
            json!(if success { "success" } else { "paused" }),
        );
        self.hub.publish("ai.stream.done", Value::Object(payload));
        self.status(execution, if success { "done" } else { "aborted" }, false);
        self.changed(&execution.session_id);
    }

    /// Contexts the catalog turn would use. A room has no private memory, so both counts stay 0.
    fn catalog_memory_report(&self, room_id: &str) -> Result<Value, PlaneError> {
        let personal_id = self.tobot_session_id()?;
        let boss_id = self.toboss_session_id()?;
        let room_session = self.own_toboss_room_id(room_id)?;
        let personal =
            private_memory(&personal_id).ok_or_else(|| PlaneError::bad("invalid session id"))?;
        let boss = private_memory(&boss_id).ok_or_else(|| PlaneError::bad("invalid session id"))?;
        let room_memory = private_memory(&room_session);
        let (reflect, retain) = if room_memory.is_none() {
            (0, 0)
        } else {
            (1, 1)
        };
        Ok(json!({
            "personal": {
                "sessionId": personal_id,
                "context": personal.scope,
                "retainContext": personal.scope,
            },
            "private": {
                "sessionId": boss_id,
                "context": boss.scope,
                "retainContext": boss.scope,
                "receivesPersonalFacts": boss.scope == personal.scope,
            },
            "room": {
                "sessionId": room_session,
                "reflect": reflect,
                "retain": retain,
            }
        }))
    }

    /// Same settlement as a cancelled agent turn. The prose row stays.
    fn finish_held(
        &self,
        execution: &Execution,
        assistant: &mut Message,
        stream: &mut TurnStream,
        epoch: u64,
    ) {
        let _gate = self.lock_settle();
        if self.turn_epoch.load(Ordering::SeqCst) != epoch {
            self.lock_live().remove(&execution.session_id);
            return;
        }
        let chunks = stream.abort();
        self.emit(execution, chunks);
        assistant.status = "paused".into();
        assistant.searchable_text = searchable_text(&stream.parts);
        assistant.data = json!({ "parts": stream.parts });
        assistant.updated_at = now_iso();
        if self.store.save_message(assistant.clone()).is_ok() {
            self.touch_session(&execution.session_id);
        }
        self.lock_live().remove(&execution.session_id);
        let mut payload = execution.envelope();
        payload.insert("isTopicDone".into(), json!(true));
        payload.insert("status".into(), json!("paused"));
        self.hub.publish("ai.stream.done", Value::Object(payload));
        self.status(execution, "aborted", false);
        self.changed(&execution.session_id);
    }

    #[allow(clippy::too_many_arguments)]
    async fn run(
        self: Arc<Self>,
        execution: Execution,
        session: Session,
        agent: Agent,
        model: String,
        wire: TurnWire,
        mut assistant: Message,
        mut abort: watch::Receiver<bool>,
        mut steer_rx: mpsc::UnboundedReceiver<PendingSteer>,
        epoch: u64,
    ) {
        if self.turn_epoch.load(Ordering::SeqCst) != epoch {
            self.lock_live().remove(&session.id);
            self.record_process_interrupt(&session.id, &assistant.id);
            return;
        }
        let mut stream = TurnStream::new(&assistant.id);
        if let Some(proof) = super::workbench_proof::Proof::from_parts(assistant.parts()) {
            let chunks = stream.proof(&proof);
            self.emit(&execution, chunks);
        }
        let outcome = self
            .drive(
                &execution,
                &session,
                &agent,
                &model,
                &wire,
                &mut stream,
                &mut abort,
                &mut steer_rx,
            )
            .await;
        self.prove_workbench_turn(
            &execution,
            &session,
            &agent,
            &model,
            &wire,
            &mut assistant,
            &mut stream,
            &mut abort,
            &mut steer_rx,
            epoch,
            &outcome,
        )
        .await;
        let resume = self
            .runtimes
            .lock()
            .await
            .get(&session.id)
            .map(|runtime| runtime.acp_session.clone());
        let stale = {
            let _gate = self.lock_settle();
            if self.turn_epoch.load(Ordering::SeqCst) != epoch {
                self.lock_live().remove(&session.id);
                true
            } else {
                let failure = match &outcome {
                    Outcome::Finished(reason) => {
                        let chunks = stream.finish(finish_reason(reason));
                        self.emit(&execution, chunks);
                        assistant.status = "success".into();
                        None
                    }
                    Outcome::Cancelled => {
                        let chunks = stream.abort();
                        self.emit(&execution, chunks);
                        assistant.status = "paused".into();
                        None
                    }
                    Outcome::Failed(message) => {
                        let chunks = stream.error(message);
                        self.emit(&execution, chunks);
                        assistant.status = "error".into();
                        Some(message.clone())
                    }
                };
                assistant.searchable_text = searchable_text(&stream.parts);
                assistant.data = json!({ "parts": stream.parts });
                assistant.runtime_resume_token = resume;
                assistant.updated_at = now_iso();
                if let Err(error) = self.store.save_message(assistant.clone()) {
                    eprintln!("studio: agent turn was not saved: {error}");
                } else {
                    self.touch_session(&session.id);
                }
                self.lock_live().remove(&session.id);

                let mut payload = execution.envelope();
                payload.insert("isTopicDone".into(), json!(true));
                let terminal = match failure {
                    None => {
                        let status = if assistant.status == "paused" {
                            "paused"
                        } else {
                            "success"
                        };
                        payload.insert("status".into(), json!(status));
                        self.hub.publish("ai.stream.done", Value::Object(payload));
                        if status == "paused" {
                            "aborted"
                        } else {
                            "done"
                        }
                    }
                    Some(message) => {
                        payload.insert(
                            "error".into(),
                            json!({ "name": "Error", "message": message, "stack": null }),
                        );
                        self.hub.publish("ai.stream.error", Value::Object(payload));
                        "error"
                    }
                };
                self.status(&execution, terminal, false);
                self.changed(&session.id);
                false
            }
        };
        if stale {
            self.discard_runtime(&session.id).await;
            return;
        }
        if assistant.status == "success" {
            let origin = crate::org_login::default_origin();
            let slug = self.config.slug.clone();
            let key = wire.memory_key.clone();
            let user_text = wire.human.clone();
            let answer = assistant.searchable_text.clone();
            let org = wire.org_id.clone();
            if let Some((_, room)) = toboss_room_parts(&session.id) {
                let room = room.to_owned();
                let _ = tokio::task::spawn_blocking(move || {
                    super::tobot_memory::room_retain(
                        &origin, &slug, &org, &room, &key, &user_text, &answer,
                    );
                })
                .await;
            } else if let Some(memory) = private_memory(&session.id) {
                let scope = memory.scope;
                let label = memory.label;
                let _ = tokio::task::spawn_blocking(move || {
                    super::tobot_memory::retain_exchange_in(
                        &origin, &slug, &key, &user_text, &answer, scope, &org, label,
                    );
                })
                .await;
            }
        }
    }

    /// Query this turn's bank. A group session reads the room bank.
    /// Personal ToBot and private ToBoss read their own banks. An empty result leaves the prompt.
    async fn recall_tobot(&self, session: &Session, human: &str, org_id: &str) -> Option<String> {
        if human.trim().is_empty() {
            return None;
        }
        let origin = crate::org_login::default_origin();
        let slug = self.config.slug.clone();
        let human = human.to_owned();
        let org_id = org_id.to_owned();
        if let Some((_, room)) = toboss_room_parts(&session.id) {
            let room = room.to_owned();
            return tokio::task::spawn_blocking(move || {
                super::tobot_memory::room_recall_note(&origin, &slug, &org_id, &room, &human)
            })
            .await
            .ok()
            .flatten();
        }
        let memory = private_memory(&session.id)?;
        let scope = memory.scope.to_owned();
        tokio::task::spawn_blocking(move || {
            super::tobot_memory::recall_note_in(&origin, &slug, &human, &scope, &org_id)
        })
        .await
        .ok()
        .flatten()
    }

    fn spoken_prompt(&self, session: &Session, base: &str, fresh: bool) -> String {
        if fresh {
            seed_tobot_transcript(&self.store, session, base)
        } else {
            base.to_owned()
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn drive(
        &self,
        execution: &Execution,
        session: &Session,
        agent: &Agent,
        model: &str,
        wire: &TurnWire,
        stream: &mut TurnStream,
        abort: &mut watch::Receiver<bool>,
        steer_rx: &mut mpsc::UnboundedReceiver<PendingSteer>,
    ) -> Outcome {
        if *abort.borrow() {
            return Outcome::Cancelled;
        }
        if wire.review_candidates.len() == 1 && wire.segments.is_empty() {
            return Outcome::Failed("The selected Word document could not be read.".into());
        }
        let (runtime, fresh) = match self
            .runtime(session, agent, model, wire.host.as_ref(), &wire.components)
            .await
        {
            Ok(runtime) => runtime,
            Err(message) => return Outcome::Failed(message),
        };
        if *abort.borrow() {
            self.discard_runtime(&session.id).await;
            return Outcome::Cancelled;
        }
        if let Err(message) = apply_turn_settings(
            &runtime,
            wire.reasoning_effort,
            &self.live_permission_mode(&session.id, agent.permission_mode()),
        )
        .await
        {
            return Outcome::Failed(message);
        }
        self.status(execution, "streaming", false);
        let mut human = wire.human.clone();
        let reading = if wire.review_candidates.len() > 1 {
            let question = Question {
                question: "请选择要校核的 Word 文档。".into(),
                choices: wire
                    .review_candidates
                    .iter()
                    .enumerate()
                    .map(|(index, path)| super::question::Choice {
                        id: format!("question-{index}"),
                        label: path.clone(),
                    })
                    .collect(),
                allow_free_text: false,
            };
            let values = match self
                .question_values(
                    execution,
                    &runtime,
                    stream,
                    &question,
                    abort,
                    &runtime.acp_session,
                    "review-source",
                )
                .await
            {
                Ok(Some(values)) => values,
                Ok(None) => return Outcome::Cancelled,
                Err(message) => return Outcome::Failed(message),
            };
            let selected = question.choices.iter().position(|choice| {
                values.get("choice").and_then(Value::as_str) == Some(&choice.id)
            });
            let Some(path) = selected.and_then(|index| wire.review_candidates.get(index)) else {
                return Outcome::Failed("Review source selection is invalid.".into());
            };
            if *abort.borrow() {
                return Outcome::Cancelled;
            }
            human = wire
                .human
                .lines()
                .filter(|line| {
                    !wire
                        .review_candidates
                        .iter()
                        .any(|path| path == line.trim())
                })
                .collect::<Vec<_>>()
                .join("\n");
            human.push_str(&format!("\n{path}\n\nHost-confirmed source selection: the user selected the Word document above. Review that document now; the source question has been answered."));
            let Some(reading) = super::review_prompt::read_review_document(
                path,
                wire.review_agent,
                &wire.review_instructions,
            ) else {
                return Outcome::Failed("The selected Word document could not be read.".into());
            };
            Some(reading)
        } else if !wire.segments.is_empty() {
            Some(super::review_prompt::ReviewReading {
                segments: wire.segments.clone(),
                document: wire.document.clone(),
                document_revision: wire.document_revision.clone(),
                text_revision: wire.text_revision.clone(),
                agent_review: wire.review_agent,
                instructions: wire.review_instructions.clone(),
            })
        } else {
            None
        };
        if let Some(reading) = reading {
            if *abort.borrow() {
                return Outcome::Cancelled;
            }
            if !reading.document.is_empty() {
                let Some(revision) = reading.document_revision.as_deref() else {
                    return Outcome::Failed("The reviewed Word snapshot has no revision.".into());
                };
                let chunks = stream.review_document(&reading.document, revision);
                self.checkpoint_pending(execution, stream);
                self.emit(execution, chunks);
            }
            let outcome = {
                let mut inbound = runtime.inbound.lock().await;
                self.review_segments(
                    execution,
                    session,
                    &runtime,
                    &reading,
                    &human,
                    wire.turn.as_ref(),
                    stream,
                    abort,
                    &mut inbound,
                    steer_rx,
                    &wire.org_id,
                )
                .await
            };
            if let Outcome::Failed(message) = &outcome {
                if agent_process_died(message) {
                    self.discard_runtime(&session.id).await;
                }
            }
            return outcome;
        }

        let mut runtime = runtime;
        let memory = self.recall_tobot(session, &wire.human, &wire.org_id).await;
        let mut prompt = self.spoken_prompt(session, &wire.prompt, fresh);
        let turn = if is_toboss_room_session(&session.id) {
            super::tobot_memory::merge_room_memory_turn(wire.turn.as_ref(), memory.as_deref())
        } else {
            super::tobot_memory::merge_memory_turn(wire.turn.as_ref(), memory.as_deref())
        };
        let mut attempt = 0u8;
        loop {
            let end = {
                let mut inbound = runtime.inbound.lock().await;
                let mut ignored = String::new();
                self.run_prompt(
                    execution,
                    &runtime,
                    &runtime.acp_session,
                    &prompt,
                    &wire.media,
                    turn.as_ref(),
                    stream,
                    abort,
                    &mut inbound,
                    steer_rx,
                    &mut ignored,
                    true,
                    &self.live_permission_mode(&session.id, agent.permission_mode()),
                    &wire.org_id,
                )
                .await
            };
            match end {
                PromptEnd::Finished(reason) => return Outcome::Finished(reason),
                PromptEnd::Cancelled => return Outcome::Cancelled,
                PromptEnd::Failed(message) if retryable_agent_error(&message) && attempt == 0 => {
                    attempt = 1;
                }
                PromptEnd::Failed(message) if retryable_agent_error(&message) && attempt == 1 => {
                    self.discard_runtime(&session.id).await;
                    let restarted = match self
                        .runtime(session, agent, model, wire.host.as_ref(), &wire.components)
                        .await
                    {
                        Ok(runtime) => runtime,
                        Err(message) => return Outcome::Failed(message),
                    };
                    runtime = restarted.0;
                    if let Err(message) = apply_turn_settings(
                        &runtime,
                        wire.reasoning_effort,
                        &self.live_permission_mode(&session.id, agent.permission_mode()),
                    )
                    .await
                    {
                        return Outcome::Failed(message);
                    }
                    prompt = self.spoken_prompt(session, &wire.prompt, restarted.1);
                    attempt = 2;
                }
                PromptEnd::Failed(message) => {
                    if !keeps_runtime(&message) {
                        self.discard_runtime(&session.id).await;
                    }
                    return Outcome::Failed(message);
                }
            }
        }
    }

    /// A segment gets one visible correction after validation failure, on its own worker session.
    #[allow(clippy::too_many_arguments)]
    async fn review_segments(
        &self,
        execution: &Execution,
        session: &Session,
        runtime: &SessionRuntime,
        reading: &super::review_prompt::ReviewReading,
        human: &str,
        turn: Option<&Value>,
        stream: &mut TurnStream,
        abort: &mut watch::Receiver<bool>,
        inbound: &mut mpsc::UnboundedReceiver<Inbound>,
        steer_rx: &mut mpsc::UnboundedReceiver<PendingSteer>,
        org_id: &str,
    ) -> Outcome {
        let total = reading.segments.len();
        let mut accepted = Vec::new();
        let mut seen = HashSet::new();
        self.store_reading_memory(&session.id, reading, "");
        for (index, segment) in reading.segments.iter().enumerate() {
            if *abort.borrow() {
                return Outcome::Cancelled;
            }
            let chunks = stream.review_progress(index, total, segment);
            self.checkpoint_pending(execution, stream);
            self.emit(execution, chunks);
            if total > 1 {
                self.emit_text(
                    execution,
                    stream,
                    &format!("正在审查第 {}/{} 段。\n", index + 1, total),
                );
            }
            let worker = match runtime
                .client
                .request(
                    "session/new",
                    session_new_params(
                        &runtime.cwd,
                        &runtime.mcp_servers,
                        &runtime.tools,
                        Some(&json!({"sourceReview":true})),
                        &runtime.components,
                        runtime.plugin_selection.as_ref(),
                    ),
                )
                .await
            {
                Ok(opened) => match opened.get("sessionId").and_then(Value::as_str) {
                    Some(id) => id.to_owned(),
                    None => return Outcome::Failed("a3s-code returned no session id.".into()),
                },
                Err(message) => return Outcome::Failed(message),
            };
            let original_prompt = review_segment_prompt(
                human,
                index,
                &reading.segments,
                reading.agent_review,
                &reading.instructions,
            );
            let mut prompt = original_prompt.clone();
            let mut checked = Err("Document review did not return a verified result.");
            for attempt in 0..2 {
                if *abort.borrow() {
                    if attempt == 1 {
                        self.emit(execution, stream.review_retry(index, &runtime.model, None));
                    }
                    return Outcome::Cancelled;
                }
                let mut model_text = String::new();
                let end = self
                    .run_prompt(
                        execution,
                        runtime,
                        &worker,
                        &prompt,
                        &[],
                        turn,
                        stream,
                        abort,
                        inbound,
                        steer_rx,
                        &mut model_text,
                        false,
                        "default",
                        org_id,
                    )
                    .await;
                if attempt == 1 {
                    self.emit(execution, stream.review_retry(index, &runtime.model, None));
                }
                if *abort.borrow() {
                    return Outcome::Cancelled;
                }
                match end {
                    PromptEnd::Finished(_) => {}
                    PromptEnd::Cancelled => return Outcome::Cancelled,
                    PromptEnd::Failed(message) => return Outcome::Failed(message),
                }
                #[cfg(test)]
                super::reference_tasks_live_tests::record_review_response(&model_text);
                checked = accepted_finding_fences(&model_text, segment);
                match &checked {
                    Ok(_) => break,
                    Err(message) if attempt == 0 => {
                        prompt = format!(
                            "{original_prompt}\nHost validation rejected the previous response: {message}\nReview the same source and selected scope again. Copy evidenceQuote exactly from this segment, preserving every character. Return a complete response satisfying one response-contract branch."
                        );
                        self.emit(
                            execution,
                            stream.review_retry(index, &runtime.model, Some(message)),
                        );
                    }
                    Err(_) => {}
                }
            }
            let findings = match checked {
                Ok(findings) => findings,
                Err(message) => return Outcome::Failed(message.to_owned()),
            };
            for finding in findings {
                if seen.insert(finding.key) {
                    self.emit_text(execution, stream, &format!("{}\n\n", finding.fence));
                    accepted.push(finding.fence);
                }
            }
            self.store_reading_memory(&session.id, reading, &accepted.join("\n\n"));
        }
        let findings = if accepted.is_empty() {
            self.emit_text(execution, stream, "[]");
            "[]".to_owned()
        } else {
            accepted.join("\n\n")
        };
        self.store_reading_memory(&session.id, reading, &findings);
        Outcome::Finished("end_turn".into())
    }

    fn emit_text(&self, execution: &Execution, stream: &mut TurnStream, text: &str) {
        let chunks = stream.update(&json!({
            "sessionUpdate": "agent_message_chunk",
            "content": { "type": "text", "text": text },
        }));
        self.emit(execution, chunks);
    }

    #[allow(clippy::too_many_arguments)]
    async fn run_prompt(
        &self,
        execution: &Execution,
        runtime: &SessionRuntime,
        acp_session: &str,
        prompt: &str,
        media: &[Value],
        turn: Option<&Value>,
        stream: &mut TurnStream,
        abort: &mut watch::Receiver<bool>,
        inbound: &mut mpsc::UnboundedReceiver<Inbound>,
        steer_rx: &mut mpsc::UnboundedReceiver<PendingSteer>,
        collected: &mut String,
        mirror: bool,
        permission_mode: &str,
        org_id: &str,
    ) -> PromptEnd {
        let mut blocks = vec![json!({ "type": "text", "text": prompt })];
        blocks.extend(media.iter().cloned());
        let mut prompt_params = json!({
            "sessionId": acp_session,
            "prompt": blocks,
        });
        let prompt_id = new_id();
        let mut observer = super::execution_observation::Observer::new(acp_session, &prompt_id);
        prompt_params["_meta"] = json!({ "promptId": prompt_id, "a3sExecutionObservation": 1 });
        if let Some(turn) = turn {
            prompt_params["_meta"]["a3sTurn"] = turn.clone();
        }
        let prompt_request = runtime.client.request("session/prompt", prompt_params);
        tokio::pin!(prompt_request);
        let mut cancelled = false;
        let mut agent_gone = false;
        let mut steer_open = true;
        loop {
            tokio::select! {
                biased;
                result = &mut prompt_request => {
                    while let Ok(message) = inbound.try_recv() {
                        if let Inbound::Notification { method, params } = message {
                            self.absorb_update(
                                execution, runtime, stream, &method, &params, acp_session, collected, mirror, &mut observer,
                            );
                        }
                    }
                    return match result {
                        Ok(value) => {
                            let reason = value
                                .get("stopReason")
                                .and_then(Value::as_str)
                                .unwrap_or("end_turn");
                            if cancelled || reason == "cancelled" {
                                PromptEnd::Cancelled
                            } else {
                                PromptEnd::Finished(reason.to_owned())
                            }
                        }
                        Err(_) if cancelled => PromptEnd::Cancelled,
                        Err(message) => PromptEnd::Failed(message),
                    };
                }
                message = inbound.recv(), if !agent_gone => match message {
                    None => agent_gone = true,
                    Some(Inbound::Notification { method, params }) => {
                        self.absorb_update(
                            execution, runtime, stream, &method, &params, acp_session, collected, mirror, &mut observer,
                        );
                    }
                    Some(Inbound::Request { id, method, params }) => {
                        if method == "_a3s/encyclopedia/search" {
                            let query = params
                                .get("query")
                                .and_then(Value::as_str)
                                .unwrap_or("")
                                .to_owned();
                            let slug = self.config.slug.clone();
                            let org = org_id.to_owned();
                            let result = if cancelled || *abort.borrow() {
                                Err("encyclopedia search cancelled".to_owned())
                            } else {
                                match tokio::task::spawn_blocking(move || {
                                    encyclopedia_hits(&org, &query, |org, query| {
                                        fetch_drive_passages(&slug, org, query)
                                    })
                                })
                                .await
                                {
                                    Ok(value) => value,
                                    Err(_) => Err("encyclopedia search unavailable".to_owned()),
                                }
                            };
                            match result {
                                Ok(hits) => {
                                    let _ = runtime
                                        .client
                                        .respond(id, json!({ "passages": hits }))
                                        .await;
                                }
                                Err(reason) => {
                                    let _ = runtime
                                        .client
                                        .respond_error(id, METHOD_NOT_FOUND, &reason)
                                        .await;
                                }
                            }
                        } else if method == "_a3s/document/read" || method == "_a3s/document/release_run" {
                            let release = method == "_a3s/document/release_run";
                            let result = if release || (!cancelled && !*abort.borrow()) {
                                if let Some(reader) = &runtime.document_reader {
                                    let reader = Arc::clone(reader);
                                    let session = acp_session.to_owned();
                                    tokio::task::spawn_blocking(move || {
                                        if release {
                                            reader.release_run(&session, params)
                                        } else {
                                            reader.request(&session, params)
                                        }
                                    })
                                        .await
                                        .unwrap_or(Err(crate::workspace_document::DocumentError::Unavailable))
                                } else {
                                    Err(crate::workspace_document::DocumentError::NotFound)
                                }
                            } else {
                                Err(crate::workspace_document::DocumentError::NotFound)
                            };
                            match result {
                                Ok(value) => {
                                    let _ = runtime.client.respond(id, value).await;
                                }
                                Err(error) => {
                                    let _ = runtime
                                        .client
                                        .respond_error(id, METHOD_NOT_FOUND, &error.to_string())
                                        .await;
                                }
                            }
                        } else if method == "_a3s/form/request" && !mirror {
                            let _ = runtime
                                .client
                                .respond_error(id, METHOD_NOT_FOUND, "form needs the live session")
                                .await;
                        } else if method == "_a3s/form/request" {
                            let stopped = self.form_request(execution, runtime, stream, id, &params, abort, acp_session, &prompt_id).await;
                            if stopped && !cancelled {
                                cancelled = true;
                                cancel(runtime, acp_session).await;
                            }
                        } else if method == "session/request_permission" && !mirror && stream.capture().is_none() {
                            let options = params
                                .get("options")
                                .and_then(Value::as_array)
                                .cloned()
                                .unwrap_or_default();
                            let _ = runtime
                                .client
                                .respond(id, permission_outcome(&options, false))
                                .await;
                        } else if method == "session/request_permission" {
                            let stop = match Question::from_permission(&params) {
                                Ok(Some(question)) => match self.question_request(execution, runtime, stream, id, &params, question, abort, acp_session, &prompt_id).await {
                                    Ok(stopped) => stopped,
                                    Err(message) => {
                                        cancel(runtime, acp_session).await;
                                        return PromptEnd::Failed(message);
                                    }
                                },
                                Ok(None) => self
                                .permission(
                                    execution,
                                    runtime,
                                    stream,
                                    id,
                                    &params,
                                    abort,
                                    permission_mode,
                                )
                                .await,
                                Err(message) => {
                                    let _ = runtime.client.respond_error(id, -32602, message).await;
                                    cancel(runtime, acp_session).await;
                                    return PromptEnd::Failed(message.to_owned());
                                }
                            };
                            if stop && !cancelled {
                                cancelled = true;
                                cancel(runtime, acp_session).await;
                            }
                        } else {
                            let _ = runtime
                                .client
                                .respond_error(id, METHOD_NOT_FOUND, "method not supported")
                                .await;
                        }
                    }
                },
                changed = abort.changed(), if !cancelled => {
                    if changed.is_err() || *abort.borrow() {
                        cancelled = true;
                        cancel(runtime, acp_session).await;
                    }
                }
                pending = steer_rx.recv(), if steer_open => match pending {
                    Some(pending) => {
                        let result = runtime
                            .client
                            .request(
                                "_x.ai/interject",
                                json!({ "sessionId": acp_session, "text": pending.text }),
                            )
                            .await;
                        let _ = pending.done.send(result.map(|_| ()));
                    }
                    None => steer_open = false,
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn absorb_update(
        &self,
        execution: &Execution,
        runtime: &SessionRuntime,
        stream: &mut TurnStream,
        method: &str,
        params: &Value,
        acp_session: &str,
        collected: &mut String,
        mirror: bool,
        observer: &mut super::execution_observation::Observer,
    ) {
        if method == super::execution_observation::METHOD {
            if (mirror || stream.capture().is_some())
                && let Some(snapshot) = observer.accept(params)
            {
                let status = {
                    let mut live = self.lock_live();
                    live.get_mut(&execution.session_id)
                        .filter(|live| live.owner == execution.anchor_id)
                        .map(|live| {
                            live.observed = snapshot;
                            (live.status.clone(), !live.approvals.is_empty())
                        })
                };
                if let Some((status, awaiting)) = status {
                    self.status(execution, &status, awaiting);
                }
            }
            return;
        }
        if method != "session/update"
            || params.get("sessionId").and_then(Value::as_str) != Some(acp_session)
        {
            return;
        }
        if !mirror {
            #[cfg(test)]
            super::reference_tasks_live_tests::record_review_tool_update(params);
            if let Some(text) = params
                .get("update")
                .filter(|update| {
                    update.get("sessionUpdate").and_then(Value::as_str)
                        == Some("agent_message_chunk")
                })
                .and_then(chunk_text)
            {
                collected.push_str(text);
            }
            if stream.capture().is_some() {
                if let Some(update) = params.get("update") {
                    let _ = stream.update(update);
                    if matches!(
                        update.get("sessionUpdate").and_then(Value::as_str),
                        Some("tool_call" | "tool_call_update")
                    ) {
                        self.checkpoint_pending(execution, stream);
                        self.changed(&execution.session_id);
                    }
                }
            }
            return;
        }
        self.apply_update(execution, runtime, stream, method, params);
    }

    fn apply_update(
        &self,
        execution: &Execution,
        runtime: &SessionRuntime,
        stream: &mut TurnStream,
        method: &str,
        params: &Value,
    ) {
        if method != "session/update"
            || params.get("sessionId").and_then(Value::as_str) != Some(runtime.acp_session.as_str())
        {
            return;
        }
        if let Some(update) = params.get("update") {
            let chunks = stream.update(update);
            self.emit(execution, chunks);
        }
    }

    /// Returns true when the user stopped the turn while the request was pending.
    #[allow(clippy::too_many_arguments)]
    /// One `_a3s/form/request`: surface the form part, wait for the member's
    /// submit, and answer the agent with the values so the turn continues.
    async fn form_request(
        &self,
        execution: &Execution,
        runtime: &SessionRuntime,
        stream: &mut TurnStream,
        id: Value,
        params: &Value,
        abort: &mut watch::Receiver<bool>,
        acp_session: &str,
        prompt_id: &str,
    ) -> bool {
        let form_id = params
            .get("formId")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let spec = params
            .get("spec")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        if form_id.trim().is_empty()
            || form_id.len() > 128
            || form_id.chars().any(char::is_control)
            || spec.is_empty()
            || spec.len() > 65_536
            || super::forms::FormFormat::parse(
                params.get("format").and_then(Value::as_str).unwrap_or(""),
            ) != Ok(super::forms::FormFormat::OpenUi)
        {
            let _ = runtime
                .client
                .respond_error(id, METHOD_NOT_FOUND, "form request is malformed")
                .await;
            return false;
        }
        let Some(mut form) = self.register_form(execution, acp_session, prompt_id, &spec) else {
            let _ = runtime
                .client
                .respond_error(id, METHOD_NOT_FOUND, "form registry is unavailable")
                .await;
            return *abort.borrow();
        };
        let chunks = stream.form_request(&form.binding, &spec);
        self.checkpoint_pending(execution, stream);
        self.emit(execution, chunks);
        self.status(execution, "awaiting-form", true);

        let outcome = tokio::select! {
            biased;
            _ = abort.changed() => None,
            values = form.receive() => values.ok(),
        };
        let Some(values) = outcome else {
            let _ = runtime
                .client
                .respond_error(id, METHOD_NOT_FOUND, "form cancelled")
                .await;
            return true;
        };
        if *abort.borrow() {
            let _ = runtime
                .client
                .respond_error(id, METHOD_NOT_FOUND, "form cancelled")
                .await;
            return true;
        }
        let chunks = stream.form_answered(&form.binding, &values);
        self.checkpoint_pending(execution, stream);
        self.emit(execution, chunks);
        self.status(execution, "streaming", false);
        let _ = runtime
            .client
            .respond(id, json!({ "values": values }))
            .await;
        false
    }

    #[allow(clippy::too_many_arguments)]
    async fn question_request(
        &self,
        execution: &Execution,
        runtime: &SessionRuntime,
        stream: &mut TurnStream,
        id: Value,
        params: &Value,
        question: Question,
        abort: &mut watch::Receiver<bool>,
        acp_session: &str,
        prompt_id: &str,
    ) -> Result<bool, String> {
        if params.get("sessionId").and_then(Value::as_str) != Some(acp_session) {
            let _ = runtime
                .client
                .respond_error(id, -32602, "question session mismatch")
                .await;
            return Err("Question session does not match the live prompt.".into());
        }
        let Some(values) = self
            .question_values(
                execution,
                runtime,
                stream,
                &question,
                abort,
                acp_session,
                prompt_id,
            )
            .await?
        else {
            let _ = runtime
                .client
                .respond(id, json!({"outcome":{"outcome":"cancelled"}}))
                .await;
            return Ok(true);
        };
        let response = question
            .answer(&values)
            .ok_or("Question answer is invalid.")?;
        runtime.client.respond(id, response).await?;
        Ok(false)
    }

    #[allow(clippy::too_many_arguments)]
    async fn question_values(
        &self,
        execution: &Execution,
        runtime: &SessionRuntime,
        stream: &mut TurnStream,
        question: &Question,
        abort: &mut watch::Receiver<bool>,
        acp_session: &str,
        prompt_id: &str,
    ) -> Result<Option<FormValues>, String> {
        if *abort.borrow() {
            return Ok(None);
        }
        let registered = {
            let live = self.lock_live();
            live.get(&execution.session_id)
                .and_then(|turn| Self::form_owner(execution, acp_session, prompt_id, turn))
                .and_then(|owner| self.forms.register_question(owner, question.clone()))
        };
        let Some((mut form, spec)) = registered else {
            if *abort.borrow() {
                return Ok(None);
            }
            return Err("Question registry is unavailable.".into());
        };
        let chunks = stream.question_request(&form.binding, &spec);
        self.checkpoint_pending(execution, stream);
        self.emit(execution, chunks);
        self.status(execution, "awaiting-form", true);
        let values = tokio::select! {
            biased;
            _ = wait_for_abort(abort) => None,
            _ = runtime.client.exited() => return Err("a3s-code exited while waiting for an answer.".into()),
            _ = tokio::time::sleep(Duration::from_secs(1800)) => return Err("Question expired without an answer.".into()),
            values = form.receive() => values.ok(),
        };
        let Some(values) = values.filter(|_| !*abort.borrow()) else {
            return Ok(None);
        };
        question
            .answer(&values)
            .ok_or("Question answer is invalid.")?;
        let chunks = stream.form_answered(&form.binding, &values);
        self.checkpoint_pending(execution, stream);
        self.emit(execution, chunks);
        self.status(execution, "streaming", false);
        Ok(Some(values))
    }

    async fn permission(
        &self,
        execution: &Execution,
        runtime: &SessionRuntime,
        stream: &mut TurnStream,
        id: Value,
        params: &Value,
        abort: &mut watch::Receiver<bool>,
        permission_mode: &str,
    ) -> bool {
        #[cfg(test)]
        super::reference_tasks_live_tests::record_review_permission(params);
        let tool_call = params.get("toolCall").cloned().unwrap_or_else(|| json!({}));
        let options = params
            .get("options")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let chunks = stream.ensure_tool(&tool_call);
        if stream.capture().is_none() {
            self.emit(execution, chunks);
        }

        // The composer can change the mode while this turn is running. The
        // request already on screen keeps its own decision; the next action
        // uses the mode stored now.
        let mode = self.live_permission_mode(&execution.session_id, permission_mode);
        // A running turn already started under a mode the agent accepted.
        // This request is still decided here when the agent asks.
        let _ = apply_mode(runtime, &mode).await;
        match tobot_permission_act(&execution.session_id, &mode, &tool_call) {
            PermissionAct::Allow => {
                let _ = runtime
                    .client
                    .respond(id, permission_outcome(&options, true))
                    .await;
                return false;
            }
            PermissionAct::Deny => {
                let _ = runtime
                    .client
                    .respond(id, permission_outcome(&options, false))
                    .await;
                return false;
            }
            PermissionAct::Ask => {}
        }

        let approval_id = new_id();
        let tool_call_id = tool_call
            .get("toolCallId")
            .and_then(Value::as_str)
            .unwrap_or(&approval_id)
            .to_owned();
        let (sender, receiver) = oneshot::channel();
        if let Some(turn) = self.lock_live().get_mut(&execution.session_id) {
            turn.approvals.insert(approval_id.clone(), sender);
        }
        let chunks = stream.approval_request(&approval_id, &tool_call_id);
        // Persist before publish. A reload, or the live oracle on another
        // worker, reads the stored command as soon as the card is visible.
        self.checkpoint_pending(execution, stream);
        if stream.capture().is_none() {
            self.emit(execution, chunks);
        } else {
            self.changed(&execution.session_id);
        }
        self.status(execution, "awaiting-approval", true);

        let wait = tokio::select! {
            answer = receiver => PermissionWait::Answer(answer.unwrap_or(false)),
            _ = wait_for_abort(abort) => PermissionWait::Stopped,
            _ = runtime.client.exited() => PermissionWait::Died,
        };
        let died = matches!(wait, PermissionWait::Died);
        let (approved, stopped) = match wait {
            PermissionWait::Answer(approved) => (approved, false),
            PermissionWait::Stopped => (false, true),
            PermissionWait::Died => (false, false),
        };
        if let Some(turn) = self.lock_live().get_mut(&execution.session_id) {
            turn.approvals.remove(&approval_id);
        }
        if died {
            return false;
        }
        stream.approval_responded(&approval_id, &tool_call_id, approved);
        if !stopped {
            self.status(execution, "streaming", false);
        }
        let _ = runtime
            .client
            .respond(id, permission_outcome(&options, approved))
            .await;
        stopped
    }

    async fn runtime(
        &self,
        session: &Session,
        agent: &Agent,
        model: &str,
        host: Option<&Value>,
        components: &[String],
    ) -> Result<(Arc<SessionRuntime>, bool), String> {
        let mut tools = installed_session_tools(&self.config);
        let plugin_selection = super::plugin_selection::current(&self.config, &tools).await?;
        if plugin_selection.is_some() {
            tools.clear();
        }
        let mcp_servers = installed_mcp_servers(
            &self.config,
            plugin_selection
                .as_ref()
                .map(|binding| binding.mcp_names.as_slice())
                .unwrap_or(&[]),
        );
        let admitted = host_for_session(&session.id, host);
        let wiki = admitted
            .as_ref()
            .and_then(|value| value.get("assets"))
            .and_then(|assets| assets.get("wiki"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        let role = admitted
            .as_ref()
            .and_then(|value| value.get("role"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        let mut runtimes = self.runtimes.lock().await;
        if let Some(runtime) = runtimes.get(&session.id).cloned() {
            if runtime.model == model
                && runtime.cwd == session.workspace.path
                && runtime.mcp_servers == mcp_servers
                && runtime.tools == tools
                && runtime.plugin_selection == plugin_selection
                && runtime.wiki == wiki
                && runtime.role == role
                && runtime.components == components
            {
                // Standing instructions were admitted when this process started.
                apply_mode(
                    &runtime,
                    &self.live_permission_mode(&session.id, agent.permission_mode()),
                )
                .await?;
                super::acp::admit_wiki_client(&runtime.wiki, &runtime.client);
                return Ok((runtime, false));
            }
            runtime.client.kill().await;
            runtimes.remove(&session.id);
        }
        let runtime = Arc::new(
            self.launch(
                session,
                model,
                mcp_servers,
                tools,
                plugin_selection,
                admitted.as_ref(),
                components,
            )
            .await?,
        );
        if let Err(error) = apply_mode(
            &runtime,
            &self.live_permission_mode(&session.id, agent.permission_mode()),
        )
        .await
        {
            runtime.client.kill().await;
            return Err(error);
        }
        runtimes.insert(session.id.clone(), Arc::clone(&runtime));
        super::acp::admit_wiki_client(&runtime.wiki, &runtime.client);
        Ok((runtime, true))
    }

    async fn launch(
        &self,
        session: &Session,
        model: &str,
        mcp_servers: Vec<Value>,
        tools: Vec<Value>,
        plugin_selection: Option<super::plugin_selection::Binding>,
        host: Option<&Value>,
        components: &[String],
    ) -> Result<SessionRuntime, String> {
        let binary = self
            .config
            .binary
            .clone()
            .or_else(discovered_agent_binary)
            .ok_or_else(|| {
                format!(
                    "a3s-code ACP agent was not found. Build target/debug/a3s-code-acp, bundle agent/a3s-code-acp with the app, install ~/.a3s/bin/a3s-code-acp, or set {}.",
                    launcher::BINARY_ENV
                )
            })?;
        let (provider, model_id) = split_model_id(model)
            .ok_or_else(|| format!("Model {model} is not providerId::modelId."))?;
        let credentials = (self.config.credentials)(provider)
            .ok_or_else(|| "登录后通过统一网关使用模型。".to_owned())?;
        let cwd = PathBuf::from(&session.workspace.path);
        if !cwd.is_dir() {
            return Err(format!(
                "Workspace folder {} does not exist.",
                cwd.display()
            ));
        }
        let config_dir = self
            .config
            .root
            .clone()
            .unwrap_or_else(std::env::temp_dir)
            .join("runtime");
        let config =
            launcher::write_config(&config_dir, &session.id, provider, model_id, &credentials)
                .map_err(|error| format!("Could not write the a3s-code config: {error}"))?;
        let mut args = vec![
            "--cwd".to_owned(),
            cwd.to_string_lossy().into_owned(),
            "--model".to_owned(),
            format!("{provider}/{model_id}"),
            "--config".to_owned(),
            config.to_string_lossy().into_owned(),
        ];
        let document_reader = self
            .config
            .workspace_documents
            .as_ref()
            .map(|documents| {
                super::document_read::DocumentReader::new(Arc::clone(documents), &cwd).map(Arc::new)
            })
            .transpose()
            .map_err(|error| error.to_string())?;
        if let Some(binding) = &plugin_selection {
            binding.append_launch_args(&mut args);
        }
        let (client, inbound) = AcpClient::spawn(&binary, &args, &cwd)
            .map_err(|error| format!("Could not start a3s-code: {error}"))?;
        let announced = mcp_servers.clone();
        let announced_tools = tools.clone();
        let handshake = async {
            let initialized = client
                .request(
                    "initialize",
                    json!({
                        "protocolVersion": 1,
                        "clientCapabilities": {},
                        "clientInfo": { "name": "a3s-studio", "version": env!("CARGO_PKG_VERSION") },
                    }),
                )
                .await?;
            #[cfg(test)]
            record_test_initialize(&initialized);
            let _ = &initialized;
            let mut session_new = session_new_params(
                cwd.to_string_lossy(),
                &announced,
                &announced_tools,
                host,
                components,
                plugin_selection.as_ref(),
            );
            if document_reader.is_some() {
                bind_studio_documents(&mut session_new);
            }
            #[cfg(test)]
            record_test_session_new(&session_new);
            client.request("session/new", session_new).await
        };
        let opened = match tokio::time::timeout(HANDSHAKE_TIMEOUT, handshake).await {
            Ok(Ok(opened)) => opened,
            Ok(Err(message)) => {
                client.kill().await;
                return Err(message);
            }
            Err(_) => {
                client.kill().await;
                return Err("a3s-code did not answer the ACP handshake.".to_owned());
            }
        };
        let Some(acp_session) = opened
            .get("sessionId")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
        else {
            client.kill().await;
            return Err("a3s-code returned no session id.".into());
        };
        let acp_session = acp_session.to_owned();
        Ok(SessionRuntime {
            client,
            acp_session,
            model: model.to_owned(),
            cwd: session.workspace.path.clone(),
            mcp_servers,
            tools,
            plugin_selection,
            document_reader,
            wiki: host
                .and_then(|value| value.get("assets"))
                .and_then(|assets| assets.get("wiki"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned(),
            role: host
                .and_then(|value| value.get("role"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned(),
            components: components.to_vec(),
            // Empty until the agent accepts a mode. A fresh process is not already
            // in the ask-each-time posture, so `default` still has to be sent.
            reasoning_effort: tokio::sync::Mutex::new(None),
            mode: tokio::sync::Mutex::new(String::new()),
            inbound: tokio::sync::Mutex::new(inbound),
        })
    }

    async fn discard_runtime(&self, session_id: &str) {
        if let Some(runtime) = self.runtimes.lock().await.remove(session_id) {
            runtime.client.kill().await;
        }
    }

    #[cfg(test)]
    pub(super) async fn close_test_runtimes(&self) {
        let runtimes = std::mem::take(&mut *self.runtimes.lock().await);
        for (session_id, runtime) in runtimes {
            self.abort(&session_id);
            runtime.client.kill().await;
        }
    }

    /// Write the in-progress parts without ending the turn.
    fn checkpoint_pending(&self, execution: &Execution, stream: &TurnStream) {
        let captured = stream.capture();
        let _gate = captured.map(|_| self.lock_settle());
        if captured.is_some_and(|(_, epoch)| self.turn_epoch.load(Ordering::SeqCst) != epoch) {
            return;
        }
        let message_id = captured.map(|(id, _)| id).unwrap_or(&execution.anchor_id);
        let Some(mut message) = self
            .store
            .messages(&execution.session_id)
            .into_iter()
            .find(|message| message.id == message_id)
        else {
            return;
        };
        if message.status != "pending" {
            return;
        }
        message.searchable_text = searchable_text(&stream.parts);
        message.data = json!({ "parts": stream.parts });
        message.updated_at = now_iso();
        if let Err(error) = self.store.save_message(message) {
            eprintln!("studio: ToBot turn was not checkpointed: {error}");
        } else {
            self.touch_session(&execution.session_id);
        }
    }

    /// Bump the session clock after a message write.
    /// A later scan can skip message downloads only when this clock still
    /// covers the newest stored message time.
    fn touch_session(&self, session_id: &str) {
        let Ok(mut session) = self.sessions.get(session_id) else {
            return;
        };
        let now = now_iso();
        session.last_activity_at = now.clone();
        session.updated_at = now;
        if let Err(error) = self.store.save_session(session) {
            eprintln!("studio: session time was not saved: {error}");
        }
    }

    fn emit(&self, execution: &Execution, chunks: Vec<Value>) {
        if chunks.is_empty() {
            return;
        }
        let mut live = self.lock_live();
        for chunk in chunks {
            let mut payload = execution.envelope();
            payload.insert("chunk".into(), chunk);
            let payload = Value::Object(payload);
            if let Some(turn) = live.get_mut(&execution.session_id) {
                turn.buffered.push(payload.clone());
            }
            self.hub.publish("ai.stream.chunk", payload);
        }
    }

    /// Mirrors the renderer's `topic.stream.statuses.<topicId>` shared-cache entry.
    fn status(&self, execution: &Execution, status: &str, awaiting: bool) {
        let mut live_turns = self.lock_live();
        if live_turns
            .get(&execution.session_id)
            .is_some_and(|live| live.owner != execution.anchor_id)
        {
            return;
        }
        let mut summary_changed = false;
        if !is_resident_session(&execution.session_id) {
            match ExecutionStatus::parse(status) {
                Ok(status) => {
                    match self.store.save_session_execution(
                        &execution.session_id,
                        execution.summary(status),
                        false,
                    ) {
                        Ok(changed) => summary_changed = changed,
                        Err(error) => {
                            summary_changed = true;
                            eprintln!(
                                "studio: {} session execution summary was not saved: {error}",
                                status.as_str()
                            );
                        }
                    }
                }
                Err(_) => eprintln!("studio: unknown session execution status: {status}"),
            }
        }
        let observed = live_turns.get_mut(&execution.session_id).and_then(|live| {
            live.status = status.to_owned();
            live.observed.clone()
        });
        let live = matches!(
            status,
            "pending" | "streaming" | "awaiting-approval" | "awaiting-form"
        );
        let mut entry = json!({
            "status": status,
            "activeExecutions": if live { vec![execution.active()] } else { Vec::new() },
            "awaitingApprovalAnchors": if awaiting { vec![execution.active()] } else { Vec::new() },
        });
        if let Some(observed) = observed {
            entry["nativeExecution"] = json!(observed);
        }
        if !live {
            entry["lastCompletedAt"] = json!(chrono::Utc::now().timestamp_millis());
        }
        self.hub.publish(
            "topic.status",
            json!({ "topicId": execution.topic_id, "status": entry }),
        );
        drop(live_turns);
        if summary_changed {
            self.changed(&execution.session_id);
        }
    }

    fn changed(&self, session_id: &str) {
        self.hub
            .publish("data.changed", json!({ "sessionId": session_id }));
    }

    fn lock_live(&self) -> std::sync::MutexGuard<'_, HashMap<String, LiveTurn>> {
        self.live
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn lock_settle(&self) -> std::sync::MutexGuard<'_, ()> {
        self.settle_gate
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn clear_review_memory(&self, session_id: &str) {
        self.lock_review().remove(session_id);
        if let Some(path) = self.review_memory_path(session_id) {
            let _ = std::fs::remove_file(path);
        }
    }

    fn lock_review(&self) -> std::sync::MutexGuard<'_, HashMap<String, ReviewMemory>> {
        self.review_memory
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Findings stay available for every later turn. An empty saved review does not
    /// revive fences from an older assistant message.
    fn review_dossier(&self, session_id: &str) -> Option<ReviewMemory> {
        if let Some(mut memory) = self.load_review_memory(session_id) {
            if memory.findings.trim().is_empty() {
                return None;
            }
            if memory.document.is_empty() && memory.text_revision.is_none() {
                memory.document = self.transcript_document(session_id);
            }
            return Some(memory);
        }
        let recovered = self.transcript_findings(session_id);
        if recovered.trim().is_empty() {
            return None;
        }
        let document = self.transcript_document(session_id);
        self.store_review_memory(session_id, &document, &recovered);
        Some(ReviewMemory {
            document,
            findings: recovered,
            document_revision: None,
            text_revision: None,
        })
    }

    fn transcript_document(&self, session_id: &str) -> String {
        self.store
            .messages(session_id)
            .into_iter()
            .filter(|message| message.role == "user")
            .find_map(|message| review_document_path(&message.searchable_text).map(str::to_owned))
            .unwrap_or_default()
    }

    fn transcript_findings(&self, session_id: &str) -> String {
        self.store
            .messages(session_id)
            .into_iter()
            .filter(|message| message.role == "assistant")
            .map(|message| finding_fence_blocks(&message.searchable_text))
            .filter(|block| !block.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    fn load_review_memory(&self, session_id: &str) -> Option<ReviewMemory> {
        if let Some(memory) = self.lock_review().get(session_id).cloned() {
            return Some(memory);
        }
        let path = self.review_memory_path(session_id)?;
        let value: Value = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
        let memory = ReviewMemory {
            document: value
                .get("document")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned(),
            findings: value.get("findings")?.as_str()?.to_owned(),
            document_revision: value
                .get("documentRevision")
                .and_then(Value::as_str)
                .map(str::to_owned),
            text_revision: value
                .get("textRevision")
                .and_then(Value::as_str)
                .map(str::to_owned),
        };
        self.lock_review()
            .insert(session_id.to_owned(), memory.clone());
        Some(memory)
    }

    fn store_review_memory(&self, session_id: &str, document: &str, findings: &str) {
        self.write_review_memory(
            session_id,
            &ReviewMemory {
                document: document.to_owned(),
                findings: findings.to_owned(),
                document_revision: None,
                text_revision: None,
            },
        );
    }

    fn store_reading_memory(
        &self,
        session_id: &str,
        reading: &super::review_prompt::ReviewReading,
        findings: &str,
    ) {
        self.write_review_memory(
            session_id,
            &ReviewMemory {
                document: reading.document.clone(),
                findings: findings.to_owned(),
                document_revision: reading.document_revision.clone(),
                text_revision: reading.text_revision.clone(),
            },
        );
    }

    fn write_review_memory(&self, session_id: &str, memory: &ReviewMemory) {
        self.lock_review()
            .insert(session_id.to_owned(), memory.clone());
        let Some(path) = self.review_memory_path(session_id) else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let mut value = json!({
            "document": memory.document,
            "findings": memory.findings,
        });
        if let Some(revision) = &memory.text_revision {
            value["textRevision"] = json!(revision);
        }
        if let Some(revision) = &memory.document_revision {
            value["documentRevision"] = json!(revision);
        }
        let bytes = serde_json::to_vec(&value).unwrap_or_default();
        let _ = std::fs::write(path, bytes);
    }

    fn review_memory_path(&self, session_id: &str) -> Option<PathBuf> {
        if session_id.is_empty() || session_id.contains(['/', '\\']) {
            return None;
        }
        Some(
            self.store
                .root()?
                .join("review-memory")
                .join(format!("{session_id}.json")),
        )
    }

    fn live_permission_mode(&self, session_id: &str, fallback: &str) -> String {
        let session_mode = self.store.session_permission_mode(session_id);
        let agent_mode = self
            .store
            .session(session_id)
            .and_then(|session| session.agent_id)
            .and_then(|agent_id| self.store.agent(&agent_id))
            .map(|agent| agent.permission_mode().to_owned());
        resolve_live_permission_mode(
            session_mode.as_deref(),
            agent_mode.as_deref(),
            fallback,
            session_id,
        )
    }
}

/// The plane state directory is `<tenant>/agent-plane`. Installed connectors
/// are the sibling `<tenant>/capabilities/mcp.json`, in the ACP shape.
fn installed_mcp_servers(config: &PlaneConfig, native: &[String]) -> Vec<Value> {
    let mut servers = config
        .root
        .as_ref()
        .and_then(|root| root.parent())
        .map(|tenant| {
            crate::capabilities::acp_session_servers_excluding_native(
                &tenant.join("capabilities"),
                native,
            )
        })
        .unwrap_or_default();
    if let Some(endpoint) = &config.schedule_mcp {
        servers.push(crate::agent_plane::schedule_mcp::schedule_mcp_server(
            endpoint,
        ));
    }
    if let Some(endpoint) = &config.mail_mcp {
        servers.push(crate::agent_plane::mail_mcp::mail_mcp_server(endpoint));
        servers.push(crate::agent_plane::work_mcp::work_mcp_server(
            &endpoint.base_url,
            &endpoint.token,
        ));
    }
    // Same loopback and bearer as the plane. Announced only while 演示文稿 is installed.
    if let Some((base_url, token)) = plane_loopback(config) {
        if presenton_plugin_installed(config) {
            servers.push(crate::agent_plane::presenton_mcp::presenton_mcp_server(
                base_url, token,
            ));
        }
    }
    servers
}

fn plane_loopback(config: &PlaneConfig) -> Option<(&str, &str)> {
    if let Some(endpoint) = &config.mail_mcp {
        return Some((endpoint.base_url.as_str(), endpoint.token.as_str()));
    }
    config
        .schedule_mcp
        .as_ref()
        .map(|endpoint| (endpoint.base_url.as_str(), endpoint.token.as_str()))
}

fn presenton_plugin_installed(config: &PlaneConfig) -> bool {
    config
        .root
        .as_ref()
        .and_then(|root| root.parent())
        .is_some_and(|tenant| {
            crate::agent_plane::presenton_mcp::plugin_installed(&tenant.join("capabilities"))
        })
}

/// Plugin tool entries from `<tenant>/capabilities/tools.json`.
fn installed_session_tools(config: &PlaneConfig) -> Vec<Value> {
    config
        .root
        .as_ref()
        .and_then(|root| root.parent())
        .map(|tenant| crate::capabilities::acp_session_tools(&tenant.join("capabilities")))
        .unwrap_or_default()
}

/// One ACP `session/new` body. Launch and the review worker both use it.
/// `NewSessionRequest` keeps `cwd`, `mcpServers`, and `_meta`, including the
/// same pinned plugin selection for both standing and document-worker sessions.
fn session_new_params(
    cwd: impl AsRef<str>,
    mcp_servers: &[Value],
    tools: &[Value],
    host: Option<&Value>,
    components: &[String],
    selection: Option<&super::plugin_selection::Binding>,
) -> Value {
    let mut session_new = json!({
        "cwd": cwd.as_ref(),
        "mcpServers": mcp_servers,
    });
    let mut meta = Map::new();
    if let Some(host) = host {
        meta.insert("a3sHost".to_owned(), host.clone());
    }
    if !tools.is_empty() {
        meta.insert("a3sTools".to_owned(), json!(tools));
    }
    if !components.is_empty() {
        meta.insert("a3sHarness".to_owned(), json!({ "components": components }));
    }
    if !meta.is_empty() {
        session_new["_meta"] = Value::Object(meta);
    }
    if let Some(binding) = selection {
        binding.bind_session(&mut session_new);
    }
    session_new
}

fn bind_studio_documents(params: &mut Value) {
    if params.get("_meta").is_none() {
        params["_meta"] = json!({});
    }
    params["_meta"]["a3sStudioDocuments"] = json!(true);
}

/// What the workbench permission mode does with one tool request.
enum PermissionAct {
    Allow,
    Deny,
    Ask,
}

/// Reads, searches, and plan updates. Everything else can change the workspace
/// or run a command, so it stays behind a confirmation unless the mode says
/// otherwise.
///
/// a3s-code labels `update_plan` as "Updating plan" and puts `TodoWrite` on
/// `rawInput.variant`. A display title must not hide the tool kind: `download`
/// is `kind: fetch`, and a command is `kind: execute`.
/// The current task's 演示文稿. Exact tool names only. It does not change the workspace.
fn is_deck_tool(tool_call: &Value) -> bool {
    const NAMES: [&str; 4] = [
        "mcp__presenton__generate_deck",
        "mcp__presenton__read_deck",
        "mcp__presenton__edit_outline",
        "mcp__presenton__edit_slide",
    ];
    tool_signals(tool_call)
        .iter()
        .any(|signal| NAMES.contains(&signal.as_str()))
}

fn is_observe_tool(tool_call: &Value) -> bool {
    let signals = tool_signals(tool_call);
    if signals.iter().any(|signal| is_mutation_signal(signal)) {
        return false;
    }
    signals.iter().any(|signal| is_observe_signal(signal))
}

fn tool_signals(tool_call: &Value) -> Vec<String> {
    let mut signals = Vec::new();
    for key in ["toolName", "name", "kind", "title"] {
        if let Some(value) = tool_call.get(key).and_then(Value::as_str) {
            push_signal(&mut signals, value);
        }
    }
    if let Some(variant) = tool_call
        .pointer("/rawInput/variant")
        .and_then(Value::as_str)
    {
        push_signal(&mut signals, variant);
    }
    signals
}

fn push_signal(signals: &mut Vec<String>, raw: &str) {
    let normalized = raw.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return;
    }
    let head = normalized
        .split(|ch: char| ch.is_whitespace() || ch == ':' || ch == '/')
        .next()
        .unwrap_or(normalized.as_str());
    let head = head.strip_prefix("builtin_").unwrap_or(head);
    if head.is_empty() {
        return;
    }
    signals.push(head.to_owned());
    if normalized.as_str() != head {
        signals.push(normalized);
    }
}

fn is_mutation_signal(signal: &str) -> bool {
    matches!(
        signal,
        "bash"
            | "shell"
            | "execute"
            | "edit"
            | "write"
            | "patch"
            | "delete"
            | "move"
            | "download"
            | "task"
            | "parallel_task"
    )
}

fn is_observe_signal(signal: &str) -> bool {
    matches!(
        signal,
        "read"
            | "glob"
            | "grep"
            | "search"
            | "ls"
            | "list"
            | "webfetch"
            | "web_fetch"
            | "websearch"
            | "web_search"
            | "fetch"
            | "think"
            | "plan"
            | "update_plan"
            | "updating plan"
            | "todo"
            | "todowrite"
            | "todo_write"
    )
}

fn accepted_permission_mode(mode: &str) -> Option<&str> {
    match mode {
        "default" | "plan" | "auto" | "bypassPermissions" => Some(mode),
        _ => None,
    }
}

/// Session mode, then the agent, then the caller. Unknown values fall through.
/// They are not rewritten to ask-every-time or full access. `plan` is kept.
fn resolve_live_permission_mode(
    session_mode: Option<&str>,
    agent_mode: Option<&str>,
    fallback: &str,
    session_id: &str,
) -> String {
    let chosen = session_mode
        .and_then(accepted_permission_mode)
        .or_else(|| agent_mode.and_then(accepted_permission_mode))
        .or_else(|| accepted_permission_mode(fallback));
    match chosen {
        Some(mode) => execution_mode(session_id, mode),
        None if is_tobot_session(session_id) => "bypassPermissions".to_owned(),
        None => String::new(),
    }
}

/// Personal ToBot runs with full access. The workbench mode on the shared
/// code agent does not apply to that session.
fn execution_mode(session_id: &str, configured: &str) -> String {
    if is_tobot_session(session_id) {
        "bypassPermissions".to_owned()
    } else {
        configured.to_owned()
    }
}

/// ToBot allows every tool. Other resident sessions allow reads and ask for writes.
fn tobot_permission_act(session_id: &str, mode: &str, tool_call: &Value) -> PermissionAct {
    if is_tobot_session(session_id) {
        return PermissionAct::Allow;
    }
    if is_resident_session(session_id) && is_observe_tool(tool_call) {
        return PermissionAct::Allow;
    }
    permission_act(mode, tool_call)
}

fn permission_act(mode: &str, tool_call: &Value) -> PermissionAct {
    // Plan denies mutations. Anything outside the four postures is refused,
    // not asked and not allowed.
    if !matches!(mode, "default" | "plan" | "auto" | "bypassPermissions") {
        return PermissionAct::Deny;
    }
    if mode == "bypassPermissions" {
        return PermissionAct::Allow;
    }
    if is_deck_tool(tool_call) {
        return if mode == "plan" {
            PermissionAct::Deny
        } else {
            PermissionAct::Allow
        };
    }
    let observe = is_observe_tool(tool_call);
    match mode {
        "plan" => {
            if observe {
                PermissionAct::Allow
            } else {
                PermissionAct::Deny
            }
        }
        "auto" => {
            if observe {
                PermissionAct::Allow
            } else {
                PermissionAct::Ask
            }
        }
        _ => PermissionAct::Ask,
    }
}

fn tobot_model(body: &Value) -> Result<String, PlaneError> {
    match body.get("model").and_then(Value::as_str) {
        Some(model) => {
            let Some((provider, _)) = split_model_id(model) else {
                return Err(PlaneError::bad("model must be providerId::modelId"));
            };
            if provider != "a3s" {
                return Err(PlaneError::bad("model must use the signed-in gateway"));
            }
            Ok(model.to_owned())
        }
        None => crate::providers::first_signed_in_model()
            .ok_or_else(|| PlaneError::bad("no gateway model")),
    }
}

fn tobot_ready(session_id: &str, model: &str, warm: bool) -> Value {
    json!({
        "agentId": BUILTIN_CODE_AGENT_ID,
        "sessionId": session_id,
        "assetId": BUILTIN_CODE_AGENT_ASSET_ID,
        "releaseId": BUILTIN_CODE_AGENT_RELEASE_ID,
        "providerKind": BUILTIN_CODE_PROVIDER_KIND,
        "model": model,
        "warm": warm,
    })
}

/// Studio permission modes are the workbench postures. a3s-code admits
/// `default`, `plan`, `auto`, and `yolo`; retired names stay on `default`.
fn acp_mode_id(permission_mode: &str) -> &'static str {
    match permission_mode {
        "plan" => "plan",
        "auto" => "auto",
        "bypassPermissions" => "yolo",
        _ => "default",
    }
}

async fn apply_mode(runtime: &SessionRuntime, permission_mode: &str) -> Result<(), String> {
    // Unknown must not stay on the ask-every-time posture the ACP session
    // opens with, and must not become full access.
    let wanted = match permission_mode {
        "default" => "default",
        "auto" => "auto",
        "bypassPermissions" => "yolo",
        _ => "plan",
    };
    let mut current = runtime.mode.lock().await;
    if *current == wanted {
        return Ok(());
    }
    match runtime
        .client
        .request(
            "session/set_mode",
            json!({ "sessionId": runtime.acp_session, "modeId": wanted }),
        )
        .await
    {
        Ok(_) => {
            *current = wanted.to_owned();
            Ok(())
        }
        Err(error) => Err(format!(
            "Could not apply the execution mode, so this turn was not started. {error}"
        )),
    }
}

async fn cancel(runtime: &SessionRuntime, acp_session: &str) {
    let _ = runtime
        .client
        .notify("session/cancel", json!({ "sessionId": acp_session }))
        .await;
}

fn chunk_text(update: &Value) -> Option<&str> {
    let content = update.get("content")?;
    if content.get("type").and_then(Value::as_str) != Some("text") {
        return None;
    }
    content.get("text").and_then(Value::as_str)
}

enum PermissionWait {
    Answer(bool),
    Stopped,
    Died,
}

pub(super) const INTERRUPT_PREFIX: &str = "执行已中断，运行进程已不存在。";
pub(super) const INTERRUPTED_RETRYING: &str = "执行已中断，运行进程已不存在。正在重新运行。";
pub(super) const INTERRUPTED_EXHAUSTED: &str = "执行已中断，运行进程已不存在。请重新运行任务。";
pub(super) const INTERRUPTED_RUN: &str = INTERRUPTED_EXHAUSTED;

fn replace_interrupt_sentence(text: &str, reason: &str) -> String {
    match text.find(INTERRUPT_PREFIX) {
        Some(index) => format!("{}{reason}", &text[..index]),
        None => reason.to_owned(),
    }
}

fn replace_error_part(data: &mut Value, reason: &str) {
    let Some(parts) = data.get_mut("parts").and_then(Value::as_array_mut) else {
        return;
    };
    for part in parts {
        if part.get("type").and_then(Value::as_str) == Some("data-error") {
            if let Some(object) = part.get_mut("data").and_then(Value::as_object_mut) {
                object.insert("message".into(), json!(reason));
            }
        }
    }
}

fn agent_process_died(message: &str) -> bool {
    message.contains("exited")
}

/// A context or provider failure can be asked again. Auth and rate limits cannot.
fn retryable_agent_error(message: &str) -> bool {
    if keeps_runtime(message) {
        return false;
    }
    let lower = message.to_ascii_lowercase();
    lower.contains("internal error")
        || lower.contains("context length")
        || lower.contains("maximum context")
        || lower.contains("context window")
        || lower.contains("payload")
        || lower.contains("http 500")
        || lower.contains("server error")
}

fn keeps_runtime(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("rate limit") || lower.contains("unauthorized") || lower.contains("api key")
}

async fn wait_for_abort(abort: &mut watch::Receiver<bool>) {
    loop {
        if *abort.borrow() {
            return;
        }
        if abort.changed().await.is_err() {
            std::future::pending::<()>().await;
        }
    }
}

/// a3s-code proceeds only on `allow-once`; a refusal picks `reject-once` when offered.
fn permission_outcome(options: &[Value], approved: bool) -> Value {
    let ids: Vec<&str> = options
        .iter()
        .filter_map(|option| option.get("optionId").and_then(Value::as_str))
        .collect();
    let chosen = if approved {
        ids.iter().find(|id| **id == "allow-once").copied()
    } else {
        ids.iter().find(|id| **id == "reject-once").copied()
    };
    match chosen {
        Some(option) => json!({ "outcome": { "outcome": "selected", "optionId": option } }),
        None => json!({ "outcome": { "outcome": "cancelled" } }),
    }
}

struct TurnWire {
    reasoning_effort: Option<ReasoningEffort>,
    retry: bool,
    policy: Option<super::workbench_proof::Policy>,
    prompt: String,
    /// ACP image blocks. The text block stays first so the session sees the words.
    media: Vec<Value>,
    human: String,
    host: Option<Value>,
    turn: Option<Value>,
    /// Word path when this turn reads a document. Empty on a later turn.
    document: String,
    document_revision: Option<String>,
    text_revision: Option<String>,
    /// Empty unless this turn is a segmented document read.
    segments: Vec<String>,
    /// Explicit source choices; none is read before the owned question is answered.
    review_candidates: Vec<String>,
    review_agent: bool,
    review_instructions: String,
    /// Admitted Meta Harness assemble. Empty keeps the agent's default list.
    components: Vec<String>,
    /// User message id. The memory write uses it as the idempotency key.
    memory_key: String,
    /// Team org for this turn. Empty and `personal` do not read the drive.
    org_id: String,
}

/// One team encyclopedia hit. `name` is the page heading; `place` is the folder or passage id.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub(crate) struct EncyclopediaHit {
    pub name: String,
    pub place: String,
    pub body: String,
}

/// Hits for one agent-chosen encyclopedia search.
/// A personal center, an empty org, or a non-team id does not call `fetch`.
/// A failed read returns the reason.
pub(crate) fn encyclopedia_hits(
    org: &str,
    query: &str,
    fetch: impl FnOnce(&str, &str) -> Result<Vec<EncyclopediaHit>, String>,
) -> Result<Vec<EncyclopediaHit>, String> {
    let Some(org) = team_org(org) else {
        return Ok(Vec::new());
    };
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }
    let hits = fetch(org, query)?;
    Ok(hits
        .into_iter()
        .filter(|hit| !hit.body.trim().is_empty())
        .collect())
}

fn team_org(raw: &str) -> Option<&str> {
    let raw = raw.trim();
    if raw.is_empty() || raw.eq_ignore_ascii_case("personal") {
        return None;
    }
    if !crate::org_login::org_id_ok(raw) || uuid::Uuid::parse_str(raw).is_err() {
        return None;
    }
    Some(raw)
}

fn fetch_drive_passages(
    slug: &str,
    org: &str,
    query: &str,
) -> Result<Vec<EncyclopediaHit>, String> {
    let Some(token) = crate::member_session::MemberSessionStore::open(slug).token() else {
        eprintln!("encyclopedia passages org={org} status=0");
        return Err("0".into());
    };
    let origin = crate::org_login::default_origin();
    let path = format!("/v1/api/org/drive/{org}/encyclopedia-passages");
    let mut asked = query.trim().chars().take(2_000).collect::<String>();
    asked.retain(|ch| ch != '\0');
    let body = serde_json::to_vec(&serde_json::json!({ "query": asked }))
        .unwrap_or_else(|_| b"{}".to_vec());
    let (status, _, _, payload) =
        crate::org_login::forward_member_api(&origin, "POST", &path, &body, &token);
    if !(200..300).contains(&status) {
        eprintln!("encyclopedia passages org={org} status={status}");
        return Err(status.to_string());
    }
    let value: Value = match serde_json::from_slice(&payload) {
        Ok(value) => value,
        Err(_) => {
            eprintln!("encyclopedia passages org={org} status={status}");
            return Err(status.to_string());
        }
    };
    if value.get("success") == Some(&Value::Bool(false)) {
        eprintln!("encyclopedia passages org={org} status={status}");
        return Err(status.to_string());
    }
    let data = value.get("data").unwrap_or(&value);
    let Some(rows) = data.get("passages").and_then(Value::as_array) else {
        return Err("encyclopedia search response has no passages".into());
    };
    Ok(rows.iter().filter_map(passage_hit).collect())
}

fn passage_hit(row: &Value) -> Option<EncyclopediaHit> {
    let body = row.get("body").and_then(Value::as_str)?.trim();
    if body.is_empty() {
        return None;
    }
    let heading = row.get("heading").and_then(Value::as_str).unwrap_or("");
    let folder = row.get("folder_name").and_then(Value::as_str).unwrap_or("");
    let id = row.get("id").and_then(Value::as_str).unwrap_or("");
    let place = match team_citation(row) {
        Some(citation) => citation,
        None if folder.trim().is_empty() => id.trim().to_owned(),
        None => folder.trim().to_owned(),
    };
    Some(EncyclopediaHit {
        name: passage_name(heading, folder),
        place,
        body: body.to_owned(),
    })
}

/// `team/<file>/<passage>@<generation>`: the renderer opens the file and flags a newer generation.
/// The old scorer's rows carry no file id and keep the folder as the place.
fn team_citation(row: &Value) -> Option<String> {
    let uuid = |key: &str| {
        row.get(key)
            .and_then(Value::as_str)
            .and_then(|raw| uuid::Uuid::parse_str(raw).ok())
    };
    let generation = row.get("generation").and_then(Value::as_i64)?;
    Some(format!(
        "team/{}/{}@{generation}",
        uuid("file_id")?,
        uuid("passage_id")?
    ))
}

fn passage_name(heading: &str, folder: &str) -> String {
    let mut name = heading.trim();
    while let Some(rest) = name.strip_prefix('#') {
        name = rest.trim();
    }
    if !name.is_empty() {
        return name.to_owned();
    }
    let folder = folder.trim();
    if folder.is_empty() {
        "encyclopedia".to_owned()
    } else {
        folder.to_owned()
    }
}

/// Identity for the ToBot session only. Task sessions keep "You are A3S Code".
const TOBOT_ROLE: &str = "You are ToBot";
const TOBOT_GUIDELINES: &str = "Your name is ToBot. Introduce yourself as ToBot.";
const TOBOSS_ROLE: &str = "You are ToBot";
const TOBOSS_GUIDELINES: &str = "Your name is ToBot. Introduce yourself as ToBot.";
const TOBOT_MEMORY_BOUNDARY: &str = "Member memory is not a file in the workspace. When this turn lists memory facts, they are already in the host turn constraints. Do not search the workspace and do not run ls, search, glob, or bash to find a memory archive.";

fn tobot_guidelines(body: &str) -> String {
    format!("{body} {TOBOT_MEMORY_BOUNDARY}")
}

/// Resident `session/new` carries that agent's role. Other sessions keep the host they sent.
fn host_for_session(session_id: &str, host: Option<&Value>) -> Option<Value> {
    if !is_resident_session(session_id) {
        return Some(delivery_host(host.cloned().unwrap_or_else(|| json!({}))));
    }
    let mut value = match host {
        Some(Value::Object(map)) => Value::Object(map.clone()),
        _ => json!({}),
    };
    let (role, persona_guidelines) = resident_persona(session_id, Some(&value));
    value["role"] = json!(role);
    let caller_guidelines = value
        .get("guidelines")
        .and_then(Value::as_str)
        .unwrap_or("");
    let personality = value.get("persona").and_then(Value::as_str).unwrap_or("");
    if caller_guidelines.is_empty() || !personality.is_empty() {
        value["guidelines"] = json!(persona_guidelines);
    }
    if let Some(object) = value.as_object_mut() {
        object.remove("personaName");
        object.remove("persona");
    }
    Some(delivery_host(value))
}

const DELIVERY_CONTRACT: &str = include_str!("delivery_contract.md");

fn delivery_host(mut host: Value) -> Value {
    let contract = DELIVERY_CONTRACT.trim();
    // Preserve the caller's existing 8000-character allowance beside the contract.
    let caller = sanitize_host_text(
        host.get("guidelines").and_then(Value::as_str).unwrap_or(""),
        8000,
    );
    host["guidelines"] = json!(if caller.is_empty() {
        contract.to_owned()
    } else {
        format!("{caller}\n\n{contract}")
    });
    host
}

fn resident_persona(session_id: &str, host: Option<&Value>) -> (String, String) {
    if is_toboss_session(session_id) || is_toboss_room_session(session_id) {
        named_persona(host, "ToBot", TOBOSS_ROLE, TOBOSS_GUIDELINES)
    } else {
        tobot_persona(host)
    }
}

/// Name and personality change the role string, so a warm process restarts.
fn tobot_persona(host: Option<&Value>) -> (String, String) {
    named_persona(host, "ToBot", TOBOT_ROLE, TOBOT_GUIDELINES)
}

fn named_persona(
    host: Option<&Value>,
    fallback: &str,
    plain_role: &str,
    plain_guidelines: &str,
) -> (String, String) {
    let name = host
        .and_then(|value| value.get("personaName"))
        .and_then(Value::as_str)
        .map(|text| sanitize_host_text(text, 32))
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| fallback.to_owned());
    let personality = host
        .and_then(|value| value.get("persona"))
        .and_then(Value::as_str)
        .map(|text| sanitize_host_text(text, 400))
        .unwrap_or_default();
    if name == fallback && personality.is_empty() {
        return (plain_role.to_owned(), tobot_guidelines(plain_guidelines));
    }
    let role = if personality.is_empty() {
        format!("You are {name}")
    } else {
        format!("You are {name}. Personality: {personality}")
    };
    let guidelines = if personality.is_empty() {
        format!("Your name is {name}. Introduce yourself as {name}.")
    } else {
        format!(
            "Your name is {name}. Introduce yourself as {name}. Your personality is: {personality}. Keep that personality in how you speak and decide."
        )
    };
    (role, tobot_guidelines(&guidelines))
}

/// Assets for one session. A turn's absolute `hostContext.wiki` wins.
/// A personal bot otherwise searches the app-workspace wiki.
fn session_assets(config: &PlaneConfig, session_id: &str, body: &Value) -> Option<Value> {
    let assets = config
        .root
        .as_ref()
        .and_then(|root| root.parent())
        .and_then(crate::local_assets::handoff_value);
    if let Some(wiki) = crate::local_assets::studio_wiki_root(body) {
        return Some(crate::local_assets::with_studio_wiki(assets, &wiki));
    }
    if is_resident_session(session_id) {
        if let Some(wiki) = config.resident_wiki() {
            return Some(crate::local_assets::with_studio_wiki(assets, &wiki));
        }
    }
    assets
}

/// `_meta.a3sHost` for a new ACP process. Unknown fields are dropped.
/// A non-empty local catalog is attached even when the turn has no host text.
fn acp_host(body: &Value, assets: Option<&Value>) -> Option<Value> {
    let host = body.get("hostContext").and_then(Value::as_object);
    if host.is_none() && assets.is_none() {
        return None;
    }
    let guidelines = sanitize_host_text(
        host.and_then(|host| host.get("guidelines"))
            .and_then(Value::as_str)
            .unwrap_or(""),
        8000,
    );
    let review = if host
        .and_then(|host| host.get("review"))
        .and_then(Value::as_str)
        == Some("subagent")
    {
        "subagent"
    } else {
        "inline"
    };
    let mut value = json!({
        "guidelines": guidelines,
        "review": review,
    });
    if let Some(language) = host
        .and_then(|host| host.get("outputLanguage"))
        .and_then(Value::as_str)
    {
        if language == "zh-CN" || language == "en-US" {
            value["outputLanguage"] = json!(language);
        }
    }
    if let Some(assets) = assets {
        value["assets"] = assets.clone();
    }
    let persona_name = sanitize_host_text(
        host.and_then(|host| host.get("personaName"))
            .and_then(Value::as_str)
            .unwrap_or(""),
        32,
    );
    let persona = sanitize_host_text(
        host.and_then(|host| host.get("persona"))
            .and_then(Value::as_str)
            .unwrap_or(""),
        400,
    );
    if !persona_name.is_empty() {
        value["personaName"] = json!(persona_name);
    }
    if !persona.is_empty() {
        value["persona"] = json!(persona);
    }
    Some(value)
}

/// `INSTRUCTIONS.md` plus `SKILL.md` when the installed agent requires a `docx-review` skill.
fn document_review_instructions(config: &PlaneConfig, body: &Value) -> Option<String> {
    let raw = body
        .get("turnContext")
        .and_then(|turn| turn.get("agentId"))
        .and_then(Value::as_str)?
        .trim();
    if raw.is_empty() || raw.len() > 64 || raw.contains('\0') {
        return None;
    }
    let tenant = config.root.as_ref().and_then(|root| root.parent());
    crate::review_plugin::docx_review_instructions(tenant, raw)
}

/// `turnContext.messagesCatalog=mtc` plus a group id. Any other marker is refused.
fn catalog_room_id(body: &Value) -> Result<Option<String>, PlaneError> {
    let Some(turn) = body.get("turnContext").and_then(Value::as_object) else {
        return Ok(None);
    };
    let Some(marker) = turn.get("messagesCatalog").and_then(Value::as_str) else {
        return Ok(None);
    };
    if marker != "mtc" {
        return Err(PlaneError::bad("unknown messages catalog"));
    }
    let room = turn.get("roomId").and_then(Value::as_str).unwrap_or("");
    if !member_uuid_ok(room) {
        return Err(PlaneError::bad("invalid session id"));
    }
    Ok(Some(room.to_owned()))
}

/// A declared schema with no submitted values parks the new task on an OpenUI form.
fn parameter_form_pending(body: &Value) -> Result<bool, PlaneError> {
    let Some(turn) = body.get("turnContext").and_then(Value::as_object) else {
        return Ok(false);
    };
    let Some(schema) = turn.get("parametersSchema").and_then(Value::as_str) else {
        return Ok(false);
    };
    if a3s_use_extension::parse_agent_parameter_schema(schema.as_bytes()).is_err() {
        return Err(PlaneError::bad("agent parameters are invalid"));
    }
    Ok(turn.get("agentRuntime").is_some() && turn.get("parameters").is_none())
}

fn parameter_openui(schema_text: &str) -> Result<String, PlaneError> {
    let schema = a3s_use_extension::parse_agent_parameter_schema(schema_text.as_bytes())
        .map_err(|_| PlaneError::bad("agent parameters are invalid"))?;
    let mut lines = Vec::new();
    let mut children = Vec::new();
    for (index, field) in schema.fields.iter().enumerate() {
        let id = format!("n{index}");
        let line = parameter_field_line(&id, field)
            .ok_or_else(|| PlaneError::bad("agent parameters are invalid"))?;
        lines.push(line);
        children.push(id);
    }
    lines.push("submit = Button(\"submit\", \"提交\")".to_owned());
    children.push("submit".to_owned());
    lines.push(format!("root = Card(\"参数\", [{}])", children.join(", ")));
    Ok(lines.join("\n"))
}

fn parameter_field_line(
    id: &str,
    field: &a3s_use_extension::AgentParameterField,
) -> Option<String> {
    let name = serde_json::to_string(&field.name).ok()?;
    let label = serde_json::to_string(&field.title).ok()?;
    let string_enum = !field.enum_values.is_empty()
        && field
            .enum_values
            .iter()
            .all(|value| value.as_str().is_some());
    if string_enum {
        let selected = serde_json::to_string(field.enum_values.first()?.as_str()?).ok()?;
        let mut options = Vec::new();
        for value in &field.enum_values {
            options.push(serde_json::to_string(value.as_str()?).ok()?);
        }
        let required = if field.required { ", true" } else { "" };
        return Some(format!(
            "{id} = Select({name}, {label}, {selected}, [{}]{required})",
            options.join(", ")
        ));
    }
    if matches!(field.kind, a3s_use_extension::AgentParameterKind::String)
        && field.max_length.is_some_and(|length| length > 80)
        && field.enum_values.is_empty()
    {
        let required = if field.required { ", true" } else { "" };
        return Some(format!("{id} = Textarea({name}, {label}, \"\"{required})"));
    }
    if matches!(field.kind, a3s_use_extension::AgentParameterKind::Integer)
        && field.enum_values.is_empty()
        && let (Some(minimum), Some(maximum)) = (field.minimum, field.maximum)
        && maximum >= minimum
        && maximum - minimum <= 100.0
    {
        return Some(format!(
            "{id} = Slider({name}, {label}, {minimum}, {minimum}, {maximum}, 1)"
        ));
    }
    if matches!(field.kind, a3s_use_extension::AgentParameterKind::Boolean) {
        return Some(format!("{id} = Checkbox({name}, {label}, false)"));
    }
    if matches!(field.kind, a3s_use_extension::AgentParameterKind::String) {
        if field.required {
            return Some(format!(
                "{id} = Input({name}, {label}, \"\", \"text\", null, null, null, true)"
            ));
        }
        return Some(format!("{id} = Input({name}, {label}, \"\", \"text\")"));
    }
    let initial = field.minimum.unwrap_or(0.0);
    if field.required {
        Some(format!(
            "{id} = Input({name}, {label}, {initial}, \"number\", null, null, null, true)"
        ))
    } else {
        Some(format!(
            "{id} = Input({name}, {label}, {initial}, \"number\")"
        ))
    }
}

fn spawn_parameter_form(
    service: &Arc<TurnService>,
    body: Value,
    execution: Execution,
    mut abort_rx: watch::Receiver<bool>,
) {
    let schema = body
        .get("turnContext")
        .and_then(|turn| turn.get("parametersSchema"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let Ok(spec) = parameter_openui(schema) else {
        fail_parameter_turn(service, &execution, "agent parameters are invalid");
        return;
    };
    let Some(mut form) =
        service.register_form(&execution, &execution.anchor_id, "aix-parameters", &spec)
    else {
        fail_parameter_turn(
            service,
            &execution,
            "agent parameter form was not registered",
        );
        return;
    };
    let mut stream = TurnStream::new(execution.anchor_id.clone());
    let chunks = stream.form_request(&form.binding, &spec);
    service.checkpoint_pending(&execution, &stream);
    service.emit(&execution, chunks);
    service.status(&execution, "awaiting-form", true);
    let service = Arc::clone(service);
    let caps = service.capability_root();
    let binding = form.binding.clone();
    tokio::spawn(async move {
        let outcome = tokio::select! {
            biased;
            _ = abort_rx.changed() => None,
            values = form.receive() => values.ok(),
        };
        if *abort_rx.borrow() {
            finish_parameter_form(
                &service,
                &execution,
                &mut stream,
                None,
                &binding,
                &body,
                caps.as_deref(),
            )
            .await;
            return;
        }
        finish_parameter_form(
            &service,
            &execution,
            &mut stream,
            outcome,
            &binding,
            &body,
            caps.as_deref(),
        )
        .await;
    });
}

fn fail_parameter_turn(service: &TurnService, execution: &Execution, message: &str) {
    let mut stream = TurnStream::new(&execution.anchor_id);
    let chunks = stream.error(message);
    service.emit(execution, chunks);
    service.checkpoint_pending(execution, &stream);
    if let Some(mut saved) = service
        .store
        .messages(&execution.session_id)
        .into_iter()
        .find(|item| item.id == execution.anchor_id)
    {
        saved.status = "error".into();
        saved.searchable_text = message.to_owned();
        saved.data = json!({ "parts": stream.parts });
        saved.updated_at = now_iso();
        let _ = service.store.save_message(saved);
    }
    service.lock_live().remove(&execution.session_id);
    service.status(execution, "error", false);
    service.changed(&execution.session_id);
}

fn stream_host_deltas(stream: &mut TurnStream, deltas: &[String]) -> Vec<Value> {
    let mut chunks = Vec::new();
    for delta in deltas {
        chunks.extend(stream.update(&json!({
            "sessionUpdate": "agent_message_chunk",
            "content": { "type": "text", "text": delta }
        })));
    }
    chunks.extend(stream.finish("stop"));
    chunks
}

async fn finish_parameter_form(
    service: &TurnService,
    execution: &Execution,
    stream: &mut TurnStream,
    values: Option<FormValues>,
    binding: &FormBinding,
    body: &Value,
    caps: Option<&Path>,
) {
    let Some(values) = values else {
        let chunks = stream.abort();
        service.emit(execution, chunks);
        persist_parameter_message(service, execution, stream, "error");
        service.lock_live().remove(&execution.session_id);
        service.status(execution, "error", false);
        service.changed(&execution.session_id);
        return;
    };
    let schema_text = body
        .get("turnContext")
        .and_then(|turn| turn.get("parametersSchema"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let checked = a3s_use_extension::parse_agent_parameter_schema(schema_text.as_bytes())
        .ok()
        .and_then(|schema| {
            let raw = values
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect::<Map<String, Value>>();
            a3s_use_extension::validate_agent_parameter_values(&schema, &raw).ok()
        });
    let Some(checked) = checked else {
        let chunks = stream.error("agent parameters are invalid");
        service.emit(execution, chunks);
        persist_parameter_message(service, execution, stream, "error");
        service.lock_live().remove(&execution.session_id);
        service.status(execution, "error", false);
        service.changed(&execution.session_id);
        return;
    };
    let chunks = stream.form_answered(binding, &values);
    service.checkpoint_pending(execution, stream);
    service.emit(execution, chunks);
    service.status(execution, "streaming", false);
    let mut next = body.clone();
    if let Some(turn) = next.get_mut("turnContext").and_then(Value::as_object_mut) {
        turn.insert("parameters".to_owned(), Value::Object(checked));
    }
    let cloud_turn = super::cloud_runtime::is_cloud_turn(&next);
    let mut cloud_base = None;
    let mut cloud_bearer = None;
    if cloud_turn {
        let Some(endpoint) = service
            .config
            .cloud_agent_runtime
            .as_ref()
            .filter(|endpoint| !endpoint.base.is_empty())
        else {
            let chunks = stream.error(super::cloud_runtime::ENDPOINT_MISSING);
            service.emit(execution, chunks);
            persist_parameter_message(service, execution, stream, "error");
            service.lock_live().remove(&execution.session_id);
            service.status(execution, "error", false);
            service.changed(&execution.session_id);
            return;
        };
        match super::cloud_runtime::resolve_bearer(&service.config.slug, endpoint.bearer.as_deref())
        {
            Ok(bearer) => {
                cloud_base = Some(endpoint.base.clone());
                cloud_bearer = bearer;
            }
            Err(message) => {
                let chunks = stream.error(&message);
                service.emit(execution, chunks);
                persist_parameter_message(service, execution, stream, "error");
                service.lock_live().remove(&execution.session_id);
                service.status(execution, "error", false);
                service.changed(&execution.session_id);
                return;
            }
        }
    }
    match super::controllers::declared_agent_output(
        &next,
        caps,
        cloud_base.as_deref(),
        cloud_bearer.as_deref(),
    )
    .await
    {
        Ok(deltas) if deltas.iter().any(|delta| !delta.is_empty()) => {
            let chunks = stream_host_deltas(stream, &deltas);
            service.emit(execution, chunks);
            persist_parameter_message(service, execution, stream, "success");
            service.lock_live().remove(&execution.session_id);
            let mut payload = execution.envelope();
            payload.insert("isTopicDone".into(), json!(true));
            payload.insert("status".into(), json!("success"));
            service
                .hub
                .publish("ai.stream.done", Value::Object(payload));
            service.status(execution, "done", false);
            service.changed(&execution.session_id);
        }
        Ok(_) => {
            let chunks = stream.error("agent runtime output is missing");
            service.emit(execution, chunks);
            persist_parameter_message(service, execution, stream, "error");
            service.lock_live().remove(&execution.session_id);
            service.status(execution, "error", false);
            service.changed(&execution.session_id);
        }
        Err(error) => {
            let chunks = stream.error(&error.to_string());
            service.emit(execution, chunks);
            persist_parameter_message(service, execution, stream, "error");
            service.lock_live().remove(&execution.session_id);
            service.status(execution, "error", false);
            service.changed(&execution.session_id);
        }
    }
}

fn persist_parameter_message(
    service: &TurnService,
    execution: &Execution,
    stream: &TurnStream,
    status: &str,
) {
    let Some(mut message) = service
        .store
        .messages(&execution.session_id)
        .into_iter()
        .find(|item| item.id == execution.anchor_id)
    else {
        return;
    };
    message.status = status.to_owned();
    message.searchable_text = searchable_text(&stream.parts);
    message.data = json!({ "parts": stream.parts });
    message.updated_at = now_iso();
    if service.store.save_message(message).is_ok() {
        service.touch_session(&execution.session_id);
    }
}

/// User text plus a validated parameter object for the ACP `session/prompt` body.
/// `acp_turn` does not carry parameters, so they have to be in this prompt.
fn agent_parameter_prompt(user_text: &str, body: &Value) -> Result<String, PlaneError> {
    let Some(turn) = body.get("turnContext").and_then(Value::as_object) else {
        return Ok(user_text.to_owned());
    };
    if !turn.contains_key("parameters") {
        return Ok(user_text.to_owned());
    }
    let Some(schema_text) = turn.get("parametersSchema").and_then(Value::as_str) else {
        return Err(PlaneError::bad("agent parameters are invalid"));
    };
    let Some(values) = turn.get("parameters").and_then(Value::as_object) else {
        return Err(PlaneError::bad("agent parameters are invalid"));
    };
    let schema = a3s_use_extension::parse_agent_parameter_schema(schema_text.as_bytes())
        .map_err(|_| PlaneError::bad("agent parameters are invalid"))?;
    let checked = a3s_use_extension::validate_agent_parameter_values(&schema, values)
        .map_err(|_| PlaneError::bad("agent parameters are invalid"))?;
    let encoded = serde_json::to_string(&Value::Object(checked))
        .map_err(|_| PlaneError::bad("agent parameters are invalid"))?;
    Ok(format!("{user_text}\n\n智能体参数\n{encoded}"))
}

/// `_meta.a3sTurn` for this prompt. Empty constraints stay off the wire.
fn acp_turn(body: &Value) -> Option<Value> {
    let turn = body.get("turnContext")?.as_object()?;
    let mut directives = Vec::new();
    if let Some(items) = turn.get("directives").and_then(Value::as_array) {
        for item in items.iter().take(8) {
            let Some(text) = item.as_str() else {
                continue;
            };
            let text = sanitize_host_text(text, 500);
            if !text.is_empty() {
                directives.push(text);
            }
        }
    }
    let research = turn
        .get("research")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if directives.is_empty() && !research {
        return None;
    }
    Some(json!({ "directives": directives, "research": research }))
}

fn sanitize_host_text(raw: &str, limit: usize) -> String {
    let cleaned: String = raw.chars().filter(|ch| *ch != '\0').collect();
    cleaned.trim().chars().take(limit).collect()
}

/// Prior ToBot turns copied into a freshly started a3s-code process.
const TRANSCRIPT_TURNS: usize = 16;
const TRANSCRIPT_CHARS: usize = 6000;

/// A new process has an empty ACP session. Earlier stored lines go in front of this prompt.
/// A warm process already has those lines, so this stays off the next turn.
fn seed_tobot_transcript(store: &PlaneStore, session: &Session, prompt: &str) -> String {
    if !is_resident_session(&session.id) {
        return prompt.to_owned();
    }
    let prior = prior_tobot_transcript(store, &session.id);
    if prior.is_empty() {
        return prompt.to_owned();
    }
    format!("{prior}\n\n{prompt}")
}

fn prior_tobot_transcript(store: &PlaneStore, session_id: &str) -> String {
    let messages = store.messages(session_id);
    let mut rows: Vec<&Message> = messages.iter().collect();
    while rows
        .last()
        .is_some_and(|message| message.role == "assistant" && message.status == "pending")
    {
        rows.pop();
    }
    if rows.last().is_some_and(|message| message.role == "user") {
        rows.pop();
    }
    let spoken: Vec<&Message> = rows
        .into_iter()
        .filter(|message| message.status != "pending")
        .filter(|message| message.role == "user" || message.role == "assistant")
        .filter(|message| !message_plain(message).is_empty())
        .collect();
    if spoken.is_empty() {
        return String::new();
    }
    let start = spoken.len().saturating_sub(TRANSCRIPT_TURNS);
    let mut lines = Vec::new();
    for message in &spoken[start..] {
        let who = if message.role == "user" {
            "User"
        } else {
            "Assistant"
        };
        lines.push(format!("{who}: {}", message_plain(message)));
    }
    let mut body = lines.join("\n");
    let count = body.chars().count();
    if count > TRANSCRIPT_CHARS {
        body = body.chars().skip(count - TRANSCRIPT_CHARS).collect();
    }
    format!(
        "Earlier messages in this chat, oldest first. Use this list when the user asks what you talked about. If a fact is not in this list, say you do not have that record.\n\n{body}"
    )
}

fn message_plain(message: &Message) -> String {
    let from_parts = searchable_text(message.parts());
    let text = if from_parts.trim().is_empty() {
        message.searchable_text.clone()
    } else {
        from_parts
    };
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed.chars().take(500).collect()
}

/// Pictures under 8 MiB travel as ACP image blocks. Larger images stay on disk.
const IMAGE_INLINE_MAX: usize = 8 * 1024 * 1024;
/// Text inlined into the prompt. The rest stays in the workspace copy.
const TEXT_INLINE_CHARS: usize = 8000;

struct PreparedPrompt {
    text: String,
    /// `{ "type": "image", "data", "mimeType" }` blocks, after the text block.
    media: Vec<Value>,
}

enum StagedFile {
    /// Not a picture and not a composer upload. Leave it out of the prompt.
    Skip,
    /// The path was named, but the bytes could not be copied.
    Unreadable { name: String },
    /// The prompt should mention the file, and no bytes are attached.
    Noted { note: String },
    Ready {
        name: String,
        media_type: String,
        relative: String,
        bytes: Vec<u8>,
    },
}

/// Prompt text plus image blocks the model can see without calling a tool.
///
/// Composer uploads are copied under `tobot-inbox/`. A readable picture is
/// copied too, even when it is outside that upload root. A picture that cannot
/// be read is named in the prompt. Other outside paths stay out.
fn prompt_for_turn(parts: &[Value], workspace: &Path) -> PreparedPrompt {
    let text = parts
        .iter()
        .filter(|part| part.get("type").and_then(Value::as_str) == Some("text"))
        .filter_map(|part| part.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("\n");
    let mut notes = Vec::new();
    let mut media = Vec::new();
    for part in parts
        .iter()
        .filter(|part| part.get("type").and_then(Value::as_str) == Some("file"))
    {
        match stage_composer_file(part, workspace) {
            StagedFile::Skip => {}
            StagedFile::Unreadable { name } => {
                notes.push(format!("Could not read \"{name}\"."));
            }
            StagedFile::Noted { note } => notes.push(note),
            StagedFile::Ready {
                name,
                media_type,
                relative,
                bytes,
            } => notes.push(describe_file(
                &name,
                &media_type,
                &relative,
                &bytes,
                &mut media,
            )),
        }
    }
    if notes.is_empty() {
        return PreparedPrompt { text, media };
    }
    let mut body = String::from(
        "The user attached files.\n\
         Pictures in this prompt are images you can see. Text files are included below. Other files are in this workspace at the listed paths.\n",
    );
    for note in &notes {
        body.push('\n');
        body.push_str(note);
    }
    if !text.is_empty() {
        body.push_str("\n\n");
        body.push_str(&text);
    }
    PreparedPrompt { text: body, media }
}

fn describe_file(
    name: &str,
    media_type: &str,
    relative: &str,
    bytes: &[u8],
    media: &mut Vec<Value>,
) -> String {
    let mime = effective_mime(media_type, name);
    let size = bytes.len();
    if mime.starts_with("image/") {
        if let Some(block) = inline_image_block(bytes, &mime) {
            media.push(block);
            return format!(
                "Picture \"{name}\" is attached in this prompt ({mime}, {size} bytes). You can see it."
            );
        }
        return format!(
            "Picture \"{name}\" is {mime}, {size} bytes, at {relative}. It is too large to attach inline."
        );
    }
    if let Some(decoded) = inline_text(&mime, name, bytes) {
        let total = decoded.chars().count();
        let shown: String = decoded.chars().take(TEXT_INLINE_CHARS).collect();
        let trailer = if total > TEXT_INLINE_CHARS {
            format!("\n[truncated {TEXT_INLINE_CHARS}/{total} chars]")
        } else {
            String::new()
        };
        return format!(
            "Text file \"{name}\" ({mime}, {size} bytes) at {relative}:\n{shown}{trailer}"
        );
    }
    format!("File \"{name}\" is {mime}, {size} bytes, at {relative}.")
}

fn inline_image_block(bytes: &[u8], mime: &str) -> Option<Value> {
    if !mime.starts_with("image/") || bytes.len() > IMAGE_INLINE_MAX {
        return None;
    }
    Some(json!({
        "type": "image",
        "data": base64::engine::general_purpose::STANDARD.encode(bytes),
        "mimeType": mime,
    }))
}

/// UTF-8 text the model should read in the prompt. Images, video, and audio stay out.
fn inline_text(mime: &str, name: &str, bytes: &[u8]) -> Option<String> {
    if mime.starts_with("image/") || mime.starts_with("video/") || mime.starts_with("audio/") {
        return None;
    }
    if mime == "application/pdf" {
        return None;
    }
    let textual = mime.starts_with("text/")
        || mime == "application/json"
        || mime == "application/xml"
        || mime.ends_with("+json")
        || mime.ends_with("+xml")
        || text_extension(name)
        || (extension_of(name).is_empty() || mime == "application/octet-stream");
    if !textual || bytes.contains(&0) {
        return None;
    }
    std::str::from_utf8(bytes).ok().map(str::to_owned)
}

fn effective_mime(declared: &str, name: &str) -> String {
    let lower = declared.trim().to_ascii_lowercase();
    if !lower.is_empty() && lower != "application/octet-stream" {
        return lower;
    }
    match extension_of(name).as_str() {
        "png" => "image/png".to_owned(),
        "jpg" | "jpeg" => "image/jpeg".to_owned(),
        "gif" => "image/gif".to_owned(),
        "webp" => "image/webp".to_owned(),
        "bmp" => "image/bmp".to_owned(),
        "svg" => "image/svg+xml".to_owned(),
        "mp4" | "m4v" => "video/mp4".to_owned(),
        "webm" => "video/webm".to_owned(),
        "mov" => "video/quicktime".to_owned(),
        "mp3" => "audio/mpeg".to_owned(),
        "wav" => "audio/wav".to_owned(),
        "ogg" => "audio/ogg".to_owned(),
        "m4a" => "audio/mp4".to_owned(),
        "pdf" => "application/pdf".to_owned(),
        "json" => "application/json".to_owned(),
        "xml" => "application/xml".to_owned(),
        "txt" | "md" | "csv" | "log" => "text/plain".to_owned(),
        _ if lower.is_empty() => "application/octet-stream".to_owned(),
        _ => lower,
    }
}

fn text_extension(name: &str) -> bool {
    matches!(
        extension_of(name).as_str(),
        "txt"
            | "md"
            | "markdown"
            | "json"
            | "jsonl"
            | "xml"
            | "csv"
            | "tsv"
            | "yaml"
            | "yml"
            | "toml"
            | "html"
            | "htm"
            | "css"
            | "js"
            | "jsx"
            | "ts"
            | "tsx"
            | "mjs"
            | "cjs"
            | "rs"
            | "py"
            | "go"
            | "java"
            | "c"
            | "h"
            | "cpp"
            | "hpp"
            | "cc"
            | "hh"
            | "cs"
            | "rb"
            | "php"
            | "sh"
            | "bash"
            | "zsh"
            | "sql"
            | "log"
            | "ini"
            | "cfg"
            | "conf"
            | "env"
            | "acl"
            | "vue"
            | "svelte"
            | "rst"
            | "tex"
            | "lock"
            | "gitignore"
            | "dockerfile"
            | "makefile"
    )
}

fn extension_of(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name).trim();
    if let Some(ext) = base.strip_prefix('.') {
        if !ext.is_empty() && !ext.contains('.') {
            return ext.to_ascii_lowercase();
        }
    }
    match base.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() && !ext.is_empty() => ext.to_ascii_lowercase(),
        _ => String::new(),
    }
}

fn composer_upload_root() -> PathBuf {
    std::env::temp_dir().join("a3s-studio-composer")
}

fn declared_media(part: &Value) -> String {
    part.get("mediaType")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_owned()
}

fn file_part_name(part: &Value) -> String {
    inbox_file_name(
        part.get("filename")
            .and_then(Value::as_str)
            .filter(|name| !name.is_empty())
            .unwrap_or("file"),
    )
}

/// Copy one attached file into the session workspace.
/// Files already in the session workspace, and files in the composer upload
/// directory, are copied into the inbox. A readable picture is copied even
/// when it sits outside those roots. Any other outside path is skipped.
/// A failed copy is reported.
fn stage_composer_file(part: &Value, workspace: &Path) -> StagedFile {
    let name = file_part_name(part);
    let picture = image_attachment(part, &name);
    let Some(url) = part.get("url").and_then(Value::as_str) else {
        return if picture {
            StagedFile::Unreadable { name }
        } else {
            StagedFile::Skip
        };
    };
    let Some(raw) = url.strip_prefix("file://") else {
        return if picture {
            StagedFile::Unreadable { name }
        } else {
            StagedFile::Skip
        };
    };
    let source = PathBuf::from(percent_decode(raw));
    let root = composer_upload_root();
    let canonical_root = root.canonicalize().ok();
    let Ok(canonical) = source.canonicalize() else {
        if source.starts_with(&root) || picture {
            return StagedFile::Unreadable { name };
        }
        return StagedFile::Skip;
    };
    let under_upload = canonical_root
        .as_ref()
        .is_some_and(|root| canonical.starts_with(root));
    let under_workspace = workspace
        .canonicalize()
        .ok()
        .is_some_and(|root| canonical.starts_with(&root));
    let allowed = under_upload || under_workspace;
    if !canonical.is_file() {
        if allowed || picture {
            return StagedFile::Unreadable { name };
        }
        return StagedFile::Skip;
    }
    if !allowed && picture {
        return stage_picture(&canonical, workspace, &name, part);
    }
    if !allowed {
        return StagedFile::Skip;
    }
    let Some((relative, bytes)) = copy_into_inbox(&canonical, workspace, &name) else {
        return StagedFile::Unreadable { name };
    };
    StagedFile::Ready {
        name,
        media_type: declared_media(part),
        relative,
        bytes,
    }
}

fn image_attachment(part: &Value, name: &str) -> bool {
    let mime = effective_mime(&declared_media(part), name);
    mime.starts_with("image/") && mime != "image/svg+xml"
}

/// A picture outside the composer root is inlined only when it fits the image cap.
fn stage_picture(source: &Path, workspace: &Path, name: &str, part: &Value) -> StagedFile {
    let meta = match std::fs::metadata(source) {
        Ok(meta) if meta.is_file() => meta,
        _ => {
            return StagedFile::Unreadable {
                name: name.to_owned(),
            };
        }
    };
    if meta.len() > IMAGE_INLINE_MAX as u64 {
        return StagedFile::Noted {
            note: format!(
                "Picture \"{name}\" is too large to attach ({} bytes).",
                meta.len()
            ),
        };
    }
    let Some((relative, bytes)) = copy_into_inbox(source, workspace, name) else {
        return StagedFile::Unreadable {
            name: name.to_owned(),
        };
    };
    StagedFile::Ready {
        name: name.to_owned(),
        media_type: declared_media(part),
        relative,
        bytes,
    }
}

fn copy_into_inbox(source: &Path, workspace: &Path, name: &str) -> Option<(String, Vec<u8>)> {
    let bytes = std::fs::read(source).ok()?;
    let inbox = workspace.join("tobot-inbox");
    std::fs::create_dir_all(&inbox).ok()?;
    let file_name = format!("{}_{name}", new_id());
    let destination = inbox.join(&file_name);
    std::fs::write(&destination, &bytes).ok()?;
    Some((format!("tobot-inbox/{file_name}"), bytes))
}

fn inbox_file_name(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or("file");
    let cleaned: String = base
        .chars()
        .filter(|ch| *ch != '\0' && !ch.is_control())
        .take(80)
        .collect();
    let trimmed = cleaned.trim().trim_start_matches('.');
    if trimmed.is_empty() {
        "file".to_owned()
    } else {
        trimmed.to_owned()
    }
}

fn title(prompt: &str) -> String {
    let line = prompt
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("")
        .trim();
    let mut title: String = line.chars().take(TITLE_CHARS).collect();
    if line.chars().count() > TITLE_CHARS {
        title.push('…');
    }
    title
}

/// Sidebar name. The prepared model prompt is not a name: its first line is the file scaffold.
fn visible_title(parts: &[Value]) -> String {
    let text = parts
        .iter()
        .filter(|part| part.get("type").and_then(Value::as_str) == Some("text"))
        .filter_map(|part| part.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("\n");
    let from_text = title(&text);
    if !from_text.is_empty() {
        return from_text;
    }
    parts
        .iter()
        .filter(|part| part.get("type").and_then(Value::as_str) == Some("file"))
        .find_map(|part| {
            part.get("filename")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(title)
        })
        .filter(|name| !name.is_empty())
        .unwrap_or_default()
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[index + 1..index + 3]).ok();
            if let Some(byte) = hex.and_then(|hex| u8::from_str_radix(hex, 16).ok()) {
                out.push(byte);
                index += 3;
                continue;
            }
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `None` when a test hides every on-disk agent so the missing-binary error
/// can be reached while this checkout's debug build is still present.
fn discovered_agent_binary() -> Option<PathBuf> {
    #[cfg(test)]
    if !binary_lookup_enabled() {
        return None;
    }
    launcher::find_binary()
}

#[cfg(test)]
fn record_test_initialize(initialized: &Value) {
    *TEST_INITIALIZE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(initialized.clone());
}

#[cfg(test)]
fn record_test_session_new(params: &Value) {
    *TEST_SESSION_NEW
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(params.clone());
}

#[cfg(test)]
fn binary_lookup_enabled() -> bool {
    *TEST_BINARY_LOOKUP
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
static TEST_INITIALIZE: Mutex<Option<Value>> = Mutex::new(None);

#[cfg(test)]
static TEST_SESSION_NEW: Mutex<Option<Value>> = Mutex::new(None);

#[cfg(test)]
static TEST_BINARY_LOOKUP: Mutex<bool> = Mutex::new(true);

#[cfg(test)]
#[path = "document_read_route_tests.rs"]
mod document_read_route_tests;

#[cfg(test)]
#[path = "review_session_binding_tests.rs"]
mod review_session_binding_tests;

#[cfg(test)]
#[path = "question_route_tests.rs"]
mod question_route_tests;

#[cfg(test)]
#[path = "review_text_route_tests.rs"]
mod review_text_route_tests;

#[cfg(test)]
#[path = "review_repair_tests.rs"]
mod review_repair_tests;

#[cfg(test)]
#[path = "review_selection_tests.rs"]
mod review_selection_tests;

#[cfg(test)]
#[path = "review_read_failure_tests.rs"]
mod review_read_failure_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn harness_assemble_rides_on_session_new() {
        let lean = session_new_params(
            "/tmp",
            &[],
            &[],
            None,
            &[
                "system".into(),
                "tools".into(),
                "budget".into(),
                "infer".into(),
            ],
            None,
        );
        assert_eq!(
            lean["_meta"]["a3sHarness"]["components"],
            json!(["system", "tools", "budget", "infer"])
        );
        let plain = session_new_params("/tmp", &[], &[], None, &[], None);
        let mut documents = plain.clone();
        bind_studio_documents(&mut documents);
        assert_eq!(documents["_meta"]["a3sStudioDocuments"], json!(true));
        assert!(plain.get("_meta").is_none());
    }

    #[test]
    fn encyclopedia_search_runs_only_for_a_team_query() {
        let org = "0b9d2a4e-1c3f-4a5b-8d6e-7f8091a2b3c4";
        let mut calls = 0u32;
        let personal = encyclopedia_hits("personal", "hello", |_, _| {
            calls += 1;
            Ok(vec![hit("secret")])
        });
        assert!(personal.unwrap().is_empty());
        let missing = encyclopedia_hits("", "hello", |_, _| {
            calls += 1;
            Ok(vec![hit("secret")])
        });
        assert!(missing.unwrap().is_empty());
        let named = encyclopedia_hits("team_one", "hello", |_, _| {
            calls += 1;
            Ok(vec![hit("secret")])
        });
        assert!(named.unwrap().is_empty());
        let blank = encyclopedia_hits(org, "  ", |_, _| {
            calls += 1;
            Ok(vec![hit("secret")])
        });
        assert!(blank.unwrap().is_empty());
        assert_eq!(calls, 0);
        let failed = encyclopedia_hits(org, "报销", |_, _| Err("down".into()));
        assert_eq!(failed, Err("down".into()));
        let ready = encyclopedia_hits(org, "报销", |asked, query| {
            calls += 1;
            assert_eq!(asked, org);
            assert_eq!(query, "报销");
            Ok(vec![
                EncyclopediaHit {
                    name: "报销".into(),
                    place: "财务".into(),
                    body: "这是部门句子".into(),
                },
                EncyclopediaHit {
                    name: "blank".into(),
                    place: String::new(),
                    body: "   ".into(),
                },
            ])
        })
        .expect("ready");
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].name, "报销");
        assert_eq!(ready[0].body, "这是部门句子");
        assert_eq!(calls, 1);
        let named = passage_hit(&json!({
            "id": "p1",
            "heading": "# 报销",
            "folder_name": "财务",
            "body": "差旅标准"
        }))
        .expect("hit");
        assert_eq!(named.name, "报销");
        assert_eq!(named.place, "财务");
        assert_eq!(named.body, "差旅标准");
    }

    fn hit(body: &str) -> EncyclopediaHit {
        EncyclopediaHit {
            name: "note".into(),
            place: "folder".into(),
            body: body.into(),
        }
    }

    #[test]
    fn approval_picks_allow_once_and_refusal_picks_reject_once() {
        let options = vec![
            json!({ "optionId": "allow-once", "kind": "allow_once" }),
            json!({ "optionId": "reject-once", "kind": "reject_once" }),
        ];
        assert_eq!(
            permission_outcome(&options, true)["outcome"]["optionId"],
            "allow-once"
        );
        assert_eq!(
            permission_outcome(&options, false)["outcome"]["optionId"],
            "reject-once"
        );
        let questions = vec![json!({ "optionId": "question-0" })];
        assert_eq!(
            permission_outcome(&questions, true)["outcome"]["optionId"],
            "question-0"
        );
        assert_eq!(
            permission_outcome(&questions, false)["outcome"]["outcome"],
            "cancelled"
        );
    }

    #[test]
    fn live_permission_mode_keeps_each_session_and_refuses_unknown() {
        let store = PlaneStore::open(None);
        store
            .set_session_permission_mode("task-a", "bypassPermissions")
            .unwrap();
        store.set_session_permission_mode("task-a", "plan").unwrap();
        store.set_session_permission_mode("task-b", "auto").unwrap();
        store.set_session_permission_mode("task-b", "nope").unwrap();
        assert_eq!(
            store.session_permission_mode("task-a").as_deref(),
            Some("plan")
        );
        assert_eq!(
            store.session_permission_mode("task-b").as_deref(),
            Some("auto")
        );
        assert_eq!(
            resolve_live_permission_mode(
                Some("bypassPermissions"),
                Some("default"),
                "default",
                "task-a"
            ),
            "bypassPermissions"
        );
        assert_eq!(
            resolve_live_permission_mode(
                Some("auto"),
                Some("bypassPermissions"),
                "default",
                "task-b"
            ),
            "auto"
        );
        assert_eq!(
            resolve_live_permission_mode(Some("plan"), Some("auto"), "default", "task-a"),
            "plan"
        );
        assert_eq!(
            resolve_live_permission_mode(Some("nope"), Some("plan"), "auto", "task-a"),
            "plan"
        );
        let refused = resolve_live_permission_mode(Some("nope"), Some("nope"), "nope", "task-a");
        assert_ne!(refused, "default");
        assert_ne!(refused, "bypassPermissions");
        assert!(refused.is_empty());
        assert_eq!(
            resolve_live_permission_mode(None, Some("default"), "bypassPermissions", "task-a"),
            "default"
        );
        assert_eq!(
            resolve_live_permission_mode(
                Some("default"),
                Some("auto"),
                "default",
                BUILTIN_TOBOT_SESSION_ID
            ),
            "bypassPermissions"
        );
    }

    const MODE_AGENT: &str = r#"#!/usr/bin/env node
const fs = require('fs')
const path = require('path')
const rl = require('readline').createInterface({ input: process.stdin })
const trace = path.join(__dirname, 'mode-trace.json')
const rows = []
rl.on('line', (line) => {
  let message
  try { message = JSON.parse(line) } catch (e) { return }
  rows.push(message)
  fs.writeFileSync(trace, JSON.stringify(rows))
  if (message.method && Object.prototype.hasOwnProperty.call(message, 'id')) {
    process.stdout.write(JSON.stringify({ jsonrpc: '2.0', id: message.id, result: {} }) + '\n')
  }
})
"#;

    fn permission_params(tool: Value) -> Value {
        json!({
            "toolCall": tool,
            "options": [
                { "optionId": "allow-once" },
                { "optionId": "reject-once" }
            ]
        })
    }

    async fn mode_trace(path: &std::path::Path) -> Vec<Value> {
        let mut last = Vec::new();
        for _ in 0..100 {
            if let Ok(text) = std::fs::read_to_string(path) {
                if let Ok(rows) = serde_json::from_str::<Vec<Value>>(&text) {
                    let modes = rows
                        .iter()
                        .filter(|row| row["method"] == "session/set_mode")
                        .count();
                    let outcomes = rows
                        .iter()
                        .filter(|row| row["result"]["outcome"]["optionId"].is_string())
                        .count();
                    last = rows;
                    if modes >= 2 && outcomes >= 2 {
                        return last;
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        last
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn permission_denies_plan_and_unknown_off_the_default_posture() {
        let dir = std::env::temp_dir().join(format!("a3s-perm-posture-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let binary = dir.join("mode-acp");
        std::fs::write(&binary, MODE_AGENT).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let (client, inbound) = AcpClient::spawn(&binary, &[], &dir).expect("mode agent");
        let runtime = SessionRuntime {
            client,
            acp_session: "acp-session".into(),
            model: "test".into(),
            cwd: dir.to_string_lossy().into_owned(),
            mcp_servers: Vec::new(),
            tools: Vec::new(),
            plugin_selection: None,
            document_reader: None,
            wiki: String::new(),
            role: String::new(),
            components: Vec::new(),
            reasoning_effort: tokio::sync::Mutex::new(None),
            mode: tokio::sync::Mutex::new(String::new()),
            inbound: tokio::sync::Mutex::new(inbound),
        };
        let store = Arc::new(PlaneStore::open(None));
        store
            .set_session_permission_mode("task-plan", "bypassPermissions")
            .unwrap();
        store
            .set_session_permission_mode("task-plan", "plan")
            .unwrap();
        store
            .set_session_permission_mode("task-unknown", "nope")
            .unwrap();
        assert_eq!(
            store.session_permission_mode("task-plan").as_deref(),
            Some("plan")
        );
        assert!(store.session_permission_mode("task-unknown").is_none());
        let turns = TurnService::new(
            Arc::clone(&store),
            Arc::new(AgentCatalog::new(Arc::clone(&store))),
            Arc::new(SessionCatalog::new(Arc::clone(&store))),
            Arc::new(PlaneConfig {
                root: None,
                credentials: Arc::new(|_| None),
                binary: Some(binary),
                workspace_documents: None,
                schedule_mcp: None,
                mail_mcp: None,
                slug: String::new(),
                personal_wiki: None,
                cloud_agent_runtime: None,
            }),
            Arc::new(EventHub::new()),
        );
        let (_abort_tx, mut abort_rx) = watch::channel(false);
        let plan = Execution {
            session_id: "task-plan".into(),
            topic_id: "topic".into(),
            execution_id: "exec-plan".into(),
            attempt_id: 1,
            anchor_id: "anchor".into(),
        };
        let mut stream = TurnStream::capturing("msg-plan", 0);
        let plan_denied = tokio::time::timeout(
            Duration::from_secs(5),
            turns.permission(
                &plan,
                &runtime,
                &mut stream,
                json!(1),
                &permission_params(json!({
                    "toolCallId": "bash-1",
                    "title": "bash",
                    "rawInput": { "command": "rm x" }
                })),
                &mut abort_rx,
                "bypassPermissions",
            ),
        )
        .await
        .expect("plan permission hung");
        assert!(!plan_denied);
        assert_eq!(runtime.mode.lock().await.as_str(), "plan");
        *runtime.mode.lock().await = String::new();

        let unknown = Execution {
            session_id: "task-unknown".into(),
            topic_id: "topic".into(),
            execution_id: "exec-unknown".into(),
            attempt_id: 1,
            anchor_id: "anchor".into(),
        };
        let mut stream = TurnStream::capturing("msg-unknown", 0);
        let unknown_denied = tokio::time::timeout(
            Duration::from_secs(5),
            turns.permission(
                &unknown,
                &runtime,
                &mut stream,
                json!(2),
                &permission_params(json!({
                    "toolCallId": "read-1",
                    "title": "Read notes.md",
                    "kind": "read"
                })),
                &mut abort_rx,
                "nope",
            ),
        )
        .await
        .expect("unknown permission hung");
        assert!(!unknown_denied);
        assert_eq!(runtime.mode.lock().await.as_str(), "plan");

        let rows = mode_trace(&dir.join("mode-trace.json")).await;
        let modes: Vec<&str> = rows
            .iter()
            .filter(|row| row["method"] == "session/set_mode")
            .filter_map(|row| row["params"]["modeId"].as_str())
            .collect();
        let outcomes: Vec<&str> = rows
            .iter()
            .filter_map(|row| row["result"]["outcome"]["optionId"].as_str())
            .collect();
        assert_eq!(modes, ["plan", "plan"], "{rows:?}");
        assert_eq!(outcomes, ["reject-once", "reject-once"], "{rows:?}");
        assert!(rows.iter().all(|row| row["params"]["modeId"] != "default"));
        assert!(rows.iter().all(|row| row["params"]["modeId"] != "yolo"));
        runtime.client.kill().await;
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn permission_modes_ask_allow_or_refuse_without_an_extra_prompt() {
        let read = json!({ "title": "Read notes.md" });
        let edit = json!({ "title": "edit" });
        let bash = json!({ "title": "bash", "rawInput": { "command": "rm x" } });
        let unknown = json!({ "title": "custom_tool" });

        assert_eq!(acp_mode_id("default"), "default");
        assert_eq!(acp_mode_id("plan"), "plan");
        assert_eq!(acp_mode_id("auto"), "auto");
        assert_eq!(acp_mode_id("bypassPermissions"), "yolo");
        assert_eq!(acp_mode_id("acceptEdits"), "default");

        assert!(matches!(
            permission_act("default", &read),
            PermissionAct::Ask
        ));
        assert!(matches!(
            tobot_permission_act(BUILTIN_TOBOT_SESSION_ID, "default", &read),
            PermissionAct::Allow
        ));
        assert_eq!(
            execution_mode(BUILTIN_TOBOT_SESSION_ID, "default"),
            "bypassPermissions"
        );
        assert!(matches!(
            tobot_permission_act(BUILTIN_TOBOT_SESSION_ID, "default", &bash),
            PermissionAct::Allow
        ));
        assert!(matches!(
            tobot_permission_act("other-session", "default", &read),
            PermissionAct::Ask
        ));
        assert!(matches!(
            permission_act("default", &bash),
            PermissionAct::Ask
        ));

        assert!(matches!(
            permission_act("plan", &read),
            PermissionAct::Allow
        ));
        assert!(matches!(permission_act("plan", &bash), PermissionAct::Deny));
        assert!(matches!(permission_act("plan", &edit), PermissionAct::Deny));
        assert!(matches!(
            permission_act("plan", &unknown),
            PermissionAct::Deny
        ));

        assert!(matches!(
            permission_act("auto", &read),
            PermissionAct::Allow
        ));
        assert!(matches!(
            permission_act("auto", &json!({ "title": "TodoWrite" })),
            PermissionAct::Allow
        ));
        assert!(matches!(
            permission_act("auto", &json!({ "title": "update_plan" })),
            PermissionAct::Allow
        ));
        assert!(matches!(
            permission_act("plan", &json!({ "title": "update_plan" })),
            PermissionAct::Allow
        ));
        let acp_plan = json!({
            "title": "Updating plan",
            "rawInput": { "variant": "TodoWrite", "todos": [] }
        });
        assert!(matches!(
            permission_act("plan", &acp_plan),
            PermissionAct::Allow
        ));
        assert!(matches!(
            permission_act("auto", &acp_plan),
            PermissionAct::Allow
        ));
        assert!(matches!(
            permission_act("plan", &json!({ "title": "Read notes.md", "kind": "read" })),
            PermissionAct::Allow
        ));
        assert!(matches!(
            permission_act(
                "plan",
                &json!({ "title": "list the files", "kind": "execute" })
            ),
            PermissionAct::Deny
        ));
        assert!(matches!(
            permission_act("auto", &json!({ "title": "download", "kind": "fetch" })),
            PermissionAct::Ask
        ));
        assert!(matches!(
            permission_act("plan", &json!({ "title": "web_fetch", "kind": "fetch" })),
            PermissionAct::Allow
        ));
        assert!(matches!(permission_act("auto", &edit), PermissionAct::Ask));
        assert!(matches!(permission_act("auto", &bash), PermissionAct::Ask));
        assert!(matches!(
            permission_act("auto", &unknown),
            PermissionAct::Ask
        ));

        assert!(matches!(
            permission_act("bypassPermissions", &bash),
            PermissionAct::Allow
        ));
        assert!(matches!(
            permission_act("bypassPermissions", &unknown),
            PermissionAct::Allow
        ));

        for name in [
            "mcp__presenton__generate_deck",
            "mcp__presenton__read_deck",
            "mcp__presenton__edit_outline",
            "mcp__presenton__edit_slide",
        ] {
            let deck = json!({ "toolName": name });
            assert!(
                matches!(permission_act("default", &deck), PermissionAct::Allow),
                "{name}"
            );
            assert!(
                matches!(permission_act("auto", &deck), PermissionAct::Allow),
                "{name}"
            );
            assert!(
                matches!(permission_act("plan", &deck), PermissionAct::Deny),
                "{name}"
            );
        }
        assert!(matches!(
            permission_act("default", &json!({ "toolName": "generate_deck" })),
            PermissionAct::Ask
        ));
        assert!(matches!(
            permission_act("default", &json!({ "toolName": "edit_slide" })),
            PermissionAct::Ask
        ));
        assert!(matches!(
            permission_act(
                "default",
                &json!({ "toolName": "mcp__presenton__edit_slides" })
            ),
            PermissionAct::Ask
        ));
        assert!(matches!(
            permission_act(
                "auto",
                &json!({ "toolName": "mcp__presenton__read_deck_now" })
            ),
            PermissionAct::Ask
        ));
    }

    #[test]
    fn prompt_stages_composer_images_inside_the_workspace() {
        let workspace =
            std::env::temp_dir().join(format!("a3s-tobot-inbox-test-{}", std::process::id()));
        let composer = composer_upload_root();
        std::fs::create_dir_all(&workspace).expect("workspace");
        std::fs::create_dir_all(&composer).expect("composer");
        let png = b"\x89PNG-shot";
        let source = composer.join(format!("prompt-test-{}-shot.png", std::process::id()));
        let notes = composer.join(format!("prompt-test-{}-notes.txt", std::process::id()));
        let video = composer.join(format!("prompt-test-{}-clip.mp4", std::process::id()));
        let missing = composer.join(format!("prompt-test-{}-missing.txt", std::process::id()));
        std::fs::write(&source, png).expect("source");
        std::fs::write(&notes, b"alpha-note-body").expect("notes");
        std::fs::write(&video, b"\0vid").expect("video");
        let parts = vec![
            json!({ "type": "text", "text": "look" }),
            json!({
                "type": "file",
                "url": format!("file://{}", source.display()),
                "filename": "shot.png",
                "mediaType": "image/png"
            }),
            json!({
                "type": "file",
                "url": format!("file://{}", notes.display()),
                "filename": "notes.txt",
                "mediaType": "text/plain"
            }),
            json!({
                "type": "file",
                "url": format!("file://{}", video.display()),
                "filename": "clip.mp4",
                "mediaType": "video/mp4"
            }),
            json!({
                "type": "file",
                "url": format!("file://{}", missing.display()),
                "filename": "missing.txt"
            }),
            json!({ "type": "file", "url": "file:///tmp/a%20b.txt", "filename": "a b.txt" }),
        ];
        let prepared = prompt_for_turn(&parts, &workspace);
        assert!(!prepared.text.contains("Read every path"));
        assert!(
            prepared
                .text
                .contains("Pictures in this prompt are images you can see")
        );
        assert!(prepared.text.contains("Text files are included below"));
        assert!(prepared.text.contains("alpha-note-body"));
        assert!(prepared.text.contains("video/mp4"));
        assert!(prepared.text.contains("clip.mp4"));
        assert!(prepared.text.contains("Could not read \"missing.txt\""));
        assert!(prepared.text.contains("tobot-inbox/"));
        assert!(prepared.text.contains("shot.png"));
        assert!(prepared.text.contains("look"));
        assert!(!prepared.text.contains("/tmp/a b.txt"));
        assert!(!prepared.text.contains("a b.txt"));
        assert_eq!(prepared.media.len(), 1);
        assert_eq!(prepared.media[0]["type"], "image");
        assert_eq!(prepared.media[0]["mimeType"], "image/png");
        assert_eq!(
            prepared.media[0]["data"],
            base64::engine::general_purpose::STANDARD.encode(png)
        );
        let inbox = workspace.join("tobot-inbox");
        let mut found_png = false;
        let mut count = 0;
        for entry in std::fs::read_dir(&inbox).expect("inbox") {
            count += 1;
            let entry = entry.expect("entry");
            if std::fs::read(entry.path()).expect("bytes") == png {
                found_png = true;
            }
        }
        assert_eq!(count, 3);
        assert!(found_png);
        let _ = std::fs::remove_dir_all(&workspace);
        let _ = std::fs::remove_file(&source);
        let _ = std::fs::remove_file(&notes);
        let _ = std::fs::remove_file(&video);
    }

    #[test]
    fn prompt_inlines_a_readable_picture_outside_the_composer_root() {
        let workspace =
            std::env::temp_dir().join(format!("a3s-tobot-outside-{}", std::process::id()));
        let picture = std::env::temp_dir().join(format!(
            "a3s-tobot-outside-shot-{}-{}.png",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&workspace).expect("workspace");
        let png = b"\x89PNG-outside";
        std::fs::write(&picture, png).expect("picture");
        let prepared = prompt_for_turn(
            &[
                json!({ "type": "text", "text": "see" }),
                json!({
                    "type": "file",
                    "url": format!("file://{}", picture.display()),
                    "filename": "shot.png",
                    "mediaType": "image/png"
                }),
            ],
            &workspace,
        );
        assert_eq!(prepared.media.len(), 1);
        assert_eq!(prepared.media[0]["mimeType"], "image/png");
        assert!(prepared.text.contains("You can see it"));
        assert!(prepared.text.contains("see"));
        let missing = prompt_for_turn(
            &[json!({
                "type": "file",
                "url": format!("file://{}/missing-shot.png", workspace.display()),
                "filename": "missing-shot.png",
                "mediaType": "image/png"
            })],
            &workspace,
        );
        assert!(missing.media.is_empty());
        assert!(missing.text.contains("Could not read \"missing-shot.png\""));
        assert!(!missing.text.contains("You can see it"));
        let _ = std::fs::remove_dir_all(&workspace);
        let _ = std::fs::remove_file(&picture);
    }

    #[test]
    fn a_fresh_transcript_keeps_earlier_lines_and_drops_the_current_turn() {
        let dir = std::env::temp_dir().join(format!("a3s-tobot-transcript-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = PlaneStore::open(Some(dir.join("state")));
        store
            .save_message(stored_line("tobot", "user", "success", "昨天聊了价格"))
            .expect("old user");
        store
            .save_message(stored_line("tobot", "assistant", "success", "报价是 12 元"))
            .expect("old assistant");
        store
            .save_message(stored_line("tobot", "user", "success", "我们昨天聊了啥?"))
            .expect("current user");
        store
            .save_message(stored_line("tobot", "assistant", "pending", ""))
            .expect("pending");
        let prior = prior_tobot_transcript(&store, "tobot");
        assert!(prior.contains("昨天聊了价格"));
        assert!(prior.contains("报价是 12 元"));
        assert!(!prior.contains("我们昨天聊了啥?"));
        let session = Session {
            id: "tobot".into(),
            agent_id: None,
            is_name_manually_edited: true,
            name: "ToBot".into(),
            description: None,
            workspace_id: "system".into(),
            workspace: crate::agent_plane::domain::Workspace {
                id: "system".into(),
                name: "Workspace".into(),
                path: dir.display().to_string(),
                kind: "system".into(),
                order_key: "a".into(),
                created_at: "2026-10-05T00:00:00.000Z".into(),
                updated_at: "2026-10-05T00:00:00.000Z".into(),
            },
            order_key: "a".into(),
            last_activity_at: "2026-10-05T00:00:00.000Z".into(),
            created_at: "2026-10-05T00:00:00.000Z".into(),
            updated_at: "2026-10-05T00:00:00.000Z".into(),
            deleted_at: None,
        };
        let seeded = seed_tobot_transcript(&store, &session, "我们昨天聊了啥?");
        assert!(seeded.contains("Earlier messages"));
        assert!(seeded.ends_with("我们昨天聊了啥?"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn orphan_pending_tobot_rows_become_errors_when_nothing_is_live() {
        let dir = std::env::temp_dir().join(format!("a3s-tobot-orphan-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (turns, store, _sessions) = plane_fixture(&dir);
        store
            .save_message(stored_line("tobot", "assistant", "pending", ""))
            .expect("pending");
        store
            .save_message(stored_line("tobot", "assistant", "success", "你好"))
            .expect("done");
        store
            .save_message(stored_line("other", "assistant", "pending", ""))
            .expect("other");
        turns.settle_orphan_pending("tobot");
        let rows = store.messages("tobot");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].status, "error");
        assert_eq!(rows[1].status, "success");
        assert_eq!(store.messages("other")[0].status, "pending");
        turns.lock_live().insert(
            "tobot".into(),
            super::LiveTurn {
                owner: String::new(),
                execution_id: String::new(),
                status: "pending".into(),
                observed: None,
                buffered: Vec::new(),
                abort: tokio::sync::watch::channel(false).0,
                approvals: std::collections::HashMap::new(),
                steer_tx: tokio::sync::mpsc::unbounded_channel().0,
            },
        );
        store
            .save_message(stored_line("tobot", "assistant", "pending", ""))
            .expect("live pending");
        turns.settle_orphan_pending("tobot");
        assert!(
            store
                .messages("tobot")
                .iter()
                .any(|message| message.status == "pending")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn reset_tobot_clears_the_transcript_and_keeps_the_session() {
        let dir = std::env::temp_dir().join(format!("a3s-tobot-reset-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (turns, store, sessions) = plane_fixture(&dir);
        let before = sessions.ensure_builtin_tobot().expect("session");
        store
            .save_message(stored_line("tobot", "user", "success", "昨天"))
            .expect("row");
        let body = turns.reset_tobot().await.expect("reset");
        assert_eq!(body["ok"], true);
        assert!(store.messages("tobot").is_empty());
        let after = sessions.get("tobot").expect("kept");
        assert_eq!(after.id, "tobot");
        assert_eq!(after.workspace.path, before.workspace.path);
        eprintln!(
            "ORACLE reset session_id={} transcript={}",
            after.id,
            store.messages("tobot").len()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    pub(super) fn stored_line(session_id: &str, role: &str, status: &str, text: &str) -> Message {
        Message {
            id: new_id(),
            session_id: session_id.to_owned(),
            role: role.to_owned(),
            data: json!({ "parts": if text.is_empty() { Vec::<Value>::new() } else { vec![json!({ "type": "text", "text": text })] } }),
            status: status.to_owned(),
            model_id: None,
            message_snapshot: None,
            stats: None,
            delivery: None,
            searchable_text: text.to_owned(),
            runtime_resume_token: None,
            created_at: "2026-10-05T00:00:00.000Z".into(),
            updated_at: "2026-10-05T00:00:00.000Z".into(),
        }
    }

    pub(super) fn plane_fixture(
        dir: &std::path::Path,
    ) -> (Arc<TurnService>, Arc<PlaneStore>, Arc<SessionCatalog>) {
        let store = Arc::new(PlaneStore::open(Some(dir.join("state"))));
        let agents = Arc::new(AgentCatalog::new(Arc::clone(&store)));
        let sessions = Arc::new(SessionCatalog::new(Arc::clone(&store)));
        let hub = Arc::new(EventHub::new());
        let config = Arc::new(PlaneConfig {
            root: Some(dir.join("state")),
            credentials: Arc::new(|_| None),
            binary: None,
            workspace_documents: None,
            schedule_mcp: None,
            mail_mcp: None,
            slug: String::new(),
            personal_wiki: None,
            cloud_agent_runtime: None,
        });
        let turns = Arc::new(TurnService::new(
            Arc::clone(&store),
            agents,
            Arc::clone(&sessions),
            config,
            hub,
        ));
        (turns, store, sessions)
    }

    fn service_with_slug(dir: &std::path::Path, slug: &str) -> Arc<TurnService> {
        let store = Arc::new(PlaneStore::open(Some(dir.join("state"))));
        let agents = Arc::new(AgentCatalog::new(Arc::clone(&store)));
        let sessions = Arc::new(SessionCatalog::new(Arc::clone(&store)));
        let hub = Arc::new(EventHub::new());
        let config = Arc::new(PlaneConfig {
            root: Some(dir.join("state")),
            credentials: Arc::new(|_| None),
            binary: None,
            workspace_documents: None,
            schedule_mcp: None,
            mail_mcp: None,
            slug: slug.to_owned(),
            personal_wiki: None,
            cloud_agent_runtime: None,
        });
        Arc::new(TurnService::new(store, agents, sessions, config, hub))
    }

    #[test]
    fn a_member_tobot_alias_is_not_another_members_session() {
        let dir = std::env::temp_dir().join(format!(
            "a3s-tobot-alias-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let (turns, _store, sessions) = plane_fixture(&dir);
        let other = "tobot-22222222-2222-4222-8222-222222222222";
        sessions.ensure_tobot_session(other).unwrap();
        assert_eq!(turns.resolve_session_id("tobot").unwrap(), "tobot");
        assert_eq!(
            turns.resolve_session_id("plain-task").unwrap(),
            "plain-task"
        );
        assert!(turns.resolve_session_id(other).is_err());
        let room = "toboss-room-cccccccc-0000-4000-8000-000000000002";
        assert_eq!(turns.resolve_session_id(room).unwrap(), room);
        let foreign_room =
            "toboss-22222222-2222-4222-8222-222222222222-room-cccccccc-0000-4000-8000-000000000002";
        assert!(turns.resolve_session_id(foreign_room).is_err());
        sessions.ensure_toboss_session(room).unwrap();
        let scoped = service_with_slug(&dir, "toyou");
        assert!(scoped.resolve_session_id("tobot").is_err());
        assert!(scoped.resolve_session_id(other).is_err());
        assert_eq!(
            scoped.resolve_session_id("plain-task").unwrap(),
            "plain-task"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_signed_out_tenant_does_not_open_the_shared_tobot_file() {
        let dir = std::env::temp_dir().join(format!(
            "a3s-tobot-closed-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let (turns, store, sessions) = plane_fixture(&dir);
        sessions.ensure_builtin_tobot().unwrap();
        store
            .save_message(stored_line(
                "tobot",
                "assistant",
                "success",
                "legacy-secret",
            ))
            .unwrap();
        let member = "tobot-11111111-1111-4111-8111-111111111111";
        sessions.ensure_tobot_session(member).unwrap();
        store
            .save_message(stored_line(member, "assistant", "pending", ""))
            .unwrap();
        turns.settle_orphan_pending(member);
        assert_eq!(store.messages(member)[0].status, "error");
        let scoped = service_with_slug(&dir, "toyou");
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let error = runtime.block_on(scoped.ensure_tobot(&json!({ "model": "a3s::m" })));
        assert!(matches!(error, Err(PlaneError::NotFound(_))));
        assert!(
            store
                .messages("tobot")
                .iter()
                .any(|message| { message.searchable_text.contains("legacy-secret") })
        );
        assert!(
            store
                .messages(member)
                .iter()
                .all(|message| { !message.searchable_text.contains("legacy-secret") })
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn prompt_inlines_text_and_keeps_oversized_images_on_disk() {
        let mut media = Vec::new();
        let loose = describe_file(
            "readme",
            "application/octet-stream",
            "tobot-inbox/a_readme",
            b"hello readme",
            &mut media,
        );
        assert!(media.is_empty());
        assert!(loose.contains("hello readme"));

        let mut media = Vec::new();
        let pdf = describe_file(
            "brief.pdf",
            "application/pdf",
            "tobot-inbox/a_brief.pdf",
            b"%PDF",
            &mut media,
        );
        assert!(media.is_empty());
        assert!(pdf.contains("application/pdf"));
        assert!(pdf.contains("tobot-inbox/a_brief.pdf"));
        assert!(!pdf.contains("%PDF"));

        let mut media = Vec::new();
        let big = vec![9u8; IMAGE_INLINE_MAX + 1];
        let note = describe_file(
            "big.png",
            "image/png",
            "tobot-inbox/a_big.png",
            &big,
            &mut media,
        );
        assert!(media.is_empty());
        assert!(note.contains("tobot-inbox/a_big.png"));
        assert!(note.contains("too large"));

        let mut media = Vec::new();
        let long = "字".repeat(TEXT_INLINE_CHARS + 1);
        let capped = describe_file(
            "long.txt",
            "text/plain",
            "tobot-inbox/a_long.txt",
            long.as_bytes(),
            &mut media,
        );
        assert!(capped.contains(&format!(
            "[truncated {TEXT_INLINE_CHARS}/{} chars]",
            TEXT_INLINE_CHARS + 1
        )));
        assert_eq!(capped.matches('字').count(), TEXT_INLINE_CHARS);
    }

    #[test]
    fn host_meta_drops_nul_and_unknown_language() {
        let body = json!({
            "hostContext": {
                "guidelines": "带上\u{0000}这段",
                "review": "nope",
                "outputLanguage": "fr-FR"
            },
            "turnContext": {
                "directives": ["自动审查：完成前检查结论、引用和改动，并标出不确定之处。", ""],
                "research": false
            }
        });
        let host = acp_host(&body, None).expect("host");
        assert_eq!(host["guidelines"], "带上这段");
        assert_eq!(host["review"], "inline");
        assert!(host.get("outputLanguage").is_none());
        let turn = acp_turn(&body).expect("turn");
        assert_eq!(turn["research"], false);
        assert_eq!(turn["directives"].as_array().map(Vec::len), Some(1));
        assert!(acp_host(&json!({}), None).is_none());
        assert!(
            acp_turn(&json!({ "turnContext": { "directives": [], "research": false } })).is_none()
        );
    }

    #[test]
    fn agent_parameters_are_written_into_the_prompt_and_refused_when_invalid() {
        let schema = r#"{"type":"object","properties":{"days":{"type":"integer","minimum":1,"maximum":14}},"required":["days"]}"#;
        let prompt = agent_parameter_prompt(
            "请帮我请假",
            &json!({
                "turnContext": {
                    "parametersSchema": schema,
                    "parameters": { "days": 5 }
                }
            }),
        )
        .expect("prompt");
        assert!(prompt.starts_with("请帮我请假\n\n智能体参数\n"));
        assert!(prompt.contains("\"days\":5"));
        assert!(
            agent_parameter_prompt(
                "请帮我请假",
                &json!({
                    "turnContext": {
                        "parametersSchema": schema,
                        "parameters": {}
                    }
                }),
            )
            .is_err()
        );
        assert_eq!(
            agent_parameter_prompt("只要原文", &json!({})).expect("plain"),
            "只要原文"
        );
        assert_eq!(
            agent_parameter_prompt(
                "明天请假2天",
                &json!({
                    "turnContext": {
                        "parametersSchema": schema,
                        "agentRuntime": { "runtime": "cloud", "harness": "package" }
                    }
                }),
            )
            .expect("sentence stays until the form is submitted"),
            "明天请假2天"
        );
        let leave = r#"{"type":"object","properties":{"leaveType":{"type":"string","title":"请假类型","enum":["事假","病假"]},"reason":{"type":"string","title":"事由","maxLength":500}},"required":["leaveType","reason"]}"#;
        let form = parameter_openui(leave).expect("form");
        assert!(form.contains("Select(\"leaveType\", \"请假类型\", \"事假\""));
        assert!(form.contains("Textarea(\"reason\", \"事由\", \"\", true)"));
        assert!(form.contains("Button(\"submit\", \"提交\")"));
        assert!(parameter_form_pending(&json!({
            "turnContext": {
                "parametersSchema": leave,
                "agentRuntime": { "runtime": "cloud", "harness": "package", "entry": "agent/run.mjs" }
            }
        })).expect("pending"));
    }

    #[test]
    fn tobot_persona_changes_the_role_and_stays_off_task_sessions() {
        let host = acp_host(
            &json!({
                "hostContext": {
                    "personaName": "小团\u{0000}",
                    "persona": "温和、短句"
                }
            }),
            None,
        )
        .expect("host");
        let admitted = host_for_session(BUILTIN_TOBOT_SESSION_ID, Some(&host)).unwrap();
        assert_eq!(admitted["role"], "You are 小团. Personality: 温和、短句");
        assert_eq!(
            admitted["guidelines"],
            delivery_host(json!({"guidelines": tobot_guidelines(
                "Your name is 小团. Introduce yourself as 小团. Your personality is: 温和、短句. Keep that personality in how you speak and decide."
            )}))["guidelines"]
        );
        assert!(admitted.get("persona").is_none());
        assert!(admitted.get("personaName").is_none());

        let named = acp_host(
            &json!({ "hostContext": { "personaName": "小团", "persona": "" } }),
            None,
        )
        .unwrap();
        let renamed = host_for_session(BUILTIN_TOBOT_SESSION_ID, Some(&named)).unwrap();
        assert_eq!(renamed["role"], "You are 小团");
        assert_eq!(
            renamed["guidelines"],
            delivery_host(
                json!({"guidelines": tobot_guidelines("Your name is 小团. Introduce yourself as 小团.")})
            )["guidelines"]
        );

        let plain = host_for_session(BUILTIN_TOBOT_SESSION_ID, None).unwrap();
        assert_eq!(plain["role"], "You are ToBot");
        assert_eq!(
            plain["guidelines"],
            delivery_host(json!({"guidelines": tobot_guidelines(TOBOT_GUIDELINES)}))["guidelines"]
        );
        let boss = host_for_session(BUILTIN_TOBOSS_SESSION_ID, None).unwrap();
        assert_eq!(boss["role"], "You are ToBot");
        assert_eq!(
            boss["guidelines"],
            delivery_host(json!({"guidelines": tobot_guidelines(TOBOSS_GUIDELINES)}))["guidelines"]
        );
        let task = host_for_session("other", Some(&host)).unwrap();
        assert!(task.get("role").is_none());
        assert_eq!(task["personaName"], "小团");
    }

    #[test]
    fn titles_use_the_first_line() {
        assert_eq!(title("\n  fix the build  \nmore"), "fix the build");
        assert_eq!(title(&"x".repeat(50)).chars().count(), TITLE_CHARS + 1);
    }

    #[test]
    fn titles_use_the_user_sentence_not_the_file_scaffold() {
        let parts = json!([
            {"type": "text", "text": "改成简洁的周报"},
            {"type": "file", "filename": "TC02_原始周报.txt", "mediaType": "text/plain"}
        ]);
        let scaffold = "The user attached files.\n\n改成简洁的周报";
        assert_eq!(title(scaffold), "The user attached files.");
        assert_eq!(visible_title(parts.as_array().unwrap()), "改成简洁的周报");
    }

    #[test]
    fn titles_use_the_file_name_when_the_turn_has_no_sentence() {
        let parts = json!([
            {"type": "file", "filename": "TC02_原始周报.txt", "mediaType": "text/plain"}
        ]);
        assert_eq!(
            visible_title(parts.as_array().unwrap()),
            "TC02_原始周报.txt"
        );
        assert_eq!(visible_title(&[]), "");
    }

    #[test]
    fn session_launch_announces_the_schedule_task_server() {
        let config = PlaneConfig {
            root: None,
            credentials: Arc::new(|_| None),
            binary: None,
            workspace_documents: None,
            schedule_mcp: Some(crate::agent_plane::schedule_mcp::ScheduleMcpEndpoint {
                base_url: "http://127.0.0.1:9/".into(),
                token: "plane-token".into(),
            }),
            mail_mcp: None,
            slug: String::new(),
            personal_wiki: None,
            cloud_agent_runtime: None,
        };
        let servers = super::installed_mcp_servers(&config, &[]);
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0]["type"], "http");
        assert_eq!(servers[0]["name"], "schedule-task");
        assert_eq!(servers[0]["url"], "http://127.0.0.1:9/mcp/schedule-task");
        assert_eq!(servers[0]["headers"][0]["value"], "Bearer plane-token");
        assert!(!servers[0]["url"].as_str().unwrap().contains("plane-token"));
    }

    #[test]
    fn session_launch_announces_member_mail_beside_schedule() {
        let config = PlaneConfig {
            root: None,
            credentials: Arc::new(|_| None),
            binary: None,
            workspace_documents: None,
            schedule_mcp: Some(crate::agent_plane::schedule_mcp::ScheduleMcpEndpoint {
                base_url: "http://127.0.0.1:9".into(),
                token: "plane-token".into(),
            }),
            mail_mcp: Some(crate::agent_plane::mail_mcp::MailMcpEndpoint {
                base_url: "http://127.0.0.1:9".into(),
                token: "plane-token".into(),
                slug: "toyou".into(),
                origin: "http://127.0.0.1:3000".into(),
            }),
            slug: String::new(),
            personal_wiki: None,
            cloud_agent_runtime: None,
        };
        let servers = super::installed_mcp_servers(&config, &[]);
        assert_eq!(servers.len(), 3);
        assert_eq!(servers[1]["name"], "member-mail");
        assert_eq!(servers[1]["url"], "http://127.0.0.1:9/mcp/member-mail");
        assert_eq!(servers[1]["headers"][0]["value"], "Bearer plane-token");
        assert!(!servers[1].to_string().contains("a3s_ms_"));
        assert_eq!(servers[2]["name"], "member-work");
        assert_eq!(servers[2]["url"], "http://127.0.0.1:9/mcp/member-work");
        assert_eq!(servers[2]["headers"][0]["value"], "Bearer plane-token");
        assert!(servers.iter().all(|server| server["name"] != "presenton"));
    }

    #[test]
    fn session_launch_announces_presenton_only_while_the_plugin_is_installed() {
        let tenant =
            std::env::temp_dir().join(format!("a3s-presenton-session-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tenant);
        let config = PlaneConfig {
            root: Some(tenant.join("agent-plane")),
            credentials: Arc::new(|_| None),
            binary: None,
            workspace_documents: None,
            schedule_mcp: Some(crate::agent_plane::schedule_mcp::ScheduleMcpEndpoint {
                base_url: "http://127.0.0.1:9".into(),
                token: "plane-token".into(),
            }),
            mail_mcp: Some(crate::agent_plane::mail_mcp::MailMcpEndpoint {
                base_url: "http://127.0.0.1:9".into(),
                token: "plane-token".into(),
                slug: "toyou".into(),
                origin: "http://127.0.0.1:3000".into(),
            }),
            slug: String::new(),
            personal_wiki: None,
            cloud_agent_runtime: None,
        };
        let absent = super::installed_mcp_servers(&config, &[]);
        assert!(absent.iter().all(|server| server["name"] != "presenton"));

        let plugin = tenant
            .join("capabilities/plugins")
            .join(crate::agent_plane::presenton_mcp::SLUG);
        std::fs::create_dir_all(&plugin).unwrap();
        std::fs::write(
            plugin.join(".a3s-package-id"),
            crate::agent_plane::presenton_mcp::PACKAGE_ID,
        )
        .unwrap();
        std::fs::write(
            plugin.join("a3s-use-extension.acl"),
            "extension \"a3s/presenton\" {}\n",
        )
        .unwrap();
        let servers = super::installed_mcp_servers(&config, &[]);
        assert_eq!(servers.len(), 4);
        assert_eq!(servers[3]["name"], "presenton");
        assert_eq!(servers[3]["url"], "http://127.0.0.1:9/mcp/presenton");
        assert_eq!(servers[3]["headers"][0]["value"], "Bearer plane-token");
        assert!(!servers[3]["url"].as_str().unwrap().contains("plane-token"));

        let _ = std::fs::remove_dir_all(&plugin);
        let removed = super::installed_mcp_servers(&config, &[]);
        assert!(removed.iter().all(|server| server["name"] != "presenton"));
        let _ = std::fs::remove_dir_all(&tenant);
    }

    const FAKE_AGENT: &str = super::super::fake_acp::SCRIPT;

    pub(super) struct Harness {
        pub(super) turns: Arc<TurnService>,
        pub(super) store: Arc<PlaneStore>,
        pub(super) hub: Arc<EventHub>,
        pub(super) session_id: String,
        pub(super) dir: std::path::PathBuf,
    }

    pub(super) fn harness(name: &str, permission_mode: &str) -> Option<Harness> {
        harness_with(name, permission_mode, None)
    }

    fn harness_with(
        name: &str,
        permission_mode: &str,
        personal_wiki: Option<crate::agent_plane::PersonalWikiLookup>,
    ) -> Option<Harness> {
        if std::process::Command::new("node")
            .arg("--version")
            .output()
            .is_err()
        {
            eprintln!("skipping: node is not installed");
            return None;
        }
        let dir = std::env::temp_dir().join(format!("a3s-plane-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let binary = dir.join("fake-acp");
        std::fs::write(&binary, FAKE_AGENT).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let store = Arc::new(PlaneStore::open(Some(dir.join("state"))));
        let agents = Arc::new(AgentCatalog::new(Arc::clone(&store)));
        let sessions = Arc::new(SessionCatalog::new(Arc::clone(&store)));
        let hub = Arc::new(EventHub::new());
        let config = Arc::new(PlaneConfig {
            root: Some(dir.join("state")),
            credentials: Arc::new(|_| {
                Some(crate::providers::ProviderCredentials {
                    base_url: "https://example.test/v1".into(),
                    api_key: "sk-test".into(),
                })
            }),
            binary: Some(binary),
            workspace_documents: None,
            schedule_mcp: None,
            mail_mcp: None,
            slug: String::new(),
            personal_wiki,
            cloud_agent_runtime: None,
        });
        let agent = agents
            .create(&json!({
                "type": "a3s-code",
                "name": "Helper",
                "model": "openai::gpt-test",
                "configuration": { "permission_mode": permission_mode },
            }))
            .unwrap();
        let session = sessions
            .create(&json!({ "agentId": agent.id, "name": "", "workspace": { "type": "system" } }))
            .unwrap();
        let turns = Arc::new(TurnService::new(
            Arc::clone(&store),
            agents,
            sessions,
            config,
            Arc::clone(&hub),
        ));
        Some(Harness {
            turns,
            store,
            hub,
            session_id: session.id,
            dir,
        })
    }

    /// Same harness, with the signed-in gateway credential lookup and a catalog alias.
    fn gateway_harness(name: &str) -> Option<Harness> {
        if std::process::Command::new("node")
            .arg("--version")
            .output()
            .is_err()
        {
            eprintln!("skipping: node is not installed");
            return None;
        }
        let dir = std::env::temp_dir().join(format!("a3s-plane-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let binary = dir.join("fake-acp");
        std::fs::write(&binary, FAKE_AGENT).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let store = Arc::new(PlaneStore::open(Some(dir.join("state"))));
        let agents = Arc::new(AgentCatalog::new(Arc::clone(&store)));
        let sessions = Arc::new(SessionCatalog::new(Arc::clone(&store)));
        let hub = Arc::new(EventHub::new());
        let token = "a3s_ms_signed_in_member".to_owned();
        let config = Arc::new(PlaneConfig {
            root: Some(dir.join("state")),
            credentials: Arc::new(move |provider| {
                crate::providers::signed_in_gateway_credentials(provider, Some(&token))
            }),
            binary: Some(binary),
            workspace_documents: None,
            schedule_mcp: None,
            mail_mcp: None,
            slug: String::new(),
            personal_wiki: None,
            cloud_agent_runtime: None,
        });
        let agent = agents
            .create(&json!({
                "type": "a3s-code",
                "name": "Helper",
                "model": "a3s::deepseek-v4-flash",
                "configuration": { "permission_mode": "default" },
            }))
            .unwrap();
        let session = sessions
            .create(&json!({ "agentId": agent.id, "name": "", "workspace": { "type": "system" } }))
            .unwrap();
        let turns = Arc::new(TurnService::new(
            Arc::clone(&store),
            agents,
            sessions,
            config,
            Arc::clone(&hub),
        ));
        Some(Harness {
            turns,
            store,
            hub,
            session_id: session.id,
            dir,
        })
    }

    pub(super) fn message(text: &str) -> Value {
        json!({ "userMessageParts": [{ "type": "text", "text": text }] })
    }

    /// Collects events until the turn ends, running `on_event` for each one.
    pub(super) async fn until_done(
        events: &mut broadcast::Receiver<Value>,
        mut on_event: impl FnMut(&Value),
    ) -> Vec<Value> {
        let mut seen = Vec::new();
        loop {
            let event = tokio::time::timeout(Duration::from_secs(10), events.recv())
                .await
                .expect("turn timed out")
                .unwrap();
            on_event(&event);
            let channel = event["channel"].as_str().unwrap_or("").to_owned();
            seen.push(event);
            if channel == "ai.stream.done" || channel == "ai.stream.error" {
                return seen;
            }
        }
    }

    fn chunk_types(events: &[Value]) -> Vec<String> {
        events
            .iter()
            .filter(|event| event["channel"] == "ai.stream.chunk")
            .map(|event| {
                event["payload"]["chunk"]["type"]
                    .as_str()
                    .unwrap()
                    .to_owned()
            })
            .collect()
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn an_aix_runtime_result_is_stored_without_starting_acp() {
        let Some(h) = harness("aix-output", "default") else {
            return;
        };
        let curl_log = h.dir.join("curl-log");
        let curl = h.dir.join("curl");
        std::fs::write(
            &curl,
            format!(
                "#!/bin/sh\necho 1 >> '{}'\nprintf '%s' '{{\"result\":{{\"text\":\"晴\"}}}}'\n",
                curl_log.display()
            ),
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&curl, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        struct RestorePath(Option<std::ffi::OsString>);
        impl Drop for RestorePath {
            fn drop(&mut self) {
                match self.0.take() {
                    Some(value) => unsafe { std::env::set_var("PATH", value) },
                    None => unsafe { std::env::remove_var("PATH") },
                }
            }
        }
        let previous = std::env::var_os("PATH");
        let mut path = std::ffi::OsString::from(h.dir.as_os_str());
        path.push(":");
        if let Some(rest) = previous.as_ref() {
            path.push(rest);
        }
        let _restore = RestorePath(previous);
        // SAFETY: RestorePath puts the previous PATH back when this test ends.
        unsafe { std::env::set_var("PATH", &path) };
        let mut events = h.hub.subscribe();
        let forged = json!({
            "userMessageParts": [{ "type": "text", "text": "上海天气" }],
            "turnContext": {
                "directives": [],
                "research": false,
                "agentId": "a3s-vercel-weather",
                "agentRuntime": { "runtime": "remote", "card": "https://agents.example.com/card" }
            },
            "aixOutput": "多云"
        });
        let refused = h.turns.open(&h.session_id, &forged);
        assert!(refused.is_err(), "a client aixOutput is not a host result");
        assert!(h.store.messages(&h.session_id).is_empty());
        let prepared = super::super::controllers::prepare_edge_turn(forged, None)
            .await
            .expect("host remote");
        assert_eq!(prepared["aixOutput"], "晴");
        assert_ne!(prepared["aixOutput"], "多云");
        let calls = std::fs::read_to_string(&curl_log).unwrap_or_default();
        assert_eq!(calls.lines().count(), 1, "{calls}");
        let ack = h.turns.open(&h.session_id, &prepared).unwrap();
        assert_eq!(ack["mode"], "started");
        let seen = until_done(&mut events, |_| {}).await;
        assert!(chunk_types(&seen).contains(&"text-delta".to_owned()));
        assert_eq!(seen.last().unwrap()["payload"]["status"], "success");
        let rows = h.store.messages(&h.session_id);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1].status, "success");
        assert!(rows[1].searchable_text.contains("晴"));
        assert!(!rows[1].searchable_text.contains("多云"));
        assert!(!h.turns.is_live(&h.session_id));
        assert!(!h.dir.join("turn-trace.json").exists());
        assert_eq!(
            std::fs::read_to_string(&curl_log).unwrap().lines().count(),
            1
        );
        let missing = json!({
            "userMessageParts": [{ "type": "text", "text": "再问" }],
            "turnContext": {
                "directives": [],
                "research": false,
                "agentRuntime": { "runtime": "edge" }
            }
        });
        assert!(h.turns.open(&h.session_id, &missing).is_err());
        let _ = std::fs::remove_dir_all(&h.dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_forged_edge_output_does_not_start_when_limits_cannot_be_enforced() {
        let Some(h) = harness("edge-package", "default") else {
            return;
        };
        let marker = h.dir.join("node-starts");
        let script = format!(
            "const http=require('http');const fs=require('fs');fs.appendFileSync({marker},'1\\n');\
const server=http.createServer((req,res)=>{{req.on('data',()=>{{}});req.on('end',()=>{{const payload=String(req.url).includes('events:page')?JSON.stringify({{events:[{{sequence:1,type:'text_delta',text:'宿主'}},{{sequence:2,type:'text_delta',text:'晴'}},{{sequence:3,type:'completed'}}]}}):JSON.stringify({{accepted:true}});res.writeHead(200,{{'content-type':'application/json','content-length':Buffer.byteLength(payload),'connection':'close'}});res.end(payload);}});}});\
server.listen(0,'127.0.0.1',()=>{{process.stdout.write('port '+server.address().port+'\\n');}});\n",
            marker = serde_json::to_string(&marker.to_string_lossy()).unwrap()
        );
        let files = vec![a3s_use_reconcile::PackageFile {
            path: "agent/run.cjs".into(),
            bytes: script.into_bytes(),
        }];
        let digest = a3s_use_reconcile::files_sha256(&files).unwrap();
        let spec = a3s_use_reconcile::PackageSpec {
            package_id: "use/a3s/weather".into(),
            surfaces: vec![a3s_use_reconcile::SurfaceSpec {
                kind: a3s_use_reconcile::SurfaceKind::Agent,
                id: "weather".into(),
                sha256: digest.clone(),
                entry: Some("agent/run.cjs".into()),
                payload: a3s_use_reconcile::Payload::Files(files),
            }],
        };
        let root = h.dir.join("tenant");
        let caps = root.join("capabilities");
        std::fs::create_dir_all(caps.join("plugins/weather")).unwrap();
        std::fs::write(
            caps.join("plugins/weather/.a3s-package-id"),
            "use/a3s/weather",
        )
        .unwrap();
        a3s_use_reconcile::ReconcileStore::open(root.join("package-generations"))
            .unwrap()
            .apply_sync(&spec)
            .unwrap();
        let body = json!({
            "userMessageParts": [{ "type": "text", "text": "上海天气" }],
            "turnContext": {
                "directives": [],
                "research": false,
                "agentId": "weather",
                "agentRuntime": {
                    "runtime": "edge",
                    "harness": "package",
                    "entry": "agent/run.cjs",
                    "resources": { "cpuMillis": 500, "memoryBytes": 268435456 }
                }
            },
            "aixOutput": "多云"
        });
        let installed = crate::capabilities::installed_agent(&caps, "weather").unwrap();
        assert_eq!(installed.release_identity, digest);
        assert_ne!(installed.release_identity, format!("sha256:{:0>64}", "ab"));
        let command = super::super::controllers::edge_command(&body, &installed.release_identity);
        let again = super::super::controllers::edge_command(&body, &installed.release_identity);
        assert_eq!(command["identity"]["agent_release_identity"], digest);
        assert_ne!(command["identity"]["session_id"], "run");
        assert_ne!(command["identity"]["run_id"], "run");
        assert_ne!(
            command["identity"]["session_id"],
            command["identity"]["run_id"]
        );
        assert_ne!(command["identity"]["run_id"], again["identity"]["run_id"]);
        struct RestoreBoxCli(Option<std::ffi::OsString>);
        impl Drop for RestoreBoxCli {
            fn drop(&mut self) {
                match self.0.take() {
                    Some(value) => unsafe { std::env::set_var("A3S_BOX_CLI", value) },
                    None => unsafe { std::env::remove_var("A3S_BOX_CLI") },
                }
            }
        }
        let _restore_box = RestoreBoxCli(std::env::var_os("A3S_BOX_CLI"));
        unsafe { std::env::set_var("A3S_BOX_CLI", h.dir.join("missing-a3s-box")) };
        let refused = super::super::controllers::prepare_edge_turn(body.clone(), Some(&caps))
            .await
            .expect_err("box");
        let message = refused.to_string();
        assert!(
            message.contains("a3s-box is required to enforce the declared cpu and memory limits"),
            "{message}"
        );
        assert!(!marker.exists(), "the package process must not start");
        assert!(h.turns.open(&h.session_id, &body).is_err());
        assert!(h.store.messages(&h.session_id).is_empty());
        assert!(!h.dir.join("turn-trace.json").exists());
        let _ = std::fs::remove_dir_all(&h.dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_builtin_harness_is_refused_before_a_success_message() {
        let Some(h) = harness("builtin-closed", "default") else {
            return;
        };
        let body = json!({
            "userMessageParts": [{ "type": "text", "text": "上海天气" }],
            "turnContext": {
                "directives": [],
                "research": false,
                "agentRuntime": {
                    "runtime": "edge",
                    "harness": "builtin",
                    "resources": { "cpuMillis": 500, "memoryBytes": 268435456 }
                }
            }
        });
        let refused = super::super::controllers::prepare_edge_turn(body.clone(), None)
            .await
            .expect_err("builtin");
        let message = refused.to_string();
        assert!(
            message.contains("a builtin harness is not available"),
            "{message}"
        );
        assert!(!message.contains("上海天气"), "{message}");
        assert!(h.turns.open(&h.session_id, &body).is_err());
        assert!(h.store.messages(&h.session_id).is_empty());
        assert!(!h.dir.join("turn-trace.json").exists());
        let _ = std::fs::remove_dir_all(&h.dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn an_installed_review_agent_keeps_the_segmented_host_turn() {
        let root = std::env::temp_dir().join(format!("a3s-review-edge-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let acl = r#"extension "a3s/review" {
  schema_version = 3
  version        = "1.0.0"
  route          = "review"
  requires_use   = ">=0.3.0, <0.4.0"
  actions        = ["read"]
  repository {
    url      = "https://github.com/A3S-Lab/Use-Registry"
    revision = "c1dcedcd84794ce4bb0010c8d17a7eff4c62d212"
  }
  skill "review" {
    path = "skills/review/SKILL.md"
    host = "docx-review"
  }
  agent "legal-expert" {
    harness        = "builtin"
    instructions   = "agents/legal-expert/INSTRUCTIONS.md"
    runtime        = "edge"
    requires_skill = ["review"]
    optional       = false
    resources {
      cpu_millis   = 500
      memory_bytes = 268435456
    }
  }
}
"#;
        let files = [
            ("a3s-use-extension.acl", acl),
            ("agents/legal-expert/INSTRUCTIONS.md", "你是合同审查员。"),
            ("skills/review/SKILL.md", "按条款写 a3s-review-finding。"),
        ];
        let store =
            a3s_use_reconcile::ReconcileStore::open(root.join("package-generations")).unwrap();
        let spec = a3s_use_reconcile::PackageSpec {
            package_id: "use/a3s/review".into(),
            surfaces: vec![
                review_surface(
                    a3s_use_reconcile::SurfaceKind::Skill,
                    "review",
                    &files,
                    &["a3s-use-extension.acl", "skills/review/SKILL.md"],
                ),
                review_surface(
                    a3s_use_reconcile::SurfaceKind::Agent,
                    "legal-expert",
                    &files,
                    &[
                        "a3s-use-extension.acl",
                        "agents/legal-expert/INSTRUCTIONS.md",
                    ],
                ),
            ],
        };
        let published = store.apply_sync(&spec).unwrap();
        store
            .publish_selection_sync("studio-installed-plugins", None, &[published.cursor()])
            .unwrap();
        let caps = root.join("capabilities");
        std::fs::create_dir_all(caps.join("plugins/a3s-review")).unwrap();
        std::fs::write(
            caps.join("plugins/a3s-review/.a3s-package-id"),
            "use/a3s/review",
        )
        .unwrap();
        let body = json!({
            "userMessageParts": [{ "type": "text", "text": "使用智能体 法务专家：\n请审查这份合同\n/tmp/contract.docx" }],
            "turnContext": {
                "agentId": "a3s-review",
                "agentRuntime": {
                    "runtime": "edge",
                    "harness": "builtin",
                    "resources": { "cpuMillis": 500, "memoryBytes": 268435456 }
                }
            }
        });
        let prepared = super::super::controllers::prepare_edge_turn(body.clone(), Some(&caps))
            .await
            .expect("review host");
        assert!(prepared.get("aixOutput").is_none());
        assert!(prepared.get("aixHostSeal").is_none());
        let _ = std::fs::remove_dir_all(&root);

        fn review_surface(
            kind: a3s_use_reconcile::SurfaceKind,
            id: &str,
            files: &[(&str, &str)],
            keep: &[&str],
        ) -> a3s_use_reconcile::SurfaceSpec {
            let payload = files
                .iter()
                .filter(|(path, _)| keep.contains(path))
                .map(|(path, text)| a3s_use_reconcile::PackageFile {
                    path: (*path).to_owned(),
                    bytes: text.as_bytes().to_vec(),
                })
                .collect::<Vec<_>>();
            let digest = a3s_use_reconcile::files_sha256(&payload).unwrap();
            a3s_use_reconcile::SurfaceSpec {
                kind,
                id: id.to_owned(),
                sha256: digest,
                entry: None,
                payload: a3s_use_reconcile::Payload::Files(payload),
            }
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn host_deltas_stream_as_separate_text_chunks() {
        let Some(h) = harness("deltas", "default") else {
            return;
        };
        let seal = super::super::controllers::seal_host_deltas(&["宿主".into(), "晴".into()])
            .expect("seal");
        let body = json!({
            "userMessageParts": [{ "type": "text", "text": "上海天气" }],
            "turnContext": {
                "directives": [],
                "research": false,
                "agentId": "weather",
                "agentRuntime": {
                    "runtime": "edge",
                    "harness": "package",
                    "entry": "agent/run.cjs"
                }
            },
            "aixOutput": "多云",
            "aixHostSeal": seal
        });
        let mut events = h.hub.subscribe();
        h.turns.open(&h.session_id, &body).unwrap();
        let seen = until_done(&mut events, |_| {}).await;
        let deltas: Vec<_> = seen
            .iter()
            .filter(|event| {
                event["channel"] == "ai.stream.chunk"
                    && event["payload"]["chunk"]["type"] == "text-delta"
            })
            .map(|event| event["payload"]["chunk"]["delta"].as_str().unwrap_or(""))
            .collect();
        assert_eq!(deltas, ["宿主", "晴"]);
        let rows = h.store.messages(&h.session_id);
        assert_eq!(rows.len(), 2);
        assert!(rows[1].searchable_text.contains("宿主"));
        assert!(rows[1].searchable_text.contains("晴"));
        assert!(!rows[1].searchable_text.contains("多云"));
        assert!(!h.dir.join("turn-trace.json").exists());
        let _ = std::fs::remove_dir_all(&h.dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_turn_streams_and_persists_the_reply() {
        let Some(h) = harness("reply", "default") else {
            return;
        };
        let mut events = h.hub.subscribe();
        let ack = h.turns.open(&h.session_id, &message("hi there")).unwrap();
        assert_eq!(ack["mode"], "started");
        let anchor = ack["activeExecutions"][0]["anchorMessageId"]
            .as_str()
            .unwrap()
            .to_owned();
        assert_eq!(ack["reservedMessages"][1]["id"], anchor);

        let seen = until_done(&mut events, |_| {}).await;
        assert_eq!(
            chunk_types(&seen),
            [
                "start",
                "start-step",
                "reasoning-start",
                "reasoning-delta",
                "reasoning-end",
                "text-start",
                "text-delta",
                "text-end",
                "finish-step",
                "finish",
            ]
        );
        let done = seen.last().unwrap();
        assert_eq!(done["payload"]["status"], "success");
        assert_eq!(done["payload"]["anchorMessageId"], anchor);

        let rows = h.store.messages(&h.session_id);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1].status, "success");
        assert_eq!(rows[1].searchable_text, "hello");
        assert_eq!(h.store.session(&h.session_id).unwrap().name, "hi there");
        assert_eq!(h.turns.attach(&h.session_id)["status"], "not-found");
    }

    /// The scripted peer speaks the v9.1.2 session sequence, including a diff that names the file.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_v912_turn_keeps_prose_the_settled_tool_and_the_written_file() {
        let Some(h) = harness("write-file", "default") else {
            return;
        };
        let mut events = h.hub.subscribe();
        let ack = h
            .turns
            .open(&h.session_id, &message("please write-file the notes"))
            .unwrap();
        assert_eq!(ack["mode"], "started");
        let seen = until_done(&mut events, |_| {}).await;
        assert_eq!(seen.last().unwrap()["payload"]["status"], "success");

        let rows = h.store.messages(&h.session_id);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1].status, "success");
        assert_eq!(rows[1].searchable_text, "先写文件。\n写好了。");
        let parts = rows[1].parts();
        let kinds: Vec<&str> = parts
            .iter()
            .map(|part| part["type"].as_str().unwrap_or(""))
            .collect();
        assert_eq!(
            kinds,
            ["step-start", "reasoning", "text", "dynamic-tool", "text"]
        );
        assert_eq!(parts[1]["text"], "thinking");
        assert_eq!(parts[2]["text"], "先写文件。");
        assert_eq!(parts[3]["toolName"], "edit");
        assert_eq!(parts[3]["state"], "output-available");
        assert_eq!(parts[3]["files"], json!(["/workspace/notes.md"]));
        assert_eq!(parts[4]["text"], "写好了。");
    }

    fn recorded_modes(dir: &std::path::Path) -> Vec<String> {
        let value: Value = std::fs::read(dir.join("turn-trace.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or(json!({}));
        value
            .get("modes")
            .and_then(Value::as_array)
            .map(|modes| {
                modes
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default()
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn approvals_round_trip_through_the_user() {
        let Some(h) = harness("approve", "default") else {
            return;
        };
        let mut events = h.hub.subscribe();
        h.turns.open(&h.session_id, &message("ask first")).unwrap();
        let turns = Arc::clone(&h.turns);
        let seen = until_done(&mut events, |event| {
            let chunk = &event["payload"]["chunk"];
            if chunk["type"] == "tool-approval-request" {
                assert!(turns.respond_approval(chunk["approvalId"].as_str().unwrap(), true));
            }
        })
        .await;
        assert!(seen.iter().any(|event| {
            event["channel"] == "topic.status"
                && event["payload"]["status"]["status"] == "awaiting-approval"
        }));
        let rows = h.store.messages(&h.session_id);
        assert_eq!(rows[1].searchable_text, "allowed");
        assert_eq!(
            recorded_modes(&h.dir),
            ["default"],
            "ask-each-time is sent once so the agent does not run tools on its own"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn full_access_approves_without_asking() {
        let Some(h) = harness("bypass", "bypassPermissions") else {
            return;
        };
        let mut events = h.hub.subscribe();
        h.turns.open(&h.session_id, &message("ask first")).unwrap();
        let seen = until_done(&mut events, |_| {}).await;
        assert!(
            !chunk_types(&seen)
                .iter()
                .any(|kind| kind == "tool-approval-request")
        );
        assert_eq!(
            h.store.messages(&h.session_id)[1].searchable_text,
            "allowed"
        );
        assert_eq!(recorded_modes(&h.dir), ["yolo"]);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn smart_approval_runs_a_read_without_asking() {
        let Some(h) = harness("auto-read", "auto") else {
            return;
        };
        let mut events = h.hub.subscribe();
        h.turns
            .open(&h.session_id, &message("ask-read the notes"))
            .unwrap();
        let seen = until_done(&mut events, |_| {}).await;
        assert!(
            !chunk_types(&seen)
                .iter()
                .any(|kind| kind == "tool-approval-request")
        );
        assert_eq!(
            h.store.messages(&h.session_id)[1].searchable_text,
            "allowed"
        );
        assert_eq!(recorded_modes(&h.dir), ["auto"]);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn smart_approval_still_asks_before_a_command() {
        let Some(h) = harness("auto-bash", "auto") else {
            return;
        };
        let mut events = h.hub.subscribe();
        h.turns.open(&h.session_id, &message("ask first")).unwrap();
        let turns = Arc::clone(&h.turns);
        let seen = until_done(&mut events, |event| {
            let chunk = &event["payload"]["chunk"];
            if chunk["type"] == "tool-approval-request" {
                assert!(turns.respond_approval(chunk["approvalId"].as_str().unwrap(), true));
            }
        })
        .await;
        assert!(chunk_types(&seen).contains(&"tool-approval-request".to_owned()));
        assert_eq!(
            h.store.messages(&h.session_id)[1].searchable_text,
            "allowed"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn plan_only_refuses_a_command_without_a_prompt() {
        let Some(h) = harness("plan-bash", "plan") else {
            return;
        };
        let mut events = h.hub.subscribe();
        h.turns.open(&h.session_id, &message("ask first")).unwrap();
        let seen = until_done(&mut events, |_| {}).await;
        assert!(
            !chunk_types(&seen)
                .iter()
                .any(|kind| kind == "tool-approval-request")
        );
        assert_eq!(h.store.messages(&h.session_id)[1].searchable_text, "denied");
        assert_eq!(recorded_modes(&h.dir), ["plan"]);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn plan_only_allows_a_read_without_a_prompt() {
        let Some(h) = harness("plan-read", "plan") else {
            return;
        };
        let mut events = h.hub.subscribe();
        h.turns
            .open(&h.session_id, &message("ask-read the notes"))
            .unwrap();
        let seen = until_done(&mut events, |_| {}).await;
        assert!(
            !chunk_types(&seen)
                .iter()
                .any(|kind| kind == "tool-approval-request")
        );
        assert_eq!(
            h.store.messages(&h.session_id)[1].searchable_text,
            "allowed"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_mode_change_applies_to_the_next_action_in_the_same_turn() {
        let Some(h) = harness("mode-switch", "default") else {
            return;
        };
        let mut events = h.hub.subscribe();
        h.turns.open(&h.session_id, &message("ask-twice")).unwrap();
        let turns = Arc::clone(&h.turns);
        let store = Arc::clone(&h.store);
        let mut prompts = 0;
        let seen = until_done(&mut events, |event| {
            let chunk = &event["payload"]["chunk"];
            if chunk["type"] != "tool-approval-request" {
                return;
            }
            prompts += 1;
            if prompts == 1 {
                let mut agent = store.agents().into_iter().next().unwrap();
                agent.configuration = Some(json!({ "permission_mode": "bypassPermissions" }));
                store.save_agent(agent).unwrap();
                assert!(turns.respond_approval(chunk["approvalId"].as_str().unwrap(), true));
            }
        })
        .await;
        assert_eq!(prompts, 1);
        assert_eq!(
            chunk_types(&seen)
                .iter()
                .filter(|kind| *kind == "tool-approval-request")
                .count(),
            1
        );
        assert_eq!(
            h.store.messages(&h.session_id)[1].searchable_text,
            "allowed"
        );
        assert_eq!(recorded_modes(&h.dir), ["default", "yolo"]);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn inserting_a_message_steers_the_running_turn() {
        let Some(h) = harness("steer", "default") else {
            return;
        };
        let mut events = h.hub.subscribe();
        let started = h
            .turns
            .open(&h.session_id, &message("hang please"))
            .unwrap();
        assert_eq!(started["mode"], "started");
        let blocked = h.turns.open(&h.session_id, &message("second turn"));
        assert!(blocked.is_err(), "a live turn still refuses a second turn");

        let sentence = "你现在不能联网搜索了吗？";
        let inserted = h
            .turns
            .insert_into_live(&h.session_id, &message(sentence))
            .await
            .unwrap();
        assert_eq!(inserted["mode"], "injected");
        assert_eq!(
            inserted["reservedMessages"].as_array().map(Vec::len),
            Some(1)
        );
        assert_eq!(inserted["reservedMessages"][0]["role"], "user");

        let seen = until_done(&mut events, |_| {}).await;
        assert_eq!(seen.last().unwrap()["payload"]["status"], "success");
        let rows = h.store.messages(&h.session_id);
        let users: Vec<_> = rows.iter().filter(|row| row.role == "user").collect();
        assert_eq!(users.len(), 2);
        assert_eq!(users[1].searchable_text, sentence);
        let assistants: Vec<_> = rows.iter().filter(|row| row.role == "assistant").collect();
        assert_eq!(assistants.len(), 1);
        assert!(assistants[0].searchable_text.contains("inserted"));
        let trace: Value =
            serde_json::from_slice(&std::fs::read(h.dir.join("turn-trace.json")).unwrap()).unwrap();
        assert_eq!(trace["interjects"][0], sentence);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn stopping_a_turn_pauses_it() {
        let Some(h) = harness("abort", "default") else {
            return;
        };
        let mut events = h.hub.subscribe();
        h.turns
            .open(&h.session_id, &message("hang please"))
            .unwrap();
        let turns = Arc::clone(&h.turns);
        let session_id = h.session_id.clone();
        let seen = until_done(&mut events, |event| {
            if event["payload"]["chunk"]["type"] == "reasoning-delta" {
                turns.abort(&session_id);
            }
        })
        .await;
        assert_eq!(seen.last().unwrap()["payload"]["status"], "paused");
        assert!(chunk_types(&seen).contains(&"abort".to_owned()));
        assert_eq!(h.store.messages(&h.session_id)[1].status, "paused");
        let terminal = events.recv().await.unwrap();
        assert_eq!(terminal["channel"], "topic.status");
        assert_eq!(terminal["payload"]["status"]["status"], "aborted");
        assert_eq!(terminal["payload"]["status"]["activeExecutions"], json!([]));
    }

    async fn wait_assistant(
        store: &PlaneStore,
        session_id: &str,
        mut ready: impl FnMut(&Message) -> bool,
    ) {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(message) = store
                .messages(session_id)
                .into_iter()
                .find(|message| message.role == "assistant")
            {
                if ready(&message) {
                    return;
                }
            }
            if tokio::time::Instant::now() >= deadline {
                panic!("held turn did not reach the expected assistant row");
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    fn tool_part<'a>(message: &'a Message, tool_call_id: &str) -> Option<&'a Value> {
        message
            .parts()
            .iter()
            .find(|part| part["toolCallId"] == tool_call_id)
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_held_turn_stays_pending_without_starting_the_agent() {
        let Some(h) = harness("held-turn", "default") else {
            return;
        };
        h.turns.hold_turns_for_test();
        let mut events = h.hub.subscribe();
        let started = h
            .turns
            .open(&h.session_id, &message("队列保持回合"))
            .unwrap();
        assert_eq!(started["mode"], "started");
        let session_id = h.session_id.clone();
        let store = Arc::clone(&h.store);
        wait_assistant(&store, &session_id, |message| {
            message.status == "pending" && message.searchable_text.contains("回合正文还在")
        })
        .await;
        assert!(
            !h.dir.join("turn-trace.json").exists(),
            "a held turn must not start the agent"
        );
        assert!(h.turns.open(&session_id, &message("second")).is_err());

        let sentence = "结论改成三句";
        let inserted = h
            .turns
            .insert_into_live(&session_id, &message(sentence))
            .await
            .unwrap();
        assert_eq!(inserted["mode"], "injected");
        let users: Vec<_> = h
            .store
            .messages(&session_id)
            .into_iter()
            .filter(|row| row.role == "user")
            .collect();
        assert_eq!(users.len(), 2);
        assert_eq!(users[1].searchable_text, sentence);

        h.turns.abort(&session_id);
        let seen = until_done(&mut events, |_| {}).await;
        assert_eq!(seen.last().unwrap()["payload"]["status"], "paused");
        assert!(chunk_types(&seen).contains(&"abort".to_owned()));
        let assistant = h
            .store
            .messages(&session_id)
            .into_iter()
            .find(|row| row.role == "assistant")
            .unwrap();
        assert_eq!(assistant.status, "paused");
        assert!(assistant.searchable_text.contains("回合正文还在"));
        assert!(!assistant.searchable_text.contains("执行已中断"));
        let terminal = events.recv().await.unwrap();
        assert_eq!(terminal["channel"], "topic.status");
        assert_eq!(terminal["payload"]["status"]["status"], "aborted");
        assert!(!h.dir.join("turn-trace.json").exists());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_held_plan_turn_denies_then_allows_without_starting_the_agent() {
        let Some(h) = harness("held-plan", "plan") else {
            return;
        };
        h.turns.hold_turns_for_test();
        h.turns
            .open(&h.session_id, &message("规划询问回合"))
            .unwrap();
        let session_id = h.session_id.clone();
        let store = Arc::clone(&h.store);
        let turns = Arc::clone(&h.turns);
        wait_assistant(&store, &session_id, |message| {
            tool_part(message, "hold-edit")
                .is_some_and(|part| part["state"] == "approval-requested")
        })
        .await;
        let edit = store
            .messages(&session_id)
            .into_iter()
            .find(|row| row.role == "assistant")
            .unwrap();
        let approval = tool_part(&edit, "hold-edit")
            .and_then(|part| part["approval"]["id"].as_str())
            .unwrap()
            .to_owned();
        assert!(turns.respond_approval(&approval, false));
        wait_assistant(&store, &session_id, |message| {
            tool_part(message, "hold-edit").is_some_and(|part| part["state"] == "output-denied")
                && tool_part(message, "hold-read")
                    .is_some_and(|part| part["state"] == "approval-requested")
        })
        .await;
        let read = store
            .messages(&session_id)
            .into_iter()
            .find(|row| row.role == "assistant")
            .unwrap();
        let approval = tool_part(&read, "hold-read")
            .and_then(|part| part["approval"]["id"].as_str())
            .unwrap()
            .to_owned();
        assert!(turns.respond_approval(&approval, true));
        wait_assistant(&store, &session_id, |message| {
            tool_part(message, "hold-read").is_some_and(|part| part["state"] == "output-available")
        })
        .await;
        let assistant = store
            .messages(&session_id)
            .into_iter()
            .find(|row| row.role == "assistant")
            .unwrap();
        assert_eq!(assistant.status, "pending");
        assert!(assistant.searchable_text.contains("回合正文还在"));
        assert_eq!(
            tool_part(&assistant, "hold-read").and_then(|part| part["output"].as_str()),
            Some("已完成")
        );
        assert!(h.turns.is_live(&session_id));
        assert!(!h.dir.join("turn-trace.json").exists());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_held_turn_asks_when_plan_is_saved_after_open() {
        let Some(h) = harness("held-plan-late", "default") else {
            return;
        };
        h.turns.hold_turns_for_test();
        h.turns
            .open(&h.session_id, &message("规划询问回合"))
            .unwrap();
        let mut agent = h.store.agents().into_iter().next().unwrap();
        let mut configuration = agent.configuration.clone().unwrap_or_else(|| json!({}));
        configuration["permission_mode"] = json!("plan");
        agent.configuration = Some(configuration);
        h.store.save_agent(agent).unwrap();
        let session_id = h.session_id.clone();
        let store = Arc::clone(&h.store);
        wait_assistant(&store, &session_id, |message| {
            tool_part(message, "hold-edit")
                .is_some_and(|part| part["state"] == "approval-requested")
        })
        .await;
        assert!(!h.dir.join("turn-trace.json").exists());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn messages_catalog_names_tools_and_submits_one_user_message() {
        let Some(h) = harness("messages-catalog", "default") else {
            return;
        };
        let room = "11111111-1111-4111-8111-111111111111";
        let mut body = message("目录回合");
        body["turnContext"] = json!({
            "messagesCatalog": "mtc",
            "roomId": room,
        });
        let started = h.turns.open(&h.session_id, &body).unwrap();
        assert_eq!(started["liveModel"].as_bool(), Some(false));
        let personal = super::private_memory("tobot").unwrap();
        let boss = super::private_memory("toboss").unwrap();
        let room_session = super::member_toboss_room_session_id("", room).unwrap();
        assert!(super::private_memory(&room_session).is_none());
        assert_eq!(
            started["memory"]["personal"]["context"].as_str(),
            Some(personal.scope)
        );
        assert_eq!(
            started["memory"]["personal"]["retainContext"].as_str(),
            Some(personal.scope)
        );
        assert_eq!(
            started["memory"]["personal"]["sessionId"].as_str(),
            Some("tobot")
        );
        assert_eq!(
            started["memory"]["private"]["context"].as_str(),
            Some(boss.scope)
        );
        assert_eq!(
            started["memory"]["private"]["receivesPersonalFacts"].as_bool(),
            Some(boss.scope == personal.scope)
        );
        assert_eq!(
            started["memory"]["room"]["sessionId"].as_str(),
            Some(room_session.as_str())
        );
        assert_eq!(started["memory"]["room"]["reflect"].as_u64(), Some(0));
        assert_eq!(started["memory"]["room"]["retain"].as_u64(), Some(0));

        let session_id = h.session_id.clone();
        let store = Arc::clone(&h.store);
        let turns = Arc::clone(&h.turns);
        wait_assistant(&store, &session_id, |message| {
            tool_part(message, "call-minutes")
                .is_some_and(|part| part["state"] == "approval-requested")
        })
        .await;
        let minutes = store
            .messages(&session_id)
            .into_iter()
            .find(|row| row.role == "assistant")
            .unwrap();
        let names: Vec<_> = minutes
            .parts()
            .iter()
            .filter_map(|part| part["toolName"].as_str())
            .collect();
        assert!(names.contains(&"meeting-minutes"));
        assert!(names.contains(&"calendar"));
        let approval = tool_part(&minutes, "call-minutes")
            .and_then(|part| part["approval"]["id"].as_str())
            .unwrap()
            .to_owned();
        assert!(turns.respond_approval(&approval, true));
        wait_assistant(&store, &session_id, |message| {
            tool_part(message, "call-minutes")
                .is_some_and(|part| part["state"] == "output-available")
                && tool_part(message, "call-calendar")
                    .is_some_and(|part| part["state"] == "approval-requested")
        })
        .await;
        let calendar = store
            .messages(&session_id)
            .into_iter()
            .find(|row| row.role == "assistant")
            .unwrap();
        let approval = tool_part(&calendar, "call-calendar")
            .and_then(|part| part["approval"]["id"].as_str())
            .unwrap()
            .to_owned();
        assert!(turns.respond_approval(&approval, false));
        wait_assistant(&store, &session_id, |message| {
            message
                .parts()
                .iter()
                .any(|part| part["type"] == "data-form")
        })
        .await;
        let formed = store
            .messages(&session_id)
            .into_iter()
            .find(|row| row.role == "assistant")
            .unwrap();
        let binding = formed
            .parts()
            .iter()
            .find(|part| part["type"] == "data-form")
            .unwrap()["data"]["binding"]
            .clone();
        let binding: super::FormBinding = serde_json::from_value(binding).unwrap();
        let mut values = super::FormValues::new();
        values.insert("when".into(), json!("下周三 15:00"));
        assert!(turns.submit_form(binding, values));
        wait_assistant(&store, &session_id, |message| message.status == "success").await;
        let rows = store.messages(&session_id);
        let users: Vec<_> = rows.iter().filter(|row| row.role == "user").collect();
        assert_eq!(users.len(), 1);
        let assistant = rows.iter().find(|row| row.role == "assistant").unwrap();
        let answered = assistant
            .parts()
            .iter()
            .find(|part| part["type"] == "data-form")
            .unwrap();
        assert_eq!(
            answered["data"]["values"]["when"].as_str(),
            Some("下周三 15:00")
        );
        assert!(!h.turns.is_live(&session_id));
        assert!(!h.dir.join("turn-trace.json").exists());
    }

    /// The agent receives installed connectors on `session/new`, including a
    /// session that was already open before the install.
    #[tokio::test(flavor = "multi_thread")]
    async fn an_installed_connector_is_sent_on_session_new() {
        let Some(h) = harness("mcp-handoff", "default") else {
            return;
        };
        let mut events = h.hub.subscribe();
        h.turns.open(&h.session_id, &message("hi")).unwrap();
        let seen = until_done(&mut events, |_| {}).await;
        assert_eq!(seen.last().unwrap()["channel"], "ai.stream.done");
        let first: Value = serde_json::from_slice(
            &std::fs::read(h.dir.join("session-new.json")).expect("session/new was recorded"),
        )
        .unwrap();
        assert_eq!(first["mcpServers"], json!([]));

        let caps = h.dir.join("capabilities");
        std::fs::create_dir_all(&caps).unwrap();
        std::fs::write(
            caps.join("connector-config.json"),
            r#"{"github":{"GITHUB_PERSONAL_ACCESS_TOKEN":"ghp_x"},"linear":{"LINEAR_API_KEY":"lin_x"}}"#,
        )
        .unwrap();
        let catalog = vec![
            json!({ "slug": "github", "installed": true, "install": {
                "type": "mcp", "transport": "stdio", "command": "npx",
                "args": ["-y", "@modelcontextprotocol/server-github"] } }),
            json!({ "slug": "linear", "installed": true, "install": {
                "type": "mcp", "transport": "streamable_http",
                "url": "https://mcp.linear.app/mcp" } }),
            json!({ "slug": "slack", "installed": false, "install": {
                "type": "mcp", "transport": "sse", "url": "https://slack.example/sse" } }),
        ];
        crate::capabilities::reconcile_at(&caps, &crate::capabilities::desired(&catalog), |_| {
            Err(std::io::Error::other("no bundle"))
        })
        .unwrap();

        let mut events = h.hub.subscribe();
        h.turns
            .open(&h.session_id, &message("use the connector"))
            .unwrap();
        let seen = until_done(&mut events, |_| {}).await;
        assert_eq!(
            seen.last().unwrap()["channel"],
            "ai.stream.done",
            "the relaunched session still answers"
        );
        let second: Value =
            serde_json::from_slice(&std::fs::read(h.dir.join("session-new.json")).unwrap())
                .unwrap();
        let servers = second["mcpServers"]
            .as_array()
            .expect("mcpServers is the ACP array");
        let github = servers
            .iter()
            .find(|server| server["name"] == "github")
            .expect("github is on session/new");
        assert!(github.get("type").is_none());
        assert_eq!(github["command"], "npx");
        assert_eq!(
            github["args"],
            json!(["-y", "@modelcontextprotocol/server-github"])
        );
        assert_eq!(
            github["env"],
            json!([{ "name": "GITHUB_PERSONAL_ACCESS_TOKEN", "value": "ghp_x" }])
        );
        let linear = servers
            .iter()
            .find(|server| server["name"] == "linear")
            .expect("linear is on session/new");
        assert_eq!(linear["type"], "http");
        assert_eq!(linear["url"], "https://mcp.linear.app/mcp");
        assert_eq!(
            linear["headers"],
            json!([{ "name": "LINEAR_API_KEY", "value": "lin_x" }])
        );
        assert!(servers.iter().all(|server| server["name"] != "slack"));
        eprintln!("session/new mcpServers={}", second["mcpServers"]);
        let _ = std::fs::remove_dir_all(&h.dir);
    }

    /// Installed Tools bind an exact native selection and invalidate a warm process.
    #[tokio::test(flavor = "multi_thread")]
    async fn an_installed_plugin_tool_is_sent_on_session_new() {
        let Some(h) = harness("plugin-tool", "default") else {
            return;
        };
        let mut events = h.hub.subscribe();
        h.turns.open(&h.session_id, &message("before")).unwrap();
        let seen = until_done(&mut events, |_| {}).await;
        assert_eq!(seen.last().unwrap()["channel"], "ai.stream.done");
        let first: Value = serde_json::from_slice(
            &std::fs::read(h.dir.join("session-new.json")).expect("session/new was recorded"),
        )
        .unwrap();
        assert!(first.get("tools").is_none());
        assert!(first.pointer("/_meta/a3sTools").is_none());

        let caps = h.dir.join("capabilities");
        let agents = h.dir.join("agents");
        std::fs::create_dir_all(&agents).unwrap();
        let zip = {
            let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
            for (name, body) in [
                (
                    "skills/reason/SKILL.md",
                    "---\nname: reason\ndescription: Reason over the package.\n---\n# Reason\n",
                ),
                (
                    "mcp/context",
                    "#!/bin/sh\nprintf '%s\\n' 'stdio MCP fixture'\n",
                ),
                (
                    "tools/echo",
                    "#!/bin/sh\nprintf '%s\\n' '{\"message\":\"cognitive package ready\"}'\n",
                ),
                (
                    "flows/reason.ts",
                    "export async function run() {\n  return { type: \"complete\", output: null };\n}\n",
                ),
            ] {
                writer
                    .start_file(name, zip::write::SimpleFileOptions::default())
                    .unwrap();
                std::io::Write::write_all(&mut writer, body.as_bytes()).unwrap();
            }
            writer.finish().unwrap().into_inner()
        };
        let mut pins = std::collections::BTreeMap::new();
        pins.insert(
            "cognitive".to_owned(),
            crate::capabilities::PluginPin {
                require_aix: false,
                digest: crate::capabilities::digest_of(&zip),
                package_id: "acme/cognitive".to_owned(),
            },
        );
        let fetch = |slug: &str| {
            assert_eq!(slug, "cognitive");
            Ok(zip.clone())
        };
        crate::capabilities::sync_plugins(&caps, &agents, &pins, &fetch).unwrap();

        let mut events = h.hub.subscribe();
        h.turns
            .open(&h.session_id, &message("use the tool"))
            .unwrap();
        let seen = until_done(&mut events, |_| {}).await;
        assert_eq!(seen.last().unwrap()["channel"], "ai.stream.done");
        let second: Value =
            serde_json::from_slice(&std::fs::read(h.dir.join("session-new.json")).unwrap())
                .unwrap();
        assert!(second.get("tools").is_none());
        let request: agent_client_protocol::NewSessionRequest =
            serde_json::from_value(second.clone())
                .expect("session/new decodes into NewSessionRequest");
        let cursor: a3s_use_reconcile::SelectionCursor = serde_json::from_value(
            request
                .meta
                .as_ref()
                .and_then(|meta| meta.get("a3sPluginSelection"))
                .expect("native selection survives NewSessionRequest")
                .clone(),
        )
        .unwrap();
        assert!(second.pointer("/_meta/a3sTools").is_none());
        let store =
            a3s_use_reconcile::ReconcileStore::open(h.dir.join("package-generations")).unwrap();
        let selected = store.acquire_selection_sync(&cursor).unwrap();
        let publication = selected.publications().next().unwrap();
        assert_eq!(publication.package_id, "acme/cognitive");
        let tool = publication
            .surfaces
            .iter()
            .find(|surface| surface.kind == a3s_use_reconcile::SurfaceKind::Tool)
            .unwrap();
        assert!(
            std::fs::read_to_string(tool.entry.as_ref().unwrap())
                .unwrap()
                .contains("cognitive package ready")
        );
        let trace = recorded_trace(&h.dir);
        assert_eq!(
            trace["launches"][1]
                .as_array()
                .unwrap()
                .iter()
                .rev()
                .take(2)
                .cloned()
                .collect::<Vec<_>>(),
            vec![
                json!(h.dir.join("package-generations")),
                json!("--plugin-store")
            ]
        );
        assert_eq!(recorded_launch_count(&h.dir), 2);
        assert!(
            !second["mcpServers"]
                .as_array()
                .expect("mcpServers")
                .iter()
                .any(|server| server["name"] == "context")
        );
        let managed_mcp = publication
            .surfaces
            .iter()
            .find(|surface| surface.kind == a3s_use_reconcile::SurfaceKind::Mcp)
            .unwrap();
        assert!(
            managed_mcp
                .entry
                .as_ref()
                .unwrap()
                .starts_with(std::fs::canonicalize(h.dir.join("package-generations")).unwrap())
        );
        assert_eq!(managed_mcp.mcp.as_ref().unwrap()["command"], "context");
        assert!(
            managed_mcp.mcp.as_ref().unwrap()["args"]
                .as_array()
                .unwrap()
                .iter()
                .any(|arg| arg == "--stdio")
        );
        drop(selected);
        h.turns
            .open(&h.session_id, &message("reuse selection"))
            .unwrap();
        let seen = until_done(&mut events, |_| {}).await;
        assert_eq!(seen.last().unwrap()["payload"]["status"], "success");
        assert_eq!(recorded_launch_count(&h.dir), 2);
        crate::capabilities::sync_plugins(
            &caps,
            &agents,
            &std::collections::BTreeMap::new(),
            &fetch,
        )
        .unwrap();
        h.turns
            .open(&h.session_id, &message("after uninstall"))
            .unwrap();
        let seen = until_done(&mut events, |_| {}).await;
        assert_eq!(seen.last().unwrap()["payload"]["status"], "success");
        assert_eq!(
            recorded_launch_count(&h.dir),
            3,
            "withdrawn selection reused the old warm process"
        );
        let after: Value =
            serde_json::from_slice(&std::fs::read(h.dir.join("session-new.json")).unwrap())
                .unwrap();
        let after_cursor =
            serde_json::from_value(after["_meta"]["a3sPluginSelection"].clone()).unwrap();
        let after_lease = store.acquire_selection_sync(&after_cursor).unwrap();
        assert_eq!(after_lease.publications().len(), 0);
        assert_ne!(after_cursor, cursor);
        assert!(after.pointer("/_meta/a3sTools").is_none());
        drop(after_lease);
        h.turns.discard_runtime(&h.session_id).await;
        let _ = std::fs::remove_dir_all(&h.dir);
    }

    /// A catalog alias launches through the signed-in gateway. The next prompt
    /// stays on that runtime, so the peer still has the first exchange.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_catalog_alias_stays_on_the_gateway_across_turns() {
        let Some(h) = gateway_harness("gateway-turns") else {
            return;
        };
        let mut events = h.hub.subscribe();
        h.turns.open(&h.session_id, &message("first-turn")).unwrap();
        let first = until_done(&mut events, |_| {}).await;
        assert_eq!(first.last().unwrap()["payload"]["status"], "success");

        h.turns
            .open(&h.session_id, &message("second-turn"))
            .unwrap();
        let second = until_done(&mut events, |_| {}).await;
        assert_eq!(second.last().unwrap()["payload"]["status"], "success");

        let rows = h.store.messages(&h.session_id);
        let assistants: Vec<_> = rows.iter().filter(|row| row.role == "assistant").collect();
        assert_eq!(assistants.len(), 2);
        assert_eq!(assistants[0].searchable_text, "hello");
        assert_eq!(assistants[1].searchable_text, "first-turn\nsecond-turn");
        assert!(assistants.iter().all(|row| {
            row.status == "success" && row.model_id.as_deref() == Some("a3s::deepseek-v4-flash")
        }));

        let acl = std::fs::read_to_string(
            h.dir
                .join("state")
                .join("runtime")
                .join(format!("{}.acl", h.session_id)),
        )
        .unwrap();
        let origin = crate::providers::gateway_origin();
        assert!(acl.contains(&format!("base_url = \"{origin}/v1\"")));
        assert!(acl.contains("api_key = \"a3s_ms_signed_in_member\""));
        assert!(acl.contains("default_model = \"a3s/deepseek-v4-flash\""));
        assert!(acl.contains("models \"deepseek-v4-flash\" {}"));

        let trace: Value =
            serde_json::from_slice(&std::fs::read(h.dir.join("turn-trace.json")).unwrap()).unwrap();
        assert_eq!(trace["launches"].as_array().map(Vec::len), Some(1));
        let launch = trace["launches"][0].to_string();
        assert!(launch.contains("a3s/deepseek-v4-flash"));
        assert_eq!(trace["prompts"], json!(["first-turn", "second-turn"]));
        let _ = std::fs::remove_dir_all(&h.dir);
    }

    /// Opening ToBot starts one a3s-code process and the next turn stays on it.
    #[tokio::test(flavor = "multi_thread")]
    async fn tobot_warm_keeps_one_a3s_code_process() {
        let Some(h) = harness("tobot-warm", "default") else {
            return;
        };
        let requested = json!({ "model": "a3s::deepseek-v4-flash" });
        let started = h.turns.ensure_tobot(&requested).await.unwrap();
        assert_eq!(started["agentId"], "a3s-code");
        assert_eq!(started["sessionId"], "tobot");
        assert_eq!(started["providerKind"], "a3s.code");
        assert_eq!(started["assetId"], "b0aaf8f3-2d7b-51a6-b48f-44fcad1230fc");
        assert_eq!(started["releaseId"], "ad580d5a-93d9-5ed8-81cf-0e9548b4b543");
        assert_eq!(started["warm"], false);

        let mut warm = false;
        for _ in 0..50 {
            let again = h.turns.ensure_tobot(&requested).await.unwrap();
            if again["warm"] == true {
                warm = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        assert!(warm, "ToBot a3s-code process did not stay warm");

        let mut events = h.hub.subscribe();
        h.turns
            .open(BUILTIN_TOBOT_SESSION_ID, &message("tobot-turn"))
            .unwrap();
        let done = until_done(&mut events, |_| {}).await;
        assert_eq!(done.last().unwrap()["payload"]["status"], "success");
        let trace: Value =
            serde_json::from_slice(&std::fs::read(h.dir.join("turn-trace.json")).unwrap()).unwrap();
        assert_eq!(trace["launches"].as_array().map(Vec::len), Some(1));
        assert_eq!(trace["prompts"], json!(["tobot-turn"]));
        let opened: Value = serde_json::from_slice(
            &std::fs::read(h.dir.join("session-new.json")).expect("session/new"),
        )
        .unwrap();
        assert_eq!(opened["_meta"]["a3sHost"]["role"], "You are ToBot");
        let tobot_guidelines_text = opened["_meta"]["a3sHost"]["guidelines"]
            .as_str()
            .expect("guidelines");
        assert!(tobot_guidelines_text.starts_with(&tobot_guidelines(TOBOT_GUIDELINES)));
        assert!(tobot_guidelines_text.ends_with(DELIVERY_CONTRACT.trim()));
        let user = h
            .store
            .messages(BUILTIN_TOBOT_SESSION_ID)
            .into_iter()
            .find(|row| row.role == "user")
            .expect("user");
        assert_eq!(user.searchable_text, "tobot-turn");
        assert!(!user.searchable_text.contains("You are ToBot"));
        let _ = std::fs::remove_dir_all(&h.dir);
    }

    /// Saved instructions and homepage lines stay out of the stored user text.
    #[tokio::test(flavor = "multi_thread")]
    async fn standing_host_context_stays_out_of_the_user_text() {
        let Some(h) = harness("host-context", "default") else {
            return;
        };
        let mut events = h.hub.subscribe();
        let body = json!({
            "userMessageParts": [{ "type": "text", "text": "只看这一句" }],
            "hostContext": {
                "guidelines": "每次都带上",
                "review": "subagent",
                "outputLanguage": "zh-CN"
            },
            "turnContext": {
                "directives": ["自动审查：完成前检查结论、引用和改动，并标出不确定之处。"],
                "research": true
            }
        });
        h.turns.open(&h.session_id, &body).unwrap();
        let seen = until_done(&mut events, |_| {}).await;
        assert_eq!(seen.last().unwrap()["payload"]["status"], "success");

        let rows = h.store.messages(&h.session_id);
        let user = rows.iter().find(|row| row.role == "user").expect("user");
        assert_eq!(user.searchable_text, "只看这一句");
        assert!(!user.searchable_text.contains("每次都带上"));
        assert!(!user.searchable_text.contains("自动审查"));
        assert!(!user.searchable_text.contains("代码审查"));
        assert_eq!(h.store.session(&h.session_id).unwrap().name, "只看这一句");

        let opened: Value = serde_json::from_slice(
            &std::fs::read(h.dir.join("session-new.json")).expect("session/new"),
        )
        .unwrap();
        let standing = opened["_meta"]["a3sHost"]["guidelines"]
            .as_str()
            .expect("guidelines");
        assert!(standing.starts_with("每次都带上\n\n"));
        assert!(standing.ends_with(DELIVERY_CONTRACT.trim()));
        assert_eq!(opened["_meta"]["a3sHost"]["review"], "subagent");
        assert_eq!(opened["_meta"]["a3sHost"]["outputLanguage"], "zh-CN");
        assert!(opened["_meta"]["a3sHost"].get("role").is_none());

        let trace: Value =
            serde_json::from_slice(&std::fs::read(h.dir.join("turn-trace.json")).unwrap()).unwrap();
        assert_eq!(trace["prompts"], json!(["只看这一句"]));
        assert_eq!(trace["promptMeta"][0]["a3sTurn"]["research"], true);
        assert_eq!(
            trace["promptMeta"][0]["a3sTurn"]["directives"][0],
            "自动审查：完成前检查结论、引用和改动，并标出不确定之处。"
        );
        assert!(opened["_meta"]["a3sHost"].get("assets").is_none());
        let _ = std::fs::remove_dir_all(&h.dir);
    }

    /// A turn's hostContext.wiki replaces the tenant projection with the
    /// app-workspace wiki. The note body stays out of the prompt.
    #[tokio::test(flavor = "multi_thread")]
    async fn local_note_wiki_is_admitted() {
        const SENTENCE: &str = "桂花糖藕只写在本地笔记7c2e";
        let Some(h) = harness("local-note-wiki", "default") else {
            return;
        };
        let wiki = h.dir.join("app-workspace").join("wiki");
        std::fs::create_dir_all(wiki.join("local")).unwrap();
        std::fs::write(
            wiki.join("local").join("家常.md"),
            format!("# 家常\n\n{SENTENCE}\n"),
        )
        .unwrap();
        std::fs::write(
            h.dir.join("app-workspace").join("readme.md"),
            "工作区根上的句子不要被检索到9d1f\n",
        )
        .unwrap();

        let mut events = h.hub.subscribe();
        let body = json!({
            "userMessageParts": [{ "type": "text", "text": "只问这一句" }],
            "hostContext": {
                "guidelines": "",
                "review": "inline",
                "outputLanguage": "zh-CN",
                "wiki": wiki.display().to_string()
            }
        });
        h.turns.open(&h.session_id, &body).unwrap();
        let seen = until_done(&mut events, |_| {}).await;
        assert_eq!(seen.last().unwrap()["payload"]["status"], "success");

        let opened: Value = serde_json::from_slice(
            &std::fs::read(h.dir.join("session-new.json")).expect("session/new"),
        )
        .unwrap();
        assert_eq!(
            opened["_meta"]["a3sHost"]["assets"]["wiki"],
            wiki.display().to_string()
        );
        assert_eq!(
            std::path::Path::new(
                opened["_meta"]["a3sHost"]["assets"]["wiki"]
                    .as_str()
                    .unwrap()
            )
            .file_name()
            .and_then(|name| name.to_str()),
            Some("wiki")
        );
        let admitted = opened["_meta"]["a3sHost"]["assets"].to_string();
        assert!(!admitted.contains(SENTENCE));
        let trace: Value =
            serde_json::from_slice(&std::fs::read(h.dir.join("turn-trace.json")).unwrap()).unwrap();
        assert_eq!(trace["prompts"], json!(["只问这一句"]));
        assert!(!trace["prompts"][0].as_str().unwrap().contains(SENTENCE));
        let user = h
            .store
            .messages(&h.session_id)
            .into_iter()
            .find(|row| row.role == "user")
            .expect("user");
        assert_eq!(user.searchable_text, "只问这一句");
        assert!(!user.searchable_text.contains(SENTENCE));
        let _ = std::fs::remove_dir_all(&h.dir);
    }

    /// Personal ToBot searches the app-workspace wiki without a request field.
    /// The same path reuses the process. A different path starts another one.
    /// The note body stays out of the prompt.
    #[tokio::test(flavor = "multi_thread")]
    async fn tobot_admits_the_workspace_wiki() {
        const SENTENCE: &str = "桂花糖藕只写在本地笔记7c2e";
        let wiki = std::env::temp_dir().join(format!(
            "a3s-tobot-wiki-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&wiki);
        std::fs::create_dir_all(wiki.join("local")).unwrap();
        std::fs::write(
            wiki.join("local").join("家常.md"),
            format!("# 家常\n\n{SENTENCE}\n"),
        )
        .unwrap();
        let current = Arc::new(Mutex::new(wiki.clone()));
        let lookup = Arc::clone(&current);
        let Some(h) = harness_with(
            "tobot-wiki",
            "default",
            Some(Arc::new(move || Some(lookup.lock().expect("wiki").clone()))),
        ) else {
            let _ = std::fs::remove_dir_all(&wiki);
            return;
        };
        let requested = json!({ "model": "a3s::deepseek-v4-flash" });
        h.turns.ensure_tobot(&requested).await.unwrap();
        let mut warm = false;
        for _ in 0..50 {
            let again = h.turns.ensure_tobot(&requested).await.unwrap();
            if again["warm"] == true {
                warm = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        assert!(warm, "ToBot a3s-code process did not stay warm");

        let opened: Value = serde_json::from_slice(
            &std::fs::read(h.dir.join("session-new.json")).expect("session/new"),
        )
        .unwrap();
        assert_eq!(
            opened["_meta"]["a3sHost"]["assets"]["wiki"],
            wiki.display().to_string()
        );
        let admitted = opened["_meta"]["a3sHost"]["assets"].to_string();
        assert!(!admitted.contains(SENTENCE));
        let trace: Value =
            serde_json::from_slice(&std::fs::read(h.dir.join("turn-trace.json")).unwrap()).unwrap();
        assert_eq!(trace["launches"].as_array().map(Vec::len), Some(1));

        let mut events = h.hub.subscribe();
        h.turns
            .open(BUILTIN_TOBOT_SESSION_ID, &message("桂花糖藕在哪"))
            .unwrap();
        let done = until_done(&mut events, |_| {}).await;
        assert_eq!(done.last().unwrap()["payload"]["status"], "success");
        let user = h
            .store
            .messages(BUILTIN_TOBOT_SESSION_ID)
            .into_iter()
            .find(|row| row.role == "user")
            .expect("user");
        assert_eq!(user.searchable_text, "桂花糖藕在哪");
        assert!(!user.searchable_text.contains(SENTENCE));
        let after: Value =
            serde_json::from_slice(&std::fs::read(h.dir.join("turn-trace.json")).unwrap()).unwrap();
        assert_eq!(after["launches"].as_array().map(Vec::len), Some(1));
        assert_eq!(after["prompts"], json!(["桂花糖藕在哪"]));
        assert!(!after["prompts"][0].as_str().unwrap().contains(SENTENCE));

        let next = wiki.with_file_name("wiki-next");
        let _ = std::fs::remove_dir_all(&next);
        std::fs::create_dir_all(&next).unwrap();
        *current.lock().expect("wiki") = next.clone();
        h.turns.ensure_tobot(&requested).await.unwrap();
        let mut relaunched = false;
        for _ in 0..50 {
            let again = h.turns.ensure_tobot(&requested).await.unwrap();
            if again["warm"] == true {
                relaunched = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        assert!(relaunched, "a new workspace wiki did not restart ToBot");
        let restarted: Value = serde_json::from_slice(
            &std::fs::read(h.dir.join("session-new.json")).expect("session/new"),
        )
        .unwrap();
        assert_eq!(
            restarted["_meta"]["a3sHost"]["assets"]["wiki"],
            next.display().to_string()
        );
        let trace: Value =
            serde_json::from_slice(&std::fs::read(h.dir.join("turn-trace.json")).unwrap()).unwrap();
        assert_eq!(trace["launches"].as_array().map(Vec::len), Some(2));
        let _ = std::fs::remove_dir_all(&h.dir);
        let _ = std::fs::remove_dir_all(&wiki);
        let _ = std::fs::remove_dir_all(&next);
    }

    /// session/new carries the installed connector, skill, and wiki catalog.
    /// The stored user sentence and the prompt stay free of those bodies.
    #[tokio::test(flavor = "multi_thread")]
    async fn local_assets_handoff_keeps_the_user_sentence() {
        const CONNECTOR_BODY: &str = "BODY-connector-9f3a-unique";
        const SKILL_BODY: &str = "BODY-skill-9f3a-unique";
        const WIKI_BODY: &str = "BODY-wiki-9f3a-unique";
        const SECRET: &str = "do-not-leak-connector-cmd-9f3a";
        let Some(h) = harness("local-assets", "default") else {
            return;
        };
        let capabilities = h.dir.join("capabilities");
        std::fs::create_dir_all(&capabilities).unwrap();
        std::fs::write(
            capabilities.join("connector-config.json"),
            r#"{"session-notes":{"TOKEN":"do-not-leak-token-9f3a"}}"#,
        )
        .unwrap();
        let mut usage_prefix = String::from("Installed session notes");
        while usage_prefix.chars().count() < 160 {
            usage_prefix.push_str(" notes");
        }
        let usage_prefix: String = usage_prefix.chars().take(160).collect();
        let usage = format!("{usage_prefix}\n{CONNECTOR_BODY}");
        let skill_text = format!(
            "---\nname: brief-writer\ndescription: \"Write a short brief\"\n---\n{SKILL_BODY}\n"
        );
        let bundle = {
            let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
            writer
                .start_file("SKILL.md", zip::write::SimpleFileOptions::default())
                .unwrap();
            std::io::Write::write_all(&mut writer, skill_text.as_bytes()).unwrap();
            writer.finish().unwrap().into_inner()
        };
        let digest = {
            use sha2::{Digest, Sha256};
            let hash = Sha256::digest(&bundle);
            let hex: String = hash.iter().map(|byte| format!("{byte:02x}")).collect();
            format!("sha256:{hex}")
        };
        let package = bundle.clone();
        let catalog = vec![
            json!({
                "slug": "session-notes",
                "installed": true,
                "description": usage,
                "install": {
                    "type": "mcp",
                    "transport": "stdio",
                    "command": SECRET,
                    "args": ["--secret", "do-not-leak-env-9f3a"]
                }
            }),
            json!({
                "slug": "brief-writer",
                "installed": true,
                "install": { "type": "skill", "digest": digest, "size": bundle.len() }
            }),
        ];
        crate::capabilities::reconcile_at(
            &capabilities,
            &crate::capabilities::desired(&catalog),
            move |_| Ok(package.clone()),
        )
        .unwrap();
        let wiki_put = json!({
            "pages": [{
                "id": "hub_manual1",
                "title": "产品手册甲",
                "body": format!("Saved handbook page\n{WIKI_BODY}")
            }]
        })
        .to_string();
        crate::local_assets::project_wiki(&h.dir.join("wiki"), wiki_put.as_bytes()).unwrap();
        let note = std::fs::read_to_string(capabilities.join("session-notes.md")).unwrap();
        assert!(note.contains(CONNECTOR_BODY));
        assert!(!note.contains(SECRET));
        assert!(!note.contains("do-not-leak-token-9f3a"));
        let mcp = std::fs::read_to_string(capabilities.join("mcp.json")).unwrap();
        assert!(mcp.contains(SECRET));
        assert!(mcp.contains("do-not-leak-token-9f3a"));
        assert!(!mcp.contains(CONNECTOR_BODY));

        let mut events = h.hub.subscribe();
        let sentence = "今天只问天气";
        h.turns.open(&h.session_id, &message(sentence)).unwrap();
        let seen = until_done(&mut events, |_| {}).await;
        assert_eq!(seen.last().unwrap()["payload"]["status"], "success");

        let rows = h.store.messages(&h.session_id);
        let user = rows.iter().find(|row| row.role == "user").expect("user");
        assert_eq!(user.searchable_text, sentence);
        assert!(!user.searchable_text.contains(SKILL_BODY));
        assert!(!user.searchable_text.contains(WIKI_BODY));
        assert!(!user.searchable_text.contains(CONNECTOR_BODY));

        let opened: Value = serde_json::from_slice(
            &std::fs::read(h.dir.join("session-new.json")).expect("session/new"),
        )
        .unwrap();
        let catalog = opened["_meta"]["a3sHost"]["assets"]["catalog"]
            .as_array()
            .expect("catalog");
        let names: Vec<&str> = catalog
            .iter()
            .filter_map(|entry| entry.get("name").and_then(Value::as_str))
            .collect();
        assert_eq!(names, ["session-notes", "brief-writer", "产品手册甲"]);
        let catalog_text = opened["_meta"]["a3sHost"]["assets"].to_string();
        assert!(catalog_text.contains("Installed session notes"));
        assert!(catalog_text.contains("Write a short brief"));
        assert!(catalog_text.contains("Saved handbook page"));
        assert!(!catalog_text.contains(CONNECTOR_BODY));
        assert!(!catalog_text.contains(SKILL_BODY));
        assert!(!catalog_text.contains(WIKI_BODY));
        assert!(!catalog_text.contains(SECRET));
        assert!(!catalog_text.contains("do-not-leak-token-9f3a"));
        assert_eq!(
            opened["_meta"]["a3sHost"]["assets"]["connectors"],
            capabilities.display().to_string()
        );

        let trace: Value =
            serde_json::from_slice(&std::fs::read(h.dir.join("turn-trace.json")).unwrap()).unwrap();
        assert_eq!(trace["prompts"], json!([sentence]));
        let prompt = trace["prompts"][0].as_str().unwrap();
        assert!(!prompt.contains(SKILL_BODY));
        assert!(!prompt.contains(WIKI_BODY));
        assert!(!prompt.contains(CONNECTOR_BODY));
        assert!(!prompt.contains(SECRET));
        let _ = std::fs::remove_dir_all(&h.dir);
    }

    fn recorded_trace(dir: &std::path::Path) -> Value {
        std::fs::read(dir.join("turn-trace.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or(json!({}))
    }

    fn recorded_prompts(dir: &std::path::Path) -> Vec<String> {
        recorded_trace(dir)
            .get("prompts")
            .and_then(Value::as_array)
            .map(|prompts| {
                prompts
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default()
    }

    fn recorded_launch_count(dir: &std::path::Path) -> usize {
        recorded_trace(dir)
            .get("launches")
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or(0)
    }

    fn write_review_docx(path: &std::path::Path, paragraphs: &[String]) {
        use std::io::Write;
        let body: String = paragraphs
            .iter()
            .map(|paragraph| format!("<w:p><w:r><w:t>{paragraph}</w:t></w:r></w:p>"))
            .collect();
        let xml = format!(
            "<?xml version=\"1.0\"?><w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:body>{body}</w:body></w:document>"
        );
        let mut cursor = std::io::Cursor::new(Vec::new());
        {
            let mut writer = zip::ZipWriter::new(&mut cursor);
            writer
                .start_file(
                    "word/document.xml",
                    zip::write::SimpleFileOptions::default(),
                )
                .unwrap();
            writer.write_all(xml.as_bytes()).unwrap();
            writer.finish().unwrap();
        }
        std::fs::write(path, cursor.into_inner()).unwrap();
    }

    #[test]
    fn review_retry_keeps_auth_failures_off_the_context_path() {
        assert!(retryable_agent_error("context length exceeded"));
        assert!(retryable_agent_error("Internal error"));
        assert!(retryable_agent_error("payload too large"));
        assert!(!retryable_agent_error("unauthorized"));
        assert!(!retryable_agent_error("rate limit reached"));
        assert!(!retryable_agent_error("invalid api key"));
        assert!(keeps_runtime("invalid api key"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_review_walks_each_segment_and_a_later_turn_omits_the_contract() {
        let Some(h) = harness("review-segments", "default") else {
            return;
        };
        super::super::reference_tasks_live_tests::publish_reference_review(&h.dir);
        let docx = h.dir.join("contract.docx");
        let tail = "段三丙尾";
        write_review_docx(
            &docx,
            &[
                format!("{}段一甲尾", "条".repeat(3_990)),
                format!("{}段二乙尾", "条".repeat(3_990)),
                format!("{}{tail}", "条".repeat(3_990)),
            ],
        );
        let human = format!("/review 审查这个文件\n{}", docx.display());
        let mut events = h.hub.subscribe();
        let mut body = message(&human);
        body["turnContext"] = json!({"agentId":"legal-expert"});
        h.turns.open(&h.session_id, &body).unwrap();
        let seen = until_done(&mut events, |_| {}).await;
        assert_eq!(seen.last().unwrap()["payload"]["status"], "success");
        let rows = h.store.messages(&h.session_id);
        assert_eq!(rows[1].status, "success");
        assert!(rows[1].searchable_text.contains("正在审查第"));
        assert!(rows[1].searchable_text.contains("a3s-review-finding"));
        assert!(!rows[1].searchable_text.contains(tail));

        let mut events = h.hub.subscribe();
        h.turns
            .open(&h.session_id, &message("你能修复吗?"))
            .unwrap();
        let seen = until_done(&mut events, |_| {}).await;
        assert_eq!(seen.last().unwrap()["channel"], "ai.stream.done");
        assert_eq!(seen.last().unwrap()["payload"]["status"], "success");

        let prompts = recorded_prompts(&h.dir);
        let segments: Vec<_> = prompts
            .iter()
            .filter(|prompt| prompt.contains("Document segment"))
            .collect();
        assert!(segments.len() >= 3);
        assert!(segments.iter().any(|prompt| prompt.contains(tail)));
        assert!(
            segments
                .iter()
                .all(|prompt| !(prompt.contains("段一甲尾") && prompt.contains(tail)))
        );
        let follows: Vec<_> = prompts
            .iter()
            .filter(|prompt| prompt.contains("Findings:"))
            .collect();
        assert_eq!(follows.len(), 1);
        assert!(follows[0].contains("你能修复吗?"));
        assert!(follows[0].contains(&docx.display().to_string()));
        assert!(follows[0].contains("workspace goal"));
        assert!(!follows[0].contains(tail));
        assert!(!follows[0].contains("Document segment"));

        let mut events = h.hub.subscribe();
        h.turns
            .open(&h.session_id, &message("你能帮我修复合同的问题吗?"))
            .unwrap();
        let seen = until_done(&mut events, |_| {}).await;
        assert_eq!(seen.last().unwrap()["payload"]["status"], "success");
        let prompts = recorded_prompts(&h.dir);
        let follows: Vec<_> = prompts
            .iter()
            .filter(|prompt| prompt.contains("Findings:"))
            .collect();
        assert_eq!(follows.len(), 2);
        assert!(follows[1].contains("你能帮我修复合同的问题吗?"));
        assert!(follows[1].contains(&docx.display().to_string()));
        assert!(follows[1].contains("a3s-review-finding"));
        assert!(!follows[1].contains(tail));
        let _ = std::fs::remove_dir_all(&h.dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_review_follow_up_retries_an_internal_error_without_showing_it() {
        let Some(h) = harness("review-once-internal", "default") else {
            return;
        };
        let mut events = h.hub.subscribe();
        h.turns
            .open(&h.session_id, &message("please once-internal this"))
            .unwrap();
        let seen = until_done(&mut events, |_| {}).await;
        assert_eq!(seen.last().unwrap()["channel"], "ai.stream.done");
        assert_eq!(seen.last().unwrap()["payload"]["status"], "success");
        let rows = h.store.messages(&h.session_id);
        assert_eq!(rows[1].searchable_text, "hello");
        assert_eq!(recorded_prompts(&h.dir).len(), 2);
        assert_eq!(recorded_launch_count(&h.dir), 1);
        let _ = std::fs::remove_dir_all(&h.dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_review_follow_up_replaces_a_poisoned_runtime() {
        let Some(h) = harness("review-fresh-internal", "default") else {
            return;
        };
        let mut events = h.hub.subscribe();
        h.turns
            .open(&h.session_id, &message("please fresh-internal this"))
            .unwrap();
        let seen = until_done(&mut events, |_| {}).await;
        assert_eq!(seen.last().unwrap()["payload"]["status"], "success");
        assert_eq!(h.store.messages(&h.session_id)[1].searchable_text, "hello");
        assert_eq!(recorded_prompts(&h.dir).len(), 3);
        assert_eq!(recorded_launch_count(&h.dir), 2);
        let _ = std::fs::remove_dir_all(&h.dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_review_internal_error_reports_the_provider_reason() {
        let Some(h) = harness("review-stay-internal", "default") else {
            return;
        };
        let mut events = h.hub.subscribe();
        h.turns
            .open(&h.session_id, &message("please stay-internal this"))
            .unwrap();
        let seen = until_done(&mut events, |_| {}).await;
        let done = seen.last().unwrap();
        assert_eq!(done["channel"], "ai.stream.error");
        assert_eq!(
            done["payload"]["error"]["message"],
            "context length exceeded"
        );
        let _ = std::fs::remove_dir_all(&h.dir);
    }
}

#[cfg(test)]
#[path = "local_acp_tests.rs"]
mod local_acp_tests;
