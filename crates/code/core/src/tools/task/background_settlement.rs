//! Original background child completion, bound to its spawning invocation.

use super::*;
use aster_effect::{LogStore, ToolEffectObservationV1};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BackgroundChildBinding {
    parent_key: String,
    task_id: String,
    child_session_id: String,
}

impl BackgroundChildBinding {
    pub(super) fn from_context(context: &ToolContext, task_id: &str) -> Result<Option<Self>> {
        if context.fact_log.is_none() {
            return Ok(None);
        }
        let identity = context.fact_invocation.as_ref().ok_or_else(|| {
            anyhow::anyhow!("background task has no original invocation identity")
        })?;
        Ok(Some(Self {
            parent_key: identity.key()?,
            task_id: task_id.into(),
            child_session_id: format!("task-run-{task_id}"),
        }))
    }

    pub(crate) fn admits(&self, key: &str, task_id: &str) -> bool {
        self.parent_key == key
            && self.task_id == task_id
            && self.child_session_id == format!("task-run-{task_id}")
    }
}

pub(super) async fn retain(
    context: ToolContext,
    binding: BackgroundChildBinding,
    effects: ToolEffectObservationV1,
) -> Result<(), crate::capability::CapabilityEffectError> {
    // Cancellation can retain late evidence in the original logs; this closure starts no child work.
    tokio::task::spawn_blocking(move || -> Result<()> {
        let child_log = aster_effect::FileLog::open(crate::session_folder::child_effect_log_dir(&context.workspace, &binding.child_session_id)?)?;
        let facts = child_log.read(&crate::fact_control::thread_for_session(&binding.child_session_id))?;
        anyhow::ensure!(facts.last().is_some_and(|fact| fact.kind == crate::fact_control::TURN_COMPLETE_FACT_KIND), "background child has no original completion fact");
        let cut = aster_effect::cut_log(&facts);
        let log = context.fact_log.as_ref().ok_or_else(|| anyhow::anyhow!("background parent fact log missing"))?;
        let session = context.session_id.as_deref().ok_or_else(|| anyhow::anyhow!("background parent session missing"))?;
        log.append(&crate::fact_control::thread_for_session(session), &[aster_effect::NewFact {
            kind: "subagent.settled".into(), key: format!("subagent-settled:{}", binding.task_id),
            payload: serde_json::json!({
                "workspace_child":binding.task_id, "task_id":binding.task_id,
                "background_subagent":binding,
                "child_log_cut":{"seq":cut.seq,"digest":cut.digest},
                "changed_paths":effects.changes.iter().map(|change| &change.path).collect::<Vec<_>>(),
                "effects":effects,
            }),
        }], None)?;
        Ok(())
    }).await.map_err(|error| crate::capability::CapabilityEffectError::new(format!("background settlement worker failed: {error}")))?
        .map_err(|error| crate::capability::CapabilityEffectError::new(format!("background settlement retention failed: {error:#}")))
}

pub(super) async fn start(
    executor: Arc<TaskExecutor>,
    params: TaskParams,
    context: &ToolContext,
    binding: Option<BackgroundChildBinding>,
    task_id: String,
    admission: &crate::agent_api::background_tasks::BackgroundAdmission,
) -> Result<BackgroundLaunch> {
    let plan = admission.capture(context)?;
    let cancellation = plan.cancellation();
    let parent_key = context
        .fact_invocation
        .as_ref()
        .map(|identity| identity.key())
        .transpose()?;
    let parent = context
        .session_id
        .clone()
        .ok_or_else(|| anyhow::anyhow!("background parent session missing"))?;
    let child = format!("task-run-{task_id}");
    let event_tx = context.agent_event_tx.clone();
    let start = AgentEvent::SubagentStart {
        task_id: task_id.clone(),
        session_id: child.clone(),
        parent_session_id: parent.clone(),
        agent: params.agent.clone(),
        description: params.description.clone(),
        started_ms: epoch_ms(),
    };
    let progress = AgentEvent::SubagentProgress {
        task_id: task_id.clone(),
        session_id: child.clone(),
        status: "started".into(),
        metadata: serde_json::json!({"background":true,(crate::subagent_task_tracker::TASK_PROGRESS_EVENT_ID):uuid::Uuid::new_v4().to_string()}),
    };
    let security = executor
        .parent_context
        .as_ref()
        .and_then(|parent| parent.security_provider.clone());
    for event in [start, progress] {
        let event = security
            .as_deref()
            .map(|provider| crate::security::sanitize_agent_event(provider, &event))
            .unwrap_or(event);
        if let Some(tracker) = &executor.subagent_tracker {
            tracker.record_event(&event).await;
        }
        if let Some(tx) = &event_tx {
            let _ = tx.send(event);
        }
    }
    if let Some(tracker) = &executor.subagent_tracker {
        tracker
            .register_canceller(&task_id, cancellation.clone())
            .await;
    }
    let launch_tracker = executor.subagent_tracker.clone();
    let launch_events = event_tx.clone();
    let launch_security = security.clone();
    let launch_child = child.clone();
    let launch_agent = params.agent.clone();
    let context = context.clone();
    let persistence = admission.persistence().clone();
    let id = task_id.clone();
    let mut workspace_child = crate::porcelain::WorkspaceChildGuard::new(&id);
    let future = async move {
        let execution = async {
            let (guard, hooks) = plan.admit(&id, &context.workspace).await?;
            let mut executor = executor.as_ref().clone();
            if let Some(parent) = executor.parent_context.as_mut() {
                parent.hook_engine = Some(hooks);
            }
            let execution = match crate::capability::AgentCapabilitySubtask::from_background_run(
                guard.run_scope(),
                &id,
            ) {
                Ok(subtask) => {
                    executor
                        .execute_with_task_id_scoped(
                            id.clone(),
                            params.clone(),
                            ScopedTaskExecution {
                                event_tx: event_tx.clone(),
                                parent_session_id: Some(&parent),
                                emit_start: false,
                                parent_cancellation: Some(&cancellation),
                                admitted_capability_subtask: Some(subtask),
                                parallel_lifecycle: None,
                                terminal_persistence: Some(&persistence),
                            },
                        )
                        .await
                }
                Err(error) => Err(error.into()),
            };
            let close = guard.close().await;
            match (execution, close) {
                (Ok(_), Ok(_)) => Ok(()),
                (Err(error), Ok(_)) => Err(error),
                (Ok(_), Err(error)) => Err(error.into()),
                (Err(error), Err(close)) => Err(anyhow::anyhow!(
                    "{error:#}; background capability close failed: {close}"
                )),
            }
        }
        .await;
        if let Err(error) = &execution {
            let end = AgentEvent::SubagentEnd {
                task_id: id.clone(),
                session_id: child.clone(),
                agent: params.agent,
                output: format!("Background task failed: {error:#}"),
                success: false,
                finished_ms: epoch_ms(),
            };
            let end = security
                .as_deref()
                .map(|provider| crate::security::sanitize_agent_event(provider, &end))
                .unwrap_or(end);
            if let Some(tracker) = &executor.subagent_tracker {
                tracker.record_event(&end).await;
                tracker.clear_canceller(&id).await;
            }
            Box::pin(persistence.auto_save_if_enabled()).await;
            if let Some(tx) = &event_tx {
                let _ = tx.send(end);
            }
        }
        execution.map_err(|error| {
            crate::capability::CapabilityEffectError::new(format!(
                "background execution failed: {error:#}"
            ))
        })?;
        if let Some(binding) = binding {
            let effects =
                crate::porcelain::observe_workspace_child_settlement(&id, &context.workspace).await;
            retain(context.clone(), binding, effects.clone()).await?;
            workspace_child.settle_paths(
                effects
                    .changes
                    .iter()
                    .map(|change| change.path.clone())
                    .collect(),
            );
        } else {
            crate::porcelain::settle_workspace_child_guard(
                &mut workspace_child,
                &context.workspace,
            )
            .await;
        }
        let paths = crate::porcelain::peek_settled_workspace_child(&id).unwrap_or_default();
        adopt_dirtied_paths(Some(&parent), &child, &context.workspace, &paths);
        Ok(())
    };
    if let Err(error) = admission.spawn(task_id.clone(), parent_key, future) {
        let end = AgentEvent::SubagentEnd {
            task_id: task_id.clone(),
            session_id: launch_child,
            agent: launch_agent,
            output: format!("Background task admission failed: {error:#}"),
            success: false,
            finished_ms: epoch_ms(),
        };
        let end = launch_security
            .as_deref()
            .map(|provider| crate::security::sanitize_agent_event(provider, &end))
            .unwrap_or(end);
        if let Some(tracker) = launch_tracker {
            tracker.record_event(&end).await;
            tracker.clear_canceller(&task_id).await;
        }
        Box::pin(admission.persistence().auto_save_if_enabled()).await;
        if let Some(tx) = launch_events {
            let _ = tx.send(end);
        }
        return Err(error);
    }
    Ok(BackgroundLaunch {
        task_id,
        running: true,
    })
}
