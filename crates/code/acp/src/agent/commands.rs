//! Slash commands the agent handles inside a prompt, announced to the client
//! with the ACP `available_commands_update` session notification.

use super::A3sCodeAgent;
use a3s_code_core::AgentSession;
use agent_client_protocol::{self as acp, Client as _};

fn goal_command() -> acp::AvailableCommand {
    acp::AvailableCommand::new("goal", "Set, pause, resume, or clear a durable goal").input(
        acp::AvailableCommandInput::Unstructured(acp::UnstructuredCommandInput::new(
            "<objective> | status | pause | resume | clear",
        )),
    )
}

/// Commands handled by ACP itself rather than by a model turn.
pub(super) fn acp_commands() -> Vec<acp::AvailableCommand> {
    vec![
        goal_command(),
        acp::AvailableCommand::new("memory", "Inspect and manage stored memories").input(
            acp::AvailableCommandInput::Unstructured(acp::UnstructuredCommandInput::new(
                "list [query] | next <cursor> [query] | read <id> | delete <id> <revision>",
            )),
        ),
        acp::AvailableCommand::new("tasks", "Inspect background tasks or stop a task").input(
            acp::AvailableCommandInput::Unstructured(acp::UnstructuredCommandInput::new(
                "[stop <task-id>]",
            )),
        ),
    ]
}

fn merge_commands(
    core_commands: Vec<(String, String, Option<String>)>,
) -> acp::Result<Vec<acp::AvailableCommand>> {
    let mut commands = acp_commands();
    let acp_names: std::collections::HashSet<_> = commands
        .iter()
        .map(|command| command.name.clone())
        .collect();
    let mut core_names = std::collections::HashSet::new();
    for (name, description, usage) in core_commands {
        if name.is_empty() || name.starts_with('/') || name.chars().any(char::is_whitespace) {
            return Err(acp::Error::internal_error()
                .data(format!("invalid Core command protocol name: {name:?}")));
        }
        // ACP intercepts its own names before Core dispatch.
        if acp_names.contains(&name) {
            continue;
        }
        if !core_names.insert(name.clone()) {
            return Err(acp::Error::internal_error()
                .data(format!("duplicate Core command protocol name: {name}")));
        }
        let mut command = acp::AvailableCommand::new(name, description);
        if let Some(usage) = usage {
            command = command.input(acp::AvailableCommandInput::Unstructured(
                acp::UnstructuredCommandInput::new(usage),
            ));
        }
        commands.push(command);
    }
    commands.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(commands)
}

/// Snapshot the commands registered on this exact Core session generation.
pub(super) fn session_commands(session: &AgentSession) -> acp::Result<Vec<acp::AvailableCommand>> {
    merge_commands(session.command_registry().list_full())
}

impl A3sCodeAgent {
    /// Announce the commands once the session response has been written: the
    /// client only routes updates for a session id it has already received.
    pub(super) fn announce_commands(
        &self,
        session_id: &str,
        available_commands: Vec<acp::AvailableCommand>,
    ) {
        let Some(client) = self.client.borrow().clone() else {
            return;
        };
        let notification = acp::SessionNotification::new(
            acp::SessionId::new(session_id.to_string()),
            acp::SessionUpdate::AvailableCommandsUpdate(acp::AvailableCommandsUpdate::new(
                available_commands,
            )),
        );
        tokio::task::spawn_local(async move {
            tokio::task::yield_now().await;
            let _ = client.session_notification(notification).await;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(commands: &[acp::AvailableCommand]) -> Vec<&str> {
        commands
            .iter()
            .map(|command| command.name.as_str())
            .collect()
    }

    #[test]
    fn an_empty_core_catalog_contains_only_acp_commands() {
        let commands = merge_commands(Vec::new()).expect("empty Core catalog");
        assert_eq!(names(&commands), ["goal", "memory", "tasks"]);
    }

    #[test]
    fn core_descriptions_usage_and_order_are_projected() {
        let commands = merge_commands(vec![
            (
                "model".into(),
                "Show or switch the current model".into(),
                Some("/model [provider/model]".into()),
            ),
            ("help".into(), "List available commands".into(), None),
        ])
        .expect("Core catalog");
        assert_eq!(
            names(&commands),
            ["goal", "help", "memory", "model", "tasks"]
        );
        assert_eq!(commands[1].description, "List available commands");
        assert!(commands[1].input.is_none());
        assert_eq!(commands[3].description, "Show or switch the current model");
        let Some(acp::AvailableCommandInput::Unstructured(input)) = &commands[3].input else {
            panic!("model usage must be unstructured input");
        };
        assert_eq!(input.hint, "/model [provider/model]");
    }

    #[test]
    fn acp_goal_wins_a_core_name_conflict_without_a_duplicate() {
        let commands = merge_commands(vec![(
            "goal".into(),
            "Core must not shadow ACP".into(),
            Some("core usage".into()),
        )])
        .expect("ACP owns goal");
        assert_eq!(names(&commands), ["goal", "memory", "tasks"]);
        assert_eq!(
            commands[0].description,
            "Set, pause, resume, or clear a durable goal"
        );
        let Some(acp::AvailableCommandInput::Unstructured(input)) = &commands[0].input else {
            panic!("goal usage must be retained");
        };
        assert_eq!(input.hint, "<objective> | status | pause | resume | clear");
    }

    #[test]
    fn duplicate_core_names_are_rejected() {
        let error = merge_commands(vec![
            ("help".into(), "first".into(), None),
            ("help".into(), "second".into(), None),
        ])
        .expect_err("duplicate name");
        assert_eq!(
            error.data,
            Some(serde_json::json!(
                "duplicate Core command protocol name: help"
            ))
        );
    }

    #[test]
    fn invalid_protocol_names_are_rejected() {
        for name in ["", "/help", "two words"] {
            let error = merge_commands(vec![(name.into(), "invalid".into(), None)])
                .expect_err("invalid protocol name");
            assert!(error
                .data
                .and_then(|data| data.as_str().map(str::to_owned))
                .is_some_and(|data| data.contains("invalid Core command protocol name")));
        }
    }
}
