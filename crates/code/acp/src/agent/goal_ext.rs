//! Host goal commands: `/goal`, its extension method, and resume.

use super::*;

impl A3sCodeAgent {
    pub(super) async fn goal_language(
        &self,
        session_id: &acp::SessionId,
    ) -> crate::goal::GoalLanguage {
        self.sessions
            .lock()
            .await
            .get(session_id.0.as_ref())
            .map(|live| crate::goal::GoalLanguage::from_host(live.host.output_language.as_deref()))
            .unwrap_or_default()
    }

    /// Restore planning after a session rebuild while a goal is still active.
    pub(super) async fn reapply_active_goal(&self, session_key: &str) {
        let active = self
            .goals
            .lock()
            .await
            .get(session_key)
            .map(|goal| !goal.paused);
        let Some(active) = active else {
            return;
        };
        let session = self
            .sessions
            .lock()
            .await
            .get(session_key)
            .map(|live| Arc::clone(&live.session));
        let Some(session) = session else {
            return;
        };
        match session.goal_definition() {
            Ok(None) => {}
            source => {
                self.goals.lock().await.remove(session_key);
                if let Err(error) = source {
                    tracing::error!(%error, "Goal source unavailable; legacy projection disabled");
                }
                return;
            }
        }
        let mode = if active {
            a3s_code_core::PlanningMode::Enabled
        } else {
            a3s_code_core::PlanningMode::Disabled
        };
        let _ = session.set_planning_mode(mode);
    }

    pub(super) async fn handle_goal(
        &self,
        session_id: &acp::SessionId,
        session: &AgentSession,
        op: crate::goal::GoalOp,
        generation: u64,
        turn_permit: OwnedMutexGuard<()>,
        observation: Option<&crate::run_observation::Binding>,
    ) -> acp::Result<acp::PromptResponse> {
        let key = session_id.0.to_string();
        let language = self.goal_language(session_id).await;
        let invalid = |error: anyhow::Error| acp::Error::invalid_params().data(error.to_string());
        if let Some(bound) = crate::goal::apply_bound_goal(session, &op, language)
            .await
            .map_err(invalid)?
        {
            self.goals.lock().await.remove(&key);
            self.emit_text(session_id, &bound.reply).await;
            if !bound.tick {
                drop(turn_permit);
                return Ok(acp::PromptResponse::new(acp::StopReason::EndTurn));
            }
            let mut turn_permit = Some(turn_permit);
            loop {
                let command = match crate::goal::bound_goal_step(session)
                    .await
                    .map_err(invalid)?
                {
                    crate::goal::BoundGoalStep::Advance(command) => command,
                    crate::goal::BoundGoalStep::Finished => {
                        drop(turn_permit.take());
                        if matches!(language, crate::goal::GoalLanguage::English) {
                            self.emit_text(session_id, "\n\nDurable goal finished\n")
                                .await;
                        }
                        return Ok(acp::PromptResponse::new(acp::StopReason::EndTurn));
                    }
                    crate::goal::BoundGoalStep::Stopped(decision) => {
                        drop(turn_permit.take());
                        if matches!(language, crate::goal::GoalLanguage::English) {
                            self.emit_text(
                                session_id,
                                &format!("\n\n{}\n", language.decision(decision)),
                            )
                            .await;
                        }
                        return Ok(acp::PromptResponse::new(acp::StopReason::EndTurn));
                    }
                };
                let permit = match turn_permit.take() {
                    Some(permit) => permit,
                    None => {
                        return Err(
                            acp::Error::invalid_params().data("goal slice lost its turn permit")
                        );
                    }
                };
                let (run_id, response) = match session
                    .stream_goal_tick(&command)
                    .await
                    .map_err(|error| acp::Error::invalid_params().data(error.to_string()))?
                {
                    a3s_code_core::GoalTickStream::Decided(decision) => {
                        drop(permit);
                        self.emit_text(
                            session_id,
                            &format!("\n\n{}\n", language.decision(decision)),
                        )
                        .await;
                        return Ok(acp::PromptResponse::new(acp::StopReason::EndTurn));
                    }
                    a3s_code_core::GoalTickStream::Replayed(snapshot) => {
                        drop(permit);
                        let report =
                            crate::goal::bound_slice_report(session, Some(&snapshot.id), language)
                                .await
                                .map_err(invalid)?;
                        self.emit_text(session_id, &format!("\n\n{report}")).await;
                        return Ok(acp::PromptResponse::new(acp::StopReason::EndTurn));
                    }
                    a3s_code_core::GoalTickStream::Started(stream) => {
                        self.emit_text(session_id, "\n\n").await;
                        let run_id = stream.run.as_ref().map(|run| run.id().to_string());
                        let response = self
                            .drain_turn(
                                session_id,
                                session,
                                stream,
                                None,
                                generation,
                                permit,
                                observation,
                            )
                            .await;
                        (run_id, response)
                    }
                };
                let report = crate::goal::bound_slice_report(session, run_id.as_deref(), language)
                    .await
                    .map_err(invalid)?;
                self.emit_text(session_id, &format!("\n\n{report}")).await;
                let response = response?;
                if response.stop_reason == acp::StopReason::Cancelled {
                    return Ok(response);
                }
                let queue = self
                    .sessions
                    .lock()
                    .await
                    .get(&key)
                    .map(|live| Arc::clone(&live.turn_queue));
                let Some(queue) = queue else {
                    return Ok(response);
                };
                turn_permit = Some(
                    queue
                        .acquire_owned()
                        .await
                        .ok_or_else(|| acp::Error::invalid_params().data("goal queue is full"))?,
                );
            }
        }
        let applied = crate::goal::apply_session_goal(session, op)
            .await
            .map_err(|error| acp::Error::internal_error().data(error.to_string()))?;
        {
            let mut goals = self.goals.lock().await;
            if let Some(goal) = applied.next.clone() {
                goals.insert(key.clone(), goal);
            } else {
                goals.remove(&key);
            }
        }
        if let Some(reply) = applied.reply {
            self.emit_text(
                session_id,
                &language.legacy_reply(applied.next.as_ref(), &reply),
            )
            .await;
        }
        if let Some(model_prompt) = applied.model_prompt {
            return self
                .stream_turn(
                    session_id,
                    session,
                    &model_prompt,
                    generation,
                    turn_permit,
                    observation,
                    None,
                    None,
                )
                .await;
        }
        drop(turn_permit);
        Ok(acp::PromptResponse::new(acp::StopReason::EndTurn))
    }

    /// Typed goal commands are the Core DTOs; writes wait behind the turn queue.
    /// No method here ticks: a slice starts only from the user's `/goal resume`.
    pub(super) async fn handle_goal_command(
        &self,
        args: &acp::ExtRequest,
    ) -> acp::Result<acp::ExtResponse> {
        use a3s_code_core::goal_control as goal;
        enum Command {
            Definition(goal::GoalDefinitionCommandV1),
            Plan(goal::GoalPlanCommandV1),
            Control(goal::GoalControlCommandV1),
            Wait(goal::GoalWaitCommandV1),
            State,
        }
        fn parse<T: serde::de::DeserializeOwned>(params: &str) -> acp::Result<T> {
            serde_json::from_str(params)
                .map_err(|error| acp::Error::invalid_params().data(error.to_string()))
        }
        let params = args.params.get();
        let (session_id, command) = match args.method.as_ref() {
            "a3s/goal/definition" => {
                let command: goal::GoalDefinitionCommandV1 = parse(params)?;
                (command.session_id.clone(), Command::Definition(command))
            }
            "a3s/goal/plan" => {
                let command: goal::GoalPlanCommandV1 = parse(params)?;
                (command.session_id.clone(), Command::Plan(command))
            }
            "a3s/goal/control" => {
                let command: goal::GoalControlCommandV1 = parse(params)?;
                (command.session_id.clone(), Command::Control(command))
            }
            "a3s/goal/wait" => {
                let command: goal::GoalWaitCommandV1 = parse(params)?;
                (command.session_id.clone(), Command::Wait(command))
            }
            _ => {
                let request: crate::goal::GoalStateRequest = parse(params)?;
                (request.session_id, Command::State)
            }
        };
        let (session, turn_queue) = self
            .sessions
            .lock()
            .await
            .get(&session_id)
            .map(|live| (Arc::clone(&live.session), Arc::clone(&live.turn_queue)))
            .ok_or_else(|| acp::Error::resource_not_found(None))?;
        if session.is_closed() {
            return Err(acp::Error::resource_not_found(None));
        }
        fn reply<T: serde::Serialize>(
            result: a3s_code_core::error::Result<T>,
        ) -> acp::Result<Box<serde_json::value::RawValue>> {
            let value =
                result.map_err(|error| acp::Error::invalid_params().data(error.to_string()))?;
            serde_json::value::to_raw_value(&value)
                .map_err(|error| acp::Error::internal_error().data(error.to_string()))
        }
        let writes = !matches!(command, Command::State);
        let _turn_permit = if writes {
            Some(
                turn_queue
                    .acquire_owned()
                    .await
                    .ok_or_else(|| acp::Error::invalid_params().data("goal queue is full"))?,
            )
        } else {
            None
        };
        let raw = match command {
            Command::Definition(command) => reply(session.apply_goal_definition(command)),
            Command::Plan(command) => reply(session.apply_goal_plan(command)),
            Command::Control(command) => reply(session.apply_goal_control(command)),
            Command::Wait(command) => reply(session.apply_goal_wait(command)),
            // A policy session's create command needs the digest its Run admitted.
            Command::State => {
                let definition = session
                    .goal_definition()
                    .map_err(|error| acp::Error::invalid_params().data(error.to_string()))?;
                let tick = match definition.as_ref() {
                    Some(definition) if definition.state != goal::GoalState::Cancelled => {
                        // Preview the exact captured revision. Concurrent writes cause refusal, not a mixed snapshot.
                        let command = goal::GoalControlCommandV1 {
                            kind: goal::GoalCommandKindV1::Tick,
                            operation_id: format!("state-preview-{}", uuid::Uuid::new_v4()),
                            goal_id: definition.binding.goal_id.clone(),
                            session_id: definition.binding.session_id.clone(),
                            workspace: definition.binding.workspace.clone(),
                            work_policy_digest: definition.binding.work_policy_digest.clone(),
                            expected_goal_revision: definition.binding.goal_revision,
                            expected_plan_revision: definition.plan_revision,
                            expected_goal_sequence: definition.goal_sequence,
                        };
                        Some(session.preview_goal_tick(&command).await.map_err(|error| {
                            acp::Error::invalid_params().data(error.to_string())
                        })?)
                    }
                    _ => None,
                };
                reply(Ok(serde_json::json!({
                    "definition": definition,
                    "work_policy_digest": session.work_policy_digest().map_err(|error| acp::Error::invalid_params().data(error.to_string()))?,
                    "tick": tick,
                })))
            }
        }?;
        if writes {
            self.goals.lock().await.remove(&session_id);
        }
        Ok(acp::ExtResponse::new(raw.into()))
    }
}
