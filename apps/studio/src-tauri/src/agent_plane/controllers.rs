//! HTTP surface. Paths mirror the renderer's DataApi routes under `/api`, plus turn,
//! approval, and event routes standing in for the Electron `ai.stream.*` IPC.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

#[allow(unused_imports)]
use a3s_boot::{
    BootError, Result, SseEvent, SseStream, body, controller, delete, get, param, patch, post, put,
    query, sse,
};
use futures_util::StreamExt;
use serde_json::{Value, json};

use super::catalog::{AgentCatalog, PinCatalog, SessionCatalog};
use super::error::PlaneError;
use super::session_query::{ExecutionStatus, SessionQuery};
use super::tasks::TaskService;
use super::turns::{EventHub, TurnService};

static HOST_SEAL_SEQ: AtomicU64 = AtomicU64::new(1);
static HOST_SEALS: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);

/// Remember host-produced text deltas. `open` accepts only a seal from this
/// process, so a client `aixOutput` cannot be stored on its own.
pub(super) fn seal_host_deltas(deltas: &[String]) -> Option<String> {
    if deltas.is_empty() || deltas.iter().any(|delta| delta.is_empty()) {
        return None;
    }
    let payload = serde_json::to_string(deltas).ok()?;
    let token = format!(
        "{:x}-{}",
        HOST_SEAL_SEQ.fetch_add(1, Ordering::Relaxed),
        uuid::Uuid::now_v7()
    );
    let mut seals = HOST_SEALS.lock().unwrap_or_else(|error| error.into_inner());
    seals
        .get_or_insert_with(HashMap::new)
        .insert(token.clone(), payload);
    Some(token)
}

pub(super) fn take_host_deltas(token: &str) -> Option<Vec<String>> {
    if token.is_empty() {
        return None;
    }
    let mut seals = HOST_SEALS.lock().unwrap_or_else(|error| error.into_inner());
    let payload = seals.as_mut()?.remove(token)?;
    let deltas: Vec<String> = serde_json::from_str(&payload).ok()?;
    if deltas.is_empty() || deltas.iter().any(|delta| delta.is_empty()) {
        return None;
    }
    Some(deltas)
}

pub(super) async fn prepare_edge_turn(
    mut body: Value,
    caps: Option<&std::path::Path>,
) -> Result<Value> {
    // A client field is not a result. Cloud admission owns create, logs, and
    // stop, and does not fill this text from a local package.
    if let Some(object) = body.as_object_mut() {
        object.remove("aixOutput");
        object.remove("aixHostSeal");
    }
    if super::cloud_runtime::is_cloud_turn(&body) {
        return Ok(body);
    }
    if parameter_schema_waits_for_form(&body)? {
        return Ok(body);
    }
    if body
        .get("turnContext")
        .and_then(|turn| turn.get("agentRuntime"))
        .is_none()
    {
        return Ok(body);
    }
    // An installed docx-review agent stays on the segmented host turn.
    // The builtin protocol echo does not read the contract or write findings.
    if host_reviews_document(&body, caps) {
        return Ok(body);
    }
    let deltas = declared_agent_output(&body, caps, None, None).await?;
    let Some(seal) = seal_host_deltas(&deltas) else {
        return Err(BootError::from(PlaneError::bad(
            "agent runtime output is missing",
        )));
    };
    if let Some(object) = body.as_object_mut() {
        let text: String = deltas.concat();
        object.insert("aixOutput".to_owned(), json!(text));
        object.insert("aixHostSeal".to_owned(), json!(seal));
    }
    Ok(body)
}

/// The installed agent requires exactly one `docx-review` skill, so this turn
/// keeps the host's segmented read instead of a one-shot runtime echo.
fn host_reviews_document(body: &Value, caps: Option<&std::path::Path>) -> bool {
    let Some(agent_id) = body
        .get("turnContext")
        .and_then(|turn| turn.get("agentId"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|agent_id| !agent_id.is_empty())
    else {
        return false;
    };
    let Some(caps) = caps else {
        return false;
    };
    let Some(tenant) = caps.parent() else {
        return false;
    };
    crate::review_plugin::docx_review_instructions(Some(tenant), agent_id).is_some()
}

/// A declared parameter schema with no submitted values waits for the conversation form.
fn parameter_schema_waits_for_form(body: &Value) -> Result<bool> {
    let Some(turn) = body.get("turnContext").and_then(Value::as_object) else {
        return Ok(false);
    };
    let Some(schema) = turn.get("parametersSchema").and_then(Value::as_str) else {
        return Ok(false);
    };
    if a3s_use_extension::parse_agent_parameter_schema(schema.as_bytes()).is_err() {
        return Err(BootError::from(PlaneError::bad(
            "agent parameters are invalid",
        )));
    }
    Ok(turn.get("agentRuntime").is_some() && turn.get("parameters").is_none())
}

/// Run one declared agent after its parameter form has been submitted, or immediately when it has no schema.
pub(super) async fn declared_agent_output(
    body: &Value,
    caps: Option<&std::path::Path>,
    cloud_base: Option<&str>,
    cloud_bearer: Option<&str>,
) -> Result<Vec<String>> {
    let runtime = body
        .get("turnContext")
        .and_then(|turn| turn.get("agentRuntime"));
    let Some(runtime) = runtime else {
        return Err(BootError::from(PlaneError::bad(
            "agent runtime output is missing",
        )));
    };
    if runtime.get("runtime").and_then(Value::as_str) == Some("remote") {
        let card = runtime.get("card").and_then(Value::as_str).unwrap_or("");
        let prompt = user_prompt(body);
        let parameters = body
            .get("turnContext")
            .and_then(|turn| turn.get("parameters"));
        return super::edge_runtime::remote_card_text(card, &prompt, parameters)
            .await
            .map(|text| vec![text])
            .map_err(|error| BootError::from(PlaneError::bad(error)));
    }
    match runtime.get("runtime").and_then(Value::as_str) {
        Some("edge") => {}
        Some("cloud") => {
            return project_cloud_turn(body, cloud_base, cloud_bearer)
                .await
                .map(|text| vec![text]);
        }
        _ => {
            return Err(BootError::from(PlaneError::bad(
                "agent runtime output is missing",
            )));
        }
    }
    match runtime.get("harness").and_then(Value::as_str) {
        Some("package") => {
            if runtime.get("card").is_some() {
                return Err(BootError::from(PlaneError::bad(
                    "this agent cannot declare an agent card",
                )));
            }
            let slug = body
                .get("turnContext")
                .and_then(|turn| turn.get("agentId"))
                .and_then(Value::as_str)
                .unwrap_or("");
            let caps = caps.ok_or_else(|| {
                BootError::from(PlaneError::bad(
                    "package entry is not bound to an installed file",
                ))
            })?;
            let installed = crate::capabilities::installed_agent(caps, slug).map_err(|_| {
                BootError::from(PlaneError::bad(
                    "package entry is not bound to an installed file",
                ))
            })?;
            let declared = runtime.get("entry").and_then(Value::as_str).unwrap_or("");
            if !super::edge_runtime::declared_entry_matches(&installed.entry, declared) {
                return Err(BootError::from(PlaneError::bad(
                    "package entry does not match the installed file",
                )));
            }
            let resources = super::edge_runtime::require_enforceable_limits(runtime)
                .map_err(|error| BootError::from(PlaneError::bad(error)))?;
            return super::edge_box::boxed_turn_text(
                &installed.entry,
                declared,
                &resources,
                &edge_command(body, &installed.release_identity),
            )
            .await
            .map_err(|error| BootError::from(PlaneError::bad(error)));
        }
        Some("builtin") => {
            if runtime.get("entry").is_some() || runtime.get("card").is_some() {
                return Err(BootError::from(PlaneError::bad(
                    "a builtin harness cannot declare an entry or an agent card",
                )));
            }
            return super::edge_runtime::builtin_turn_text(&user_prompt(body))
                .await
                .map_err(|error| BootError::from(PlaneError::bad(error)));
        }
        _ => {
            return Err(BootError::from(PlaneError::bad(
                "edge agent harness is not in the closed set: builtin, package",
            )));
        }
    }
}

/// Form completion for a cloud declaration. This does not spawn the package.
async fn project_cloud_turn(
    body: &Value,
    cloud_base: Option<&str>,
    cloud_bearer: Option<&str>,
) -> Result<String> {
    let admission = match super::cloud_runtime::admit(body) {
        Ok(Some(admission)) => admission,
        Ok(None) => {
            return Err(BootError::from(PlaneError::bad(
                "agent runtime output is missing",
            )));
        }
        Err(error) => return Err(BootError::from(error)),
    };
    let Some(base) = cloud_base.filter(|base| !base.is_empty()) else {
        return Err(BootError::from(PlaneError::bad(
            super::cloud_runtime::ENDPOINT_MISSING,
        )));
    };
    super::cloud_runtime::terminal_text(base, &admission, cloud_bearer)
        .await
        .map_err(|error| BootError::from(PlaneError::bad(error)))
}

fn user_prompt(body: &Value) -> String {
    body.get("userMessageParts")
        .and_then(Value::as_array)
        .and_then(|parts| {
            parts
                .iter()
                .find_map(|part| part.get("text").and_then(Value::as_str))
        })
        .unwrap_or("")
        .to_owned()
}

pub(super) fn edge_command(body: &Value, release: &str) -> Value {
    let prompt = user_prompt(body);
    let session_id = uuid::Uuid::now_v7().to_string();
    let run_id = uuid::Uuid::now_v7().to_string();
    let mut start = super::edge_runtime::start_body(&prompt, &session_id, &run_id, release);
    if let Some(parameters) = body
        .get("turnContext")
        .and_then(|turn| turn.get("parameters"))
        .cloned()
    {
        if let Some(object) = start.as_object_mut() {
            object.insert("parameters".to_owned(), parameters);
        }
    }
    start
}

const DEFAULT_AGENT_PAGE: usize = 100;
const DEFAULT_SESSION_PAGE: usize = 50;

pub struct AgentsController {
    pub agents: Arc<AgentCatalog>,
    pub tasks: Arc<TaskService>,
}

#[controller("/api/agents")]
impl AgentsController {
    #[get("/")]
    async fn list(
        &self,
        #[query("search")] search: Option<String>,
        #[query("ids")] ids: Option<String>,
        #[query("page")] page: Option<String>,
        #[query("limit")] limit: Option<String>,
    ) -> Result<Value> {
        Ok(self.agents.list(
            search.as_deref(),
            &id_list(ids),
            number(page, 1)?,
            number(limit, DEFAULT_AGENT_PAGE)?,
        ))
    }

    #[post("/", status = 201)]
    async fn create(&self, #[body] body: Value) -> Result<Value> {
        Ok(json!(self.agents.create(&body)?))
    }

    #[get("/{id}")]
    async fn get(&self, #[param("id")] id: String) -> Result<Value> {
        Ok(json!(self.agents.get(&id)?))
    }

    #[patch("/{id}")]
    async fn update(&self, #[param("id")] id: String, #[body] body: Value) -> Result<Value> {
        Ok(json!(self.agents.update(&id, &body)?))
    }

    #[get("/{id}/tasks")]
    async fn tasks(
        &self,
        #[param("id")] id: String,
        #[query("page")] page: Option<String>,
        #[query("limit")] limit: Option<String>,
    ) -> Result<Value> {
        Ok(self
            .tasks
            .list(Some(&id), number(page, 1)?, number(limit, 50)?))
    }

    #[get("/{id}/tasks/{task_id}/logs")]
    async fn task_logs(
        &self,
        #[param("id")] id: String,
        #[param("task_id")] task_id: String,
        #[query("limit")] limit: Option<String>,
    ) -> Result<Value> {
        Ok(self.tasks.logs(&id, &task_id, number(limit, 50)?)?)
    }
}

pub struct TasksController {
    pub tasks: Arc<TaskService>,
}

#[controller("/api/agent-tasks")]
impl TasksController {
    #[get("/")]
    async fn list(
        &self,
        #[query("page")] page: Option<String>,
        #[query("limit")] limit: Option<String>,
    ) -> Result<Value> {
        Ok(self.tasks.list(None, number(page, 1)?, number(limit, 50)?))
    }

    #[post("/", status = 201)]
    async fn create(&self, #[body] body: Value) -> Result<Value> {
        Ok(self.tasks.create(&body)?)
    }

    #[get("/{id}")]
    async fn get(&self, #[param("id")] id: String) -> Result<Value> {
        Ok(self.tasks.get(&id)?)
    }

    #[patch("/{id}")]
    async fn update(&self, #[param("id")] id: String, #[body] body: Value) -> Result<Value> {
        let agent_id = body
            .get("agentId")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| BootError::BadRequest("agentId is required".into()))?;
        let patch = body.get("patch").cloned().unwrap_or(body);
        Ok(self.tasks.update(&agent_id, &id, &patch)?)
    }

    #[post("/{id}/pause")]
    async fn pause(&self, #[param("id")] id: String, #[body] body: Value) -> Result<Value> {
        Ok(self.tasks.set_enabled(&agent_of(&body)?, &id, false)?)
    }

    #[post("/{id}/resume")]
    async fn resume(&self, #[param("id")] id: String, #[body] body: Value) -> Result<Value> {
        Ok(self.tasks.set_enabled(&agent_of(&body)?, &id, true)?)
    }

    #[delete("/{id}")]
    async fn delete(
        &self,
        #[param("id")] id: String,
        #[query("agentId")] agent_id: Option<String>,
    ) -> Result<Value> {
        let agent_id =
            agent_id.ok_or_else(|| BootError::BadRequest("agentId is required".into()))?;
        self.tasks.delete(&agent_id, &id)?;
        Ok(Value::Null)
    }

    #[post("/{id}/run")]
    async fn run(&self, #[param("id")] id: String, #[body] body: Value) -> Result<Value> {
        self.tasks.run(&agent_of(&body)?, &id)?;
        Ok(Value::Null)
    }
}

pub struct SessionsController {
    pub sessions: Arc<SessionCatalog>,
    pub turns: Arc<TurnService>,
    pub hub: Arc<EventHub>,
}

#[controller("/api/agent-sessions")]
impl SessionsController {
    #[get("/")]
    async fn list(
        &self,
        #[query("agentId")] agent_id: Option<String>,
        #[query("workspaceId")] workspace_id: Option<String>,
        #[query("ids")] ids: Option<String>,
        #[query("cursor")] cursor: Option<String>,
        #[query("limit")] limit: Option<String>,
        #[query("inTrash")] in_trash: Option<String>,
        #[query("search")] search: Option<String>,
        #[query("status")] status: Option<String>,
        #[query("activityFrom")] activity_from: Option<String>,
        #[query("activityBefore")] activity_before: Option<String>,
    ) -> Result<Value> {
        let in_trash = in_trash
            .map(|value| value.parse::<bool>())
            .transpose()
            .map_err(|_| BootError::BadRequest("inTrash must be true or false".into()))?
            .unwrap_or(false);
        Ok(self.sessions.list(
            &SessionQuery {
                agent_id,
                workspace_id,
                ids: ids
                    .map(|value| value.split(',').map(str::to_owned).collect())
                    .unwrap_or_default(),
                in_trash,
                search,
                status: status.as_deref().map(ExecutionStatus::parse).transpose()?,
                activity_from,
                activity_before,
            },
            cursor.as_deref(),
            number(limit, DEFAULT_SESSION_PAGE)?,
        )?)
    }

    #[get("/latest")]
    async fn latest(&self, #[query("agentId")] agent_id: Option<String>) -> Result<Value> {
        Ok(self.sessions.latest(agent_id.as_deref()))
    }

    #[post("/", status = 201)]
    async fn create(&self, #[body] body: Value) -> Result<Value> {
        Ok(json!(self.sessions.create(&body)?))
    }

    #[post("/reuse-or-create")]
    async fn reuse_or_create(&self, #[body] body: Value) -> Result<Value> {
        Ok(self.sessions.reuse_or_create(&body)?)
    }

    /// Start or reuse the long-lived a3s-code process for the ToBot session.
    #[post("/tobot/ensure")]
    async fn ensure_tobot(&self, #[body] body: Value) -> Result<Value> {
        Ok(self.turns.ensure_tobot(&body).await?)
    }

    /// Stop the live ToBot turn and delete its transcript. The session id stays.
    #[post("/tobot/reset")]
    async fn reset_tobot(&self, #[body] _body: Value) -> Result<Value> {
        Ok(self.turns.reset_tobot().await?)
    }

    /// Start or reuse the long-lived a3s-code process for the ToBoss session.
    #[post("/toboss/ensure")]
    async fn ensure_toboss(&self, #[body] body: Value) -> Result<Value> {
        Ok(self.turns.ensure_toboss(&body).await?)
    }

    /// Start the ToBoss process for one group. The private session stays up.
    #[post("/toboss/room/ensure")]
    async fn ensure_toboss_room(&self, #[body] body: Value) -> Result<Value> {
        Ok(self.turns.ensure_toboss_room(&body).await?)
    }

    /// Stop the live ToBoss turn and delete its transcript. The session id stays.
    #[post("/toboss/reset")]
    async fn reset_toboss(&self, #[body] _body: Value) -> Result<Value> {
        Ok(self.turns.reset_toboss().await?)
    }

    /// `ai.agent.session.delete`. The row stays on disk with `deletedAt` set.
    #[post("/archive")]
    async fn archive(&self, #[body] body: Value) -> Result<Value> {
        let result = self.sessions.archive(&body)?;
        if let Some(ids) = result.get("deletedIds").and_then(Value::as_array) {
            self.hub
                .publish("data.changed", json!({ "sessionIds": ids }));
        }
        Ok(result)
    }

    #[get("/archive-operations/{id}")]
    async fn archive_outcome(
        &self,
        #[param("id")] id: String,
        #[query("epoch")] epoch: Option<String>,
    ) -> Result<Value> {
        let epoch =
            epoch.ok_or_else(|| BootError::BadRequest("archive epoch is required".into()))?;
        Ok(self.sessions.archive_outcome(&id, &epoch)?)
    }

    #[post("/{id}/restore")]
    async fn restore(&self, #[param("id")] id: String) -> Result<Value> {
        let session = self.sessions.restore(&id)?;
        self.hub
            .publish("data.changed", json!({ "sessionIds": [&id] }));
        Ok(json!(session))
    }

    /// `ai.agent.session.delete_permanently`. The row and its own folder are removed.
    #[post("/delete")]
    async fn delete(&self, #[body] body: Value) -> Result<Value> {
        if let Some(ids) = body.get("sessionIds").and_then(Value::as_array) {
            for id in ids.iter().filter_map(Value::as_str) {
                self.turns.abort(id);
            }
        }
        let result = self.sessions.destroy(&body)?;
        if let Some(ids) = result.get("deletedIds").and_then(Value::as_array) {
            self.hub
                .publish("data.changed", json!({ "sessionIds": ids }));
        }
        Ok(result)
    }

    #[get("/{id}")]
    async fn get(&self, #[param("id")] id: String) -> Result<Value> {
        let id = self.turns.resolve_session_id(&id)?;
        Ok(json!(self.sessions.get(&id)?))
    }

    #[patch("/{id}")]
    async fn update(&self, #[param("id")] id: String, #[body] body: Value) -> Result<Value> {
        let id = self.turns.resolve_session_id(&id)?;
        Ok(json!(self.sessions.update(&id, &body)?))
    }

    #[put("/{id}/workspace")]
    async fn set_workspace(&self, #[param("id")] id: String, #[body] body: Value) -> Result<Value> {
        let id = self.turns.resolve_session_id(&id)?;
        Ok(json!(self.sessions.set_workspace(&id, &body)?))
    }

    #[get("/{id}/messages")]
    async fn messages(
        &self,
        #[param("id")] id: String,
        #[query("cursor")] cursor: Option<String>,
        #[query("limit")] limit: Option<String>,
    ) -> Result<Value> {
        let id = self.turns.resolve_session_id(&id)?;
        self.turns.settle_orphan_pending(&id);
        Ok(self
            .sessions
            .messages(&id, cursor.as_deref(), number(limit, DEFAULT_SESSION_PAGE)?)?)
    }

    /// `ai.agent.session.fork`. Copies history through a completed assistant turn.
    #[post("/{id}/fork")]
    async fn fork(&self, #[param("id")] id: String, #[body] body: Value) -> Result<Value> {
        let id = self.turns.resolve_session_id(&id)?;
        let message_id = body
            .get("messageId")
            .and_then(Value::as_str)
            .ok_or_else(|| PlaneError::bad("messageId is required"))?;
        Ok(self.sessions.fork(&id, message_id)?)
    }

    #[get("/{id}/messages/{message_id}")]
    async fn message(
        &self,
        #[param("id")] id: String,
        #[param("message_id")] message_id: String,
    ) -> Result<Value> {
        let id = self.turns.resolve_session_id(&id)?;
        Ok(json!(self.sessions.message(&id, &message_id)?))
    }

    #[delete("/{id}/messages/{message_id}")]
    async fn delete_message(
        &self,
        #[param("id")] id: String,
        #[param("message_id")] message_id: String,
    ) -> Result<Value> {
        let id = self.turns.resolve_session_id(&id)?;
        self.turns.delete_message(&id, &message_id).await?;
        Ok(Value::Null)
    }

    /// `ai.stream.open`: persists the turn and starts a3s-code; the reply streams over events.
    #[post("/{id}/turns")]
    async fn open_turn(&self, #[param("id")] id: String, #[body] body: Value) -> Result<Value> {
        let id = self.turns.resolve_session_id(&id)?;
        // Admission precedes an AIX executor, which may already write files.
        self.sessions.get(&id)?;
        super::workbench_proof::Policy::admit_body(&body).map_err(BootError::BadRequest)?;
        if self.turns.is_live(&id) {
            return Ok(self.turns.insert_into_live(&id, &body).await?);
        }
        let caps = self.turns.capability_root();
        let body = prepare_edge_turn(body, caps.as_deref()).await?;
        Ok(self.turns.open(&id, &body)?)
    }

    /// Host-owned evidence for this authorized session.
    #[get("/{id}/proofs")]
    async fn proofs(&self, #[param("id")] id: String) -> Result<Value> {
        let id = self.turns.resolve_session_id(&id)?;
        Ok(self.turns.workbench_proofs(&id)?)
    }

    #[get("/{id}/proof-artifacts/{artifact_id}")]
    async fn proof_artifact(
        &self,
        #[param("id")] id: String,
        #[param("artifact_id")] artifact_id: String,
    ) -> Result<Value> {
        let id = self.turns.resolve_session_id(&id)?;
        Ok(self.turns.workbench_artifact(&id, &artifact_id)?)
    }

    #[post("/{id}/messages/{message_id}/proof")]
    async fn proof_action(
        &self,
        #[param("id")] id: String,
        #[param("message_id")] message_id: String,
        #[body] body: Value,
    ) -> Result<Value> {
        let id = self.turns.resolve_session_id(&id)?;
        Ok(self.turns.workbench_proof_action(&id, &message_id, body)?)
    }

    /// `ai.stream.attach`: chunks of the running turn so far.
    #[get("/{id}/turn")]
    async fn attach(&self, #[param("id")] id: String) -> Result<Value> {
        let id = self.turns.resolve_session_id(&id)?;
        Ok(self.turns.attach(&id))
    }

    /// `ai.stream.abort`.
    #[post("/{id}/turn/abort")]
    async fn abort(&self, #[param("id")] id: String) -> Result<Value> {
        let id = self.turns.resolve_session_id(&id)?;
        self.turns.abort(&id);
        Ok(json!({ "ok": true }))
    }
}

pub struct PinsController {
    pub pins: Arc<PinCatalog>,
}

#[controller("/api/pins")]
impl PinsController {
    #[get("/")]
    async fn list(&self, #[query("entityType")] entity_type: Option<String>) -> Result<Value> {
        Ok(self.pins.list(entity_type.as_deref())?)
    }

    #[post("/")]
    async fn create(&self, #[body] body: Value) -> Result<Value> {
        Ok(json!(self.pins.create(&body)?))
    }

    #[get("/{id}")]
    async fn get(&self, #[param("id")] id: String) -> Result<Value> {
        Ok(json!(self.pins.get(&id)?))
    }

    #[delete("/{id}")]
    async fn delete(&self, #[param("id")] id: String) -> Result<Value> {
        self.pins.delete(&id)?;
        Ok(Value::Null)
    }
}

pub struct ApprovalsController {
    pub turns: Arc<TurnService>,
}

#[controller("/api/agent-approvals")]
impl ApprovalsController {
    /// `ai.tool.respond_approval`.
    #[post("/{id}")]
    async fn respond(&self, #[param("id")] id: String, #[body] body: Value) -> Result<Value> {
        let approved = body
            .get("approved")
            .and_then(Value::as_bool)
            .ok_or_else(|| BootError::BadRequest("approved is required".into()))?;
        Ok(json!({ "ok": self.turns.respond_approval(&id, approved) }))
    }
}

pub struct EventsController {
    pub hub: Arc<EventHub>,
}

#[controller("/api/agent-events")]
impl EventsController {
    /// Every `ai.stream.*`, stream-status, and data-change event, as `{ channel, payload }`.
    #[sse("/")]
    async fn stream(&self) -> Result<SseStream> {
        let hello =
            futures_util::stream::iter([Ok(SseEvent::comment("connected").with_retry(2000))]);
        let events = super::events::stream(self.hub.clone());
        Ok(hello.chain(events).boxed())
    }
}

fn agent_of(body: &Value) -> Result<String> {
    body.get("agentId")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| BootError::BadRequest("agentId is required".into()))
}

fn id_list(raw: Option<String>) -> Vec<String> {
    raw.map(|value| {
        value
            .split(',')
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(str::to_owned)
            .collect()
    })
    .unwrap_or_default()
}

fn number(raw: Option<String>, default: usize) -> Result<usize> {
    match raw {
        None => Ok(default),
        Some(value) => value
            .parse()
            .map_err(|_| BootError::BadRequest(format!("{value} is not a number"))),
    }
}
