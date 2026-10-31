//! The closed set of host extension methods the agent answers beyond ACP.
//!
//! `ext_method` parses the wire method into a [`HostMethod`]; a method
//! outside the set is `method_not_found`. Goal, review, wiki, compaction,
//! interjection, session catalog, and permission posture are host features,
//! not ACP protocol.

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum HostMethod {
    ModelsList,
    FollowUp,
    GoalImport,
    GoalImportPreview,
    GoalCommand,
    Review,
    WikiSearch,
    WorkspaceSearch,
    MemoryManage,
    MemorySnapshot,
    CompactConversation,
    Interject,
    SessionInfo,
    SessionList,
    SessionDelete,
    PermissionPosture,
    SessionTranscriptPage,
    SessionPrefixFork,
    SessionTooling,
    SessionBackgroundTasks,
    SessionBackgroundTaskStop,
    SubagentCancel,
}

impl HostMethod {
    #[cfg(test)]
    pub(super) const ALL: [Self; 22] = [
        Self::ModelsList,
        Self::FollowUp,
        Self::GoalImport,
        Self::GoalImportPreview,
        Self::GoalCommand,
        Self::Review,
        Self::WikiSearch,
        Self::WorkspaceSearch,
        Self::MemoryManage,
        Self::MemorySnapshot,
        Self::CompactConversation,
        Self::Interject,
        Self::SessionInfo,
        Self::SessionList,
        Self::SessionDelete,
        Self::PermissionPosture,
        Self::SessionTranscriptPage,
        Self::SessionPrefixFork,
        Self::SessionTooling,
        Self::SessionBackgroundTasks,
        Self::SessionBackgroundTaskStop,
        Self::SubagentCancel,
    ];

    /// The wire method, or `None` outside the closed set. Compaction and
    /// interjection keep their `aster/` prefix families.
    pub(super) fn parse(method: &str) -> Option<Self> {
        Some(match method {
            "aster/models/list" => Self::ModelsList,
            "aster/followup/generate" => Self::FollowUp,
            "aster/goal/import" => Self::GoalImport,
            "aster/goal/import/preview" => Self::GoalImportPreview,
            "aster/goal/definition"
            | "aster/goal/plan"
            | "aster/goal/control"
            | "aster/goal/wait"
            | "aster/goal/state" => Self::GoalCommand,
            "aster/review/target" | "aster/review/admit" | "aster/review/settle" => Self::Review,
            "aster/wiki/search" => Self::WikiSearch,
            "aster/workspace/search/v1" => Self::WorkspaceSearch,
            "aster/memory/manage" => Self::MemoryManage,
            "aster/memory/snapshot" => Self::MemorySnapshot,
            "aster/session/info" => Self::SessionInfo,
            "aster/session/list" => Self::SessionList,
            "aster/session/delete" => Self::SessionDelete,
            "aster/permission_posture" => Self::PermissionPosture,
            "aster/session/transcript/page/v1" => Self::SessionTranscriptPage,
            "aster/session/fork/prefix/v1" => Self::SessionPrefixFork,
            "aster/session/tooling/v1" => Self::SessionTooling,
            "aster/session/background-tasks/v1" => Self::SessionBackgroundTasks,
            "aster/session/background-tasks/stop/v1" => Self::SessionBackgroundTaskStop,
            "aster/subagent/cancel" => Self::SubagentCancel,
            _ if method.starts_with("aster/compact_conversation") => Self::CompactConversation,
            _ if method.starts_with("aster/interject") => Self::Interject,
            _ => return None,
        })
    }

    /// The method name, or the family prefix for a family of methods.
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::ModelsList => "aster/models/list",
            Self::FollowUp => "aster/followup/generate",
            Self::GoalImport => "aster/goal/import",
            Self::GoalImportPreview => "aster/goal/import/preview",
            Self::GoalCommand => "aster/goal/",
            Self::Review => "aster/review/",
            Self::WikiSearch => "aster/wiki/search",
            Self::WorkspaceSearch => "aster/workspace/search/v1",
            Self::MemoryManage => "aster/memory/manage",
            Self::MemorySnapshot => "aster/memory/snapshot",
            Self::CompactConversation => "aster/compact_conversation",
            Self::Interject => "aster/interject",
            Self::SessionInfo => "aster/session/info",
            Self::SessionList => "aster/session/list",
            Self::SessionDelete => "aster/session/delete",
            Self::PermissionPosture => "aster/permission_posture",
            Self::SessionTranscriptPage => "aster/session/transcript/page/v1",
            Self::SessionPrefixFork => "aster/session/fork/prefix/v1",
            Self::SessionTooling => "aster/session/tooling/v1",
            Self::SessionBackgroundTasks => "aster/session/background-tasks/v1",
            Self::SessionBackgroundTaskStop => "aster/session/background-tasks/stop/v1",
            Self::SubagentCancel => "aster/subagent/cancel",
        }
    }
}

const TRANSCRIPT_PAGE_SCHEMA: &str = "aster.acp.session-transcript-page.v1";
const PREFIX_FORK_SCHEMA: &str = "aster.acp.session-prefix-fork.v1";
const TOOLING_SCHEMA: &str = "aster.acp.session-tooling.v1";
const TOOLING_UPDATE_METHOD: &str = "aster/session/tooling/update";
const WORKSPACE_SEARCH_SCHEMA: &str = "aster.acp.workspace-search.v1";
const BACKGROUND_TASKS_SCHEMA: &str = "aster.acp.session-background-tasks.v1";

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WorkspaceSearchParams {
    schema: String,
    session_id: String,
    operation_id: aster_core::OperationId,
    cwd: PathBuf,
    query: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TranscriptPageParams {
    schema: String,
    session_id: String,
    operation_id: aster_core::OperationId,
    cwd: PathBuf,
    cursor: Option<aster_core::store::TranscriptCursor>,
    limit: Option<usize>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PrefixForkParams {
    schema: String,
    session_id: String,
    operation_id: aster_core::OperationId,
    request: aster_core::store::PrefixForkRequest,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ToolingParams {
    schema: String,
    session_id: String,
    operation_id: aster_core::OperationId,
    cwd: PathBuf,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BackgroundTasksParams {
    schema: String,
    session_id: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BackgroundTaskStopParams {
    schema: String,
    session_id: String,
    task_id: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SubagentCancelParams {
    session_id: String,
    subagent_id: String,
}

fn require_schema(actual: &str, expected: &str) -> acp::Result<()> {
    if actual != expected {
        return Err(acp::Error::invalid_params().data(format!(
            "unsupported wire schema {actual:?}; expected {expected}"
        )));
    }
    Ok(())
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

pub(super) async fn session_tooling_projection(session: &AgentSession) -> serde_json::Value {
    let stamp = session.capability_catalog_stamp();
    let mut capabilities = session.projected_capability_names();
    capabilities.sort();
    capabilities.dedup();
    let capabilities = capabilities
        .into_iter()
        .map(|name| {
            serde_json::json!({
                "name": name,
                "available": true,
                "unavailableReason": null,
            })
        })
        .collect::<Vec<_>>();
    let mut unavailable = session
        .mcp_status()
        .await
        .into_values()
        .filter(|status| !status.enabled || !status.connected)
        .map(|status| {
            let reason = status.error.unwrap_or_else(|| {
                if !status.enabled {
                    "MCP server is disabled".to_string()
                } else {
                    "MCP server is not connected".to_string()
                }
            });
            serde_json::json!({
                "name": status.name,
                "available": false,
                "unavailableReason": reason,
            })
        })
        .collect::<Vec<_>>();
    unavailable.sort_by(|left, right| left["name"].as_str().cmp(&right["name"].as_str()));
    serde_json::json!({
        "schema": TOOLING_SCHEMA,
        "sessionId": session.session_id(),
        "tools": session.tool_definitions(),
        "capabilityStamp": {
            "generation": stamp.generation().get(),
            "digest": stamp.digest().as_str(),
        },
        "capabilities": capabilities,
        "unavailable": unavailable,
    })
}

fn memory_task_snapshot(task: aster_core::memory::MemoryExtractionTask) -> serde_json::Value {
    use aster_core::memory::MemoryExtractionStatus;
    let status = match task.status {
        MemoryExtractionStatus::Pending => "pending",
        MemoryExtractionStatus::Running => "in_progress",
        MemoryExtractionStatus::Completed => "completed",
        MemoryExtractionStatus::Failed => "error",
        MemoryExtractionStatus::Cancelled => "stopped",
    };
    serde_json::json!({"id":task.id,"type":"memory_extraction","description":"Memory extraction",
        "status":status,"startedAt":task.created_ms,"updatedAt":task.updated_ms,
        "completedAt":task.completed_ms,"summary":task.summary,
        "error":if status == "error" {task.summary} else {None},
        "durationMs":task.completed_ms.unwrap_or(task.updated_ms).saturating_sub(task.created_ms)})
}

fn background_task_snapshot(
    task: aster_core::subagent_task_tracker::SubagentTaskSnapshot,
) -> acp::Result<serde_json::Value> {
    use aster_core::subagent_task_tracker::SubagentStatus;

    let status = match task.status {
        SubagentStatus::Running => "in_progress",
        SubagentStatus::Completed => "completed",
        SubagentStatus::Failed => "error",
        SubagentStatus::Cancelled => "stopped",
        _ => {
            return Err(
                acp::Error::internal_error().data("unsupported background task status in snapshot")
            );
        }
    };
    let command = task.progress.iter().rev().find_map(|entry| {
        entry
            .metadata
            .get("command")
            .and_then(|value| value.as_str())
            .map(str::to_owned)
    });
    let disposition = task
        .progress
        .iter()
        .rev()
        .find_map(|entry| {
            entry
                .metadata
                .get("disposition")
                .and_then(|value| value.as_str())
        })
        .map(str::to_owned)
        .unwrap_or_else(|| {
            match status {
                "in_progress" => "in_progress",
                "completed" => "completed",
                "stopped" => "cancelled",
                _ => "failed",
            }
            .to_string()
        });
    let exit_code = task.progress.iter().rev().find_map(|entry| {
        entry
            .metadata
            .get("exitCode")
            .and_then(|value| value.as_i64())
    });
    let output = task.output.filter(|output| !output.is_empty());
    let error = match status {
        "error" | "stopped" => output.clone(),
        _ => None,
    };
    Ok(serde_json::json!({
        "id": task.task_id,
        "type": task.agent,
        "description": task.description,
        "status": status,
        "startedAt": task.started_ms,
        "updatedAt": task.updated_ms,
        "completedAt": task.finished_ms,
        "command": command,
        "disposition": disposition,
        "exitCode": exit_code,
        "summary": output,
        "error": error,
        "durationMs": task.finished_ms.unwrap_or(task.updated_ms).saturating_sub(task.started_ms),
    }))
}

impl AsterCodeAgent {
    pub(super) async fn answer_host_method(
        &self,
        method: HostMethod,
        args: &acp::ExtRequest,
    ) -> acp::Result<acp::ExtResponse> {
        tracing::debug!(method = method.as_str(), "host extension method");
        match method {
            HostMethod::ModelsList => {
                let current = self.model_id.lock().await.clone();
                let effort = self.effort.lock().await.map(|limits| limits.token);
                ext_json(serde_json::json!({
                    "result": self.model_state_with(&current, effort),
                    "error": null,
                }))
            }
            HostMethod::FollowUp => self.handle_follow_up(args).await,
            HostMethod::GoalImport => self.import_goal(args).await,
            HostMethod::GoalImportPreview => self.preview_goal_import(args).await,
            HostMethod::GoalCommand => self.handle_goal_command(args).await,
            HostMethod::Review => self.handle_review(args).await,
            HostMethod::WikiSearch => answer_wiki_search(args).await,
            HostMethod::WorkspaceSearch => self.handle_workspace_search(args).await,
            HostMethod::MemoryManage => self.handle_memory_management(args).await,
            HostMethod::MemorySnapshot => self.handle_memory_snapshot(args).await,
            HostMethod::CompactConversation => self.handle_compact_extension(args).await,
            HostMethod::Interject => self.handle_interject(args).await,
            HostMethod::SessionInfo => self.handle_session_info(args).await,
            HostMethod::SessionList => self.handle_session_list(args).await,
            HostMethod::SessionDelete => self.handle_session_delete(args).await,
            HostMethod::PermissionPosture => self.handle_permission_posture(args).await,
            HostMethod::SessionTranscriptPage => self.handle_transcript_page(args).await,
            HostMethod::SessionPrefixFork => self.handle_prefix_fork(args).await,
            HostMethod::SessionTooling => self.handle_session_tooling(args).await,
            HostMethod::SessionBackgroundTasks => self.handle_background_tasks(args).await,
            HostMethod::SessionBackgroundTaskStop => self.handle_background_task_stop(args).await,
            HostMethod::SubagentCancel => self.handle_subagent_cancel(args).await,
        }
    }

    async fn handle_workspace_search(
        &self,
        args: &acp::ExtRequest,
    ) -> acp::Result<acp::ExtResponse> {
        let request: WorkspaceSearchParams = serde_json::from_str(args.params.get())
            .map_err(|error| acp::Error::invalid_params().data(error.to_string()))?;
        require_schema(&request.schema, WORKSPACE_SEARCH_SCHEMA)?;
        require_session_id(&request.session_id)?;
        if request.query.trim().is_empty()
            || request.query.len() > 2048
            || request.query.contains('\0')
        {
            return Err(acp::Error::invalid_params().data("invalid workspace search query"));
        }
        let (session, workspace) = {
            let sessions = self.sessions.lock().await;
            let live = sessions
                .get(&request.session_id)
                .ok_or_else(|| acp::Error::invalid_params().data("unknown session id"))?;
            (Arc::clone(&live.session), live.workspace.clone())
        };
        workspace_scope::canonical_workspace(&workspace, Some(&request.cwd))?;
        let result = session
            .governed_tool(
                "search",
                serde_json::json!({"mode":"hybrid","query":request.query,"limit":8}),
            )
            .await
            .map_err(|error| acp::Error::internal_error().data(error.to_string()))?;
        if result.exit_code != 0 {
            return Err(acp::Error::invalid_params().data(result.output));
        }
        ext_json(serde_json::json!({
            "schema":WORKSPACE_SEARCH_SCHEMA,
            "sessionId":request.session_id,
            "operationId":request.operation_id.as_str(),
            "cwd":workspace,
            "output":result.output,
            "metadata":result.metadata,
        }))
    }

    async fn handle_transcript_page(
        &self,
        args: &acp::ExtRequest,
    ) -> acp::Result<acp::ExtResponse> {
        let request: TranscriptPageParams = serde_json::from_str(args.params.get())
            .map_err(|error| acp::Error::invalid_params().data(error.to_string()))?;
        require_schema(&request.schema, TRANSCRIPT_PAGE_SCHEMA)?;
        require_session_id(&request.session_id)?;
        let (session, workspace) = {
            let sessions = self.sessions.lock().await;
            let live = sessions
                .get(&request.session_id)
                .ok_or_else(|| acp::Error::invalid_params().data("unknown session id"))?;
            (Arc::clone(&live.session), live.workspace.clone())
        };
        workspace_scope::canonical_workspace(&workspace, Some(&request.cwd))?;
        let page = session
            .transcript_page(aster_core::store::TranscriptPageRequest {
                cursor: request.cursor,
                limit: request.limit.unwrap_or(50),
            })
            .map_err(|error| acp::Error::invalid_params().data(error.to_string()))?;
        ext_json(serde_json::json!({
            "schema": TRANSCRIPT_PAGE_SCHEMA,
            "sessionId": request.session_id,
            "operationId": request.operation_id.as_str(),
            "page": page,
        }))
    }

    async fn handle_session_tooling(
        &self,
        args: &acp::ExtRequest,
    ) -> acp::Result<acp::ExtResponse> {
        let request: ToolingParams = serde_json::from_str(args.params.get())
            .map_err(|error| acp::Error::invalid_params().data(error.to_string()))?;
        require_schema(&request.schema, TOOLING_SCHEMA)?;
        require_session_id(&request.session_id)?;
        let (session, workspace) = {
            let sessions = self.sessions.lock().await;
            let live = sessions
                .get(&request.session_id)
                .ok_or_else(|| acp::Error::invalid_params().data("unknown session id"))?;
            (Arc::clone(&live.session), live.workspace.clone())
        };
        workspace_scope::canonical_workspace(&workspace, Some(&request.cwd))?;
        let mut projection = session_tooling_projection(&session).await;
        projection["operationId"] = serde_json::json!(request.operation_id.as_str());
        ext_json(projection)
    }

    async fn handle_background_tasks(
        &self,
        args: &acp::ExtRequest,
    ) -> acp::Result<acp::ExtResponse> {
        let request: BackgroundTasksParams = serde_json::from_str(args.params.get())
            .map_err(|error| acp::Error::invalid_params().data(error.to_string()))?;
        require_schema(&request.schema, BACKGROUND_TASKS_SCHEMA)?;
        require_session_id(&request.session_id)?;
        let session = {
            let sessions = self.sessions.lock().await;
            let live = sessions
                .get(&request.session_id)
                .ok_or_else(|| acp::Error::invalid_params().data("unknown session id"))?;
            Arc::clone(&live.session)
        };
        let mut tasks = session
            .subagent_tasks()
            .await
            .into_iter()
            .map(background_task_snapshot)
            .collect::<acp::Result<Vec<_>>>()?;
        if let Some(memory) = session.memory() {
            tasks.extend(
                memory
                    .extraction_tasks(session.session_id())
                    .into_iter()
                    .map(memory_task_snapshot),
            );
        }
        ext_json(serde_json::json!({
            "schema": BACKGROUND_TASKS_SCHEMA,
            "sessionId": request.session_id,
            "tasks": tasks,
        }))
    }

    async fn handle_background_task_stop(
        &self,
        args: &acp::ExtRequest,
    ) -> acp::Result<acp::ExtResponse> {
        let request: BackgroundTaskStopParams = serde_json::from_str(args.params.get())
            .map_err(|error| acp::Error::invalid_params().data(error.to_string()))?;
        require_schema(&request.schema, BACKGROUND_TASKS_SCHEMA)?;
        require_session_id(&request.session_id)?;
        require_session_id(&request.task_id)?;
        let session = {
            let sessions = self.sessions.lock().await;
            let live = sessions
                .get(&request.session_id)
                .ok_or_else(|| acp::Error::invalid_params().data("unknown session id"))?;
            Arc::clone(&live.session)
        };
        let stopped = session.cancel_subagent_task(&request.task_id).await;
        ext_json(serde_json::Value::Bool(stopped))
    }

    async fn handle_subagent_cancel(
        &self,
        args: &acp::ExtRequest,
    ) -> acp::Result<acp::ExtResponse> {
        use aster_core::subagent_task_tracker::SubagentStatus;
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

    async fn handle_prefix_fork(&self, args: &acp::ExtRequest) -> acp::Result<acp::ExtResponse> {
        let request: PrefixForkParams = serde_json::from_str(args.params.get())
            .map_err(|error| acp::Error::invalid_params().data(error.to_string()))?;
        require_schema(&request.schema, PREFIX_FORK_SCHEMA)?;
        require_session_id(&request.session_id)?;
        require_session_id(&request.request.target_session_id)?;
        let (
            source,
            workspace,
            model_id,
            host,
            components,
            mcp_servers,
            plugin_tools,
            posture,
            denies,
        ) = {
            let sessions = self.sessions.lock().await;
            let live = sessions
                .get(&request.session_id)
                .ok_or_else(|| acp::Error::invalid_params().data("unknown session id"))?;
            let permissions = live
                .permissions
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            (
                Arc::clone(&live.session),
                live.workspace.clone(),
                live.model_id.clone(),
                live.host.clone(),
                live.components.clone(),
                live.mcp_servers.clone(),
                live.plugin_tools.clone(),
                permissions.posture,
                permissions.denies.clone(),
            )
        };
        workspace_scope::canonical_workspace(&workspace, Some(&request.request.expected_workspace))
            .map_err(|_| acp::Error::invalid_params().data("prefix fork foreign workspace"))?;
        self.ensure_core_agent().await?;
        let store_dir =
            Self::interactive_session_store_dir(self.launch.config.sessions_dir.as_deref());
        let effort = *self.effort.lock().await;
        let setup = SessionSetup {
            workspace: workspace.clone(),
            model_id: &model_id,
            effort,
            host: host.clone(),
            components: &components,
            mcp_servers: mcp_servers.clone(),
        };
        let mut options = self
            .session_options_for(&request.request.target_session_id, &setup, &store_dir)
            .await?;
        if let Some(selection) = &self.plugin_selection {
            options = selection.session_options(options);
        }
        options = crate::harness_assemble::apply(options, &components)
            .map_err(|error| acp::Error::invalid_params().data(error))?;
        // Core requires an explicit, identical policy acceptance for forks.
        // Read the admitted source policy rather than reconstructing it from
        // optional load metadata, which a resumed TUI session need not send.
        options.work_policy = source.work_policy();
        let source_cut = request.request.expected_cut.clone();
        let boundary_entry_id = request.request.boundary_entry_id.clone();
        let target_session_id = request.request.target_session_id.clone();
        let child = {
            let agent_guard = self.core_agent.lock().await;
            let agent = agent_guard
                .as_ref()
                .ok_or_else(|| acp::Error::internal_error().data("core agent missing"))?;
            agent
                .fork_session_prefix_async(&source, request.request, options)
                .await
                .map_err(|error| acp::Error::invalid_params().data(error.to_string()))?
        };
        let child = Arc::new(child);
        let asset_denies = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let prepared = async {
            self.attach_local_assets(&child, &host, &asset_denies)?;
            let mut live = LiveSession {
                turn_started: std::sync::Mutex::new(None),
                session: Arc::clone(&child),
                workspace,
                model_id,
                permissions: Arc::new(std::sync::Mutex::new(SessionPermissions::default())),
                asset_denies,
                host,
                plugin_tools: Vec::new(),
                components,
                turn_queue: Arc::new(TurnQueue::new()),
                mcp_servers,
                active_prompt_id: std::sync::Mutex::new(None),
            };
            live = self.finish_plugin_tools(live, plugin_tools).await?;
            live.set_posture(posture, denies)?;
            let prefix_cut = live
                .session
                .transcript_page(aster_core::store::TranscriptPageRequest {
                    cursor: None,
                    limit: 1,
                })
                .map_err(|error| acp::Error::internal_error().data(error.to_string()))?
                .source_cut;
            Ok::<_, acp::Error>((live, prefix_cut))
        }
        .await;
        let (live, prefix_cut) = match prepared {
            Ok(prepared) => prepared,
            Err(error) => {
                child.close().await;
                return Err(error);
            }
        };
        {
            let mut sessions = self.sessions.lock().await;
            if sessions.contains_key(&target_session_id) {
                drop(sessions);
                child.close().await;
                return Err(
                    acp::Error::invalid_params().data("prefix fork target is already registered")
                );
            }
            sessions.insert(target_session_id.clone(), live);
        }
        ext_json(serde_json::json!({
            "schema": PREFIX_FORK_SCHEMA,
            "operationId": request.operation_id.as_str(),
            "childId": target_session_id,
            "lineage": {
                "sourceSessionId": request.session_id,
                "childSessionId": target_session_id,
                "boundaryEntryId": boundary_entry_id,
            },
            "sourceCut": source_cut,
            "prefixCut": prefix_cut,
        }))
    }

    pub(super) fn announce_tooling(&self, session_id: &str, projection: serde_json::Value) {
        let Some(client) = self.client.borrow().clone() else {
            return;
        };
        let Ok(raw) = serde_json::value::to_raw_value(&projection) else {
            return;
        };
        let notification = acp::ExtNotification::new(TOOLING_UPDATE_METHOD, raw.into());
        let session_id = session_id.to_string();
        tokio::task::spawn_local(async move {
            tokio::task::yield_now().await;
            if let Err(error) = client.ext_notification(notification).await {
                tracing::warn!(session_id, %error, "tooling projection transport unavailable");
            }
        });
    }

    async fn import_goal(&self, args: &acp::ExtRequest) -> acp::Result<acp::ExtResponse> {
        let command: aster_core::goal_control::GoalDefinitionCommandV1 =
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
