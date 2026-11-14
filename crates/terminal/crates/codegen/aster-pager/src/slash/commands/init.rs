//! `/init` writes a short project overview for future AI agents.
//!
//! The command refuses to run when `AGENTS.md` already exists (checked in the
//! dispatch layer, before `run()`), then injects a structured prompt that asks
//! the model to inspect the working tree and write `AGENTS.md`. Mirrors the
//! `InjectSkill` pattern used by `/imagine` and `/loop`.

use std::path::Path;

use agent_client_protocol as acp;

use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand, slash_meta};

pub const OVERVIEW_FILENAME: &str = "AGENTS.md";

pub struct InitCommand;

/// Instructions sent on the wire. Kept as one block so the model receives an
/// unambiguous, self-contained task rather than a chat-like request.
fn init_instruction() -> String {
    [
        "Analyze the current working directory and write a concise project overview to AGENTS.md so a future AI agent can get oriented quickly.",
        "",
        "Hard constraints:",
        "- AGENTS.md does not exist yet; this was verified before the prompt was sent. Create it at the working-directory root and do not overwrite any existing file.",
        "- Never invent commands, paths, versions or conventions. Read the real build manifests, entry points, top-level directories, test layout and toolchain/config files first.",
        "- Keep it short and factual. Do not claim that any check passed unless you actually ran it.",
        "",
        "Include, using the project's primary documentation language:",
        "1. a one-line description of what the project does;",
        "2. a directory map limited to paths that actually exist;",
        "3. build, test and run commands taken from real manifests or task runners;",
        "4. the few conventions that are verifiable from files, with pointers to the authoritative documents.",
        "",
        "After writing, report the file path and a one-line summary of what you recorded.",
    ]
    .join("\n")
}

impl SlashCommand for InitCommand {
    slash_meta! {
        name: "init",
        description: "Generate an AGENTS.md project overview for AI agents",
        usage: "/init",
        takes_args: false,
        args_required: false,
    }

    /// Fail closed: never replace an overview a human or another agent wrote.
    fn pre_run_refusal(&self, cwd: &Path) -> Option<String> {
        cwd.join(OVERVIEW_FILENAME).exists().then(|| {
            format!(
                "{OVERVIEW_FILENAME} already exists in {}. /init will not overwrite it. \
                 Review or edit the existing file instead.",
                cwd.display()
            )
        })
    }

    fn run(&self, _ctx: &mut CommandExecCtx, _args: &str) -> CommandResult {
        CommandResult::InjectSkill {
            display_text: format!("/init — generate {OVERVIEW_FILENAME}"),
            prompt_blocks: vec![acp::ContentBlock::Text(acp::TextContent::new(
                init_instruction(),
            ))],
            display_as_skill: false,
            scheduled_task_preview: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_when_overview_already_exists() {
        let dir = std::env::temp_dir().join(format!(
            "aster-init-test-{}-{}",
            std::process::id(),
            line!()
        ));
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(dir.join(OVERVIEW_FILENAME), "hand-written").unwrap();
        let refusal = InitCommand.pre_run_refusal(&dir);
        assert!(refusal.is_some(), "existing overview must be refused");
        assert!(refusal.unwrap().contains("will not overwrite"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn allows_when_overview_is_absent() {
        let dir = std::env::temp_dir().join(format!(
            "aster-init-test-{}-{}",
            std::process::id(),
            line!()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        assert!(InitCommand.pre_run_refusal(&dir).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn injected_prompt_states_the_no_overwrite_rule() {
        let text = init_instruction();
        assert!(text.contains(OVERVIEW_FILENAME));
        assert!(text.contains("do not overwrite any existing file"));
        assert!(text.contains("Never invent commands"));
    }

    #[test]
    fn metadata_declares_no_arguments() {
        assert_eq!(InitCommand.name(), "init");
        assert!(!InitCommand.takes_args());
        assert!(!InitCommand.args_required());
    }
}
