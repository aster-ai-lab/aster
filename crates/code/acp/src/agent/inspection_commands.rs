//! Terminal entry points for the current memory and background-task contracts.

use super::*;
use aster_core::durable_memory_vec::management::{
    MemoryManagementFailure, MemoryManagementRequest, MemoryManagementResponse,
};

const MEMORY_USAGE: &str =
    "Usage: /memory [list [query] | next <cursor> [query] | read <id> | delete <id> <revision>]";
const TASKS_USAGE: &str = "Usage: /tasks [stop <task-id>]";

pub(super) enum InspectionCommand {
    Memory(MemoryManagementRequest),
    Tasks(Option<String>),
}

pub(super) fn parse(prompt: &str) -> Option<Result<InspectionCommand, &'static str>> {
    let (command, args) = prompt
        .trim()
        .split_once(char::is_whitespace)
        .unwrap_or((prompt.trim(), ""));
    let args = args.trim();
    match command {
        "/memory" => Some(parse_memory(args).map(InspectionCommand::Memory)),
        "/tasks" => Some(if args.is_empty() {
            Ok(InspectionCommand::Tasks(None))
        } else {
            match args.split_once(char::is_whitespace) {
                Some(("stop", id)) if !id.trim().is_empty() && id.trim().len() <= 4096 => {
                    Ok(InspectionCommand::Tasks(Some(id.trim().into())))
                }
                _ => Err(TASKS_USAGE),
            }
        }),
        _ => None,
    }
}

fn parse_memory(args: &str) -> Result<MemoryManagementRequest, &'static str> {
    if args.len() > 4096 {
        return Err(MEMORY_USAGE);
    }
    let (operation, value) = args.split_once(char::is_whitespace).unwrap_or((args, ""));
    let value = value.trim();
    let request = match operation {
        "" | "list" => MemoryManagementRequest::List {
            query: value.into(),
            memory_type: None,
            cursor: None,
            created_after: None,
            created_before: None,
        },
        "next" if !value.is_empty() => {
            // The cursor is opaque JSON owned by Core; preserve its bytes and the original filter.
            let mut reader =
                serde_json::Deserializer::from_str(value).into_iter::<serde_json::Value>();
            reader
                .next()
                .ok_or(MEMORY_USAGE)?
                .map_err(|_| MEMORY_USAGE)?;
            let end = reader.byte_offset();
            MemoryManagementRequest::List {
                query: value[end..].trim().into(),
                memory_type: None,
                cursor: Some(value[..end].into()),
                created_after: None,
                created_before: None,
            }
        }
        "read" if !value.is_empty() => MemoryManagementRequest::Read { id: value.into() },
        "delete" => {
            let (id, revision) = value.rsplit_once(char::is_whitespace).ok_or(MEMORY_USAGE)?;
            MemoryManagementRequest::Delete {
                id: id.trim_end().into(),
                revision: revision.into(),
            }
        }
        _ => return Err(MEMORY_USAGE),
    };
    request.validate().map_err(|_| MEMORY_USAGE)?;
    Ok(request)
}

impl AsterCodeAgent {
    pub(super) async fn handle_inspection_command(
        &self,
        session: &AgentSession,
        command: Result<InspectionCommand, &'static str>,
    ) -> acp::Result<String> {
        let command = command.map_err(|usage| acp::Error::invalid_params().data(usage))?;
        let query = match &command {
            InspectionCommand::Memory(MemoryManagementRequest::List { query, .. }) => query.clone(),
            _ => String::new(),
        };
        let (method, params) = match command {
            InspectionCommand::Memory(request) => (
                "aster/memory/manage",
                serde_json::json!({"workspace_root":session.workspace(),"session_id":session.session_id(),"request":request}),
            ),
            InspectionCommand::Tasks(task) => {
                let mut params = serde_json::json!({"schema":"aster.acp.session-background-tasks.v1","sessionId":session.session_id()});
                let method = if let Some(task) = task {
                    params["taskId"] = serde_json::json!(task);
                    "aster/session/background-tasks/stop/v1"
                } else {
                    "aster/session/background-tasks/v1"
                };
                (method, params)
            }
        };
        let raw = serde_json::value::to_raw_value(&params)
            .map_err(|error| acp::Error::internal_error().data(error.to_string()))?;
        let response =
            acp::Agent::ext_method(self, acp::ExtRequest::new(method, raw.into())).await?;
        let value: serde_json::Value = serde_json::from_str(response.0.get())
            .map_err(|error| acp::Error::internal_error().data(error.to_string()))?;
        if method == "aster/memory/manage" {
            let outcome: Result<MemoryManagementResponse, MemoryManagementFailure> =
                serde_json::from_value(value["outcome"].clone())
                    .map_err(|error| acp::Error::internal_error().data(error.to_string()))?;
            return match outcome {
                Ok(result) => format_memory(result, &query),
                Err(error) => Err(acp::Error::invalid_params().data(error.to_string())),
            };
        }
        if method == "aster/session/background-tasks/stop/v1" {
            return match value.as_bool() {
                Some(true) => Ok("Task stopped.".into()),
                Some(false) => Err(acp::Error::invalid_params().data(
                    "task is not running, is not cancellable, or does not belong to this session",
                )),
                None => Err(acp::Error::internal_error().data("invalid task stop response")),
            };
        }
        let tasks = value["tasks"]
            .as_array()
            .ok_or_else(|| acp::Error::internal_error().data("invalid background task snapshot"))?;
        if tasks.is_empty() {
            return Ok("No background tasks.".into());
        }
        let mut text = String::from("Background tasks\n\n");
        for task in tasks {
            // JSON quoting keeps task descriptions and identities on one terminal row.
            text.push_str(&format!(
                "- {} · {} · {}\n",
                task["id"], task["status"], task["description"]
            ));
        }
        Ok(text)
    }
}

fn format_memory(response: MemoryManagementResponse, query: &str) -> acp::Result<String> {
    match response {
        MemoryManagementResponse::List {
            items,
            total,
            next_cursor,
        } => {
            let mut text = format!("Stored memories: {total}\n\n");
            for item in items {
                let preview: String = item.content.chars().take(160).collect();
                let preview = serde_json::to_string(&preview)
                    .map_err(|error| acp::Error::internal_error().data(error.to_string()))?;
                text.push_str(&format!(
                    "- {} · {} · {preview}\n",
                    serde_json::json!(item.id),
                    item.memory_type.as_str()
                ));
            }
            if let Some(cursor) = next_cursor {
                text.push_str(&format!("\nNext page: /memory next {cursor} {query}\n"));
            }
            Ok(text)
        }
        MemoryManagementResponse::Read { item } => {
            let id = serde_json::json!(item.id);
            Ok(format!(
                "Memory {id}\nType: {}\nRevision: {}\n\n{}",
                item.memory_type.as_str(),
                item.revision,
                item.content
            ))
        }
        MemoryManagementResponse::Delete { id } => {
            Ok(format!("Deleted memory {}.", serde_json::json!(id)))
        }
        _ => Err(acp::Error::internal_error().data("unexpected memory command response")),
    }
}

#[cfg(test)]
#[path = "inspection_commands_tests.rs"]
mod tests;
