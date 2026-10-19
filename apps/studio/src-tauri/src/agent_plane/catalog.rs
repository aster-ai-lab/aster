//! Agent and session use cases behind the DataApi routes.

use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use serde_json::{Value, json};

#[cfg(test)]
use super::domain::BUILTIN_TOBOSS_SESSION_ID;
use super::domain::{
    AGENT_TYPES, Agent, BUILTIN_CODE_AGENT_ASSET_ID, BUILTIN_CODE_AGENT_ID,
    BUILTIN_CODE_AGENT_RELEASE_ID, BUILTIN_CODE_PROVIDER_KIND, BUILTIN_TOBOT_SESSION_ID, Message,
    Pin, Session, Workspace, is_knowledge_compile_session, is_resident_session,
    is_toboss_room_session, is_toboss_session, is_tobot_session, new_id, now_iso,
};
use super::error::PlaneError;
use super::session_archive::{
    ArchiveItem, ArchiveOperations, ArchiveReceipt, ArchiveRecord, ArchiveRequest, ArchiveStatus,
    operation_uuid,
};
use super::session_query::{ListedSession, SessionQuery};
use super::store::PlaneStore;

const DEFAULT_ORDER_KEY: &str = "a0";
const MAX_PAGE: usize = 500;

pub struct AgentCatalog {
    store: Arc<PlaneStore>,
}

impl AgentCatalog {
    pub fn new(store: Arc<PlaneStore>) -> Self {
        Self { store }
    }

    pub fn list(&self, search: Option<&str>, ids: &[String], page: usize, limit: usize) -> Value {
        let needle = search
            .map(str::to_lowercase)
            .filter(|value| !value.is_empty());
        let rows: Vec<Agent> = self
            .store
            .agents()
            .into_iter()
            .filter(|agent| agent.deleted_at.is_none())
            .filter(|agent| {
                agent.id != BUILTIN_CODE_AGENT_ID || ids.iter().any(|id| id == &agent.id)
            })
            .filter(|agent| ids.is_empty() || ids.contains(&agent.id))
            .filter(|agent| {
                needle
                    .as_ref()
                    .is_none_or(|needle| agent.name.to_lowercase().contains(needle))
            })
            .collect();
        let limit = limit.clamp(1, MAX_PAGE);
        let page = page.max(1);
        let items: Vec<&Agent> = rows.iter().skip((page - 1) * limit).take(limit).collect();
        json!({ "items": items, "total": rows.len(), "page": page })
    }

    pub fn get(&self, id: &str) -> Result<Agent, PlaneError> {
        self.store
            .agent(id)
            .filter(|agent| agent.deleted_at.is_none())
            .ok_or_else(|| PlaneError::NotFound(format!("agent {id}")))
    }

    pub fn create(&self, body: &Value) -> Result<Agent, PlaneError> {
        let kind = text(body, "type").ok_or_else(|| PlaneError::bad("type is required"))?;
        if !AGENT_TYPES.contains(&kind.as_str()) {
            return Err(PlaneError::bad("unsupported agent type"));
        }
        let name = text(body, "name")
            .map(|name| name.trim().to_owned())
            .filter(|name| !name.is_empty())
            .ok_or_else(|| PlaneError::bad("name is required"))?;
        let model = model_id(body, "model")?.ok_or_else(|| PlaneError::bad("model is required"))?;
        let now = now_iso();
        let agent = Agent {
            id: new_id(),
            kind,
            name,
            description: text(body, "description"),
            instructions: text(body, "instructions"),
            model_name: model_name(&model),
            model: Some(model),
            plan_model: model_id(body, "planModel")?,
            small_model: model_id(body, "smallModel")?,
            mcps: strings(body, "mcps"),
            knowledge_base_ids: strings(body, "knowledgeBaseIds"),
            disabled_tools: strings(body, "disabledTools"),
            configuration: body
                .get("configuration")
                .filter(|value| value.is_object())
                .cloned(),
            order_key: DEFAULT_ORDER_KEY.to_owned(),
            created_at: now.clone(),
            updated_at: now,
            deleted_at: None,
        };
        self.store.save_agent(agent.clone())?;
        Ok(agent)
    }

    pub fn update(&self, id: &str, patch: &Value) -> Result<Agent, PlaneError> {
        let mut agent = self.get(id)?;
        if let Some(name) = text(patch, "name") {
            let name = name.trim();
            if name.is_empty() {
                return Err(PlaneError::bad("name is required"));
            }
            agent.name = name.to_owned();
        }
        if patch.get("description").is_some() {
            agent.description = text(patch, "description");
        }
        if patch.get("instructions").is_some() {
            agent.instructions = text(patch, "instructions");
        }
        if let Some(model) = model_id(patch, "model")? {
            agent.model_name = model_name(&model);
            agent.model = Some(model);
        }
        if patch.get("planModel").is_some() {
            agent.plan_model = model_id(patch, "planModel")?;
        }
        if patch.get("smallModel").is_some() {
            agent.small_model = model_id(patch, "smallModel")?;
        }
        if patch.get("mcps").is_some() {
            agent.mcps = strings(patch, "mcps");
        }
        if patch.get("knowledgeBaseIds").is_some() {
            agent.knowledge_base_ids = strings(patch, "knowledgeBaseIds");
        }
        if patch.get("disabledTools").is_some() {
            agent.disabled_tools = strings(patch, "disabledTools");
        }
        if let Some(configuration) = patch.get("configuration").filter(|value| value.is_object()) {
            let mut merged = agent.configuration.clone().unwrap_or_else(|| json!({}));
            if let (Some(base), Some(incoming)) =
                (merged.as_object_mut(), configuration.as_object())
            {
                for (key, value) in incoming {
                    base.insert(key.clone(), value.clone());
                }
            }
            agent.configuration = Some(merged);
        }
        agent.updated_at = now_iso();
        self.store.save_agent(agent.clone())?;
        Ok(agent)
    }

    /// The platform a3s-code agent. A later call with a model refreshes that model.
    pub fn ensure_builtin_code_agent(&self, model: Option<&str>) -> Result<Agent, PlaneError> {
        if let Some(model) = model {
            if super::domain::split_model_id(model).is_none() {
                return Err(PlaneError::bad("model must be providerId::modelId"));
            }
        }
        let now = now_iso();
        if let Some(mut agent) = self.store.agent(BUILTIN_CODE_AGENT_ID) {
            let configuration = builtin_code_configuration(agent.configuration.as_ref());
            let mut changed = agent.deleted_at.take().is_some();
            if agent.kind != "a3s-code" {
                agent.kind = "a3s-code".to_owned();
                changed = true;
            }
            if agent.name != BUILTIN_CODE_AGENT_ID {
                agent.name = BUILTIN_CODE_AGENT_ID.to_owned();
                changed = true;
            }
            if agent.configuration.as_ref() != Some(&configuration) {
                agent.configuration = Some(configuration);
                changed = true;
            }
            if let Some(model) = model {
                if agent.model.as_deref() != Some(model) {
                    agent.model_name = model_name(model);
                    agent.model = Some(model.to_owned());
                    changed = true;
                }
            }
            if changed {
                agent.updated_at = now;
                self.store.save_agent(agent.clone())?;
            }
            return Ok(agent);
        }
        let model_name = model.and_then(model_name);
        let agent = Agent {
            id: BUILTIN_CODE_AGENT_ID.to_owned(),
            kind: "a3s-code".to_owned(),
            name: BUILTIN_CODE_AGENT_ID.to_owned(),
            description: None,
            instructions: None,
            model_name,
            model: model.map(str::to_owned),
            plan_model: None,
            small_model: None,
            mcps: None,
            knowledge_base_ids: None,
            disabled_tools: None,
            configuration: Some(builtin_code_configuration(None)),
            order_key: DEFAULT_ORDER_KEY.to_owned(),
            created_at: now.clone(),
            updated_at: now,
            deleted_at: None,
        };
        self.store.save_agent(agent.clone())?;
        Ok(agent)
    }
}

pub struct SessionCatalog {
    store: Arc<PlaneStore>,
    system_workspace_path: PathBuf,
    archives: ArchiveOperations,
}

impl SessionCatalog {
    pub fn new(store: Arc<PlaneStore>) -> Self {
        let system_workspace_path = store
            .root()
            .map(|root| root.join("workspace"))
            .unwrap_or_else(|| std::env::temp_dir().join("a3s-studio-agent-workspace"));
        Self {
            store,
            system_workspace_path,
            archives: ArchiveOperations::new(),
        }
    }

    pub fn list(
        &self,
        query: &SessionQuery,
        cursor: Option<&str>,
        limit: usize,
    ) -> Result<Value, PlaneError> {
        let query = query.clone().normalized()?;
        let executions = self.store.session_executions();
        let mut rows: Vec<ListedSession> = self
            .store
            .sessions()
            .into_iter()
            .filter(|session| session.deleted_at.is_some() == query.in_trash)
            .filter(|session| {
                let requested = query.ids.iter().any(|id| id == &session.id);
                (!is_resident_session(&session.id) && !is_knowledge_compile_session(session))
                    || requested
            })
            .filter(|session| matches_agent(session, query.agent_id.as_deref()))
            .filter(|session| {
                query
                    .workspace_id
                    .as_ref()
                    .is_none_or(|id| &session.workspace_id == id)
            })
            .filter(|session| query.ids.is_empty() || query.ids.contains(&session.id))
            .map(|session| ListedSession {
                execution: executions.get(&session.id).cloned(),
                session,
            })
            .filter(|row| query.matches(row))
            .map(|mut row| {
                row.session = self.bind_session_folder(row.session);
                row
            })
            .collect();
        rows.sort_by(|left, right| {
            right
                .session
                .last_activity_at
                .cmp(&left.session.last_activity_at)
                .then_with(|| left.session.id.cmp(&right.session.id))
        });
        let mut page = query.page(rows, cursor, limit)?;
        page["archiveEpoch"] = json!(self.archives.epoch);
        Ok(page)
    }

    pub fn latest(&self, agent_id: Option<&str>) -> Value {
        let session = self
            .store
            .sessions()
            .into_iter()
            .filter(|session| session.deleted_at.is_none())
            .filter(|session| !is_resident_session(&session.id))
            .filter(|session| !is_knowledge_compile_session(session))
            .filter(|session| matches_agent(session, agent_id))
            .max_by(|left, right| left.last_activity_at.cmp(&right.last_activity_at))
            .map(|session| self.bind_session_folder(session));
        json!({ "session": session })
    }

    pub fn get(&self, id: &str) -> Result<Session, PlaneError> {
        let session = self
            .store
            .session(id)
            .filter(|session| session.deleted_at.is_none())
            .ok_or_else(|| PlaneError::NotFound(format!("session {id}")))?;
        Ok(self.bind_session_folder(session))
    }

    pub fn create(&self, body: &Value) -> Result<Session, PlaneError> {
        let agent_id = text(body, "agentId");
        let id = new_id();
        let workspace = self.workspace_for(body.get("workspace"), &id)?;
        let name = text(body, "name").unwrap_or_default();
        let now = now_iso();
        let session = Session {
            id,
            agent_id,
            is_name_manually_edited: !name.trim().is_empty(),
            name: name.trim().chars().take(255).collect(),
            description: text(body, "description"),
            workspace_id: workspace.id.clone(),
            workspace,
            order_key: DEFAULT_ORDER_KEY.to_owned(),
            last_activity_at: now.clone(),
            created_at: now.clone(),
            updated_at: now,
            deleted_at: None,
        };
        self.store.save_session(session.clone())?;
        Ok(session)
    }

    /// Harness session `tobot`. A signed-in member uses `ensure_tobot_session`.
    pub fn ensure_builtin_tobot(&self) -> Result<Session, PlaneError> {
        self.ensure_tobot_session(BUILTIN_TOBOT_SESSION_ID)
    }

    /// One system session for this ToBot id. Does not copy another member's file.
    pub fn ensure_tobot_session(&self, session_id: &str) -> Result<Session, PlaneError> {
        if !is_tobot_session(session_id) {
            return Err(PlaneError::bad("invalid session id"));
        }
        self.ensure_named_session(session_id, "ToBot")
    }

    /// One system session for this ToBoss id. Does not copy another member's file.
    pub fn ensure_toboss_session(&self, session_id: &str) -> Result<Session, PlaneError> {
        if !is_toboss_session(session_id) && !is_toboss_room_session(session_id) {
            return Err(PlaneError::bad("invalid session id"));
        }
        self.ensure_named_session(session_id, "ToBot")
    }

    fn ensure_named_session(&self, session_id: &str, name: &str) -> Result<Session, PlaneError> {
        let workspace = self.system_workspace_for(session_id)?;
        let now = now_iso();
        if let Some(mut session) = self.store.session(session_id) {
            let mut changed = session.deleted_at.take().is_some();
            if session.agent_id.as_deref() != Some(BUILTIN_CODE_AGENT_ID) {
                session.agent_id = Some(BUILTIN_CODE_AGENT_ID.to_owned());
                changed = true;
            }
            if session.name != name {
                session.name = name.to_owned();
                session.is_name_manually_edited = true;
                changed = true;
            }
            if session.workspace.kind != workspace.kind || session.workspace.path != workspace.path
            {
                session.workspace_id = workspace.id.clone();
                session.workspace = workspace;
                changed = true;
            }
            if changed {
                session.updated_at = now;
                self.store.save_session(session.clone())?;
            }
            return Ok(self.bind_session_folder(session));
        }
        let session = Session {
            id: session_id.to_owned(),
            agent_id: Some(BUILTIN_CODE_AGENT_ID.to_owned()),
            is_name_manually_edited: true,
            name: name.to_owned(),
            description: None,
            workspace_id: workspace.id.clone(),
            workspace,
            order_key: DEFAULT_ORDER_KEY.to_owned(),
            last_activity_at: now.clone(),
            created_at: now.clone(),
            updated_at: now,
            deleted_at: None,
        };
        self.store.save_session(session.clone())?;
        Ok(session)
    }

    /// An untitled, message-less session for the same agent and folder is reused.
    /// A system session's folder is its own directory, so reuse matches the
    /// system kind rather than the shared default workspace.
    pub fn reuse_or_create(&self, body: &Value) -> Result<Value, PlaneError> {
        let agent_id =
            text(body, "agentId").ok_or_else(|| PlaneError::bad("agentId is required"))?;
        let source = body
            .get("workspace")
            .ok_or_else(|| PlaneError::bad("workspace is required"))?;
        let excluded = text(body, "excludeSessionId");
        if source.get("type").and_then(Value::as_str) == Some("system") {
            let reusable = self.store.sessions().into_iter().find(|session| {
                session.deleted_at.is_none()
                    && !is_resident_session(&session.id)
                    && session.agent_id.as_deref() == Some(agent_id.as_str())
                    && session.workspace.kind == "system"
                    && session.name.is_empty()
                    && excluded.as_deref() != Some(session.id.as_str())
                    && self.store.messages(&session.id).is_empty()
            });
            if let Some(session) = reusable {
                let session = self.bind_session_folder(session);
                return Ok(json!({
                    "session": session,
                    "created": false,
                    "deletedDuplicateSessionIds": []
                }));
            }
            let session = self.create(&json!({
                "agentId": agent_id,
                "name": "",
                "workspace": { "type": "system" },
            }))?;
            return Ok(
                json!({ "session": session, "created": true, "deletedDuplicateSessionIds": [] }),
            );
        }
        let workspace = self.workspace_for(Some(source), "user")?;
        let reusable = self.store.sessions().into_iter().find(|session| {
            session.deleted_at.is_none()
                && !is_resident_session(&session.id)
                && session.agent_id.as_deref() == Some(agent_id.as_str())
                && session.workspace.path == workspace.path
                && session.name.is_empty()
                && excluded.as_deref() != Some(session.id.as_str())
                && self.store.messages(&session.id).is_empty()
        });
        if let Some(session) = reusable {
            return Ok(
                json!({ "session": session, "created": false, "deletedDuplicateSessionIds": [] }),
            );
        }
        let session = self.create(&json!({
            "agentId": agent_id,
            "name": "",
            "workspace": { "type": workspace.kind, "workspace": workspace },
        }))?;
        Ok(json!({ "session": session, "created": true, "deletedDuplicateSessionIds": [] }))
    }

    pub fn update(&self, id: &str, patch: &Value) -> Result<Session, PlaneError> {
        if is_resident_session(id) {
            if let Some(agent_id) = text(patch, "agentId") {
                if agent_id != BUILTIN_CODE_AGENT_ID {
                    return Err(PlaneError::Conflict(format!(
                        "the {} session stays on a3s-code",
                        resident_label(id)
                    )));
                }
            }
        }
        let mut session = self.get(id)?;
        if let Some(name) = text(patch, "name") {
            session.name = name.trim().chars().take(255).collect();
            session.is_name_manually_edited = patch
                .get("isNameManuallyEdited")
                .and_then(Value::as_bool)
                .unwrap_or(true);
        } else if let Some(edited) = patch.get("isNameManuallyEdited").and_then(Value::as_bool) {
            session.is_name_manually_edited = edited;
        }
        if patch.get("description").is_some() {
            session.description = text(patch, "description");
        }
        if patch.get("agentId").is_some() {
            session.agent_id = text(patch, "agentId");
        }
        session.updated_at = now_iso();
        self.store.save_session(session.clone())?;
        Ok(session)
    }

    /// Hide explicit tasks without stopping execution or deleting their files.
    pub fn archive(&self, body: &Value) -> Result<Value, PlaneError> {
        let request = ArchiveRequest::parse(body)?;
        refuse_resident_session(&request.session_ids)?;
        if request
            .epoch
            .as_ref()
            .is_some_and(|epoch| epoch != &self.archives.epoch)
        {
            return Err(PlaneError::Conflict(
                "archive instance changed; review the task list again".into(),
            ));
        }
        let identified = request.operation_id.is_some();
        let operation_id = request.operation_id.unwrap_or_else(new_id);
        let mut ids = request.session_ids.clone();
        ids.sort();
        let mut records = self
            .archives
            .records
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if let Some(record) = records.get(&operation_id) {
            if record.ids != ids {
                return Err(PlaneError::Conflict(
                    "archive operation selection changed".into(),
                ));
            }
            return Ok(json!(record.receipt));
        }
        let mut deleted_ids = Vec::new();
        let mut results = Vec::new();
        for id in request.session_ids {
            let status = match self.store.set_session_archived(&id, true) {
                Ok(Some((_, true))) => {
                    deleted_ids.push(id.clone());
                    ArchiveStatus::Archived
                }
                Ok(Some((_, false))) => ArchiveStatus::AlreadyArchived,
                Ok(None) => ArchiveStatus::NotFound,
                Err(error) => {
                    if !identified {
                        return Err(error.into());
                    }
                    eprintln!(
                        "studio: {} archive item {id}: {error}",
                        ArchiveStatus::Failed.as_str()
                    );
                    ArchiveStatus::Failed
                }
            };
            results.push(ArchiveItem {
                session_id: id,
                status,
            });
        }
        if identified {
            let receipt = ArchiveReceipt {
                operation_id: operation_id.clone(),
                epoch: self.archives.epoch.clone(),
                deleted_ids,
                results,
            };
            let response = json!(receipt);
            records.insert(operation_id, ArchiveRecord { ids, receipt });
            Ok(response)
        } else {
            Ok(json!({ "deletedIds": deleted_ids }))
        }
    }

    pub fn archive_outcome(&self, id: &str, epoch: &str) -> Result<Value, PlaneError> {
        let id = operation_uuid(id)?;
        let epoch = operation_uuid(epoch)?;
        let records = self
            .archives
            .records
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let receipt = if epoch == self.archives.epoch {
            records.get(&id).map(|record| &record.receipt)
        } else {
            None
        };
        Ok(json!({ "epoch": self.archives.epoch, "receipt": receipt }))
    }

    pub fn restore(&self, id: &str) -> Result<Session, PlaneError> {
        refuse_resident_session(&[id.to_owned()])?;
        let _operations = self
            .archives
            .records
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let session = self
            .store
            .set_session_archived(id, false)?
            .map(|(session, _)| session)
            .ok_or_else(|| PlaneError::NotFound(format!("session {id}")))?;
        Ok(self.bind_session_folder(session))
    }

    /// Remove sessions so they cannot be restored. The task's own folder goes with the row.
    /// A shared parent directory is left in place.
    pub fn destroy(&self, body: &Value) -> Result<Value, PlaneError> {
        let ids = strings(body, "sessionIds")
            .filter(|ids| !ids.is_empty())
            .ok_or_else(|| PlaneError::bad("sessionIds is required"))?;
        refuse_resident_session(&ids)?;
        for id in &ids {
            self.session_directory(id)?;
        }
        let mut deleted_ids = Vec::new();
        for id in ids {
            let Some(session) = self.store.session(&id) else {
                continue;
            };
            let folder = private_task_folder(Path::new(&session.workspace.path), &id);
            if let Some(folder) = &folder {
                remove_private_folder(folder)?;
            }
            let system = self.session_directory(&id)?;
            let same = folder
                .as_ref()
                .is_some_and(|folder| paths_equal(folder, &system));
            if !same {
                remove_private_folder(&system)?;
            }
            self.store.remove_messages(&id)?;
            if let Some(pin) = self
                .store
                .pins()
                .into_iter()
                .find(|pin| pin.entity_type == "session" && pin.entity_id == id)
            {
                self.store.remove_pin(&pin.id)?;
            }
            self.store.remove_session(&id)?;
            deleted_ids.push(id);
        }
        Ok(json!({ "deletedIds": deleted_ids }))
    }

    pub fn set_workspace(&self, id: &str, source: &Value) -> Result<Session, PlaneError> {
        if is_resident_session(id) {
            return Err(PlaneError::Conflict(format!(
                "the {} session keeps its workspace",
                resident_label(id)
            )));
        }
        let mut session = self.get(id)?;
        let place = source.get("place").and_then(Value::as_str);
        let workspace = if place == Some("inbox") {
            let destination = session_folder_destination(
                source.get("path").and_then(Value::as_str).unwrap_or(""),
                &session.id,
            )?;
            relocate_task_folder(
                Path::new(&session.workspace.path),
                &destination,
                &session.id,
            )?;
            inbox_workspace(destination)
        } else {
            let mut workspace = self.workspace_for(Some(source), &session.id)?;
            if place == Some("project") {
                let destination = session_folder_destination(
                    &PathBuf::from(&workspace.path)
                        .join(&session.id)
                        .to_string_lossy(),
                    &session.id,
                )?;
                relocate_task_folder(
                    Path::new(&session.workspace.path),
                    &destination,
                    &session.id,
                )?;
                workspace.path = destination.to_string_lossy().into_owned();
            }
            workspace
        };
        session.workspace_id = workspace.id.clone();
        session.workspace = workspace;
        session.updated_at = now_iso();
        self.store.save_session(session.clone())?;
        Ok(session)
    }

    /// Pages run newest first; `nextCursor` walks toward older rows.
    pub fn messages(
        &self,
        id: &str,
        cursor: Option<&str>,
        limit: usize,
    ) -> Result<Value, PlaneError> {
        self.get(id)?;
        let mut rows = self.store.messages(id);
        rows.reverse();
        Ok(page(rows, cursor, limit))
    }

    pub fn message(&self, id: &str, message_id: &str) -> Result<Message, PlaneError> {
        self.get(id)?;
        self.store
            .messages(id)
            .into_iter()
            .find(|message| message.id == message_id)
            .ok_or_else(|| PlaneError::NotFound(format!("message {message_id}")))
    }

    /// Copies the visible transcript through a completed assistant turn into a
    /// new session on the same agent and workspace. The source turn is left running.
    pub fn fork(&self, source_id: &str, message_id: &str) -> Result<Value, PlaneError> {
        let source = self.get(source_id)?;
        let history = self.store.messages(source_id);
        let boundary = history
            .iter()
            .position(|message| message.id == message_id)
            .ok_or_else(|| PlaneError::NotFound(format!("message {message_id}")))?;
        let selected = &history[boundary];
        if selected.role != "assistant" || selected.status != "success" {
            return Err(PlaneError::bad("not_turn_boundary"));
        }
        let taken: Vec<String> = self
            .store
            .sessions()
            .into_iter()
            .filter(|session| session.deleted_at.is_none())
            .filter(|session| session.agent_id == source.agent_id)
            .map(|session| session.name)
            .collect();
        let child = self.create(&json!({
            "agentId": source.agent_id,
            "name": fork_name(&source.name, &taken),
            "workspace": workspace_body(&source),
        }))?;
        for (index, message) in history.iter().take(boundary + 1).enumerate() {
            self.store.save_message(fork_message(
                message,
                &child.id,
                source_id,
                index == boundary,
            ))?;
        }
        Ok(json!({ "sessionId": child.id }))
    }

    /// `{type:'user', workspace:{…}}` (resolved by the renderer) or `{type:'system'}`.
    /// A system workspace is this session's directory under the shared root.
    fn workspace_for(
        &self,
        source: Option<&Value>,
        session_id: &str,
    ) -> Result<Workspace, PlaneError> {
        let source = source.ok_or_else(|| PlaneError::bad("workspace is required"))?;
        match source.get("type").and_then(Value::as_str) {
            Some("user") => {
                let workspace: Workspace = source
                    .get("workspace")
                    .cloned()
                    .and_then(|value| serde_json::from_value(value).ok())
                    .ok_or_else(|| PlaneError::bad("workspace was not resolved"))?;
                if workspace.path.trim().is_empty() {
                    return Err(PlaneError::bad("workspace path is required"));
                }
                Ok(workspace)
            }
            Some("system") => self.system_workspace_for(session_id),
            _ => Err(PlaneError::bad("unsupported workspace")),
        }
    }

    /// `…/workspace/<session id>`. The shared `workspace` directory is only the parent.
    fn session_directory(&self, session_id: &str) -> Result<PathBuf, PlaneError> {
        if session_id.is_empty()
            || session_id == "."
            || session_id == ".."
            || session_id.contains('/')
            || session_id.contains('\\')
            || session_id.contains('\0')
        {
            return Err(PlaneError::bad("invalid session id"));
        }
        Ok(self.system_workspace_path.join(session_id))
    }

    fn system_workspace_for(&self, session_id: &str) -> Result<Workspace, PlaneError> {
        let directory = self.session_directory(session_id)?;
        std::fs::create_dir_all(&directory)?;
        let now = now_iso();
        Ok(Workspace {
            id: "system".to_owned(),
            name: "Workspace".to_owned(),
            path: directory.to_string_lossy().into_owned(),
            kind: "system".to_owned(),
            order_key: DEFAULT_ORDER_KEY.to_owned(),
            created_at: now.clone(),
            updated_at: now,
        })
    }

    /// Point a system session that still uses the shared default directory at its own folder.
    fn bind_session_folder(&self, mut session: Session) -> Session {
        if session.workspace.kind != "system" {
            return session;
        }
        let Ok(directory) = self.session_directory(&session.id) else {
            return session;
        };
        if paths_equal(Path::new(&session.workspace.path), &directory) {
            let _ = std::fs::create_dir_all(&directory);
            return session;
        }
        if !paths_equal(
            Path::new(&session.workspace.path),
            &self.system_workspace_path,
        ) {
            return session;
        }
        if std::fs::create_dir_all(&directory).is_err() {
            return session;
        }
        let now = now_iso();
        session.workspace.path = directory.to_string_lossy().into_owned();
        session.workspace.updated_at = now.clone();
        session.updated_at = now;
        if self.store.save_session(session.clone()).is_err() {
            return session;
        }
        session
    }
}

fn inbox_workspace(directory: PathBuf) -> Workspace {
    let now = now_iso();
    Workspace {
        id: "system".to_owned(),
        name: "Workspace".to_owned(),
        path: directory.to_string_lossy().into_owned(),
        kind: "system".to_owned(),
        order_key: DEFAULT_ORDER_KEY.to_owned(),
        created_at: now.clone(),
        updated_at: now,
    }
}

fn session_folder_destination(raw: &str, session_id: &str) -> Result<PathBuf, PlaneError> {
    let trimmed = raw.trim().trim_end_matches(['/', '\\']);
    let path = PathBuf::from(trimmed);
    if session_id.is_empty()
        || session_id.contains('/')
        || session_id.contains('\\')
        || trimmed.is_empty()
        || !path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(PlaneError::bad("invalid task folder"));
    }
    match path.file_name().and_then(|name| name.to_str()) {
        Some(name) if name == session_id => Ok(path),
        _ => Err(PlaneError::bad("invalid task folder")),
    }
}

/// The task's own directory. A shared project root is not moved; its `<session id>` child is.
fn private_task_folder(source: &Path, session_id: &str) -> Option<PathBuf> {
    if source.as_os_str().is_empty() || session_id.is_empty() {
        return None;
    }
    if source.file_name().and_then(|name| name.to_str()) == Some(session_id) {
        return Some(source.to_path_buf());
    }
    let child = source.join(session_id);
    if child.is_dir() { Some(child) } else { None }
}

/// Move this task's private folder. A shared project root stays put.
/// When the destination already holds the code-session locator, children are merged in.
fn relocate_task_folder(
    source: &Path,
    destination: &Path,
    session_id: &str,
) -> Result<(), PlaneError> {
    if session_id.is_empty() || destination.as_os_str().is_empty() {
        return Err(PlaneError::bad("invalid task folder"));
    }
    let Some(from) = private_task_folder(source, session_id) else {
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::create_dir_all(destination)?;
        return Ok(());
    };
    if paths_equal(&from, destination) {
        return Ok(());
    }
    if destination.starts_with(&from) {
        return Err(PlaneError::bad("invalid task folder"));
    }
    match std::fs::symlink_metadata(&from) {
        Ok(meta) if meta.file_type().is_symlink() || !meta.is_dir() => {
            return Err(PlaneError::bad("invalid task folder"));
        }
        Ok(_) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {
            if let Some(parent) = destination.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::create_dir_all(destination)?;
            return Ok(());
        }
        Err(error) => return Err(error.into()),
    }
    if !destination.exists() {
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::rename(&from, destination)?;
        return Ok(());
    }
    if !destination.is_dir() {
        return Err(PlaneError::bad("task folder already exists"));
    }
    for entry in std::fs::read_dir(&from)? {
        let entry = entry?;
        let target = destination.join(entry.file_name());
        if target.exists() {
            continue;
        }
        std::fs::rename(entry.path(), target)?;
    }
    let _ = std::fs::remove_dir(&from);
    Ok(())
}

/// Delete one task directory. A symlink is refused so a shared target stays put.
fn remove_private_folder(folder: &Path) -> Result<(), PlaneError> {
    match std::fs::symlink_metadata(folder) {
        Ok(meta) if meta.file_type().is_symlink() || !meta.is_dir() => {
            Err(PlaneError::bad("invalid task folder"))
        }
        Ok(_) => {
            std::fs::remove_dir_all(folder)?;
            Ok(())
        }
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn paths_equal(left: &Path, right: &Path) -> bool {
    fn trim(path: &Path) -> PathBuf {
        PathBuf::from(path.to_string_lossy().trim_end_matches(['/', '\\']))
    }
    trim(left) == trim(right)
}

const PIN_TYPES: [&str; 6] = [
    "assistant",
    "topic",
    "model",
    "agent",
    "knowledge",
    "session",
];

pub struct PinCatalog {
    store: Arc<PlaneStore>,
}

impl PinCatalog {
    pub fn new(store: Arc<PlaneStore>) -> Self {
        Self { store }
    }

    pub fn list(&self, entity_type: Option<&str>) -> Result<Value, PlaneError> {
        let entity_type = entity_type
            .filter(|value| !value.is_empty())
            .ok_or_else(|| PlaneError::bad("entityType is required"))?;
        known_pin_type(entity_type)?;
        let mut rows: Vec<Pin> = self
            .store
            .pins()
            .into_iter()
            .filter(|pin| pin.entity_type == entity_type)
            .collect();
        rows.sort_by(|left, right| {
            left.order_key
                .cmp(&right.order_key)
                .then(left.id.cmp(&right.id))
        });
        Ok(json!(rows))
    }

    /// The same entity returns the existing pin.
    pub fn create(&self, body: &Value) -> Result<Pin, PlaneError> {
        let entity_type = text(body, "entityType")
            .filter(|value| !value.is_empty())
            .ok_or_else(|| PlaneError::bad("entityType is required"))?;
        known_pin_type(&entity_type)?;
        let entity_id = text(body, "entityId")
            .filter(|value| !value.is_empty())
            .ok_or_else(|| PlaneError::bad("entityId is required"))?;
        if entity_id.len() > 512 || entity_id.chars().any(char::is_control) {
            return Err(PlaneError::bad("entityId is invalid"));
        }
        if let Some(existing) = self
            .store
            .pins()
            .into_iter()
            .find(|pin| pin.entity_type == entity_type && pin.entity_id == entity_id)
        {
            return Ok(existing);
        }
        let now = now_iso();
        let pin = Pin {
            id: new_id(),
            entity_type,
            entity_id,
            order_key: now.clone(),
            created_at: now.clone(),
            updated_at: now,
        };
        self.store.save_pin(pin.clone())?;
        Ok(pin)
    }

    pub fn get(&self, id: &str) -> Result<Pin, PlaneError> {
        self.store
            .pins()
            .into_iter()
            .find(|pin| pin.id == id)
            .ok_or_else(|| PlaneError::NotFound(format!("pin {id}")))
    }

    pub fn delete(&self, id: &str) -> Result<(), PlaneError> {
        if !self.store.remove_pin(id)? {
            return Err(PlaneError::NotFound(format!("pin {id}")));
        }
        Ok(())
    }
}

fn known_pin_type(entity_type: &str) -> Result<(), PlaneError> {
    if PIN_TYPES.contains(&entity_type) {
        Ok(())
    } else {
        Err(PlaneError::bad("entityType is not supported"))
    }
}

fn resident_label(_id: &str) -> &'static str {
    "ToBot"
}

fn refuse_resident_session(ids: &[String]) -> Result<(), PlaneError> {
    if let Some(id) = ids.iter().find(|id| is_resident_session(id)) {
        Err(PlaneError::Conflict(format!(
            "the {} session cannot be removed",
            resident_label(id)
        )))
    } else {
        Ok(())
    }
}

fn builtin_code_configuration(existing: Option<&Value>) -> Value {
    let mut config = existing
        .filter(|value| value.is_object())
        .cloned()
        .unwrap_or_else(|| json!({}));
    if let Some(object) = config.as_object_mut() {
        object.insert("builtin".into(), json!(BUILTIN_CODE_AGENT_ID));
        object.insert("assetId".into(), json!(BUILTIN_CODE_AGENT_ASSET_ID));
        object.insert("releaseId".into(), json!(BUILTIN_CODE_AGENT_RELEASE_ID));
        object.insert("providerKind".into(), json!(BUILTIN_CODE_PROVIDER_KIND));
    }
    config
}

fn matches_agent(session: &Session, agent_id: Option<&str>) -> bool {
    match agent_id {
        None => true,
        Some("unlinked") => session.agent_id.is_none(),
        Some(id) => session.agent_id.as_deref() == Some(id),
    }
}

fn page<T: serde::Serialize>(rows: Vec<T>, cursor: Option<&str>, limit: usize) -> Value {
    let start = cursor
        .and_then(|cursor| cursor.parse::<usize>().ok())
        .unwrap_or(0);
    let limit = limit.clamp(1, MAX_PAGE);
    let end = (start + limit).min(rows.len());
    let items: Vec<&T> = rows.get(start..end).unwrap_or(&[]).iter().collect();
    if end < rows.len() {
        json!({ "items": items, "nextCursor": end.to_string() })
    } else {
        json!({ "items": items })
    }
}

fn workspace_body(session: &Session) -> Value {
    if session.workspace.kind == "system" {
        json!({ "type": "system" })
    } else {
        json!({ "type": "user", "workspace": session.workspace })
    }
}

/// `hi` becomes `hi (1)`; forking `hi (1)` advances to the next free `hi (N)`.
fn fork_name(source: &str, taken: &[String]) -> String {
    let base = fork_base(source);
    let prefix = format!("{base} (");
    let mut max = 0u32;
    for name in taken {
        let Some(rest) = name.strip_prefix(&prefix) else {
            continue;
        };
        let Some(number) = rest.strip_suffix(')') else {
            continue;
        };
        if let Ok(value) = number.parse::<u32>() {
            max = max.max(value);
        }
    }
    format!("{base} ({})", max + 1)
}

fn fork_base(name: &str) -> String {
    let trimmed = name.trim();
    if let Some(open) = trimmed.rfind(" (") {
        if let Some(number) = trimmed[open + 2..].strip_suffix(')') {
            if !number.is_empty() && number.chars().all(|ch| ch.is_ascii_digit()) {
                let base = trimmed[..open].trim();
                if !base.is_empty() {
                    return base.to_owned();
                }
            }
        }
    }
    if trimmed.is_empty() {
        "Session".to_owned()
    } else {
        trimmed.to_owned()
    }
}

fn fork_message(
    source: &Message,
    session_id: &str,
    source_session_id: &str,
    boundary: bool,
) -> Message {
    let mut parts = source
        .data
        .get("parts")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    parts
        .retain(|part| part.get("type").and_then(Value::as_str) != Some("data-agent-session-fork"));
    if boundary {
        parts.push(json!({
            "type": "data-agent-session-fork",
            "data": { "sourceSessionId": source_session_id }
        }));
    }
    let mut data = source.data.clone();
    if let Some(object) = data.as_object_mut() {
        object.insert("parts".into(), Value::Array(parts));
    } else {
        data = json!({ "parts": parts });
    }
    Message {
        id: new_id(),
        session_id: session_id.to_owned(),
        role: source.role.clone(),
        data,
        status: source.status.clone(),
        model_id: source.model_id.clone(),
        message_snapshot: source.message_snapshot.clone(),
        stats: None,
        delivery: None,
        searchable_text: source.searchable_text.clone(),
        runtime_resume_token: None,
        created_at: source.created_at.clone(),
        updated_at: source.updated_at.clone(),
    }
}

fn text(body: &Value, key: &str) -> Option<String> {
    body.get(key).and_then(Value::as_str).map(str::to_owned)
}

fn strings(body: &Value, key: &str) -> Option<Vec<String>> {
    body.get(key).and_then(Value::as_array).map(|items| {
        items
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect()
    })
}

fn model_id(body: &Value, key: &str) -> Result<Option<String>, PlaneError> {
    match body.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(model)) if super::domain::split_model_id(model).is_some() => {
            Ok(Some(model.clone()))
        }
        Some(_) => Err(PlaneError::bad(format!(
            "{key} must be providerId::modelId"
        ))),
    }
}

fn model_name(model: &str) -> Option<String> {
    super::domain::split_model_id(model).map(|(_, name)| name.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalogs() -> (AgentCatalog, SessionCatalog) {
        let store = Arc::new(PlaneStore::open(None));
        (
            AgentCatalog::new(Arc::clone(&store)),
            SessionCatalog::new(store),
        )
    }

    fn workspace(path: &str) -> Value {
        json!({
            "type": "user",
            "workspace": {
                "id": "w1", "name": "proj", "path": path, "type": "user",
                "orderKey": "a0", "createdAt": "2026-01-01T00:00:00.000Z", "updatedAt": "2026-01-01T00:00:00.000Z",
            },
        })
    }

    #[test]
    fn creates_and_lists_agents() {
        let (agents, _) = catalogs();
        let agent = agents
            .create(&json!({
                "type": "a3s-code",
                "name": " Helper ",
                "model": "openai::gpt-4o",
                "configuration": { "permission_mode": "plan", "custom": 1 },
            }))
            .unwrap();
        assert_eq!(agent.name, "Helper");
        assert_eq!(agent.model_name.as_deref(), Some("gpt-4o"));
        assert_eq!(agent.permission_mode(), "plan");
        let listed = agents.list(None, &[], 1, 500);
        assert_eq!(listed["total"], 1);
        assert_eq!(listed["items"][0]["configuration"]["custom"], 1);
        assert!(listed["items"][0].get("description").is_none());

        let updated = agents
            .update(
                &agent.id,
                &json!({ "configuration": { "permission_mode": "auto" } }),
            )
            .unwrap();
        assert_eq!(updated.permission_mode(), "auto");
        assert_eq!(updated.configuration.as_ref().unwrap()["custom"], 1);
    }

    #[test]
    fn rejects_bad_agents() {
        let (agents, _) = catalogs();
        assert!(
            agents
                .create(&json!({ "type": "nope", "name": "x", "model": "a::b" }))
                .is_err()
        );
        assert!(
            agents
                .create(&json!({ "type": "pi", "name": "", "model": "a::b" }))
                .is_err()
        );
        assert!(
            agents
                .create(&json!({ "type": "pi", "name": "x", "model": "gpt" }))
                .is_err()
        );
    }

    #[test]
    fn system_session_opens_its_own_folder() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("a3s-session-folder-{stamp}"));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let store = Arc::new(PlaneStore::open(Some(root.clone())));
        let sessions = SessionCatalog::new(Arc::clone(&store));
        let session = sessions
            .create(&json!({ "agentId": "a1", "name": "pong", "workspace": { "type": "system" } }))
            .unwrap();
        let folder = root.join("workspace").join(&session.id);
        assert_eq!(PathBuf::from(&session.workspace.path), folder);
        assert!(folder.is_dir());
        assert_ne!(
            PathBuf::from(&session.workspace.path),
            root.join("workspace")
        );

        let mut shared = session.clone();
        shared.workspace.path = root.join("workspace").to_string_lossy().into_owned();
        store.save_session(shared).unwrap();
        let loaded = sessions.get(&session.id).unwrap();
        assert_eq!(PathBuf::from(&loaded.workspace.path), folder);

        let created = sessions
            .reuse_or_create(&json!({ "agentId": "a1", "workspace": { "type": "system" } }))
            .unwrap();
        assert_eq!(created["created"], true);
        let reused = sessions
            .reuse_or_create(&json!({ "agentId": "a1", "workspace": { "type": "system" } }))
            .unwrap();
        assert_eq!(reused["created"], false);
        assert_eq!(reused["session"]["id"], created["session"]["id"]);
        assert!(
            reused["session"]["workspace"]["path"]
                .as_str()
                .unwrap()
                .ends_with(created["session"]["id"].as_str().unwrap())
        );

        let project = sessions
            .create(&json!({
                "agentId": "a1",
                "name": "proj",
                "workspace": workspace("/tmp/session-project")
            }))
            .unwrap();
        assert_eq!(project.workspace.path, "/tmp/session-project");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn project_placement_moves_the_task_folder_and_inbox_moves_it_back() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("a3s-task-folder-{stamp}"));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let store = Arc::new(PlaneStore::open(Some(root.clone())));
        let sessions = SessionCatalog::new(Arc::clone(&store));
        let session = sessions
            .create(&json!({ "agentId": "a1", "name": "pong", "workspace": { "type": "system" } }))
            .unwrap();
        let from = root.join("workspace").join(&session.id);
        std::fs::write(from.join("root.txt"), "hi").unwrap();
        let project = root.join("projects").join("a3s");
        std::fs::create_dir_all(&project).unwrap();
        let moved = sessions
            .set_workspace(
                &session.id,
                &json!({
                    "type": "user",
                    "place": "project",
                    "workspace": {
                        "id": "w1",
                        "name": "a3s",
                        "path": project,
                        "type": "user",
                        "orderKey": "a0",
                        "createdAt": "2026-01-01T00:00:00.000Z",
                        "updatedAt": "2026-01-01T00:00:00.000Z"
                    }
                }),
            )
            .unwrap();
        let dest = project.join(&session.id);
        assert_eq!(PathBuf::from(&moved.workspace.path), dest);
        assert_eq!(moved.workspace.kind, "user");
        assert_eq!(moved.workspace_id, "w1");
        assert_eq!(
            std::fs::read_to_string(dest.join("root.txt")).unwrap(),
            "hi"
        );
        assert!(!from.exists());

        let inbox = root.join("sessions").join("2026-10-04").join(&session.id);
        let back = sessions
            .set_workspace(
                &session.id,
                &json!({
                    "type": "system",
                    "place": "inbox",
                    "path": inbox,
                }),
            )
            .unwrap();
        assert_eq!(PathBuf::from(&back.workspace.path), inbox);
        assert_eq!(back.workspace.kind, "system");
        assert_eq!(back.workspace_id, "system");
        assert_eq!(
            std::fs::read_to_string(inbox.join("root.txt")).unwrap(),
            "hi"
        );
        assert!(!dest.exists());

        let plain = sessions
            .set_workspace(&back.id, &workspace(project.to_str().unwrap()))
            .unwrap();
        assert_eq!(plain.workspace.path, project.to_string_lossy());
        assert!(inbox.join("root.txt").is_file());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn reuses_an_empty_session_for_the_same_folder() {
        let (_, sessions) = catalogs();
        let first = sessions
            .reuse_or_create(&json!({ "agentId": "a1", "workspace": workspace("/tmp/p") }))
            .unwrap();
        assert_eq!(first["created"], true);
        let again = sessions
            .reuse_or_create(&json!({ "agentId": "a1", "workspace": workspace("/tmp/p") }))
            .unwrap();
        assert_eq!(again["created"], false);
        assert_eq!(again["session"]["id"], first["session"]["id"]);
        let other = sessions
            .reuse_or_create(&json!({ "agentId": "a1", "workspace": workspace("/tmp/q") }))
            .unwrap();
        assert_eq!(other["created"], true);
    }

    #[test]
    fn archives_a_session_and_hides_it_from_the_list() {
        let (_, sessions) = catalogs();
        let session = sessions
            .create(&json!({ "agentId": "a1", "name": "能1", "workspace": { "type": "system" } }))
            .unwrap();
        let archived = sessions
            .archive(&json!({ "sessionIds": [session.id] }))
            .unwrap();
        assert_eq!(archived["deletedIds"][0], session.id);
        assert!(
            sessions.list(&SessionQuery::default(), None, 20).unwrap()["items"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert!(sessions.get(&session.id).is_err());
        let again = sessions
            .archive(&json!({ "sessionIds": [session.id, "missing"] }))
            .unwrap();
        assert!(again["deletedIds"].as_array().unwrap().is_empty());
    }

    #[test]
    fn keeps_the_builtin_tobot_session_out_of_the_task_list() {
        let root = std::env::temp_dir().join(format!("a3s-tobot-session-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let store = Arc::new(PlaneStore::open(Some(root.join("state"))));
        let agents = AgentCatalog::new(Arc::clone(&store));
        let sessions = SessionCatalog::new(Arc::clone(&store));
        let agent = agents
            .ensure_builtin_code_agent(Some("a3s::deepseek-v4-flash"))
            .unwrap();
        assert_eq!(agent.id, BUILTIN_CODE_AGENT_ID);
        assert_eq!(agent.kind, "a3s-code");
        assert_eq!(
            agent.configuration.as_ref().unwrap()["assetId"],
            BUILTIN_CODE_AGENT_ASSET_ID
        );
        assert_eq!(
            agent.configuration.as_ref().unwrap()["releaseId"],
            BUILTIN_CODE_AGENT_RELEASE_ID
        );
        assert_eq!(agents.list(None, &[], 1, 50)["total"], 0);
        let ids = vec![BUILTIN_CODE_AGENT_ID.to_owned()];
        assert_eq!(agents.list(None, &ids, 1, 50)["total"], 1);
        let again = agents
            .ensure_builtin_code_agent(Some("a3s::deepseek-v4-flash"))
            .unwrap();
        assert_eq!(again.model.as_deref(), Some("a3s::deepseek-v4-flash"));

        let session = sessions.ensure_builtin_tobot().unwrap();
        assert_eq!(session.id, BUILTIN_TOBOT_SESSION_ID);
        assert_eq!(session.agent_id.as_deref(), Some(BUILTIN_CODE_AGENT_ID));
        assert_eq!(session.name, "ToBot");
        let folder = root
            .join("state")
            .join("workspace")
            .join(BUILTIN_TOBOT_SESSION_ID);
        assert_eq!(PathBuf::from(&session.workspace.path), folder);
        assert!(folder.is_dir());
        assert!(
            sessions.list(&SessionQuery::default(), None, 20).unwrap()["items"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert!(sessions.latest(None)["session"].is_null());
        let listed = vec![BUILTIN_TOBOT_SESSION_ID.to_owned()];
        assert_eq!(
            sessions
                .list(
                    &SessionQuery {
                        ids: listed.clone(),
                        ..SessionQuery::default()
                    },
                    None,
                    20
                )
                .unwrap()["items"][0]["id"],
            BUILTIN_TOBOT_SESSION_ID
        );
        let kept = sessions.ensure_builtin_tobot().unwrap();
        assert_eq!(kept.id, session.id);

        let other = sessions
            .create(&json!({
                "agentId": agent.id,
                "name": "notes",
                "workspace": { "type": "system" }
            }))
            .unwrap();
        let blocked = sessions.archive(&json!({
            "sessionIds": [other.id, BUILTIN_TOBOT_SESSION_ID]
        }));
        assert!(matches!(blocked, Err(PlaneError::Conflict(_))));
        assert!(sessions.get(BUILTIN_TOBOT_SESSION_ID).is_ok());
        assert!(sessions.get(&other.id).is_ok());
        let removed = sessions.destroy(&json!({ "sessionIds": [BUILTIN_TOBOT_SESSION_ID] }));
        assert!(matches!(removed, Err(PlaneError::Conflict(_))));
        assert!(folder.is_dir());
        assert!(
            sessions
                .set_workspace(BUILTIN_TOBOT_SESSION_ID, &json!({ "type": "system" }))
                .is_err()
        );
        assert!(
            sessions
                .update(
                    BUILTIN_TOBOT_SESSION_ID,
                    &json!({ "agentId": "other-agent" })
                )
                .is_err()
        );
        let boss = sessions
            .ensure_toboss_session(BUILTIN_TOBOSS_SESSION_ID)
            .unwrap();
        assert_eq!(boss.name, "ToBot");
        assert!(
            sessions.list(&SessionQuery::default(), None, 20).unwrap()["items"]
                .as_array()
                .unwrap()
                .iter()
                .all(|row| row["id"] != BUILTIN_TOBOSS_SESSION_ID)
        );
        assert!(
            sessions
                .destroy(&json!({ "sessionIds": [BUILTIN_TOBOSS_SESSION_ID] }))
                .is_err()
        );
        assert!(
            sessions
                .set_workspace(BUILTIN_TOBOSS_SESSION_ID, &json!({ "type": "system" }))
                .is_err()
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn member_tobot_sessions_keep_separate_transcripts() {
        let root = std::env::temp_dir().join(format!(
            "a3s-tobot-members-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let store = Arc::new(PlaneStore::open(Some(root.join("state"))));
        let sessions = SessionCatalog::new(Arc::clone(&store));
        let legacy = sessions.ensure_builtin_tobot().unwrap();
        store
            .save_message(Message {
                id: "legacy".into(),
                session_id: legacy.id.clone(),
                role: "assistant".into(),
                searchable_text: "legacy-secret".into(),
                data: json!({ "parts": [{ "type": "text", "text": "legacy-secret" }] }),
                status: "success".into(),
                model_id: None,
                message_snapshot: None,
                stats: None,
                delivery: None,
                runtime_resume_token: None,
                created_at: "2026-10-05T00:00:00.000Z".into(),
                updated_at: "2026-10-05T00:00:00.000Z".into(),
            })
            .unwrap();
        let member_a = "11111111-1111-4111-8111-111111111111";
        let member_b = "22222222-2222-4222-8222-222222222222";
        let session_a = sessions
            .ensure_tobot_session(&format!("tobot-{member_a}"))
            .unwrap();
        let session_b = sessions
            .ensure_tobot_session(&format!("tobot-{member_b}"))
            .unwrap();
        assert_ne!(session_a.workspace.path, session_b.workspace.path);
        assert_ne!(session_a.workspace.path, legacy.workspace.path);
        store
            .save_message(Message {
                id: "a".into(),
                session_id: session_a.id.clone(),
                role: "user".into(),
                searchable_text: "secret-a".into(),
                data: json!({ "parts": [{ "type": "text", "text": "secret-a" }] }),
                status: "success".into(),
                model_id: None,
                message_snapshot: None,
                stats: None,
                delivery: None,
                runtime_resume_token: None,
                created_at: "2026-10-05T00:00:01.000Z".into(),
                updated_at: "2026-10-05T00:00:01.000Z".into(),
            })
            .unwrap();
        store
            .save_message(Message {
                id: "b".into(),
                session_id: session_b.id.clone(),
                role: "assistant".into(),
                searchable_text: "secret-b".into(),
                data: json!({ "parts": [{ "type": "text", "text": "secret-b" }] }),
                status: "success".into(),
                model_id: None,
                message_snapshot: None,
                stats: None,
                delivery: None,
                runtime_resume_token: None,
                created_at: "2026-10-05T00:00:02.000Z".into(),
                updated_at: "2026-10-05T00:00:02.000Z".into(),
            })
            .unwrap();
        let page_a = sessions.messages(&session_a.id, None, 50).unwrap();
        let page_b = sessions.messages(&session_b.id, None, 50).unwrap();
        let text_a = page_a.to_string();
        let text_b = page_b.to_string();
        assert!(text_a.contains("secret-a"));
        assert!(!text_a.contains("secret-b"));
        assert!(!text_a.contains("legacy-secret"));
        assert!(text_b.contains("secret-b"));
        assert!(!text_b.contains("secret-a"));
        assert!(!text_b.contains("legacy-secret"));
        assert!(
            sessions.list(&SessionQuery::default(), None, 20).unwrap()["items"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert!(
            sessions
                .destroy(&json!({ "sessionIds": [session_a.id] }))
                .is_err()
        );
        assert!(
            store
                .messages(&legacy.id)
                .iter()
                .any(|message| { message.searchable_text.contains("legacy-secret") })
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn destroys_a_task_so_it_cannot_be_restored() {
        let root = std::env::temp_dir().join(format!("a3s-destroy-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let store = Arc::new(PlaneStore::open(Some(root.join("state"))));
        let sessions = SessionCatalog::new(Arc::clone(&store));
        let session = sessions
            .create(&json!({ "agentId": "a1", "name": "gone", "workspace": { "type": "system" } }))
            .unwrap();
        let folder = PathBuf::from(&session.workspace.path);
        std::fs::write(folder.join("note.txt"), "x").unwrap();
        store
            .save_message(Message {
                id: "m1".into(),
                session_id: session.id.clone(),
                role: "user".into(),
                data: json!({ "parts": [] }),
                status: "success".into(),
                model_id: None,
                message_snapshot: None,
                stats: None,
                delivery: None,
                searchable_text: String::new(),
                runtime_resume_token: None,
                created_at: now_iso(),
                updated_at: now_iso(),
            })
            .unwrap();

        let deleted = sessions
            .destroy(&json!({ "sessionIds": [session.id] }))
            .unwrap();
        assert_eq!(deleted["deletedIds"][0], session.id);
        assert!(store.session(&session.id).is_none());
        assert!(!folder.exists());
        assert!(
            !root
                .join("state/messages")
                .join(format!("{}.json", session.id))
                .exists()
        );
        assert!(
            sessions.list(&SessionQuery::default(), None, 20).unwrap()["items"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        let again = sessions
            .destroy(&json!({ "sessionIds": [session.id] }))
            .unwrap();
        assert!(again["deletedIds"].as_array().unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn destroy_leaves_a_shared_parent_directory() {
        let root = std::env::temp_dir().join(format!("a3s-destroy-shared-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let parent = root.join("shared");
        std::fs::create_dir_all(&parent).unwrap();
        std::fs::write(parent.join("keep.txt"), "stay").unwrap();
        let store = Arc::new(PlaneStore::open(Some(root.join("state"))));
        let sessions = SessionCatalog::new(Arc::clone(&store));
        let session = sessions
            .create(&json!({
                "agentId": "a1",
                "name": "shared",
                "workspace": workspace(parent.to_str().unwrap()),
            }))
            .unwrap();
        let child = parent.join(&session.id);
        std::fs::create_dir_all(&child).unwrap();
        std::fs::write(child.join("own.txt"), "own").unwrap();

        sessions
            .destroy(&json!({ "sessionIds": [session.id] }))
            .unwrap();
        assert!(parent.join("keep.txt").is_file());
        assert!(!child.exists());
        assert!(store.session(&session.id).is_none());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn destroy_refuses_a_symlinked_task_folder() {
        let root = std::env::temp_dir().join(format!("a3s-destroy-link-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let store = Arc::new(PlaneStore::open(Some(root.join("state"))));
        let sessions = SessionCatalog::new(Arc::clone(&store));
        let mut session = sessions
            .create(&json!({ "agentId": "a1", "name": "link", "workspace": { "type": "system" } }))
            .unwrap();
        let target = root.join("target");
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(target.join("note.txt"), "keep").unwrap();
        let link = root.join(&session.id);
        std::os::unix::fs::symlink(&target, &link).unwrap();
        session.workspace.path = link.to_string_lossy().into_owned();
        store.save_session(session.clone()).unwrap();

        assert!(
            sessions
                .destroy(&json!({ "sessionIds": [session.id] }))
                .is_err()
        );
        assert!(store.session(&session.id).is_some());
        assert!(target.join("note.txt").is_file());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn pages_messages_newest_first() {
        let store = Arc::new(PlaneStore::open(None));
        let sessions = SessionCatalog::new(Arc::clone(&store));
        let session = sessions
            .create(&json!({ "agentId": "a1", "name": "", "workspace": { "type": "system" } }))
            .unwrap();
        for index in 0..3 {
            store
                .save_message(Message {
                    id: format!("m{index}"),
                    session_id: session.id.clone(),
                    role: "user".into(),
                    data: json!({ "parts": [] }),
                    status: "success".into(),
                    model_id: None,
                    message_snapshot: None,
                    stats: None,
                    delivery: None,
                    searchable_text: String::new(),
                    runtime_resume_token: None,
                    created_at: now_iso(),
                    updated_at: now_iso(),
                })
                .unwrap();
        }
        let first = sessions.messages(&session.id, None, 2).unwrap();
        assert_eq!(first["items"][0]["id"], "m2");
        assert_eq!(first["nextCursor"], "2");
        let second = sessions.messages(&session.id, Some("2"), 2).unwrap();
        assert_eq!(second["items"][0]["id"], "m0");
        assert!(second.get("nextCursor").is_none());
    }

    #[test]
    fn pins_a_session_and_keeps_it_after_reopen() {
        let root = std::env::temp_dir().join(format!("a3s-pins-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let path = root.join("state");
        let store = Arc::new(PlaneStore::open(Some(path.clone())));
        let pins = PinCatalog::new(Arc::clone(&store));
        let sessions = SessionCatalog::new(Arc::clone(&store));
        assert!(pins.list(None).is_err());
        assert!(
            pins.create(&json!({ "entityType": "nope", "entityId": "s1" }))
                .is_err()
        );
        let created = pins
            .create(&json!({ "entityType": "session", "entityId": "s1" }))
            .unwrap();
        let again = pins
            .create(&json!({ "entityType": "session", "entityId": "s1" }))
            .unwrap();
        assert_eq!(again.id, created.id);
        let listed = pins.list(Some("session")).unwrap();
        assert_eq!(listed.as_array().unwrap().len(), 1);
        assert_eq!(listed[0]["entityId"], "s1");
        assert!(
            pins.list(Some("agent"))
                .unwrap()
                .as_array()
                .unwrap()
                .is_empty()
        );

        let session = sessions
            .create(
                &json!({ "agentId": "a1", "name": "pinned", "workspace": { "type": "system" } }),
            )
            .unwrap();
        let session_pin = pins
            .create(&json!({ "entityType": "session", "entityId": session.id }))
            .unwrap();
        sessions
            .destroy(&json!({ "sessionIds": [session.id] }))
            .unwrap();
        assert!(pins.get(&session_pin.id).is_err());

        drop(pins);
        drop(sessions);
        drop(store);
        let store = Arc::new(PlaneStore::open(Some(path)));
        let pins = PinCatalog::new(store);
        assert_eq!(pins.list(Some("session")).unwrap()[0]["id"], created.id);
        pins.delete(&created.id).unwrap();
        assert!(
            pins.list(Some("session"))
                .unwrap()
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert!(pins.delete(&created.id).is_err());
        let _ = std::fs::remove_dir_all(&root);
    }

    fn stored_message(session_id: &str, role: &str, status: &str, text: &str) -> Message {
        Message {
            id: new_id(),
            session_id: session_id.to_owned(),
            role: role.to_owned(),
            data: json!({ "parts": [{ "type": "text", "text": text }] }),
            status: status.to_owned(),
            model_id: None,
            message_snapshot: None,
            stats: Some(json!({ "tokens": 3 })),
            delivery: Some(json!({ "queued": true })),
            searchable_text: text.to_owned(),
            runtime_resume_token: Some("resume".into()),
            created_at: "2026-10-05T00:00:00.000Z".into(),
            updated_at: "2026-10-05T00:00:00.000Z".into(),
        }
    }

    #[test]
    fn forks_history_through_a_completed_assistant_turn() {
        let store = Arc::new(PlaneStore::open(None));
        let sessions = SessionCatalog::new(Arc::clone(&store));
        let source = sessions
            .create(&json!({
                "agentId": "agent-1",
                "name": "hi",
                "workspace": workspace("/tmp/fork-project")
            }))
            .unwrap();
        let user = stored_message(&source.id, "user", "success", "hello");
        let assistant = stored_message(&source.id, "assistant", "success", "reply");
        let later = stored_message(&source.id, "user", "success", "after");
        store.save_message(user.clone()).unwrap();
        store.save_message(assistant.clone()).unwrap();
        store.save_message(later).unwrap();

        let pending = stored_message(&source.id, "assistant", "pending", "");
        store.save_message(pending.clone()).unwrap();
        assert!(sessions.fork(&source.id, &pending.id).is_err());
        store.remove_messages(&source.id).unwrap();
        store.save_message(user.clone()).unwrap();
        store.save_message(assistant.clone()).unwrap();
        store
            .save_message(stored_message(&source.id, "user", "success", "after"))
            .unwrap();

        let forked = sessions.fork(&source.id, &assistant.id).unwrap();
        let child_id = forked["sessionId"].as_str().unwrap();
        let child = sessions.get(child_id).unwrap();
        assert_eq!(child.name, "hi (1)");
        assert_eq!(child.agent_id.as_deref(), Some("agent-1"));
        assert_eq!(child.workspace.path, "/tmp/fork-project");
        let copied = store.messages(child_id);
        assert_eq!(copied.len(), 2);
        assert_ne!(copied[0].id, user.id);
        assert_ne!(copied[1].id, assistant.id);
        assert!(copied[1].stats.is_none());
        assert!(copied[1].runtime_resume_token.is_none());
        assert_eq!(
            copied[1].data["parts"][1]["data"]["sourceSessionId"],
            source.id
        );
        assert_eq!(store.messages(&source.id).len(), 3);

        let again = sessions.fork(&source.id, &assistant.id).unwrap();
        assert_eq!(
            sessions
                .get(again["sessionId"].as_str().unwrap())
                .unwrap()
                .name,
            "hi (2)"
        );
    }
}
