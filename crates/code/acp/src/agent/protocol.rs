use super::*;
use std::path::Path;

async fn load_legacy_goal_if_needed(
    session: &AgentSession,
    workspace: &Path,
    session_id: &str,
) -> acp::Result<Option<crate::goal::DurableGoal>> {
    if session
        .goal_definition()
        .map_err(|error| acp::Error::invalid_params().data(error.to_string()))?
        .is_some()
    {
        return Ok(None);
    }
    crate::goal::load_goal(workspace, session_id)
        .await
        .map_err(|error| acp::Error::invalid_params().data(error.to_string()))
}

#[async_trait::async_trait(?Send)]
impl acp::Agent for AsterCodeAgent {
    async fn initialize(
        &self,
        args: acp::InitializeRequest,
    ) -> acp::Result<acp::InitializeResponse> {
        let started = std::time::Instant::now();
        let rules = initialize_rules_from_meta(args.meta.as_ref())?;
        let permissions = ClientPermissionRules::from_meta(args.meta.as_ref())?;
        let terminal_markdown = match args
            .meta
            .as_ref()
            .and_then(|meta| meta.get("aster/terminalMarkdown"))
        {
            None => false,
            Some(serde_json::Value::Bool(value)) => *value,
            Some(_) => {
                return Err(
                    acp::Error::invalid_params().data("aster/terminalMarkdown must be a boolean")
                );
            }
        };
        let discovery = match args
            .meta
            .as_ref()
            .and_then(|meta| meta.get("aster/modelDiscovery"))
        {
            None => false,
            Some(serde_json::Value::Bool(value)) => *value,
            Some(_) => {
                return Err(
                    acp::Error::invalid_params().data("aster/modelDiscovery must be a boolean")
                );
            }
        };
        self.client_model_discovery.set(discovery);
        *self.client_rules.borrow_mut() = rules;
        *self.client_permissions.borrow_mut() = permissions;
        self.client_terminal_markdown.set(terminal_markdown);
        self.client_reads_files
            .set(args.client_capabilities.fs.read_text_file);
        let cwd = self.launch.workspace.to_string_lossy().to_string();
        let current = self.model_id.lock().await.clone();
        let effort_token = self.effort.lock().await.map(|limits| limits.token);
        let model_state = self.model_state_with(&current, effort_token);
        let response = acp::InitializeResponse::new(acp::ProtocolVersion::V1)
            .agent_capabilities(
                acp::AgentCapabilities::new()
                    .load_session(true)
                    .prompt_capabilities(acp::PromptCapabilities::new().image(true)),
            )
            .auth_methods(Vec::new())
            .meta(
                serde_json::json!({
                    "grokShell": false,
                    "asterCode": true,
                    "agentVersion": env!("CARGO_PKG_VERSION"),
                    "currentWorkingDirectory": cwd,
                    "modelState": model_state,
                    // Legacy initialize extension: global ACP handlers only. Core
                    // commands are session-specific and arrive after new/load.
                    "availableCommands": commands::acp_commands(),
                    "cancelRewind": false,
                    "sessionRecap": false,
                    "feedbackTraceOffer": false,
                    "aster": self.workspace_admission_meta(),
                    "openModelConfig": self.first_run_config,
                    // P1-8 / P7: mechanism feature-stage registry digest + per-switch
                    // admission (Removed → false). Not a remote rollout percentage.
                    "featureSwitchRegistryDigest": aster_core::capability::feature_switch_registry_digest()
                        .map(|digest| digest.to_string())
                        .unwrap_or_else(|_| String::new()),
                    "featureSwitches": feature_switch_admission_meta(),
                    // P7 deepen: stage map so hosts can distinguish Stable vs
                    // Experimental without mistaking enabled=true for Stable.
                    "featureSwitchStages": feature_switch_stage_meta(),
                })
                .as_object()
                .cloned(),
            );
        tracing::info!(
            target: "aster_acp::retrieval_phase",
            phase = "initialize",
            elapsed_ms = started.elapsed().as_secs_f64() * 1_000.0,
            "ACP phase completed"
        );
        Ok(response)
    }

    async fn authenticate(
        &self,
        _args: acp::AuthenticateRequest,
    ) -> acp::Result<acp::AuthenticateResponse> {
        Ok(acp::AuthenticateResponse::default())
    }

    async fn new_session(
        &self,
        args: acp::NewSessionRequest,
    ) -> acp::Result<acp::NewSessionResponse> {
        let started = std::time::Instant::now();
        let posture = PermissionPosture::from_meta(args.meta.as_ref())?;
        let mut host = crate::host_context::studio_host_from_meta(args.meta.as_ref())
            .map_err(|error| acp::Error::invalid_params().data(error))?;
        if self.unattended.is_none()
            && !args
                .meta
                .as_ref()
                .is_some_and(|meta| meta.contains_key("asterEngineering"))
        {
            host.work_policy = Some(
                crate::engineering::named_policy(crate::engineering::DEFAULT_INTERACTIVE_POLICY)
                    .map_err(|error| acp::Error::internal_error().data(error))?,
            );
        }
        self.validate_plugin_selection(args.meta.as_ref(), &args.mcp_servers)?;
        let plugin_tools = crate::plugin_tools::entries_from_meta(args.meta.as_ref());
        let components = crate::harness_assemble::components_from_meta(args.meta.as_ref())
            .map_err(|error| acp::Error::invalid_params().data(error))?;
        self.admit_studio_documents(args.meta.as_ref());
        let session_id = uuid::Uuid::new_v4().to_string();
        let workspace = if args.cwd.as_os_str().is_empty() {
            self.launch.workspace.clone()
        } else {
            args.cwd.clone()
        };
        if let Some(host) = &self.plugin_selection {
            host.validate_workspace(&self.launch.workspace)
                .and_then(|_| host.validate_workspace(&workspace))
                .map_err(|_| {
                    acp::Error::invalid_params().data("native plugin workspace is not admitted")
                })?;
        }
        let session_kind = match args.meta.as_ref().and_then(|meta| meta.get("sessionKind")) {
            None => None,
            Some(serde_json::Value::String(kind)) if kind == "interactive" => {
                Some(aster_core::session_folder::SessionKind::Interactive)
            }
            Some(serde_json::Value::String(kind)) if kind == "headless" => {
                Some(aster_core::session_folder::SessionKind::Headless)
            }
            Some(_) => {
                return Err(acp::Error::invalid_params().data("unsupported session kind"));
            }
        };
        match session_kind {
            Some(kind) => aster_core::session_folder::prepare_session_home_with_kind(
                &workspace,
                &session_id,
                kind,
            ),
            None => aster_core::session_folder::prepare_session_home(&workspace, &session_id),
        }
        .map_err(|error| acp::Error::internal_error().data(error.to_string()))?;
        let session_mcp = session_mcp_configs(&args.mcp_servers);
        let mcp_reservation = self
            .sync_session_mcp_servers(Some(session_id.as_ref()), &args.mcp_servers)
            .await?;
        let model_id = self.model_id.lock().await.clone();
        self.refresh_model(&model_id, false).await;
        let effort = *self.effort.lock().await;
        let live = self
            .open_core_session(
                &session_id,
                SessionSetup {
                    workspace: workspace.clone(),
                    model_id: &model_id,
                    effort,
                    host,
                    components: &components,
                    mcp_servers: session_mcp,
                },
                false,
            )
            .await?;
        let mut live = self.finish_plugin_tools(live, plugin_tools).await?;
        if let Some(posture) = posture {
            live.set_posture(posture, Vec::new())?;
        }
        let modes = live
            .permissions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .posture
            .modes();
        let effort_token = live.effort.map(|limits| limits.token);
        let models = self.model_state_with(&live.model_id, effort_token);
        let goal = match live.session.goal_definition() {
            Ok(Some(_)) => Ok(None),
            Ok(None) => crate::goal::load_goal(&workspace, &session_id)
                .await
                .map_err(|error| acp::Error::invalid_params().data(error.to_string())),
            Err(error) => Err(acp::Error::invalid_params().data(error.to_string())),
        };
        let goal = match goal {
            Ok(goal) => goal,
            Err(error) => {
                live.session.close().await;
                return Err(error);
            }
        };
        let available_commands = commands::session_commands(&live.session)?;
        let tooling = session_tooling_projection(&live.session).await;
        let usage_session = Arc::clone(&live.session);
        // A session may be resumed before its first turn triggers auto-save.
        if let Err(error) = live.session.save().await {
            live.session.close().await;
            return Err(acp::Error::internal_error().data(error.to_string()));
        }
        self.sessions.lock().await.insert(session_id.clone(), live);
        drop(mcp_reservation);
        if let Some(goal) = goal {
            self.goals.lock().await.insert(session_id.clone(), goal);
        } else {
            self.goals.lock().await.remove(&session_id);
        }
        self.reapply_active_goal(&session_id).await;

        let config_options = Self::model_config_options(&models);
        self.announce_commands(&session_id, available_commands);
        self.announce_tooling(&session_id, tooling);
        self.announce_session_usage(&session_id, &usage_session);
        let response = acp::NewSessionResponse::new(acp::SessionId::new(session_id))
            .modes(modes)
            .models(models)
            .config_options(config_options);
        tracing::info!(
            target: "aster_acp::retrieval_phase",
            phase = "new_session",
            session_id = %response.session_id,
            elapsed_ms = started.elapsed().as_secs_f64() * 1_000.0,
            "ACP phase completed"
        );
        Ok(response)
    }

    async fn load_session(
        &self,
        args: acp::LoadSessionRequest,
    ) -> acp::Result<acp::LoadSessionResponse> {
        let posture = PermissionPosture::from_meta(args.meta.as_ref())?;
        let host = crate::host_context::studio_host_from_meta(args.meta.as_ref())
            .map_err(|error| acp::Error::invalid_params().data(error))?;
        self.validate_plugin_selection(args.meta.as_ref(), &args.mcp_servers)?;
        let session_id = args.session_id.0.to_string();
        let loading = self
            .session_loads
            .begin(&session_id)
            .map_err(|_| acp::Error::invalid_params().data("session load already in progress"))?;
        let workspace = self.admit_resume(&session_id, &args.cwd).await?;
        let queue = self
            .sessions
            .lock()
            .await
            .get(&session_id)
            .map(|live| Arc::clone(&live.turn_queue));
        let _turn_permit = match queue {
            Some(queue) => Some(queue.acquire_owned().await.ok_or_else(|| {
                acp::Error::invalid_params().data("session operation queue is full")
            })?),
            None => None,
        };
        if let Some(host) = &self.plugin_selection {
            host.validate_workspace(&self.launch.workspace)
                .and_then(|_| host.validate_workspace(&workspace))
                .map_err(|_| {
                    acp::Error::invalid_params().data("native plugin workspace is not admitted")
                })?;
        }
        let components = crate::harness_assemble::components_from_meta(args.meta.as_ref())
            .map_err(|error| acp::Error::invalid_params().data(error))?;
        let current = self.sessions.lock().await.get(&session_id).map(|live| {
            (
                live.model_id.clone(),
                live.effort,
                Arc::clone(&live.session),
            )
        });
        let restore_selection = current.is_none() && self.unattended.is_none();
        let (model_id, mut effort, legacy_goal) = match current {
            Some((model_id, effort, session)) => (
                model_id,
                effort,
                Some(load_legacy_goal_if_needed(&session, &workspace, &session_id).await?),
            ),
            None => {
                let default_model = self.model_id.lock().await.clone();
                let saved_model = args
                    .meta
                    .as_ref()
                    .filter(|_| restore_selection && !self.launch_model_override)
                    .and_then(|meta| meta.get("aster/sessionModelId"))
                    .and_then(serde_json::Value::as_str);
                if let Some(saved_model) = saved_model.filter(|model| !self.known_model(model)) {
                    return Err(acp::Error::invalid_params()
                        .data(format!("saved session model is unavailable: {saved_model}")));
                }
                let model_id = saved_model.unwrap_or(&default_model);
                let effort = if model_id == default_model || self.launch_effort_override {
                    *self.effort.lock().await
                } else {
                    None
                };
                (model_id.to_owned(), effort, None)
            }
        };
        self.refresh_model(&model_id, false).await;
        if restore_selection
            && !self.launch_effort_override
            && args
                .meta
                .as_ref()
                .and_then(|meta| meta.get("reasoningEffortModelId"))
                .and_then(serde_json::Value::as_str)
                == Some(model_id.as_str())
        {
            effort = models::selected_effort(args.meta.as_ref(), effort)?
                .filter(|saved| self.model_supports_effort(&model_id, saved.token));
        }
        let plugin_tools = crate::plugin_tools::entries_from_meta(args.meta.as_ref());
        self.admit_studio_documents(args.meta.as_ref());
        let session_mcp = session_mcp_configs(&args.mcp_servers);
        let mcp_reservation = self
            .sync_session_mcp_servers(Some(session_id.as_ref()), &args.mcp_servers)
            .await?;
        let setup = SessionSetup {
            workspace: workspace.clone(),
            model_id: &model_id,
            effort,
            host,
            components: &components,
            mcp_servers: session_mcp,
        };
        let prepare = async |live| {
            let mut live = self.finish_plugin_tools(live, plugin_tools).await?;
            let goal = match legacy_goal {
                Some(goal) => goal,
                None => load_legacy_goal_if_needed(&live.session, &workspace, &session_id).await?,
            };
            let commands = commands::session_commands(&live.session)?;
            if let Some(posture) = posture {
                live.set_posture(posture, Vec::new())?;
            }
            Ok((live, goal, commands))
        };
        let (modes, models, transcript, goal, available_commands) = {
            let mut sessions = self.sessions.lock().await;
            let (live, goal, commands) = if let Some(current) = sessions.get(&session_id) {
                let prepared = self.replace_core_session(current, setup, prepare).await?;
                session_open::start_workspace_indexing(&prepared.0);
                prepared
            } else {
                drop(sessions);
                let built = self.open_core_session(&session_id, setup, true).await?;
                let session = Arc::clone(&built.session);
                let prepared = match prepare(built).await {
                    Ok(prepared) => prepared,
                    Err(error) => {
                        session.close().await;
                        return Err(error);
                    }
                };
                sessions = self.sessions.lock().await;
                prepared
            };
            let modes = live
                .permissions
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .posture
                .modes();
            let effort_token = live.effort.map(|limits| limits.token);
            let models = self.model_state_with(&live.model_id, effort_token);
            let transcript = Arc::clone(&live.session);
            sessions.insert(session_id.clone(), live);
            (modes, models, transcript, goal, commands)
        };
        drop(mcp_reservation);
        loading.ready();
        let interrupted_state = transcript
            .interrupted_turn_state()
            .unwrap_or(aster_core::fact_control::InterruptedTurnState::Idle);
        let active_goal_resumable = match transcript.goal_definition() {
            Ok(Some(goal)) => {
                goal.state == aster_core::goal_control::GoalState::Active
                    && goal.plan_accepted
                    && !goal.ready_steps().is_empty()
            }
            Ok(None) => goal.as_ref().is_some_and(|legacy| !legacy.paused),
            Err(_) => false,
        };
        if let Some(goal) = goal {
            self.goals.lock().await.insert(session_id.clone(), goal);
        } else {
            self.goals.lock().await.remove(&session_id);
        }
        self.reapply_active_goal(&session_id).await;
        // The pager keeps `isReplay` updates only while this load is in flight
        // (and for a short grace after). Emit before the response returns.
        if let Err(error) = self
            .replay_loaded_transcript(&args.session_id, &transcript)
            .await
        {
            tracing::warn!(%session_id, %error, "session committed but transcript replay delivery failed");
        }
        let config_options = Self::model_config_options(&models);
        self.announce_commands(&session_id, available_commands);
        self.announce_tooling(&session_id, session_tooling_projection(&transcript).await);
        self.announce_session_usage(&session_id, &transcript);
        let mut meta = acp::Meta::new();
        meta.insert(
            "interruptedTurnResumable".into(),
            serde_json::json!(
                interrupted_state == aster_core::fact_control::InterruptedTurnState::Resumable
            ),
        );
        meta.insert(
            "interruptedTurnUnresolvedTool".into(),
            serde_json::json!(
                interrupted_state == aster_core::fact_control::InterruptedTurnState::UnresolvedTool
            ),
        );
        meta.insert(
            "activeGoalResumable".into(),
            serde_json::json!(active_goal_resumable),
        );
        Ok(acp::LoadSessionResponse::new()
            .modes(modes)
            .models(models)
            .config_options(config_options)
            .meta(Some(meta)))
    }

    /// ACP session modes: the Aster permission/mode surface. `plan` enables
    /// maker planning; every other mode runs with the session's configured
    /// planning default.
    async fn set_session_mode(
        &self,
        args: acp::SetSessionModeRequest,
    ) -> acp::Result<acp::SetSessionModeResponse> {
        let session_key = args.session_id.0.to_string();
        let posture = PermissionPosture::parse(args.mode_id.0.as_ref())?;
        self.sessions
            .lock()
            .await
            .get_mut(&session_key)
            .ok_or_else(|| acp::Error::invalid_params().data("unknown session id"))?
            .set_posture(posture, Vec::new())?;
        self.emit_permission_mode(args.session_id, posture).await;
        Ok(acp::SetSessionModeResponse::new())
    }

    async fn set_session_model(
        &self,
        args: acp::SetSessionModelRequest,
    ) -> acp::Result<acp::SetSessionModelResponse> {
        let model_id = args.model_id.0.to_string();
        if !self.known_model(&model_id) {
            return Err(acp::Error::invalid_params().data(format!("unknown model id: {model_id}")));
        }

        let session_key = args.session_id.0.to_string();
        let turn_queue = self
            .sessions
            .lock()
            .await
            .get(&session_key)
            .map(|live| Arc::clone(&live.turn_queue))
            .ok_or_else(|| acp::Error::invalid_params().data("unknown session id"))?;
        let _turn_permit = turn_queue
            .acquire_owned()
            .await
            .ok_or_else(|| acp::Error::invalid_params().data("session operation queue is full"))?;
        let changed = self
            .sessions
            .lock()
            .await
            .get(&session_key)
            .is_some_and(|live| live.model_id != model_id);
        self.refresh_model(&model_id, changed).await;
        let effort;
        // Keep permission updates and replacement in one commit. Queued turns
        // look the session up again after acquiring this same queue.
        {
            let mut sessions = self.sessions.lock().await;
            let current = sessions
                .get(&session_key)
                .ok_or_else(|| acp::Error::invalid_params().data("unknown session id"))?;
            effort = self.select_model_effort(&model_id, args.meta.as_ref(), current.effort)?;
            let live = self
                .replace_core_session(
                    current,
                    SessionSetup {
                        workspace: current.workspace.clone(),
                        model_id: &model_id,
                        effort,
                        host: current.host.clone(),
                        components: &current.components,
                        mcp_servers: current.mcp_servers.clone(),
                    },
                    async |live| {
                        self.finish_plugin_tools(live, current.plugin_tools.clone())
                            .await
                    },
                )
                .await?;
            session_open::start_workspace_indexing(&live);
            sessions.insert(session_key.clone(), live);
        }
        self.reapply_active_goal(&session_key).await;
        *self.model_id.lock().await = model_id;
        *self.effort.lock().await = effort;
        let tooling = {
            let sessions = self.sessions.lock().await;
            sessions
                .get(&session_key)
                .map(|live| Arc::clone(&live.session))
        };
        if let Some(session) = tooling {
            self.announce_tooling(&session_key, session_tooling_projection(&session).await);
        }

        self.emit_model_catalog().await;
        Ok(acp::SetSessionModelResponse::new().meta(Some(
            [(
                "reasoningEffort".into(),
                serde_json::json!(effort.map_or("default", |selection| selection.token)),
            )]
            .into_iter()
            .collect(),
        )))
    }

    async fn set_session_config_option(
        &self,
        args: acp::SetSessionConfigOptionRequest,
    ) -> acp::Result<acp::SetSessionConfigOptionResponse> {
        if args.config_id.0.as_ref() != MODEL_CONFIG_ID {
            return Err(acp::Error::invalid_params().data(format!(
                "unknown config option: {} (expected {MODEL_CONFIG_ID})",
                args.config_id.0
            )));
        }
        let acp::SessionConfigOptionValue::ValueId { value } = args.value else {
            return Err(acp::Error::invalid_params().data("model config option takes a model id"));
        };
        let model_id = value.0.to_string();
        self.set_session_model(
            acp::SetSessionModelRequest::new(args.session_id, acp::ModelId::new(model_id.clone()))
                .meta(args.meta),
        )
        .await?;
        let effort_token = self.effort.lock().await.map(|limits| limits.token);
        let models = self.model_state_with(&model_id, effort_token);
        Ok(acp::SetSessionConfigOptionResponse::new(
            Self::model_config_options(&models),
        ))
    }

    async fn prompt(&self, args: acp::PromptRequest) -> acp::Result<acp::PromptResponse> {
        tracing::info!(target: "perf_probe", "prompt_enter");
        let requested_posture = PermissionPosture::from_meta(args.meta.as_ref())?;
        let send_now = match args.meta.as_ref().and_then(|meta| meta.get("sendNow")) {
            Some(serde_json::Value::Bool(value)) => *value,
            Some(_) => {
                return Err(acp::Error::invalid_params().data("sendNow must be a boolean"));
            }
            None => false,
        };
        let generate_title = match args
            .meta
            .as_ref()
            .and_then(|meta| meta.get("generateTitle"))
        {
            Some(serde_json::Value::Bool(value)) => *value,
            Some(_) => {
                return Err(acp::Error::invalid_params().data("generateTitle must be a boolean"));
            }
            None => false,
        };
        let resume_interrupted = match args
            .meta
            .as_ref()
            .and_then(|meta| meta.get("aster/resumeInterruptedTurn"))
        {
            Some(serde_json::Value::Bool(value)) => *value,
            Some(_) => {
                return Err(acp::Error::invalid_params()
                    .data("aster/resumeInterruptedTurn must be a boolean"));
            }
            None => false,
        };
        let session_key = args.session_id.0.to_string();
        let turn_started = std::time::Instant::now();
        let turn_started_ms = super::emit::unix_now_ms() as i64;
        let turn_queue = self
            .sessions
            .lock()
            .await
            .get(&session_key)
            .map(|live| Arc::clone(&live.turn_queue))
            .ok_or_else(|| acp::Error::invalid_params().data("unknown session id"))?;

        let (prompt, attachments) = Self::prompt_content(&args.prompt)?;
        if !attachments.is_empty() {
            let sessions = self.sessions.lock().await;
            let live = sessions
                .get(&session_key)
                .ok_or_else(|| acp::Error::invalid_params().data("unknown session id"))?;
            if self
                .launch
                .config
                .list_models()
                .iter()
                .any(|(provider, model)| {
                    format!("{}/{}", provider.name, model.id) == live.model_id
                        && !model.modalities.input.is_empty()
                        && !model.modalities.input.iter().any(|input| input == "image")
                })
            {
                return Err(acp::Error::invalid_params()
                    .data("the selected model does not support image input; select an image-capable model"));
            }
        }
        if !attachments.is_empty()
            && (self.unattended.is_some()
                || crate::goal::parse_goal_command(&prompt).is_some()
                || inspection_commands::parse(&prompt).is_some())
        {
            return Err(acp::Error::invalid_params()
                .data("image attachments are supported for ordinary model turns"));
        }
        if prompt.trim().is_empty() && attachments.is_empty() && !resume_interrupted {
            return Ok(acp::PromptResponse::new(acp::StopReason::EndTurn));
        }
        if resume_interrupted && (!attachments.is_empty() || self.unattended.is_some()) {
            return Err(acp::Error::invalid_params()
                .data("aster/resumeInterruptedTurn only supports interactive text turns"));
        }
        let output_schema = match args.meta.as_ref().and_then(|meta| meta.get("outputSchema")) {
            Some(schema) if schema.is_object() => {
                aster_core::validate_output_schema(schema)
                    .map_err(|error| acp::Error::invalid_params().data(error.to_string()))?;
                Some(schema.clone())
            }
            Some(_) => {
                return Err(
                    acp::Error::invalid_params().data("outputSchema must be a JSON Schema object")
                );
            }
            None => None,
        };
        if output_schema.is_some()
            && (self.unattended.is_some()
                || crate::goal::parse_goal_command(&prompt).is_some()
                || inspection_commands::parse(&prompt).is_some())
        {
            return Err(acp::Error::invalid_params()
                .data("outputSchema is supported only for ordinary model turns"));
        }
        if let Err(message) = configured_model_id(&self.model_id.lock().await) {
            return Err(acp::Error::invalid_params().data(message));
        }
        // Reserve the queue slot before cancelling an active turn for `sendNow`,
        // so a prompt rejected by MAX_QUEUED_PROMPTS never disrupts the running turn.
        // Owned permit so a cancelled turn can move the guard into a detached join.
        let slot = match turn_queue.reserve() {
            Some(slot) => slot,
            None => {
                return Err(acp::Error::invalid_params().data(format!(
                    "prompt queue is full ({MAX_QUEUED_PROMPTS} waiting behind the active \
                     turn); the session is still working through queued prompts"
                )));
            }
        };
        if send_now {
            self.cancel(acp::CancelNotification::new(args.session_id.clone()))
                .await?;
        }
        let turn_permit = turn_queue.lock_reserved(slot).await;
        let session = {
            let mut sessions = self.sessions.lock().await;
            let live = sessions
                .get_mut(&session_key)
                .ok_or_else(|| acp::Error::invalid_params().data("unknown session id"))?;
            if let Some(posture) = requested_posture {
                live.set_posture(posture, Vec::new())?;
            }
            Arc::clone(&live.session)
        };
        // Echo the client's prompt id on every notification (the ACP
        // prompt-ack contract): the pager disarms its ack watch on the
        // first update naming the prompt.
        let client_prompt_id = args
            .meta
            .as_ref()
            .and_then(|meta| meta.get("promptId"))
            .and_then(|v| v.as_str())
            .map(str::to_string);
        let generation = {
            let mut generations = self.turn_generation.lock().await;
            let slot = generations.entry(session_key.clone()).or_default();
            slot.number = slot.number.saturating_add(1);
            slot.prompt_id = client_prompt_id.clone();
            slot.number
        };
        if let Some(live) = self.sessions.lock().await.get(&session_key) {
            *live
                .turn_started
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(turn_started);
            *live
                .turn_started_ms
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(turn_started_ms);
        }
        let notify_result: acp::Result<()> = async {
            let client = self.client.borrow().clone();
            if let (Some(prompt_id), Some(client)) = (client_prompt_id.as_deref(), client) {
                // The pager adopts a queued turn from the running queue identity
                // before accepting its deltas; the prompt response alone is too late.
                let params = serde_json::value::to_raw_value(&serde_json::json!({
                    "sessionId": session_key,
                    "runningPromptId": prompt_id,
                    "runningText": prompt,
                    "runningKind": "prompt",
                }))
                .map_err(|error| acp::Error::internal_error().data(error.to_string()))?;
                client
                    .ext_notification(acp::ExtNotification::new(
                        "aster/queue/changed",
                        params.into(),
                    ))
                    .await?;
            }
            Ok(())
        }
        .await;
        if let Err(error) = notify_result {
            let result = self
                .finish_prompt(
                    &session_key,
                    generation,
                    client_prompt_id.as_deref(),
                    Err(error),
                )
                .await;
            drop(turn_permit);
            return result;
        }
        // After queue identity: unlock Thinking / streamStartMs before hooks/LLM.
        // Must not precede `_aster/queue/changed` (ACP `queue-running-identity`).
        self.emit_turn_started_chrome(&args.session_id).await;
        if let Some(posture) = requested_posture {
            self.emit_permission_mode(args.session_id.clone(), posture)
                .await;
        }
        if let Some(unattended) = &self.unattended {
            // A task that starts with `/goal` is task text, not a goal command.
            let response = self
                .unattended_prompt(
                    &args.session_id,
                    &session,
                    &prompt,
                    generation,
                    turn_permit,
                    unattended,
                )
                .await;
            return self
                .finish_prompt(
                    &session_key,
                    generation,
                    client_prompt_id.as_deref(),
                    response,
                )
                .await;
        }
        let turn = crate::host_context::turn_context_from_meta(args.meta.as_ref());
        let observation = crate::run_observation::Binding::from_meta(args.meta.as_ref());
        let (mut response, held_permit) = if resume_interrupted {
            if output_schema.is_some() {
                drop(turn_permit);
                return Err(acp::Error::invalid_params()
                    .data("aster/resumeInterruptedTurn does not accept outputSchema"));
            }
            let (retry_observer, retry_forwarder) = self.provider_retry_forwarder(&args.session_id);
            let (response, returned_permit) = aster_core::llm::with_retry_observer(
                retry_observer,
                self.stream_turn(
                    &args.session_id,
                    &session,
                    "",
                    &[],
                    generation,
                    turn_permit,
                    observation.as_ref(),
                    None,
                    requested_posture,
                    None,
                    true,
                ),
            )
            .await;
            let _ = retry_forwarder.await;
            (response, returned_permit)
        } else if let Some(op) = crate::goal::parse_goal_command(&prompt) {
            tracing::info!(target: "perf_probe", "goal_command_branch");
            (
                self.handle_goal(
                    &args.session_id,
                    &session,
                    op,
                    generation,
                    turn_permit,
                    observation.as_ref(),
                )
                .await,
                None,
            )
        } else if let Some(command) = inspection_commands::parse(&prompt) {
            let result = self.handle_inspection_command(&session, command).await;
            let response = match result {
                Ok(text) => {
                    self.emit_text(&args.session_id, &text, false).await;
                    Ok(acp::PromptResponse::new(acp::StopReason::EndTurn))
                }
                Err(error) => Err(error),
            };
            (response, Some(turn_permit))
        } else {
            let prepared: acp::Result<(String, Option<ResearchGate>)> = async {
                let goal = if session
                    .goal_definition()
                    .map_err(|error| acp::Error::invalid_params().data(error.to_string()))?
                    .is_some()
                {
                    None
                } else {
                    crate::goal::apply_session_goal(&session, crate::goal::GoalOp::Status)
                        .await
                        .map_err(|error| acp::Error::invalid_params().data(error.to_string()))?
                        .next
                };
                {
                    let mut goals = self.goals.lock().await;
                    if let Some(goal) = &goal {
                        goals.insert(session_key.clone(), goal.clone());
                    } else {
                        goals.remove(&session_key);
                    }
                }
                let prompt = crate::goal::standing_goal_prompt(goal.as_ref(), &prompt);
                let prompt = crate::host_context::apply_turn_directives(&prompt, &turn);
                let gate = if turn.research || turn.plan_first {
                    let sessions = self.sessions.lock().await;
                    let live = sessions
                        .get(&session_key)
                        .ok_or_else(|| acp::Error::invalid_params().data("unknown session id"))?;
                    Some(ResearchGate::enter(live)?)
                } else {
                    None
                };
                Ok((prompt, gate))
            }
            .await;
            match prepared {
                Ok((prompt, gate)) => {
                    let (retry_observer, retry_forwarder) =
                        self.provider_retry_forwarder(&args.session_id);
                    let (response, returned_permit) = aster_core::llm::with_retry_observer(
                        retry_observer,
                        self.stream_turn(
                            &args.session_id,
                            &session,
                            &prompt,
                            &attachments,
                            generation,
                            turn_permit,
                            observation.as_ref(),
                            gate,
                            requested_posture,
                            output_schema.as_ref(),
                            false,
                        ),
                    )
                    .await;
                    let _ = retry_forwarder.await;
                    (response, returned_permit)
                }
                Err(error) => (Err(error), Some(turn_permit)),
            }
        };
        if generate_title && output_schema.is_none() {
            match &mut response {
                Ok(result) if result.stop_reason == acp::StopReason::EndTurn => {
                    if let Some(title) = session.generate_session_title(&prompt).await {
                        result
                            .meta
                            .get_or_insert_with(acp::Meta::new)
                            .insert("sessionTitle".into(), title.into());
                    }
                }
                _ => {}
            }
        }
        self.emit_xai_session_update(
            &args.session_id,
            serde_json::json!({
                "sessionUpdate":"model_info",
                "sessionTotalTokens":session.recorded_usage_tokens().2
            }),
        )
        .await;
        tracing::info!(target: "perf_probe", "prompt_return");
        let finished = self
            .finish_prompt(
                &session_key,
                generation,
                client_prompt_id.as_deref(),
                response,
            )
            .await;
        drop(held_permit);
        finished
    }

    async fn ext_method(&self, args: acp::ExtRequest) -> acp::Result<acp::ExtResponse> {
        match HostMethod::parse(args.method.as_ref()) {
            Some(method) => self.answer_host_method(method, &args).await,
            None => Err(acp::Error::method_not_found().data(args.method.to_string())),
        }
    }

    async fn cancel(&self, args: acp::CancelNotification) -> acp::Result<()> {
        let session_key = args.session_id.0.to_string();
        let prompt_id = match args.meta.as_ref().and_then(|meta| meta.get("promptId")) {
            None => None,
            Some(serde_json::Value::String(id)) if !id.is_empty() => Some(id.as_str()),
            Some(_) => {
                return Err(
                    acp::Error::invalid_params().data("cancel promptId must be a nonempty string")
                );
            }
        };
        if prompt_id.is_none() {
            if let Some(compaction) = self.manual_compactions.borrow().get(&session_key) {
                compaction.cancel();
                return Ok(());
            }
        }
        let generation = {
            let generations = self.turn_generation.lock().await;
            let current = generations.get(&session_key);
            if prompt_id
                .is_some_and(|id| current.and_then(|turn| turn.prompt_id.as_deref()) != Some(id))
            {
                return Ok(());
            }
            current.map(|turn| turn.number).unwrap_or(0)
        };
        self.cancelled_generation
            .lock()
            .await
            .insert(session_key.clone(), generation);
        self.turn_cancel.notify_waiters();
        let session = {
            let sessions = self.sessions.lock().await;
            sessions.get(&session_key).map(|live| live.session.clone())
        };
        if let Some(session) = session {
            let run = session.current_run().await;
            if self
                .turn_generation
                .lock()
                .await
                .get(&session_key)
                .map(|turn| turn.number)
                .unwrap_or(0)
                != generation
            {
                return Ok(());
            }
            if let Some(run) = run {
                run.cancel().await;
            }
            let cancel_subagents = args
                .meta
                .as_ref()
                .and_then(|meta| meta.get("cancelSubagents"))
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(true);
            if cancel_subagents {
                for task in session.pending_subagent_tasks().await {
                    session.cancel_subagent_task(&task.task_id).await;
                }
            }
        }
        Ok(())
    }
}

/// Initialize meta map: each governed switch id → enabled (Removed is false).
fn feature_switch_admission_meta() -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for spec in aster_core::capability::FEATURE_SWITCHES {
        let enabled = aster_core::capability::feature_switch_enabled(spec.id).unwrap_or(false);
        map.insert(spec.id.to_string(), serde_json::Value::Bool(enabled));
    }
    serde_json::Value::Object(map)
}

/// Initialize meta map: each governed switch id → stage string.
fn feature_switch_stage_meta() -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for spec in aster_core::capability::FEATURE_SWITCHES {
        map.insert(
            spec.id.to_string(),
            serde_json::Value::String(spec.stage.as_str().to_string()),
        );
    }
    serde_json::Value::Object(map)
}
