use super::*;

#[async_trait::async_trait(?Send)]
impl acp::Agent for A3sCodeAgent {
    async fn initialize(
        &self,
        args: acp::InitializeRequest,
    ) -> acp::Result<acp::InitializeResponse> {
        let rules = initialize_rules_from_meta(args.meta.as_ref())?;
        let permissions = ClientPermissionRules::from_meta(args.meta.as_ref())?;
        *self.client_rules.borrow_mut() = rules;
        *self.client_permissions.borrow_mut() = permissions;
        self.client_reads_files
            .set(args.client_capabilities.fs.read_text_file);
        let cwd = self.launch.workspace.to_string_lossy().to_string();
        let current = self.model_id.lock().await.clone();
        let effort_token = self.effort.lock().await.map(|limits| limits.token);
        let model_state = self.model_state_with(&current, effort_token);
        Ok(acp::InitializeResponse::new(acp::ProtocolVersion::V1)
            .agent_capabilities(
                acp::AgentCapabilities::new()
                    .load_session(true)
                    .prompt_capabilities(acp::PromptCapabilities::new().image(true)),
            )
            .auth_methods(Vec::new())
            .meta(
                serde_json::json!({
                    "grokShell": false,
                    "a3sCode": true,
                    "agentVersion": env!("CARGO_PKG_VERSION"),
                    "currentWorkingDirectory": cwd,
                    "modelState": model_state,
                    // Legacy initialize extension: global ACP handlers only. Core
                    // commands are session-specific and arrive after new/load.
                    "availableCommands": commands::acp_commands(),
                    "cancelRewind": false,
                    "sessionRecap": false,
                    "feedbackTraceOffer": false,
                    "a3s": self.workspace_admission_meta(),
                    "openModelConfig": self.first_run_config,
                })
                .as_object()
                .cloned(),
            ))
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
        let posture = PermissionPosture::from_meta(args.meta.as_ref())?;
        let mut host = crate::host_context::studio_host_from_meta(args.meta.as_ref())
            .map_err(|error| acp::Error::invalid_params().data(error))?;
        if !args
            .meta
            .as_ref()
            .is_some_and(|meta| meta.contains_key("a3sEngineering"))
        {
            host.work_policy = Some(
                crate::engineering::named_policy("engineering-v1")
                    .map_err(|error| acp::Error::internal_error().data(error))?,
            );
        }
        self.validate_plugin_selection(args.meta.as_ref(), &args.mcp_servers)?;
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
                Some(a3s_code_core::session_folder::SessionKind::Interactive)
            }
            Some(serde_json::Value::String(kind)) if kind == "headless" => {
                Some(a3s_code_core::session_folder::SessionKind::Headless)
            }
            Some(_) => {
                return Err(acp::Error::invalid_params().data("unsupported session kind"));
            }
        };
        match session_kind {
            Some(kind) => a3s_code_core::session_folder::prepare_session_home_with_kind(
                &workspace,
                &session_id,
                kind,
            ),
            None => a3s_code_core::session_folder::prepare_session_home(&workspace, &session_id),
        }
        .map_err(|error| acp::Error::internal_error().data(error.to_string()))?;
        let session_mcp = session_mcp_configs(&args.mcp_servers);
        self.sync_session_mcp_servers(Some(session_id.as_ref()), &args.mcp_servers)
            .await?;
        let model_id = self.model_id.lock().await.clone();
        let effort = *self.effort.lock().await;
        let plugin_tools = crate::plugin_tools::entries_from_meta(args.meta.as_ref());
        let components = crate::harness_assemble::components_from_meta(args.meta.as_ref())
            .map_err(|error| acp::Error::invalid_params().data(error))?;
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
        let effort_token = self.effort.lock().await.map(|limits| limits.token);
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
        // A session may be resumed before its first turn triggers auto-save.
        if let Err(error) = live.session.save().await {
            live.session.close().await;
            return Err(acp::Error::internal_error().data(error.to_string()));
        }
        self.sessions.lock().await.insert(session_id.clone(), live);
        if let Some(goal) = goal {
            self.goals.lock().await.insert(session_id.clone(), goal);
        } else {
            self.goals.lock().await.remove(&session_id);
        }
        self.reapply_active_goal(&session_id).await;

        let config_options = Self::model_config_options(&models);
        self.announce_commands(&session_id, available_commands);
        self.announce_tooling(&session_id, tooling);
        Ok(
            acp::NewSessionResponse::new(acp::SessionId::new(session_id))
                .modes(modes)
                .models(models)
                .config_options(config_options),
        )
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
        let loading = self.session_loads.begin(&session_id);
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
        self.admit_studio_documents(args.meta.as_ref());
        let session_mcp = session_mcp_configs(&args.mcp_servers);
        self.sync_session_mcp_servers(Some(session_id.as_ref()), &args.mcp_servers)
            .await?;
        let current_model = self
            .sessions
            .lock()
            .await
            .get(&session_id)
            .map(|live| live.model_id.clone());
        let model_id = match current_model {
            Some(model) => model,
            None => self.model_id.lock().await.clone(),
        };
        let effort = *self.effort.lock().await;
        let plugin_tools = crate::plugin_tools::entries_from_meta(args.meta.as_ref());
        let setup = SessionSetup {
            workspace: workspace.clone(),
            model_id: &model_id,
            effort,
            host,
            components: &components,
            mcp_servers: session_mcp,
        };
        let live = {
            let sessions = self.sessions.lock().await;
            if let Some(current) = sessions.get(&session_id) {
                self.replace_core_session(current, setup).await?
            } else {
                drop(sessions);
                self.open_core_session(&session_id, setup, true).await?
            }
        };
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
        let effort_token = self.effort.lock().await.map(|limits| limits.token);
        let models = self.model_state_with(&live.model_id, effort_token);
        let transcript = Arc::clone(&live.session);
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
        self.sessions.lock().await.insert(session_id.clone(), live);
        loading.ready();
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
            self.sessions.lock().await.remove(&session_id);
            transcript.close().await;
            return Err(error);
        }
        let config_options = Self::model_config_options(&models);
        self.announce_commands(&session_id, available_commands);
        self.announce_tooling(&session_id, tooling);
        Ok(acp::LoadSessionResponse::new()
            .modes(modes)
            .models(models)
            .config_options(config_options))
    }

    /// ACP session modes: the A3S permission/mode surface. `plan` enables
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

        let effort = models::selected_effort(args.meta.as_ref(), *self.effort.lock().await)?;

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
        // Keep permission updates and replacement in one commit. Queued turns
        // look the session up again after acquiring this same queue.
        {
            let mut sessions = self.sessions.lock().await;
            let current = sessions
                .get(&session_key)
                .ok_or_else(|| acp::Error::invalid_params().data("unknown session id"))?;
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
                )
                .await?;
            let live = self
                .finish_plugin_tools(live, current.plugin_tools.clone())
                .await?;
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

        Ok(acp::SetSessionModelResponse::new())
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
        let session_key = args.session_id.0.to_string();
        let turn_started = std::time::Instant::now();
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
        if prompt.trim().is_empty() && attachments.is_empty() {
            return Ok(acp::PromptResponse::new(acp::StopReason::EndTurn));
        }
        let output_schema = match args.meta.as_ref().and_then(|meta| meta.get("outputSchema")) {
            Some(schema) if schema.is_object() => {
                a3s_code_core::validate_output_schema(schema)
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
        if let Err(message) = configured_model_id(&self.model_id.lock().await) {
            return Err(acp::Error::invalid_params().data(message));
        }
        if send_now {
            self.cancel(acp::CancelNotification::new(args.session_id.clone()))
                .await?;
        }
        // Queue behind an active turn instead of failing: the pager's
        // send-now / queue-reissue expect the agent to accept and serialize.
        // Owned so a cancelled turn can move the guard into a detached join.
        let turn_permit = match turn_queue.acquire_owned().await {
            Some(permit) => permit,
            None => {
                return Err(acp::Error::invalid_params().data(format!(
                    "prompt queue is full ({MAX_QUEUED_PROMPTS} waiting behind the active \
                     turn); the session is still working through queued prompts"
                )));
            }
        };
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
        let generation = {
            let mut generations = self.turn_generation.lock().await;
            let slot = generations.entry(session_key.clone()).or_insert(0);
            *slot = slot.saturating_add(1);
            *slot
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
        if let Some(live) = self.sessions.lock().await.get(&session_key) {
            *live
                .turn_started
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(turn_started);
            *live
                .active_prompt_id
                .lock()
                .unwrap_or_else(|p| p.into_inner()) = client_prompt_id.clone();
        }
        if send_now {
            let client = self.client.borrow().clone();
            if let (Some(prompt_id), Some(client)) = (client_prompt_id.as_deref(), client) {
                // The pager adopts a send-now turn from the running queue identity
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
                        "a3s/queue/changed",
                        params.into(),
                    ))
                    .await?;
            }
        }
        if let Some(posture) = requested_posture {
            self.emit_permission_mode(args.session_id.clone(), posture)
                .await;
        }
        if output_schema.is_some()
            && (self.unattended.is_some()
                || crate::goal::parse_goal_command(&prompt).is_some()
                || inspection_commands::parse(&prompt).is_some())
        {
            return Err(acp::Error::invalid_params()
                .data("outputSchema is supported only for ordinary model turns"));
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
            return self.finish_prompt(&session_key, generation, response).await;
        }
        let turn = crate::host_context::turn_context_from_meta(args.meta.as_ref());
        let observation = crate::run_observation::Binding::from_meta(args.meta.as_ref());
        let mut response = if let Some(op) = crate::goal::parse_goal_command(&prompt) {
            tracing::info!(target: "perf_probe", "goal_command_branch");
            self.handle_goal(
                &args.session_id,
                &session,
                op,
                generation,
                turn_permit,
                observation.as_ref(),
            )
            .await
        } else if let Some(command) = inspection_commands::parse(&prompt) {
            let result = self.handle_inspection_command(&session, command).await;
            let response = match result {
                Ok(text) => {
                    self.emit_text(&args.session_id, &text).await;
                    Ok(acp::PromptResponse::new(acp::StopReason::EndTurn))
                }
                Err(error) => Err(error),
            };
            drop(turn_permit);
            response
        } else {
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
            )
            .await
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
        tracing::info!(target: "perf_probe", "prompt_return");
        self.finish_prompt(&session_key, generation, response).await
    }

    async fn ext_method(&self, args: acp::ExtRequest) -> acp::Result<acp::ExtResponse> {
        match HostMethod::parse(args.method.as_ref()) {
            Some(method) => self.answer_host_method(method, &args).await,
            None => Err(acp::Error::method_not_found().data(args.method.to_string())),
        }
    }

    async fn cancel(&self, args: acp::CancelNotification) -> acp::Result<()> {
        let session_key = args.session_id.0.to_string();
        let generation = self
            .turn_generation
            .lock()
            .await
            .get(&session_key)
            .copied()
            .unwrap_or(0);
        self.cancelled_generation
            .lock()
            .await
            .insert(session_key.clone(), generation);
        let session = {
            let sessions = self.sessions.lock().await;
            sessions.get(&session_key).map(|live| live.session.clone())
        };
        if let Some(session) = session {
            let _ = session.cancel().await;
        }
        self.turn_cancel.notify_waiters();
        Ok(())
    }
}
