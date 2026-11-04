//! Query, wait for, or cancel an existing child without launching it again.

use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::subagent_task_tracker::InMemorySubagentTaskTracker;
use crate::tools::{Tool, ToolCapabilities, ToolContext, ToolOutput};

const MAX_WAIT_MS: u64 = 300_000;

pub(super) enum TaskControlTool {
    Output(Arc<InMemorySubagentTaskTracker>),
    Stop(Arc<InMemorySubagentTaskTracker>),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OutputRequest {
    task_id: String,
    #[serde(default)]
    timeout_ms: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StopRequest {
    task_id: String,
}

#[async_trait]
impl Tool for TaskControlTool {
    fn name(&self) -> &str {
        match self {
            Self::Output(_) => "get_task_output",
            Self::Stop(_) => "kill_task",
        }
    }

    fn description(&self) -> &str {
        match self {
            Self::Output(_) => "Query an existing child task by task_id. Set timeout_ms to wait up to 300000 ms; 0 polls. A timeout leaves the task running. Never relaunch a task to wait for it.",
            Self::Stop(_) => "Request cancellation of an existing child task by task_id. Returns the observed status; cancellation does not mean cleanup has finished.",
        }
    }

    fn parameters(&self) -> Value {
        let mut properties = json!({"task_id":{"type":"string","minLength":1,"maxLength":128}});
        if matches!(self, Self::Output(_)) {
            properties["timeout_ms"] =
                json!({"type":"integer","minimum":0,"maximum":MAX_WAIT_MS,"default":0});
        }
        json!({"type":"object","properties":properties,"required":["task_id"],"additionalProperties":false})
    }

    fn capabilities(&self, _args: &Value) -> ToolCapabilities {
        match self {
            Self::Output(_) => ToolCapabilities::parallel_safe_read(4),
            Self::Stop(_) => ToolCapabilities::conservative(),
        }
    }

    async fn execute(&self, args: &Value, context: &ToolContext) -> Result<ToolOutput> {
        let parent = context
            .session_id
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("task control requires a session"))?;
        let (tracker, id, timeout_ms) = match self {
            Self::Output(tracker) => {
                let request: OutputRequest = serde_json::from_value(args.clone())?;
                anyhow::ensure!(
                    request.timeout_ms <= MAX_WAIT_MS,
                    "timeout_ms exceeds 300000"
                );
                (tracker, request.task_id, request.timeout_ms)
            }
            Self::Stop(tracker) => {
                let request: StopRequest = serde_json::from_value(args.clone())?;
                (tracker, request.task_id, 0)
            }
        };
        anyhow::ensure!(
            !id.is_empty() && id.len() <= 128,
            "task_id must contain 1 to 128 bytes"
        );
        let task = tracker
            .get(&id)
            .await
            .filter(|task| task.parent_session_id == parent);
        let Some(mut task) = task else {
            return Ok(ToolOutput::error("Task not found in this session."));
        };
        match self {
            Self::Output(_) if timeout_ms > 0 => {
                let cancellation = context.cancellation_token();
                let Some(observed) = tracker
                    .wait_for_parent(
                        parent,
                        &id,
                        Duration::from_millis(timeout_ms),
                        &cancellation,
                    )
                    .await?
                else {
                    return Ok(ToolOutput::error("Task not found in this session."));
                };
                task = observed;
            }
            Self::Stop(_) => {
                let requested = tracker.cancel(&id).await;
                task = tracker.get(&id).await.unwrap_or(task);
                return Ok(ToolOutput::success(serde_json::to_string(
                    &json!({"cancellation_requested":requested,"task":task}),
                )?));
            }
            _ => {}
        }
        Ok(ToolOutput::success(serde_json::to_string(&task)?))
    }
}

#[cfg(test)]
#[path = "task_control_tests.rs"]
mod tests;
