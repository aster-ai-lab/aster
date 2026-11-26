//! Live checklist / plan updates for the host UI (Codex-compatible schema).
//!
//! Replaces the session task list and emits [`AgentEvent::TaskUpdated`] so
//! hosts that pin a plan panel (and session persistence) stay in sync.

use crate::agent::AgentEvent;
use crate::planning::{Task, TaskStatus};
use crate::tools::types::{Tool, ToolCapabilities, ToolContext, ToolOutput};
use anyhow::Result;
use async_trait::async_trait;

/// Keep presentation and persistence bounded (aligned with CLI projection).
const MAX_PLAN_TASKS: usize = 256;
const MAX_STEP_CHARS: usize = 480;

pub struct UpdatePlanTool;

#[async_trait]
impl Tool for UpdatePlanTool {
    fn name(&self) -> &str {
        "update_plan"
    }

    fn description(&self) -> &str {
        "Replace task checklist (full plan array each call; 2–8 steps for multi-step work, one in_progress; skip trivial tasks)."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "plan": {
                    "type": "array",
                    "description": "Ordered steps.",
                    "minItems": 1,
                    "maxItems": MAX_PLAN_TASKS,
                    "items": {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": {
                            "step": {
                                "type": "string",
                                "description": "Step label."
                            },
                            "status": {
                                "type": "string",
                                "enum": [
                                    "pending",
                                    "in_progress",
                                    "completed",
                                    "failed",
                                    "skipped",
                                    "cancelled"
                                ]
                            },
                            "id": {
                                "type": "string",
                                "description": "Optional ID."
                            }
                        },
                        "required": ["step", "status"]
                    }
                }
            },
            "required": ["plan"],
            "examples": [
                {
                    "plan": [
                        {"step": "Inspect render helpers", "status": "completed", "id": "1"},
                        {"step": "Wire pagination into explore_detail", "status": "in_progress", "id": "2"},
                        {"step": "Add tests and docs", "status": "pending", "id": "3"}
                    ]
                }
            ]
        })
    }

    fn capabilities(&self, _args: &serde_json::Value) -> ToolCapabilities {
        ToolCapabilities {
            read_only: true,
            idempotent: true,
            resumable: false,
            cancellation_safe: true,
            supports_pagination: false,
            max_parallelism: 1,
            output_kind: crate::tools::ToolOutputKind::Text,
        }
    }

    async fn execute(&self, args: &serde_json::Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let parsed = match parse_plan_args(args) {
            Ok(parsed) => parsed,
            Err(message) => return Ok(ToolOutput::error(message)),
        };

        let session_id = ctx.session_id.clone().unwrap_or_default();
        if let Some(tx) = ctx.agent_event_tx.as_ref() {
            let _ = tx.send(AgentEvent::TaskUpdated {
                session_id,
                tasks: parsed.tasks.clone(),
            });
        }

        Ok(
            ToolOutput::success(summarize_plan(&parsed.tasks)).with_metadata(serde_json::json!({
                "discarded_chars": parsed.discarded_chars,
                "step_char_budget": MAX_STEP_CHARS,
            })),
        )
    }
}

struct ParsedPlan {
    tasks: Vec<Task>,
    /// Characters dropped when step labels hit [`MAX_STEP_CHARS`] (E4 honesty).
    discarded_chars: usize,
}

fn parse_plan_args(args: &serde_json::Value) -> Result<ParsedPlan, String> {
    let rows = args
        .get("plan")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "plan must be a non-empty array".to_string())?;
    if rows.is_empty() {
        return Err("plan must contain at least one step".to_string());
    }
    if rows.len() > MAX_PLAN_TASKS {
        return Err(format!(
            "plan has {} steps; maximum is {MAX_PLAN_TASKS}",
            rows.len()
        ));
    }

    let mut tasks = Vec::with_capacity(rows.len());
    let mut discarded_chars = 0usize;
    let mut seen_ids = std::collections::HashSet::new();
    // A2: plan execution keys by step content — duplicate prose fans out
    // duplicate delegated work. Refuse same truncated content in one plan.
    let mut seen_content = std::collections::HashSet::new();
    let mut in_progress = 0usize;
    for (index, row) in rows.iter().enumerate() {
        let step = row
            .get("step")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|step| !step.is_empty())
            .ok_or_else(|| format!("plan[{index}].step must be a non-empty string"))?;
        let (content, dropped) = truncate_chars(step, MAX_STEP_CHARS);
        discarded_chars = discarded_chars.saturating_add(dropped);
        let status = row
            .get("status")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| format!("plan[{index}].status is required"))
            .and_then(parse_status)?;
        let id = row
            .get("id")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| format!("plan-{}", index + 1));
        if !seen_ids.insert(id.clone()) {
            return Err(format!("plan step id '{id}' is duplicated"));
        }
        if !seen_content.insert(content.clone()) {
            return Err(format!(
                "plan step content is duplicated at plan[{index}]; each step must be unique"
            ));
        }
        if status == TaskStatus::InProgress {
            in_progress += 1;
            if in_progress > 1 {
                return Err(
                    "plan may have at most one in_progress step; finish or demote the others"
                        .to_string(),
                );
            }
        }
        let mut task = Task::new(id, content);
        task.status = status;
        tasks.push(task);
    }
    Ok(ParsedPlan {
        tasks,
        discarded_chars,
    })
}

fn parse_status(status: &str) -> Result<TaskStatus, String> {
    match status.trim().to_ascii_lowercase().as_str() {
        "pending" => Ok(TaskStatus::Pending),
        "in_progress" | "in-progress" | "active" => Ok(TaskStatus::InProgress),
        "completed" | "complete" | "done" => Ok(TaskStatus::Completed),
        "failed" | "error" => Ok(TaskStatus::Failed),
        "skipped" => Ok(TaskStatus::Skipped),
        "cancelled" | "canceled" => Ok(TaskStatus::Cancelled),
        // Names the closed set, matching the canonical TaskStatus parse
        // error (the model-facing tool must not drift from it).
        other => Err(format!(
            "unknown plan status {other:?}; the closed set is pending, in_progress, \
             completed, failed, skipped, cancelled"
        )),
    }
}

fn truncate_chars(value: &str, max_chars: usize) -> (String, usize) {
    let total = value.chars().count();
    let mut out = String::new();
    for (count, ch) in value.chars().enumerate() {
        if count >= max_chars {
            break;
        }
        out.push(ch);
    }
    let retained = out.chars().count();
    (out, total.saturating_sub(retained))
}

fn summarize_plan(tasks: &[Task]) -> String {
    let total = tasks.len();
    let completed = tasks
        .iter()
        .filter(|task| task.status == TaskStatus::Completed)
        .count();
    let active = tasks
        .iter()
        .find(|task| task.status == TaskStatus::InProgress)
        .map(|task| task.content.as_str());
    match active {
        Some(step) => format!("Updated plan: {completed}/{total} done · in progress: {step}"),
        None => format!("Updated plan: {completed}/{total} done"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::broadcast;

    #[tokio::test]
    async fn update_plan_emits_task_updated_and_summarizes() {
        let (tx, mut rx) = broadcast::channel(8);
        let ctx = ToolContext::new(std::path::PathBuf::from("/tmp"))
            .with_session_id("session-1")
            .with_agent_event_tx(tx);

        let args = serde_json::json!({
            "plan": [
                {"step": "First", "status": "completed", "id": "a"},
                {"step": "Second", "status": "in_progress"},
                {"step": "Third", "status": "pending"}
            ]
        });
        let output = UpdatePlanTool.execute(&args, &ctx).await.unwrap();
        assert!(output.success);
        assert!(output.content.contains("1/3 done"));
        assert!(output.content.contains("Second"));

        let event = rx.try_recv().expect("TaskUpdated event");
        match event {
            AgentEvent::TaskUpdated { session_id, tasks } => {
                assert_eq!(session_id, "session-1");
                assert_eq!(tasks.len(), 3);
                assert_eq!(tasks[0].id, "a");
                assert_eq!(tasks[0].status, TaskStatus::Completed);
                assert_eq!(tasks[1].id, "plan-2");
                assert_eq!(tasks[1].status, TaskStatus::InProgress);
                assert_eq!(tasks[2].status, TaskStatus::Pending);
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[tokio::test]
    async fn update_plan_rejects_empty_step_and_unknown_status() {
        let ctx = ToolContext::new(std::path::PathBuf::from("/tmp"));
        let empty = UpdatePlanTool
            .execute(
                &serde_json::json!({"plan": [{"step": "  ", "status": "pending"}]}),
                &ctx,
            )
            .await
            .unwrap();
        assert!(!empty.success);

        let bad_status = UpdatePlanTool
            .execute(
                &serde_json::json!({"plan": [{"step": "x", "status": "doing"}]}),
                &ctx,
            )
            .await
            .unwrap();
        assert!(!bad_status.success);
        assert!(bad_status.content.contains("unknown plan status"));
    }

    #[tokio::test]
    async fn update_plan_rejects_a_step_id_that_collides_with_another_step() {
        let ctx = ToolContext::new(std::path::PathBuf::from("/tmp"));
        let collided = UpdatePlanTool
            .execute(
                &serde_json::json!({
                    "plan": [
                        {"step": "First", "status": "pending"},
                        {"step": "Second", "status": "completed", "id": "plan-1"}
                    ]
                }),
                &ctx,
            )
            .await
            .unwrap();
        assert!(!collided.success);
        assert!(
            collided.content.contains("duplicated"),
            "an explicit id must not reuse an automatic id: {}",
            collided.content
        );
    }

    #[tokio::test]
    async fn update_plan_rejects_duplicate_step_content_and_multi_in_progress() {
        let ctx = ToolContext::new(std::path::PathBuf::from("/tmp"));
        let dup_content = UpdatePlanTool
            .execute(
                &serde_json::json!({
                    "plan": [
                        {"step": "Same work", "status": "pending", "id": "a"},
                        {"step": "Same work", "status": "pending", "id": "b"}
                    ]
                }),
                &ctx,
            )
            .await
            .unwrap();
        assert!(!dup_content.success);
        assert!(
            dup_content.content.contains("content is duplicated"),
            "{}",
            dup_content.content
        );

        let multi = UpdatePlanTool
            .execute(
                &serde_json::json!({
                    "plan": [
                        {"step": "One", "status": "in_progress", "id": "a"},
                        {"step": "Two", "status": "in_progress", "id": "b"}
                    ]
                }),
                &ctx,
            )
            .await
            .unwrap();
        assert!(!multi.success);
        assert!(
            multi.content.contains("at most one in_progress"),
            "{}",
            multi.content
        );
    }

    #[test]
    fn parse_plan_args_bounds_length() {
        let rows: Vec<_> = (0..MAX_PLAN_TASKS + 1)
            .map(|i| serde_json::json!({"step": format!("s{i}"), "status": "pending"}))
            .collect();
        let Err(err) = parse_plan_args(&serde_json::json!({ "plan": rows })) else {
            panic!("expected plan length refusal");
        };
        assert!(err.contains("maximum"));
    }

    #[test]
    fn update_plan_capabilities_are_read_only_and_idempotent() {
        let caps = UpdatePlanTool.capabilities(&serde_json::json!({}));
        assert!(caps.read_only);
        assert!(caps.idempotent);
        assert!(caps.cancellation_safe);
        assert!(!caps.resumable);
        assert!(!caps.supports_pagination);
        assert_eq!(caps.max_parallelism, 1);
    }

    #[test]
    fn parse_plan_args_rejects_empty_plan() {
        let Err(err) = parse_plan_args(&serde_json::json!({ "plan": [] })) else {
            panic!("expected empty plan refusal");
        };
        assert!(err.contains("at least one step"));
    }

    #[test]
    fn truncate_chars_stops_at_max_and_summarize_without_in_progress() {
        let long: String = "字".repeat(MAX_STEP_CHARS + 8);
        let (truncated, discarded) = truncate_chars(&long, MAX_STEP_CHARS);
        assert_eq!(truncated.chars().count(), MAX_STEP_CHARS);
        assert_eq!(discarded, 8);

        let tasks = vec![
            {
                let mut task = Task::new("1", "done");
                task.status = TaskStatus::Completed;
                task
            },
            {
                let mut task = Task::new("2", "waiting");
                task.status = TaskStatus::Pending;
                task
            },
        ];
        let summary = summarize_plan(&tasks);
        assert_eq!(summary, "Updated plan: 1/2 done");
        assert!(!summary.contains("in progress"));
    }

    #[tokio::test]
    async fn update_plan_reports_discarded_chars_when_steps_truncate() {
        let ctx = ToolContext::new(std::path::PathBuf::from("/tmp"));
        let long: String = "a".repeat(MAX_STEP_CHARS + 12);
        let output = UpdatePlanTool
            .execute(
                &serde_json::json!({
                    "plan": [{"step": long, "status": "pending", "id": "long"}]
                }),
                &ctx,
            )
            .await
            .unwrap();
        assert!(output.success);
        let meta = output.metadata.expect("discard metadata");
        assert_eq!(meta.get("discarded_chars"), Some(&serde_json::json!(12)));
        assert_eq!(
            meta.get("step_char_budget"),
            Some(&serde_json::json!(MAX_STEP_CHARS))
        );
    }
}
