//! Opening and replacing the core session behind an ACP session.

use super::*;

impl A3sCodeAgent {
    pub(super) async fn ensure_core_agent(&self) -> acp::Result<()> {
        let mut guard = self.core_agent.lock().await;
        if guard.is_some() {
            return Ok(());
        }
        let mut config = self.launch.config.clone();
        if config.goal_control.is_none() {
            let now = crate::goal::host_clock_ms()
                .map_err(|error| acp::Error::internal_error().data(error.to_string()))?;
            config.goal_control = match &self.unattended {
                Some(profile) => profile.goal_control(now),
                None => crate::goal::default_continuation_policy(now).map(Some),
            }
            .map_err(|error| acp::Error::internal_error().data(error.to_string()))?;
        }
        let agent = Agent::from_config(config)
            .await
            .map_err(|error| acp::Error::internal_error().data(error.to_string()))?;
        // SEC-SAMPLE1 wiring: arm per-server sampling behind all three
        // preconditions (consent bridge, per-server ceiling,
        // generation-enforced cap). Runs once, with the agent.
        if let Err(error) = self.arm_sampling_consent(&agent).await {
            tracing::warn!(error = %error, "sampling stays refused: arming failed");
        }
        *guard = Some(agent);
        Ok(())
    }

    pub(super) fn core_session_options(
        session_id: &str,
        model_id: &str,
        effort: Option<EffortLimits>,
    ) -> SessionOptions {
        let mut options = SessionOptions::new()
            .with_assistant_final_response(true)
            .with_auto_save(true)
            .with_session_id(session_id.to_string());
        if let Ok(model_id) = super::configured_model_id(model_id) {
            options = options.with_model(model_id.to_string());
        }
        options = options
            .with_max_tool_rounds(0)
            .with_auto_compact(true)
            .with_auto_compact_threshold(a3s_code_core::store::DEFAULT_AUTO_COMPACT_THRESHOLD);
        let mut guideline = String::new();
        if let Some(limits) = effort {
            options = options
                .with_reasoning_effort(limits.token)
                .with_thinking_budget(limits.thinking_budget)
                .with_max_parallel_tasks(limits.max_parallel_tasks)
                .with_max_continuation_turns(limits.max_continuation_turns);
            if limits.token == "ultracode" {
                options = options
                    .with_planning_mode(a3s_code_core::PlanningMode::Auto)
                    .with_auto_delegation_enabled(true)
                    .with_auto_parallel_delegation(true)
                    .with_manual_delegation_enabled(true);
            }
            guideline = format!(
                "The thinking budget is a maximum, not a quota. thinking_budget={}. Stop when the user's outcome is done.",
                limits.thinking_budget
            );
            if !a3s_code_core::llm::model_sends_native_effort(model_id) {
                if let Some(extra) = effort_guideline(limits.token) {
                    guideline.push('\n');
                    guideline.push_str(extra);
                }
            }
        }
        if !guideline.is_empty() {
            guideline.push('\n');
        }
        guideline.push_str(SESSION_OUTCOME_BOUNDARY);
        options = options.with_prompt_slots(
            a3s_code_core::SystemPromptSlots::default()
                .with_role("You are Aster, an expert AI coding agent.")
                .with_guidelines(guideline),
        );
        options
    }

    /// Snapshot directory for the interactive core session.
    ///
    /// ACL `sessions_dir` wins. Otherwise the store sits beside the pager tree
    /// at `<product home>/session-store`, not inside `<product home>/sessions`.
    /// The pager scans every child of `sessions` as a workspace bucket, and the
    /// file store's `v1` directory would be one of those buckets.
    pub(super) fn interactive_session_store_dir(configured: Option<&std::path::Path>) -> PathBuf {
        interactive_session_store_dir_from(
            configured,
            std::env::var_os("A3S_HOME").as_deref(),
            std::env::var_os("HOME").as_deref(),
        )
    }

    #[cfg(test)]
    pub(super) fn host_session_options(
        session_id: &str,
        setup: &SessionSetup<'_>,
        config: &a3s_code_core::CodeConfig,
        session_store_dir: &std::path::Path,
    ) -> acp::Result<SessionOptions> {
        Self::host_session_options_with_embedding(
            session_id,
            setup,
            config,
            session_store_dir,
            None,
        )
    }

    fn host_session_options_with_embedding(
        session_id: &str,
        setup: &SessionSetup<'_>,
        config: &a3s_code_core::CodeConfig,
        session_store_dir: &std::path::Path,
        embedding: Option<Arc<dyn a3s_code_core::EmbeddingProvider>>,
    ) -> acp::Result<SessionOptions> {
        let options = Self::core_session_options(session_id, setup.model_id, setup.effort);
        let mut slots = crate::host_context::merge_studio_host(
            options.prompt_slots.clone().unwrap_or_default(),
            &setup.host,
        );
        let options = match setup.host.completion_attestation {
            Some(crate::host_context::CompletionAttestation::DiskContent)
                if !setup.host.source_review =>
            {
                options.with_completion_attestor(std::sync::Arc::new(
                    a3s_code_core::completion_attestor::DiskContentAttestor::new(&setup.workspace),
                ))
            }
            _ => options,
        };
        let home = a3s_code_core::hooks::command_hook_home();
        let trust = match a3s_code_core::hooks::command_hook_trust_path(&setup.workspace) {
            Ok(path) => Some(path),
            Err(error) => {
                tracing::error!(%error, "hook trust path is unavailable");
                None
            }
        };
        Self::host_session_options_with_hook_roots(
            options,
            &mut slots,
            setup,
            config,
            session_store_dir,
            home.as_deref(),
            trust.as_deref(),
            embedding,
        )
    }

    pub(super) async fn session_options_for(
        &self,
        session_id: &str,
        setup: &SessionSetup<'_>,
        session_store_dir: &std::path::Path,
    ) -> acp::Result<SessionOptions> {
        let retrieval = self
            .launch
            .config
            .workspace_retrieval
            .filter(|_| !setup.host.source_review && setup.host.knowledge_compile.is_none());
        let (embedding, unavailable) = match retrieval {
            Some(retrieval) if retrieval.enabled && retrieval.local_cpu => {
                local_cpu_embedding().await
            }
            Some(retrieval) if retrieval.enabled => (
                None,
                Some(
                    "workspace_retrieval requires an explicitly selected local_cpu backend"
                        .to_owned(),
                ),
            ),
            _ => (None, None),
        };
        let options = match &self.unattended {
            Some(unattended) => unattended
                .session_options(
                    session_id,
                    setup.model_id,
                    interactive_skill_dirs(&setup.workspace, &self.launch.config.skill_dirs),
                )
                .map(|options| match embedding {
                    Some(provider) => options.with_workspace_retrieval(
                        a3s_code_core::WorkspaceRetrievalOptions::new(provider),
                    ),
                    None => options,
                })
                .map_err(|error| acp::Error::internal_error().data(error)),
            None => Self::host_session_options_with_embedding(
                session_id,
                setup,
                &self.launch.config,
                session_store_dir,
                embedding,
            ),
        }?;
        let mut options = match &self.launch.config.tool_presentation_profile {
            Some(profile) => options.with_tool_presentation_profile(profile.clone()),
            None => options,
        };
        if let Some(rules) = self.client_rules.borrow().as_ref() {
            let mut slots = options.prompt_slots.clone().unwrap_or_default();
            append_slot_extra(
                &mut slots,
                &format!("<human_rules>\n{rules}\n</human_rules>"),
            );
            options = options.with_prompt_slots(slots);
        }
        if let Some(reason) = unavailable {
            tracing::error!(%reason, "configured local workspace retrieval is unavailable");
            let mut slots = options.prompt_slots.clone().unwrap_or_default();
            append_slot_extra(
                &mut slots,
                &format!(
                    "Workspace semantic retrieval unavailable: {reason}. grep, glob, bm25 and hybrid remain available; hybrid uses available evidence channels. No remote backend was activated."
                ),
            );
            options = options.with_prompt_slots(slots);
        }
        let options = self.with_client_reads(options, session_id, &setup.workspace);
        if setup.host.source_review {
            // A fixed checker keeps the capability ceiling empty even after a mode change.
            let mut options = options;
            options.hook_executor = None;
            Ok(options.with_isolated_context().with_permission_policy(
                a3s_code_core::permissions::PermissionPolicy::new().deny("*"),
            ))
        } else if let Some(scope) = &setup.host.knowledge_compile {
            let mut options = options;
            options.hook_executor = None;
            let guard = crate::knowledge_compile::CompileGuard::new(scope, &setup.workspace)
                .map_err(|error| acp::Error::invalid_params().data(error))?;
            Ok(options
                .with_isolated_context()
                .with_completion_attestor(Arc::new(
                    a3s_code_core::completion_attestor::DiskContentAttestor::new(&setup.workspace),
                ))
                .with_permission_policy(crate::knowledge_compile::policy(&[]))
                .with_permission_checker(Arc::new(guard)))
        } else {
            Ok(options)
        }
    }

    pub(super) async fn open_core_session(
        &self,
        session_id: &str,
        setup: SessionSetup<'_>,
        restore: bool,
    ) -> acp::Result<LiveSession> {
        let store_dir =
            Self::interactive_session_store_dir(self.launch.config.sessions_dir.as_deref());
        let stored = if restore {
            Some(Self::resume_snapshot_store(&store_dir, session_id, &setup.workspace).await?)
        } else {
            None
        };
        if let Err(message) = super::configured_model_id(setup.model_id) {
            return Err(acp::Error::invalid_params().data(message));
        }
        self.ensure_core_agent().await?;
        let options = self
            .session_options_for(session_id, &setup, &store_dir)
            .await?;
        let agent_guard = self.core_agent.lock().await;
        let agent = agent_guard
            .as_ref()
            .ok_or_else(|| acp::Error::internal_error().data("core agent missing"))?;
        let options = match &self.plugin_selection {
            Some(selection) => selection.session_options(options),
            None => options,
        };
        let options = crate::harness_assemble::apply(options, setup.components)
            .map_err(|error| acp::Error::invalid_params().data(error))?;
        let session = match stored {
            Some(store) => {
                agent
                    .resume_session_async(session_id, options.with_session_store(store))
                    .await
            }
            None => {
                debug_assert!(!restore);
                agent
                    .session_async(setup.workspace.to_string_lossy().to_string(), Some(options))
                    .await
            }
        }
        .map_err(|error| acp::Error::internal_error().data(error.to_string()))?;
        let names = session.tool_names();
        let command_tool = if names.iter().any(|name| name == "bash") {
            "bash"
        } else if names.iter().any(|name| name == "run_terminal_command") {
            "run_terminal_command"
        } else {
            "none"
        };
        tracing::info!(
            command_tool,
            model = %setup.model_id,
            "acp session ready"
        );
        if session
            .work_policy_digest()
            .map_err(|error| acp::Error::internal_error().data(error.to_string()))?
            .is_some()
        {
            crate::engineering::register_scenarios(&session)
                .map_err(|error| acp::Error::internal_error().data(error))?;
        }
        let asset_denies = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let permissions =
            SessionPermissions::with_client_rules(self.client_permissions.borrow().clone());
        if !setup.host.source_review
            && setup.host.knowledge_compile.is_none()
            && (self.attach_local_assets(&session, &setup.host, &asset_denies)?
                || !permissions.client_rules.is_empty())
        {
            session
                .set_permission_policy(permissions.policy())
                .map_err(|error| acp::Error::internal_error().data(error.to_string()))?;
        }
        let live = LiveSession {
            turn_started: std::sync::Mutex::new(None),
            session: Arc::new(session),
            workspace: setup.workspace,
            model_id: setup.model_id.to_string(),
            permissions: Arc::new(std::sync::Mutex::new(permissions)),
            asset_denies,
            host: setup.host,
            plugin_tools: Vec::new(),
            components: setup.components.to_vec(),
            turn_queue: Arc::new(TurnQueue::new()),
            mcp_servers: setup.mcp_servers,
            active_prompt_id: std::sync::Mutex::new(None),
        };
        start_workspace_indexing(&live);
        Ok(live)
    }

    /// Register each installed plugin executable on the live session.
    pub(super) fn admit_plugin_tools(
        session: &AgentSession,
        entries: &[serde_json::Value],
    ) -> acp::Result<()> {
        for tool in crate::plugin_tools::from_entries(entries) {
            session
                .register_dynamic_tool(tool)
                .map_err(|error| acp::Error::internal_error().data(error.to_string()))?;
        }
        Ok(())
    }

    pub(super) async fn finish_plugin_tools(
        &self,
        mut live: LiveSession,
        entries: Vec<serde_json::Value>,
    ) -> acp::Result<LiveSession> {
        let result = async {
            if !live.host.source_review && live.host.knowledge_compile.is_none() {
                if live.host.office_tools {
                    live.session
                        .register_dynamic_tool(Arc::new(
                            crate::office_read::OfficeReadTool::new(&live.workspace)
                                .map_err(|error| {
                                    acp::Error::internal_error().data(error.to_string())
                                })?
                                .with_host(
                                    self.client_reads_files
                                        .get()
                                        .then(|| self.document_bridge())
                                        .flatten(),
                                    live.session.session_id().to_owned(),
                                ),
                        ))
                        .map_err(|error| acp::Error::internal_error().data(error.to_string()))?;
                    live.session
                        .register_dynamic_tool(Arc::new(
                            crate::office_file::OfficeFileTool::new(&live.workspace).map_err(
                                |error| acp::Error::internal_error().data(error.to_string()),
                            )?,
                        ))
                        .map_err(|error| acp::Error::internal_error().data(error.to_string()))?;
                }
                live.session
                    .register_dynamic_tool(Arc::new(crate::output_check::OutputCheckTool))
                    .map_err(|error| acp::Error::internal_error().data(error.to_string()))?;
            }
            if let Some(host) = &self.plugin_selection {
                host.apply(&live.session)
                    .await
                    .map_err(|error| acp::Error::internal_error().data(error.to_string()))
            } else {
                Self::admit_plugin_tools(&live.session, &entries)
            }
        }
        .await;
        if let Err(error) = result {
            live.session.close().await;
            return Err(error);
        }
        if let Some(documents) = self.document_bridge() {
            let result = documents
                .tool(&live.workspace, live.session.session_id())
                .and_then(|tool| live.session.register_dynamic_tool(tool).map_err(Into::into));
            if let Err(error) = result {
                live.session.close().await;
                return Err(acp::Error::internal_error().data(error.to_string()));
            }
        }
        live.plugin_tools = entries;
        Ok(live)
    }

    /// Rebuild a session that is already registered. `session_async` rejects a
    /// live id (`session is already live or being built`); model and effort
    /// changes have to replace that session in place.
    pub(super) async fn replace_core_session(
        &self,
        current: &LiveSession,
        setup: SessionSetup<'_>,
    ) -> acp::Result<LiveSession> {
        self.ensure_core_agent().await?;
        let store_dir =
            Self::interactive_session_store_dir(self.launch.config.sessions_dir.as_deref());
        let mut options = self
            .session_options_for(current.session.session_id(), &setup, &store_dir)
            .await?;
        let agent_guard = self.core_agent.lock().await;
        let agent = agent_guard
            .as_ref()
            .ok_or_else(|| acp::Error::internal_error().data("core agent missing"))?;
        if let Some(selection) = &self.plugin_selection {
            options = selection.session_options(options);
        }
        options = crate::harness_assemble::apply(options, setup.components)
            .map_err(|error| acp::Error::invalid_params().data(error))?;
        let session = if let Some(store) = current.session.session_store() {
            options = options.with_session_store(store);
            agent
                .replace_session_async(&current.session, options)
                .await
                .map_err(|error| acp::Error::internal_error().data(error.to_string()))?
        } else {
            // Nothing was persisted, so closing frees the id for a fresh session.
            current.session.close().await;
            agent
                .session_async(setup.workspace.to_string_lossy().to_string(), Some(options))
                .await
                .map_err(|error| acp::Error::internal_error().data(error.to_string()))?
        };
        if session
            .work_policy_digest()
            .map_err(|error| acp::Error::internal_error().data(error.to_string()))?
            .is_some()
        {
            crate::engineering::register_scenarios(&session)
                .map_err(|error| acp::Error::internal_error().data(error))?;
        }
        let asset_denies = Arc::clone(&current.asset_denies);
        if setup.host.knowledge_compile.is_none() {
            self.attach_local_assets(&session, &setup.host, &asset_denies)?;
        }
        let mut live = LiveSession {
            turn_started: std::sync::Mutex::new(None),
            session: Arc::new(session),
            workspace: setup.workspace,
            model_id: setup.model_id.to_string(),
            permissions: Arc::clone(&current.permissions),
            asset_denies,
            host: setup.host,
            plugin_tools: Vec::new(),
            components: setup.components.to_vec(),
            turn_queue: Arc::clone(&current.turn_queue),
            mcp_servers: setup.mcp_servers,
            active_prompt_id: std::sync::Mutex::new(None),
        };
        let posture = live
            .permissions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .posture;
        live.set_posture(posture, Vec::new())?;
        start_workspace_indexing(&live);
        Ok(live)
    }

    /// Register `open_local_asset` when the tenant catalog is non-empty.
    pub(super) fn attach_local_assets(
        &self,
        session: &AgentSession,
        host: &crate::host_context::StudioHost,
        denies: &std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    ) -> acp::Result<bool> {
        if host.source_review || host.knowledge_compile.is_some() {
            return Ok(false);
        }
        let bridge = host
            .workspace_wiki_search
            .then(|| self.document_bridge())
            .flatten();
        let memory_bridge = host
            .workspace_memory_search
            .then(|| self.document_bridge())
            .flatten();
        let roots = host.assets.clone().unwrap_or_default();
        let catalog_empty = crate::local_assets::load_catalog(&roots).is_empty();
        let wiki =
            roots
                .wiki
                .as_ref()
                .and_then(|path| match crate::wiki_search::shared_index(path) {
                    Ok(index) => Some(index),
                    Err(error) => {
                        tracing::warn!(%error, "wiki index did not start");
                        None
                    }
                });
        if catalog_empty && wiki.is_none() && bridge.is_none() && memory_bridge.is_none() {
            return Ok(false);
        }
        if !catalog_empty {
            session
                .register_dynamic_tool(Arc::new(crate::local_assets::OpenLocalAssetTool::new(
                    roots,
                    std::sync::Arc::clone(denies),
                )))
                .map_err(|error| acp::Error::internal_error().data(error.to_string()))?;
        }
        if wiki.is_some() || bridge.is_some() {
            let mut tool =
                crate::wiki_search::SearchWikiTool::new(wiki, std::sync::Arc::clone(denies));
            if let Some(bridge) = bridge {
                tool = tool
                    .with_host(bridge, session.session_id(), session.workspace())
                    .map_err(|error| acp::Error::internal_error().data(error.to_string()))?;
            }
            session
                .register_dynamic_tool(Arc::new(tool))
                .map_err(|error| acp::Error::internal_error().data(error.to_string()))?;
        }
        if let Some(bridge) = memory_bridge {
            let tool = crate::memory_search::SearchMemoryTool::new(
                bridge,
                session.session_id(),
                session.workspace(),
                Arc::clone(denies),
            )
            .map_err(|error| acp::Error::internal_error().data(error.to_string()))?;
            session
                .register_dynamic_tool(Arc::new(tool))
                .map_err(|error| acp::Error::internal_error().data(error.to_string()))?;
        }
        Ok(true)
    }

    /// Sync the agent-global MCP manager to the UNION of the config baseline
    /// and every live session's server set. Previously the last session's
    /// desired set won alone, so opening a second session with different
    /// `mcp_servers` silently deleted the first session's servers.
    pub(super) async fn sync_session_mcp_servers(
        &self,
        for_session: Option<&str>,
        servers: &[acp::McpServer],
    ) -> acp::Result<()> {
        self.ensure_core_agent().await?;
        let guard = self.core_agent.lock().await;
        let agent = guard
            .as_ref()
            .ok_or_else(|| acp::Error::internal_error().data("core agent missing"))?;
        let baseline = {
            let mut cached = self.config_mcp.lock().await;
            if cached.is_none() {
                *cached = Some(agent.global_mcp_configs().await);
            }
            cached.clone().unwrap_or_default()
        };
        let live: Vec<(String, Vec<a3s_code_core::mcp::McpServerConfig>)> = {
            let sessions = self.sessions.lock().await;
            sessions
                .iter()
                .map(|(key, live)| (key.clone(), live.mcp_servers.clone()))
                .collect()
        };
        let desired = union_mcp_desired(baseline, &live, for_session, servers)
            .map_err(|conflict| acp::Error::invalid_params().data(conflict))?;
        agent
            .sync_global_mcp_servers(desired)
            .await
            .map_err(|error| acp::Error::internal_error().data(error.to_string()))
    }
}

fn start_workspace_indexing(live: &LiveSession) {
    if live.host.source_review || live.host.knowledge_compile.is_some() {
        return;
    }
    let session = Arc::downgrade(&live.session);
    tokio::spawn(async move {
        let Some(session) = session.upgrade() else {
            return;
        };
        match session.prepare_workspace_index().await {
            Ok(true) | Err(a3s_code_core::CodeError::SessionClosed { .. }) => {}
            Ok(false) => tracing::warn!(
                "workspace a3s-vec index unavailable; verified retrieval fallback remains active"
            ),
            Err(error) => tracing::warn!(%error, "workspace index preparation failed"),
        }
    });
}

/// The desired global set: baseline ∪ every live session's servers,
/// skipping the session being re-modeled (its new set merges in via
/// `add` instead). With no skip and no additions this is exactly the
/// session-close resync: baseline ∪ the remaining sessions.
///
/// Same-name merges follow the ORIGIN: within one origin (the baseline
/// or one session) later-wins is the update rule, and two origins
/// holding the SAME definition dedupe to one entry — but a name held by
/// one origin under a DIFFERENT definition than another origin's is a
/// cross-session takeover of that name and is REFUSED, naming both
/// origins. The `mcp__<server>__<tool>` prefix routes by name, so
/// silently re-pointing it would redirect another live session's tool
/// source.
pub(super) fn union_mcp_desired(
    baseline: Vec<a3s_code_core::mcp::McpServerConfig>,
    live: &[(String, Vec<a3s_code_core::mcp::McpServerConfig>)],
    skip: Option<&str>,
    add: &[acp::McpServer],
) -> Result<Vec<a3s_code_core::mcp::McpServerConfig>, String> {
    let mut desired: Vec<(a3s_code_core::mcp::McpServerConfig, &str)> = baseline
        .into_iter()
        .map(|server| (server, "the configuration baseline"))
        .collect();
    for (key, servers) in live {
        if skip == Some(key.as_str()) {
            continue;
        }
        for server in servers {
            merge_with_origin(&mut desired, server.clone(), key)?;
        }
    }
    let add_origin = skip.unwrap_or("the new session");
    for server in session_mcp_configs(add) {
        merge_with_origin(&mut desired, server, add_origin)?;
    }
    Ok(desired.into_iter().map(|(server, _)| server).collect())
}

/// Merge one server into the desired set by name, tracking which origin
/// holds the name: a same-origin collision is that origin updating
/// itself (later wins), a same-definition collision across origins
/// dedupes, and a different-definition collision across origins refuses.
pub(super) fn merge_with_origin<'a>(
    desired: &mut Vec<(a3s_code_core::mcp::McpServerConfig, &'a str)>,
    server: a3s_code_core::mcp::McpServerConfig,
    origin: &'a str,
) -> Result<(), String> {
    match desired
        .iter()
        .position(|(entry, _)| entry.name == server.name)
    {
        Some(index) => {
            let (existing, held_by) = &desired[index];
            if held_by == &origin || same_server_definition(existing, &server) {
                desired[index] = (server, origin);
                Ok(())
            } else {
                Err(format!(
                    "mcp server name `{}` is already held by {held_by:?} with a different \
                     definition; {origin:?} must use another name (the mcp__{0}__ tool \
                     prefix routes by name)",
                    server.name,
                ))
            }
        }
        None => {
            desired.push((server, origin));
            Ok(())
        }
    }
}

/// Canonical-definition equality: `McpServerConfig` carries a custom
/// `Deserialize` and no `PartialEq`, so definitions compare through
/// their serialization (serde_json maps are ordered — deterministic for
/// this plain-data type).
pub(super) fn same_server_definition(
    left: &a3s_code_core::mcp::McpServerConfig,
    right: &a3s_code_core::mcp::McpServerConfig,
) -> bool {
    match (serde_json::to_value(left), serde_json::to_value(right)) {
        (Ok(left), Ok(right)) => left == right,
        // Serialize cannot fail for this type; treat a failure as
        // inequality rather than silently merging unknowns.
        _ => false,
    }
}

pub(super) fn session_mcp_configs(
    servers: &[acp::McpServer],
) -> Vec<a3s_code_core::mcp::McpServerConfig> {
    servers.iter().filter_map(acp_mcp_to_config).collect()
}

pub(super) fn acp_mcp_to_config(
    server: &acp::McpServer,
) -> Option<a3s_code_core::mcp::McpServerConfig> {
    use a3s_code_core::mcp::{McpServerConfig, McpTransportConfig};

    let hosted = |name: &str,
                  url: &str,
                  headers: &[acp::HttpHeader],
                  meta: Option<&acp::Meta>,
                  sse: bool|
     -> Option<a3s_code_core::mcp::McpServerConfig> {
        if name.is_empty() || url.is_empty() {
            return None;
        }
        let headers = headers
            .iter()
            .filter(|header| !header.name.is_empty())
            .map(|header| (header.name.clone(), header.value.clone()))
            .collect();
        let transport = if sse {
            McpTransportConfig::Http {
                url: url.to_string(),
                headers,
            }
        } else {
            McpTransportConfig::StreamableHttp {
                url: url.to_string(),
                headers,
            }
        };
        Some(McpServerConfig {
            name: name.to_string(),
            transport,
            enabled: true,
            env: HashMap::new(),
            oauth: None,
            tool_timeout_secs: mcp_tool_timeout(meta)?,
        })
    };

    match server {
        acp::McpServer::Stdio(stdio) => {
            let command = stdio.command.to_string_lossy();
            if stdio.name.is_empty() || command.is_empty() {
                return None;
            }
            let env = stdio
                .env
                .iter()
                .filter(|variable| !variable.name.is_empty())
                .map(|variable| (variable.name.clone(), variable.value.clone()))
                .collect();
            Some(McpServerConfig {
                name: stdio.name.clone(),
                transport: McpTransportConfig::Stdio {
                    command: command.into_owned(),
                    args: stdio.args.clone(),
                },
                enabled: true,
                env,
                oauth: None,
                tool_timeout_secs: mcp_tool_timeout(stdio.meta.as_ref())?,
            })
        }
        acp::McpServer::Http(http) => hosted(
            &http.name,
            &http.url,
            &http.headers,
            http.meta.as_ref(),
            false,
        ),
        acp::McpServer::Sse(sse) => {
            hosted(&sse.name, &sse.url, &sse.headers, sse.meta.as_ref(), true)
        }
        _ => None,
    }
}

fn mcp_tool_timeout(meta: Option<&acp::Meta>) -> Option<u64> {
    match meta.and_then(|meta| meta.get("aster/toolTimeoutSecs")) {
        None => Some(60),
        Some(value) => value
            .as_u64()
            .filter(|seconds| (1..=3600).contains(seconds)),
    }
}

#[cfg(test)]
#[path = "session_mcp_timeout_tests.rs"]
mod session_mcp_timeout_tests;

/// The host's registered provider (BYO model engine) serves the trusted
/// local_cpu opt-in; the product itself ships no engine.
async fn local_cpu_embedding() -> (
    Option<Arc<dyn a3s_code_core::EmbeddingProvider>>,
    Option<String>,
) {
    match super::local_embedding::local_embedding_provider(
        tokio_util::sync::CancellationToken::new(),
    )
    .await
    {
        Ok(provider) => (Some(provider), None),
        Err(error) => (None, Some(error.to_string())),
    }
}
