//! The closed set of host extension methods the agent answers beyond ACP.
//!
//! `ext_method` parses the wire method into a [`HostMethod`]; a method
//! outside the set is `method_not_found`. Goal, review, wiki, compaction,
//! interjection, session catalog, and permission posture are host features,
//! not ACP protocol.

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum HostMethod {
    GoalImport,
    GoalImportPreview,
    GoalCommand,
    Review,
    WikiSearch,
    MemoryManage,
    CompactConversation,
    Interject,
    SessionInfo,
    SessionList,
    SessionDelete,
    PermissionPosture,
    SubagentCancel,
}

impl HostMethod {
    #[cfg(test)]
    pub(super) const ALL: [Self; 13] = [
        Self::GoalImport,
        Self::GoalImportPreview,
        Self::GoalCommand,
        Self::Review,
        Self::WikiSearch,
        Self::MemoryManage,
        Self::CompactConversation,
        Self::Interject,
        Self::SessionInfo,
        Self::SessionList,
        Self::SessionDelete,
        Self::PermissionPosture,
        Self::SubagentCancel,
    ];

    /// The wire method, or `None` outside the closed set. Compaction and
    /// interjection keep their `a3s/` prefix families.
    pub(super) fn parse(method: &str) -> Option<Self> {
        Some(match method {
            "a3s/goal/import" => Self::GoalImport,
            "a3s/goal/import/preview" => Self::GoalImportPreview,
            "a3s/goal/definition"
            | "a3s/goal/plan"
            | "a3s/goal/control"
            | "a3s/goal/wait"
            | "a3s/goal/state" => Self::GoalCommand,
            "a3s/review/target" | "a3s/review/admit" | "a3s/review/settle" => Self::Review,
            "a3s/wiki/search" => Self::WikiSearch,
            "a3s/memory/manage" => Self::MemoryManage,
            "a3s/session/info" => Self::SessionInfo,
            "a3s/session/list" => Self::SessionList,
            "a3s/session/delete" => Self::SessionDelete,
            "a3s/permission_posture" => Self::PermissionPosture,
            "a3s/subagent/cancel" => Self::SubagentCancel,
            _ if method.starts_with("a3s/compact_conversation") => Self::CompactConversation,
            _ if method.starts_with("a3s/interject") => Self::Interject,
            _ => return None,
        })
    }

    /// The method name, or the family prefix for a family of methods.
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::GoalImport => "a3s/goal/import",
            Self::GoalImportPreview => "a3s/goal/import/preview",
            Self::GoalCommand => "a3s/goal/",
            Self::Review => "a3s/review/",
            Self::WikiSearch => "a3s/wiki/search",
            Self::MemoryManage => "a3s/memory/manage",
            Self::CompactConversation => "a3s/compact_conversation",
            Self::Interject => "a3s/interject",
            Self::SessionInfo => "a3s/session/info",
            Self::SessionList => "a3s/session/list",
            Self::SessionDelete => "a3s/session/delete",
            Self::PermissionPosture => "a3s/permission_posture",
            Self::SubagentCancel => "a3s/subagent/cancel",
        }
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SubagentCancelParams {
    session_id: String,
    subagent_id: String,
}

fn require_session_id(value: &str) -> acp::Result<()> {
    if value.trim().is_empty() || value.len() > 4096 {
        return Err(acp::Error::invalid_params().data("invalid session identity"));
    }
    Ok(())
}

fn ext_json(value: serde_json::Value) -> acp::Result<acp::ExtResponse> {
    let raw = serde_json::value::to_raw_value(&value)
        .map_err(|error| acp::Error::internal_error().data(error.to_string()))?;
    Ok(acp::ExtResponse::new(raw.into()))
}

impl A3sCodeAgent {
    async fn handle_subagent_cancel(
        &self,
        args: &acp::ExtRequest,
    ) -> acp::Result<acp::ExtResponse> {
        use a3s_code_core::subagent_task_tracker::SubagentStatus;
        let request: SubagentCancelParams = serde_json::from_str(args.params.get())
            .map_err(|error| acp::Error::invalid_params().data(error.to_string()))?;
        require_session_id(&request.session_id)?;
        require_session_id(&request.subagent_id)?;
        let session = {
            let sessions = self.sessions.lock().await;
            let live = sessions
                .get(&request.session_id)
                .ok_or_else(|| acp::Error::invalid_params().data("unknown session id"))?;
            Arc::clone(&live.session)
        };
        let cancelled = session.cancel_subagent_task(&request.subagent_id).await;
        let outcome = if cancelled {
            serde_json::json!({"kind":"cancelled"})
        } else {
            match session.subagent_task(&request.subagent_id).await {
                Some(task) => {
                    let status = match task.status {
                        SubagentStatus::Completed => "completed",
                        SubagentStatus::Failed => "failed",
                        SubagentStatus::Cancelled => "cancelled",
                        _ => return Err(acp::Error::internal_error()
                            .data("subagent cancellation handle is unavailable; task may still be running")),
                    };
                    serde_json::json!({"kind":"already_finished","status":status})
                }
                None => serde_json::json!({"kind":"not_found"}),
            }
        };
        ext_json(serde_json::json!({"result": {
            "subagentId":request.subagent_id, "cancelled":cancelled, "outcome":outcome
        }, "error":null}))
    }

    pub(super) async fn answer_host_method(
        &self,
        method: HostMethod,
        args: &acp::ExtRequest,
    ) -> acp::Result<acp::ExtResponse> {
        tracing::debug!(method = method.as_str(), "host extension method");
        match method {
            HostMethod::GoalImport => self.import_goal(args).await,
            HostMethod::GoalImportPreview => self.preview_goal_import(args).await,
            HostMethod::GoalCommand => self.handle_goal_command(args).await,
            HostMethod::Review => self.handle_review(args).await,
            HostMethod::WikiSearch => answer_wiki_search(args).await,
            HostMethod::MemoryManage => self.handle_memory_management(args).await,
            HostMethod::CompactConversation => self.handle_compact_extension(args).await,
            HostMethod::Interject => self.handle_interject(args).await,
            HostMethod::SessionInfo => self.handle_session_info(args).await,
            HostMethod::SessionList => self.handle_session_list(args).await,
            HostMethod::SessionDelete => self.handle_session_delete(args).await,
            HostMethod::PermissionPosture => self.handle_permission_posture(args).await,
            HostMethod::SubagentCancel => self.handle_subagent_cancel(args).await,
        }
    }

    async fn import_goal(&self, args: &acp::ExtRequest) -> acp::Result<acp::ExtResponse> {
        let command: a3s_code_core::goal_control::GoalDefinitionCommandV1 =
            serde_json::from_str(args.params.get())
                .map_err(|error| acp::Error::invalid_params().data(error.to_string()))?;
        let session_id = command.session_id.clone();
        let (session, turn_queue) = self
            .sessions
            .lock()
            .await
            .get(&session_id)
            .map(|live| (Arc::clone(&live.session), Arc::clone(&live.turn_queue)))
            .ok_or_else(|| acp::Error::resource_not_found(None))?;
        // A retiring legacy turn can still remove its source after Core returns.
        let _turn_permit = turn_queue
            .acquire_owned()
            .await
            .ok_or_else(|| acp::Error::invalid_params().data("goal import queue is full"))?;
        let imported = crate::goal::commit_import(&session, command)
            .await
            .map_err(|error| acp::Error::invalid_params().data(error.to_string()))?;
        self.goals.lock().await.remove(&session_id);
        let raw = serde_json::value::to_raw_value(&imported)
            .map_err(|error| acp::Error::internal_error().data(error.to_string()))?;
        Ok(acp::ExtResponse::new(raw.into()))
    }

    async fn preview_goal_import(&self, args: &acp::ExtRequest) -> acp::Result<acp::ExtResponse> {
        let request: crate::goal::GoalImportPreviewRequest =
            serde_json::from_str(args.params.get())
                .map_err(|error| acp::Error::invalid_params().data(error.to_string()))?;
        let session = self
            .sessions
            .lock()
            .await
            .get(&request.session_id)
            .map(|live| Arc::clone(&live.session))
            .ok_or_else(|| acp::Error::resource_not_found(None))?;
        if session.is_closed() {
            return Err(acp::Error::resource_not_found(None));
        }
        if session
            .goal_definition()
            .map_err(|error| acp::Error::invalid_params().data(error.to_string()))?
            .is_some()
        {
            return Err(acp::Error::invalid_params().data("goal.conflict: session already bound"));
        }
        let preview = crate::goal::preview_import(session.workspace(), &request)
            .await
            .map_err(|error| acp::Error::invalid_params().data(error.to_string()))?;
        let raw = serde_json::value::to_raw_value(&preview)
            .map_err(|error| acp::Error::internal_error().data(error.to_string()))?;
        Ok(acp::ExtResponse::new(raw.into()))
    }
}

#[cfg(test)]
#[path = "host_methods_tests.rs"]
mod tests;
