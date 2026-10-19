//! Built-in MCP server the scheduled-task skill calls. a3s-code reaches it over
//! loopback with the plane bearer; the model never sees that credential.

use std::sync::Arc;

#[allow(unused_imports)]
use a3s_boot::{Result, body, controller, post};
use serde_json::{Value, json};

use super::catalog::AgentCatalog;
use super::error::PlaneError;
use super::tasks::TaskService;

pub struct ScheduleMcpEndpoint {
    pub base_url: String,
    pub token: String,
}

pub(crate) fn schedule_mcp_server(endpoint: &ScheduleMcpEndpoint) -> Value {
    let base = endpoint.base_url.trim_end_matches('/');
    json!({
        "type": "http",
        "name": "schedule-task",
        "url": format!("{base}/mcp/schedule-task"),
        "headers": [{ "name": "Authorization", "value": format!("Bearer {}", endpoint.token) }],
    })
}

pub struct ScheduleTaskMcpController {
    pub tasks: Arc<TaskService>,
    pub agents: Arc<AgentCatalog>,
}

#[controller("/mcp/schedule-task")]
impl ScheduleTaskMcpController {
    #[post("/")]
    async fn rpc(&self, #[body] body: Value) -> Result<Value> {
        Ok(handle_rpc(&self.tasks, &self.agents, &body))
    }
}

fn handle_rpc(tasks: &TaskService, agents: &AgentCatalog, body: &Value) -> Value {
    let method = body.get("method").and_then(Value::as_str).unwrap_or("");
    let id = body.get("id").cloned();
    if id.is_none() {
        return Value::Null;
    }
    let result = match method {
        "initialize" => json!({
            "protocolVersion": body
                .pointer("/params/protocolVersion")
                .and_then(Value::as_str)
                .unwrap_or("2024-11-05"),
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "schedule-task", "version": "1.0.0" }
        }),
        "ping" => json!({}),
        "tools/list" => json!({ "tools": tools() }),
        "tools/call" => call_tool(tasks, agents, body.get("params").unwrap_or(&Value::Null)),
        _ => {
            return json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32601, "message": "method not found" }
            });
        }
    };
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn tools() -> Value {
    json!([
        {
            "name": "create_scheduled_task",
            "description": "Create a scheduled task after the user confirms the name, schedule, and what it should do.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "name": { "type": "string" },
                    "prompt": { "type": "string" },
                    "cron": { "type": "string", "description": "Five-field cron: minute hour day-of-month month day-of-week." },
                    "model": { "type": "string" },
                    "agentId": { "type": "string" },
                    "skills": { "type": "array", "items": { "type": "string" } },
                    "workspace": {
                        "type": "object",
                        "description": "Omit to leave the task unbound from a workspace. Use {\"type\":\"user\",\"workspaceId\":\"...\"} to bind one."
                    },
                    "files": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "path": { "type": "string" },
                                "name": { "type": "string" }
                            },
                            "required": ["path"]
                        }
                    }
                },
                "required": ["name", "prompt", "cron"]
            }
        },
        {
            "name": "list_scheduled_tasks",
            "description": "List scheduled tasks.",
            "inputSchema": { "type": "object", "properties": {} }
        }
    ])
}

fn call_tool(tasks: &TaskService, agents: &AgentCatalog, params: &Value) -> Value {
    let name = params.get("name").and_then(Value::as_str).unwrap_or("");
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let outcome = match name {
        "create_scheduled_task" => create_task(tasks, agents, &arguments),
        "list_scheduled_tasks" => Ok(tasks.list(None, 1, 50).to_string()),
        _ => Err(format!("unknown tool {name}")),
    };
    match outcome {
        Ok(text) => json!({ "content": [{ "type": "text", "text": text }], "isError": false }),
        Err(message) => {
            json!({ "content": [{ "type": "text", "text": message }], "isError": true })
        }
    }
}

fn create_task(
    tasks: &TaskService,
    agents: &AgentCatalog,
    arguments: &Value,
) -> std::result::Result<String, String> {
    let agent_id = arguments
        .get("agentId")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .or_else(|| sole_agent_id(agents))
        .ok_or_else(|| "agentId is required".to_owned())?;
    let mut body = json!({
        "agentId": agent_id,
        "name": arguments.get("name").cloned().unwrap_or(Value::Null),
        "prompt": arguments.get("prompt").cloned().unwrap_or(Value::Null),
        "trigger": {
            "kind": "cron",
            "expr": arguments.get("cron").and_then(Value::as_str).unwrap_or("").trim()
        },
        "workspace": arguments
            .get("workspace")
            .cloned()
            .unwrap_or_else(|| json!({ "type": "system" }))
    });
    if let Some(model) = arguments.get("model").and_then(Value::as_str) {
        if !model.is_empty() {
            body["model"] = json!(model);
        }
    }
    if let Some(skills) = arguments.get("skills").filter(|value| value.is_array()) {
        body["skills"] = skills.clone();
    }
    if let Some(files) = file_args(arguments.get("files")) {
        body["files"] = files;
    }
    tasks
        .create(&body)
        .map(|created| created.to_string())
        .map_err(|error| plane_message(&error))
}

fn file_args(value: Option<&Value>) -> Option<Value> {
    let list = value?.as_array()?;
    let files: Vec<Value> = list
        .iter()
        .filter_map(|file| {
            let path = file.get("path")?.as_str()?.trim();
            if path.is_empty() {
                return None;
            }
            let name = file
                .get("name")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(str::to_owned)
                .unwrap_or_else(|| {
                    path.rsplit(['/', '\\'])
                        .next()
                        .filter(|name| !name.is_empty())
                        .unwrap_or(path)
                        .to_owned()
                });
            Some(json!({ "path": path, "name": name }))
        })
        .collect();
    Some(Value::Array(files))
}

fn plane_message(error: &PlaneError) -> String {
    match error {
        PlaneError::NotFound(message)
        | PlaneError::BadRequest(message)
        | PlaneError::Conflict(message)
        | PlaneError::Internal(message) => message.clone(),
    }
}

fn sole_agent_id(agents: &AgentCatalog) -> Option<String> {
    let listed = agents.list(None, &[], 1, 2);
    let items = listed.get("items")?.as_array()?;
    if items.len() != 1 {
        return None;
    }
    items.first()?.get("id")?.as_str().map(str::to_owned)
}
