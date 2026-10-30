//! Running one prompt turn: streaming core events, draining, and settling.

use super::*;

impl A3sCodeAgent {
    pub(super) fn prompt_text(prompt: &[acp::ContentBlock]) -> acp::Result<String> {
        let unsupported = |kind: &str| {
            acp::Error::invalid_params().data(format!(
                "unsupported prompt content block `{kind}`; this agent accepts text only and did not send the request to the model"
            ))
        };
        let mut text = Vec::new();
        for block in prompt {
            match block {
                acp::ContentBlock::Text(content) => {
                    if content
                        .meta
                        .as_ref()
                        .is_some_and(|meta| meta.contains_key("bash_command"))
                    {
                        return Err(acp::Error::invalid_params()
                            .data("Direct shell commands are unavailable with the Code backend."));
                    }
                    text.push(content.text.as_str());
                }
                acp::ContentBlock::Image(_) => return Err(unsupported("image")),
                acp::ContentBlock::Audio(_) => return Err(unsupported("audio")),
                acp::ContentBlock::Resource(_) => return Err(unsupported("resource")),
                acp::ContentBlock::ResourceLink(_) => return Err(unsupported("resource_link")),
                _ => return Err(unsupported("unknown")),
            }
        }
        Ok(text.join("\n"))
    }

    pub(super) async fn handle_event(
        &self,
        session_id: &acp::SessionId,
        session: &AgentSession,
        state: &mut TurnState,
        event: AgentEvent,
    ) {
        let observation = state
            .observer
            .as_mut()
            .and_then(|observer| observer.event(&event));
        self.emit_run_observation(observation).await;
        match event {
            AgentEvent::TextDelta { text } => {
                if !text.is_empty() {
                    state.saw_text = true;
                }
                state.delta_text.push_str(&text);
                self.emit_text(session_id, &text).await;
            }
            AgentEvent::ReasoningDelta { text } => {
                self.emit_thought(session_id, &text).await;
            }
            AgentEvent::ToolStart { .. } => {
                // Streaming and the fact runner both announce a call before its
                // arguments exist. Painting that announcement opens a second
                // Run row for one execution, and it also paints a twin that
                // the same-response collapse never runs.
            }
            AgentEvent::ToolExecutionStart {
                id,
                name,
                args: tool_args,
            } => match note_tool_row(&mut state.painted_tools, &id) {
                ToolRowPaint::Open => {
                    self.emit_tool_call(session_id, &id, &name, Some(&tool_args))
                        .await;
                }
                ToolRowPaint::Update => {
                    self.emit_tool_call_started(session_id, &id, &name, &tool_args)
                        .await;
                }
            },
            AgentEvent::ToolOutputDelta { id, name, delta } => {
                self.emit_tool_call_output_delta(session_id, &id, &name, &delta)
                    .await;
            }
            AgentEvent::ToolEnd {
                id,
                name,
                args: tool_args,
                output,
                exit_code,
                metadata,
                ..
            } => {
                let same_response_duplicate = metadata
                    .as_ref()
                    .and_then(|value| value.get("guard"))
                    .and_then(|value| value.as_str())
                    == Some("same_response_duplicate");
                if state.painted_tools.contains(&id) || !same_response_duplicate {
                    if state.painted_tools.insert(id.clone()) {
                        self.emit_tool_call(session_id, &id, &name, tool_args.as_ref())
                            .await;
                    }
                    self.emit_tool_call_completed(
                        session_id,
                        &id,
                        &name,
                        tool_args.as_ref(),
                        &output,
                        exit_code,
                        metadata.as_ref(),
                    )
                    .await;
                }
            }
            AgentEvent::ConfirmationRequired {
                tool_id,
                tool_name,
                args: tool_args,
                ..
            } => {
                // Yolo still parks bash, then auto-approves. That announcement
                // is the same id as ToolExecutionStart. Record it here or the
                // pager opens a second Run row for one fact.
                match note_tool_row(&mut state.painted_tools, &tool_id) {
                    ToolRowPaint::Open => {
                        self.emit_tool_call(session_id, &tool_id, &tool_name, Some(&tool_args))
                            .await;
                    }
                    ToolRowPaint::Update => {
                        self.emit_tool_call_started(session_id, &tool_id, &tool_name, &tool_args)
                            .await;
                    }
                }
                let approved = self
                    .request_tool_permission(
                        session_id,
                        &tool_id,
                        &tool_name,
                        &tool_args,
                        session.workspace(),
                    )
                    .await;
                let reason = if approved {
                    Some("approved via ACP request_permission".into())
                } else {
                    Some("rejected via ACP request_permission".into())
                };
                if let Err(error) = session.confirm_tool_use(&tool_id, approved, reason).await {
                    self.emit_text(
                        session_id,
                        &format!("confirm {tool_name} failed: {error}\n"),
                    )
                    .await;
                }
            }
            AgentEvent::TaskUpdated { tasks, .. } => {
                self.emit_plan(session_id, &tasks).await;
            }
            AgentEvent::PlanningEnd { plan, .. } => {
                self.emit_plan(session_id, &plan.steps).await;
            }
            AgentEvent::SubagentStart {
                task_id,
                session_id: child_session_id,
                parent_session_id,
                agent,
                description,
                started_ms,
            } => {
                state
                    .subagent_started_ms
                    .insert(task_id.clone(), started_ms);
                self.emit_subagent_start(
                    session_id,
                    &task_id,
                    &child_session_id,
                    &parent_session_id,
                    &agent,
                    &description,
                )
                .await;
            }
            AgentEvent::SubagentProgress {
                task_id,
                session_id: child_session_id,
                status,
                metadata,
            } => {
                let started_ms = state
                    .subagent_started_ms
                    .get(&task_id)
                    .copied()
                    .unwrap_or(0);
                let progress = state.child_progress.entry(task_id.clone()).or_default();
                note_child_progress(progress, &status, &metadata);
                self.emit_subagent_progress(
                    session_id,
                    &task_id,
                    &child_session_id,
                    &status,
                    session.context_window_tokens() as u64,
                    &metadata,
                    started_ms,
                    progress,
                )
                .await;
            }
            AgentEvent::SubagentEnd {
                task_id,
                session_id: child_session_id,
                output,
                success,
                finished_ms,
                ..
            } => {
                let started_ms = state
                    .subagent_started_ms
                    .remove(&task_id)
                    .unwrap_or(finished_ms);
                let progress = state.child_progress.remove(&task_id).unwrap_or_default();
                self.emit_subagent_end(
                    session_id,
                    &task_id,
                    &child_session_id,
                    success,
                    &output,
                    started_ms,
                    finished_ms,
                    &progress,
                )
                .await;
            }
            AgentEvent::End {
                text,
                verification_summary,
                completion_terminal,
                forced_stop,
                ..
            } => {
                state.end_text = Some(text.clone());
                state.end_outcome = Some(turn_outcome_payload(
                    &completion_terminal,
                    &verification_summary,
                    forced_stop.as_ref(),
                ));
                state.stop_reason = forced_stop.map(|reason| match reason {
                    a3s_code_core::harness_loop::ForcedStop::ResponseTruncated => {
                        acp::StopReason::MaxTokens
                    }
                    a3s_code_core::harness_loop::ForcedStop::ToolBudget
                    | a3s_code_core::harness_loop::ForcedStop::ReadOnlyEvidence => {
                        acp::StopReason::MaxTurnRequests
                    }
                });
                if should_emit_end_text(state.saw_text, &text) {
                    self.emit_text(session_id, &text).await;
                }
            }
            AgentEvent::Error { message } => {
                state.turn_failure = Some(message);
            }
            AgentEvent::ToolEffectUnresolved { error } => {
                state.turn_failure = Some(error.to_string());
                state.reconciliation = Some(error);
            }
            AgentEvent::ModelUsageBound { snapshot } => {
                let used = snapshot.reported_prompt_tokens as u64;
                let size = session.context_window_tokens() as u64;
                if used > 0 && size > 0 {
                    let update = usage_update(&snapshot, size, &session.durable_transcript());
                    self.emit(acp::SessionNotification::new(
                        session_id.clone(),
                        acp::SessionUpdate::UsageUpdate(update),
                    ))
                    .await;
                }
            }
            AgentEvent::AutoCompact {
                phase,
                tokens_used,
                context_window,
                tokens_after,
            } => {
                let (_, percentage) = context_usage(context_window, tokens_used);
                let update = if phase == "started" {
                    serde_json::json!({
                        "sessionUpdate": "auto_compact_started",
                        "tokens_used": tokens_used,
                        "context_window": context_window,
                        "percentage": percentage,
                        "reason": "threshold",
                    })
                } else {
                    serde_json::json!({
                        "sessionUpdate": "auto_compact_completed",
                        "tokens_before": tokens_used,
                        "tokens_after": tokens_after,
                    })
                };
                self.emit_xai_session_update(session_id, update).await;
            }
            AgentEvent::PermissionDenied {
                tool_id,
                tool_name,
                args: tool_args,
                reason,
            } => {
                self.emit_tool_call(session_id, &tool_id, &tool_name, Some(&tool_args))
                    .await;
                self.emit_tool_call_completed(
                    session_id,
                    &tool_id,
                    &tool_name,
                    Some(&tool_args),
                    &reason,
                    1,
                    None,
                )
                .await;
            }
            AgentEvent::UserQuestion {
                question_id,
                question,
                options,
                allow_free_text,
                form,
            } => {
                if !claim_question_prompt(&mut state.prompted_questions, &question_id) {
                    return;
                }
                if self.unattended.is_some() {
                    let answer = match form {
                        Some(_) => serde_json::Value::Null.to_string(),
                        None => crate::unattended::QUESTION_ANSWER.to_string(),
                    };
                    let _ = a3s_code_core::ask_user::answer(&question_id, &answer);
                    return;
                }
                if let Some(form) = form.as_deref() {
                    // Closed-catalog form: the client renders it and answers
                    // with the submitted values JSON.
                    let client = self.client.borrow().clone();
                    let values = match client {
                        Some(client) => {
                            crate::form_request::request_form_in_session(
                                &client,
                                session_id.0.as_ref(),
                                &question_id,
                                &question,
                                form,
                            )
                            .await
                        }
                        None => Err("form client is unavailable".into()),
                    };
                    let answer = values.unwrap_or_else(|message| {
                        serde_json::json!([
                            a3s_code_core::ask_user::USER_FORM_ERROR_SCHEMA,
                            message
                        ])
                    });
                    let _ = a3s_code_core::ask_user::answer(&question_id, &answer.to_string());
                    return;
                }
                if !question_has_choices(&options) {
                    self.emit_text(
                        session_id,
                        &format!("\nquestion {question_id}: {question}\n"),
                    )
                    .await;
                }
                if let Some(text) = self
                    .request_question_answer(
                        session_id,
                        &question_id,
                        &question,
                        &options,
                        allow_free_text,
                    )
                    .await
                {
                    let _ = a3s_code_core::ask_user::answer(&question_id, &text);
                } else {
                    // A dismissed question has no answer to wake the core's
                    // question wait. Cancel its owned turn instead of parking it.
                    let _ =
                        acp::Agent::cancel(self, acp::CancelNotification::new(session_id.clone()))
                            .await;
                }
            }
            AgentEvent::GoalAchieved {
                goal,
                total_steps,
                duration_ms,
            } => {
                state.achieved_goal = Some((goal, total_steps, duration_ms));
            }
            _ => {}
        }
    }

    pub(super) async fn stream_turn(
        &self,
        session_id: &acp::SessionId,
        session: &AgentSession,
        prompt: &str,
        generation: u64,
        turn_permit: OwnedMutexGuard<()>,
        observation: Option<&crate::run_observation::Binding>,
        mut research_gate: Option<ResearchGate>,
        requested_posture: Option<PermissionPosture>,
    ) -> acp::Result<acp::PromptResponse> {
        let governing_goal = self.goals.lock().await.get(session_id.0.as_ref()).cloned();
        // Keep a mode update from overtaking the policy captured for this prompt.
        let mut sessions = self.sessions.lock().await;
        if let Some(posture) = requested_posture {
            sessions
                .get_mut(session_id.0.as_ref())
                .ok_or_else(|| acp::Error::invalid_params().data("unknown session id"))?
                .set_posture(posture, Vec::new())?;
        }
        let admitted = session.stream_with_run(prompt, None).await;
        drop(sessions);
        // The run owns its frozen policy now. Restore before draining events
        // or releasing the turn permit so the next queued turn sees its mode.
        if let Some(gate) = research_gate.as_mut() {
            gate.restore()?;
        }
        if let Some(posture) = requested_posture {
            self.emit_permission_mode(session_id.clone(), posture).await;
        }
        let stream = match admitted {
            Ok(parts) => parts,
            Err(error) => {
                drop(turn_permit);
                return Err(acp::Error::internal_error().data(error.to_string()));
            }
        };
        self.drain_turn(
            session_id,
            session,
            stream,
            governing_goal,
            generation,
            turn_permit,
            observation,
        )
        .await
    }

    /// Render one admitted Run's events, answer its confirmations and honour
    /// `cancel`, exactly as for a prompt turn.
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn drain_turn(
        &self,
        session_id: &acp::SessionId,
        session: &AgentSession,
        stream: a3s_code_core::AgentRunStream,
        governing_goal: Option<crate::goal::DurableGoal>,
        generation: u64,
        turn_permit: OwnedMutexGuard<()>,
        observation: Option<&crate::run_observation::Binding>,
    ) -> acp::Result<acp::PromptResponse> {
        let mut observer = stream.run.and_then(|run| {
            observation.map(|binding| {
                crate::run_observation::Observer::new(run, session_id.0.to_string(), binding)
            })
        });
        self.emit_run_observation(observer.as_mut().and_then(|observer| observer.opened()))
            .await;
        let mut rx = stream.events;
        let mut join = stream.worker;

        let mut state = TurnState {
            observer,
            turn_failure: None,
            reconciliation: None,
            saw_text: false,
            delta_text: String::new(),
            end_text: None,
            end_outcome: None,
            stop_reason: None,
            achieved_goal: None,
            subagent_started_ms: HashMap::new(),
            child_progress: HashMap::new(),
            prompted_questions: HashSet::new(),
            painted_tools: HashSet::new(),
        };
        let session_key = session_id.0.to_string();
        let drain_started = std::time::Instant::now();
        loop {
            // Register before the map read. `notify_waiters` stores no permit.
            let notified = self.turn_cancel.notified();
            if self.turn_was_cancelled(&session_key, generation).await {
                return self
                    .leave_cancelled_turn(
                        session_id,
                        session,
                        &mut state,
                        &mut rx,
                        join,
                        turn_permit,
                    )
                    .await;
            }
            tokio::select! {
                biased;
                _ = notified => continue,
                event = rx.recv() => {
                    match event {
                        Some(event) => {
                            tokio::select! {
                                biased;
                                _ = self.wait_for_turn_cancel(&session_key, generation) => {},
                                _ = self.handle_event(session_id, session, &mut state, event) => {},
                            }
                        }
                        None => break,
                    }
                }
            }
        }
        let events_drained = drain_started.elapsed();
        tracing::info!(
            target: "perf_probe",
            delta_chars = state.delta_text.chars().count(),
            delta_preview = %state.delta_text.chars().take(120).collect::<String>(),
            end_preview = %state.end_text.as_deref().unwrap_or("").chars().take(120).collect::<String>(),
            "turn_text_assembly"
        );
        // A worker that died without emitting a terminal event closed the
        // channel above; the join result is the only remaining evidence.
        // Swallowing it reported a panicked or aborted worker as a clean
        // EndTurn — indistinguishable from real completion.
        let joined = loop {
            let notified = self.turn_cancel.notified();
            if self.turn_was_cancelled(&session_key, generation).await {
                return self
                    .leave_cancelled_turn(
                        session_id,
                        session,
                        &mut state,
                        &mut rx,
                        join,
                        turn_permit,
                    )
                    .await;
            }
            tokio::select! {
                biased;
                _ = notified => continue,
                result = &mut join => break result,
            }
        };
        let join_failure = match joined {
            Ok(()) => None,
            Err(join_error) => Some(worker_join_failure(join_error)),
        };
        if let (Some(message), None) = (&join_failure, &state.turn_failure) {
            tracing::error!(
                session = %session_id.0,
                detail = %message,
                "turn worker died without a terminal event"
            );
            state.turn_failure = Some(message.clone());
        }
        if state.turn_failure.is_none() && state.end_text.is_none() {
            state.turn_failure = Some("turn worker exited without a terminal event".into());
        }
        if state.turn_failure.is_none()
            && !self
                .turn_was_cancelled(session_id.0.as_ref(), generation)
                .await
        {
            if let Some((goal, total_steps, duration_ms)) = state.achieved_goal.take() {
                match crate::goal::retire_achieved_durable_goal(session, governing_goal.as_ref())
                    .await
                {
                    Ok(true) => {
                        self.goals.lock().await.remove(session_id.0.as_ref());
                        let reply = match self.goal_language(session_id).await {
                            crate::goal::GoalLanguage::Chinese => "\n目标已完成并清除。\n".into(),
                            crate::goal::GoalLanguage::English => format!(
                                "\ngoal achieved and cleared ({total_steps} steps, {duration_ms} ms): {goal}\n"
                            ),
                        };
                        self.emit_text(session_id, &reply).await;
                    }
                    Ok(false) => {}
                    Err(error) => state.turn_failure = Some(error.to_string()),
                }
            }
        }
        // Emit the outcome BEFORE the response resolves: the client reads
        // the same FIFO wire, so the terminal marker it builds on the
        // response can carry this outcome.
        if let Some(outcome) = state.end_outcome.take() {
            self.emit_turn_outcome(session_id, outcome).await;
        }
        self.close_run_observation(&mut state).await;
        drop(turn_permit);
        tracing::info!(
            target: "perf_probe",
            events_drained_ms = events_drained.as_millis() as u64,
            join_ms = (drain_started.elapsed() - events_drained).as_millis() as u64,
            "stream_turn_tail"
        );
        if let Some(message) = state.turn_failure {
            if let Some(error) = state.reconciliation {
                return Err(reconciliation_error(&error));
            }
            return Err(agent_failure_error(&message));
        }
        Ok(acp::PromptResponse::new(
            state.stop_reason.unwrap_or(acp::StopReason::EndTurn),
        ))
    }

    pub(super) async fn turn_was_cancelled(&self, session_key: &str, generation: u64) -> bool {
        let recorded = self
            .cancelled_generation
            .lock()
            .await
            .get(session_key)
            .copied();
        crate::turn_cancel::generation_was_cancelled(recorded, generation)
    }

    async fn wait_for_turn_cancel(&self, session_key: &str, generation: u64) {
        loop {
            let notified = self.turn_cancel.notified();
            if self.turn_was_cancelled(session_key, generation).await {
                return;
            }
            notified.await;
        }
    }

    /// Return `Cancelled` without awaiting a stuck public join.
    ///
    /// A short drain lets a tool that is already waiting append `cancelled`.
    /// The original worker must finish its capability cleanup and release
    /// proof. The public join retains the turn lock while that happens.
    pub(super) async fn leave_cancelled_turn(
        &self,
        session_id: &acp::SessionId,
        session: &AgentSession,
        state: &mut TurnState,
        rx: &mut tokio::sync::mpsc::Receiver<AgentEvent>,
        join: JoinHandle<()>,
        turn_permit: OwnedMutexGuard<()>,
    ) -> acp::Result<acp::PromptResponse> {
        let deadline = tokio::time::Instant::now() + crate::turn_cancel::CANCEL_EVENT_DRAIN;
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                break;
            }
            tokio::select! {
                biased;
                _ = tokio::time::sleep(remaining) => break,
                event = rx.recv() => {
                    match event {
                        Some(event) => {
                            if matches!(
                                event,
                                AgentEvent::ConfirmationRequired { .. } | AgentEvent::UserQuestion { .. }
                            ) {
                                continue;
                            }
                            // A slow client must not outlive the cancelled
                            // turn's drain deadline.
                            if tokio::time::timeout_at(
                                deadline,
                                self.handle_event(session_id, session, state, event),
                            )
                            .await
                            .is_err()
                            {
                                break;
                            }
                        }
                        None => break,
                    }
                }
            }
        }
        self.settle_public_join(join, turn_permit).await;
        self.close_run_observation(state).await;
        tracing::info!(target: "perf_probe", "turn_cancel_return");
        Ok(acp::PromptResponse::new(acp::StopReason::Cancelled))
    }

    pub(super) async fn settle_public_join(
        &self,
        mut join: JoinHandle<()>,
        turn_permit: OwnedMutexGuard<()>,
    ) {
        tokio::select! {
            biased;
            result = &mut join => {
                let _ = result;
                drop(turn_permit);
            }
            _ = tokio::time::sleep(std::time::Duration::from_millis(200)) => {
                crate::turn_cancel::detach_turn_permit(turn_permit, join);
            }
        }
    }

    /// A cancel for this generation wins over both a normal end and a stream error.
    /// The word "cancel" inside an unrelated error is not a cancellation.
    pub(super) async fn finish_prompt(
        &self,
        session_key: &str,
        generation: u64,
        response: acp::Result<acp::PromptResponse>,
    ) -> acp::Result<acp::PromptResponse> {
        let elapsed = {
            let sessions = self.sessions.lock().await;
            let current =
                self.turn_generation.lock().await.get(session_key).copied() == Some(generation);
            if let Some(live) = sessions.get(session_key).filter(|_| current) {
                *live
                    .active_prompt_id
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
                live.turn_started
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .take()
                    .map(|started| started.elapsed())
            } else {
                None
            }
        };
        let cancelled = self
            .cancelled_generation
            .lock()
            .await
            .get(session_key)
            .copied()
            == Some(generation);
        if cancelled {
            return Ok(acp::PromptResponse::new(acp::StopReason::Cancelled));
        }
        // WC-NOTIFY1: the completion signal the agent itself can emit —
        // a terminal bell on stderr when the configured floor elapsed.
        // Focus detection and OS-level notifications stay with the
        // terminal host; the payload never carries content.
        if let Some(elapsed) = elapsed {
            if let Some(notifications) = &self.launch.config.notifications {
                if completion_bell_due(notifications, elapsed) {
                    eprint!("\x07");
                    tracing::info!(seconds = elapsed.as_secs_f32(), "run completion bell");
                }
            }
        }
        response
    }
}

/// End restates a reply that already streamed. Emit it only when no deltas arrived.
pub(super) fn should_emit_end_text(saw_text: bool, text: &str) -> bool {
    !saw_text && !text.is_empty()
}

/// The `x.ai/turn/outcome` notification payload carrying how the run closed
/// at the completion gate, so the client's terminal marker can say more than
/// a duration. Fields are additive; clients ignore unknown keys.
pub(super) fn turn_outcome_payload(
    terminal: &a3s_code_core::harness_loop::CompletionTerminal,
    summary: &a3s_code_core::verification::VerificationSummary,
    forced_stop: Option<&a3s_code_core::harness_loop::ForcedStop>,
) -> serde_json::Value {
    let mut payload = serde_json::json!({
        "sessionUpdate": "turn_outcome",
        "terminal": terminal_kind(terminal),
        "status": format!("{:?}", summary.status).to_lowercase(),
        "requiredChecks": summary.required_check_count,
        "failedChecks": summary.failed_check_count,
    });
    if let Some(forced) = forced_stop {
        payload["forced"] =
            serde_json::json!(a3s_code_core::harness_loop::ForcedStop::as_str(*forced));
    }
    payload
}

pub(super) fn terminal_kind(
    terminal: &a3s_code_core::harness_loop::CompletionTerminal,
) -> &'static str {
    match terminal {
        a3s_code_core::harness_loop::CompletionTerminal::Narrative => "narrative",
        a3s_code_core::harness_loop::CompletionTerminal::Verified { .. } => "verified",
        a3s_code_core::harness_loop::CompletionTerminal::Waived { .. } => "waived",
        a3s_code_core::harness_loop::CompletionTerminal::Distinct => "distinct",
    }
}

/// A Core `AgentEvent::Error` is a failed turn. The pager already renders
/// `Err` as TurnFailed; `Ok(EndTurn)` would say the turn completed.
pub(super) fn agent_failure_error(message: &str) -> acp::Error {
    acp::Error::internal_error().data(message.to_string())
}

pub(super) fn reconciliation_error(
    error: &a3s_code_core::harness_loop::ToolEffectUnresolved,
) -> acp::Error {
    acp::Error::internal_error().data(serde_json::json!({
        "kind": "tool_effect_unresolved",
        "code": "tool_effect_unresolved",
        "message": error.to_string(),
        "retryable": false,
        "recovery": { "action": "reconcile_tool_effects", "sameSession": true,
            "obligationIds": error.obligation_ids },
    }))
}

/// The failure text for a turn worker that died without a terminal event:
/// a panic names its payload, anything else is reported as an abort. This
/// is the difference between a visible failed turn and a silent clean one.
pub(super) fn worker_join_failure(join_error: tokio::task::JoinError) -> String {
    let detail = match join_error.try_into_panic() {
        Ok(payload) => payload
            .downcast_ref::<&str>()
            .map(|text| (*text).to_string())
            .or_else(|| payload.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "unknown panic payload".to_string()),
        Err(join_error) => format!("worker aborted: {join_error}"),
    };
    format!("turn worker died: {detail}")
}

fn estimated_user_message_tokens(messages: &[a3s_code_core::Message]) -> usize {
    let bytes = messages
        .iter()
        .filter(|message| message.role == "user" && message.transcript_visibility.is_product())
        .map(|message| {
            message.role.len()
                + 4
                + message
                    .content
                    .iter()
                    .map(|block| match block {
                        a3s_code_core::llm::ContentBlock::Text { text } => text.len(),
                        _ => 0,
                    })
                    .sum::<usize>()
        })
        .sum::<usize>();
    bytes.saturating_add(3) / 4
}

fn usage_update(
    snapshot: &a3s_code_core::harness_evidence::ModelUsageSnapshotV1,
    size: u64,
    transcript: &[a3s_code_core::Message],
) -> acp::UsageUpdate {
    // The wire struct has no output/cache fields, so the complete accounting
    // rides _meta. Context composition is explicitly an estimate, not billing.
    let mut update = acp::UsageUpdate::new(snapshot.reported_prompt_tokens as u64, size);
    let estimated_user_tokens = estimated_user_message_tokens(transcript);
    update.meta = Some(serde_json::Map::from_iter([
        (
            "completionTokens".to_string(),
            serde_json::json!(snapshot.reported_completion_tokens),
        ),
        (
            "totalTokens".to_string(),
            serde_json::json!(snapshot.reported_total_tokens),
        ),
        (
            "cacheReadTokens".to_string(),
            serde_json::json!(snapshot.reported_cache_read_tokens),
        ),
        (
            "cacheWriteTokens".to_string(),
            serde_json::json!(snapshot.reported_cache_write_tokens),
        ),
        (
            "callSequence".to_string(),
            serde_json::json!(snapshot.call_sequence),
        ),
        (
            "estimatedPromptTokens".to_string(),
            serde_json::json!(snapshot.estimated_prompt_tokens),
        ),
        (
            "estimatedUserTokens".to_string(),
            serde_json::json!(estimated_user_tokens),
        ),
        (
            "estimatedOtherContextTokens".to_string(),
            serde_json::json!(snapshot
                .estimated_prompt_tokens
                .saturating_sub(estimated_user_tokens)),
        ),
    ]));
    update
}

#[cfg(test)]
mod usage_meta_tests {
    use super::*;

    fn snapshot() -> a3s_code_core::harness_evidence::ModelUsageSnapshotV1 {
        a3s_code_core::harness_evidence::ModelUsageSnapshotV1 {
            schema: "test".into(),
            call_sequence: 7,
            input_snapshot_digest: "input-digest".into(),
            estimated_prompt_tokens: 100,
            reported_prompt_tokens: 90,
            reported_completion_tokens: 40,
            reported_total_tokens: 130,
            reported_cache_read_tokens: Some(10),
            reported_cache_write_tokens: Some(5),
            tool_results: a3s_code_core::harness_evidence::ToolResultContextUsageV1 {
                total_count: 0,
                unique_count: 0,
                repeated_count: 0,
                content_bytes: 0,
                repeated_content_bytes: 0,
                estimated_tokens: 0,
                repeated_estimated_tokens: 0,
                contents_digest: None,
                repeated_contents_digest: None,
            },
            snapshot_digest: "digest".into(),
        }
    }

    #[test]
    fn the_usage_update_carries_the_full_accounting_in_meta() {
        let transcript = vec![
            a3s_code_core::Message::user("你好"),
            a3s_code_core::Message::user_wire("runtime steering is not a user bubble"),
        ];
        let wire = serde_json::to_value(usage_update(&snapshot(), 32_768, &transcript)).unwrap();
        assert_eq!(wire["used"], 90);
        assert_eq!(wire["size"], 32_768);
        assert_eq!(wire["_meta"]["completionTokens"], 40);
        assert_eq!(wire["_meta"]["totalTokens"], 130);
        assert_eq!(wire["_meta"]["cacheReadTokens"], 10);
        assert_eq!(wire["_meta"]["cacheWriteTokens"], 5);
        assert_eq!(wire["_meta"]["callSequence"], 7);
        assert_eq!(wire["_meta"]["estimatedPromptTokens"], 100);
        assert_eq!(wire["_meta"]["estimatedUserTokens"], 4);
        assert_eq!(wire["_meta"]["estimatedOtherContextTokens"], 96);
    }
}
