//! Missing sidecar: headless `spawn_agent` returns the interactive error text.
use std::ffi::OsString;

use a3s_code_shell::agent::config::Config as AgentConfig;
use tokio_util::sync::CancellationToken;

use super::{HeadlessOptions, OutputFormat};

struct RestoreAgentBin(Option<OsString>);

impl Drop for RestoreAgentBin {
    fn drop(&mut self) {
        match self.0.take() {
            Some(value) => unsafe { std::env::set_var("A3S_ACP_AGENT_BIN", value) },
            None => unsafe { std::env::remove_var("A3S_ACP_AGENT_BIN") },
        }
    }
}

fn blank_options() -> HeadlessOptions {
    HeadlessOptions {
        session_id: None,
        resume: None,
        resume_title_pinned: false,
        cwd: None,
        yolo: false,
        trust: false,
        output_format: OutputFormat::Plain,
        include_partial_messages: false,
        json_schema: None,
        model: None,
        rules: None,
        system_prompt_override: None,
        continue_last_session: false,
        fork_session: false,
        worktree: None,
        worktree_ref: None,
        restore_code: false,
        agent: None,
        agents_json: None,
        cli_tools: None,
        cli_disallowed_tools: None,
        disable_web_search: false,
        allow_rules: Vec::new(),
        deny_rules: Vec::new(),
        max_turns: None,
        permission_mode_flag: None,
        reasoning_effort: None,
        wait_for_background: true,
        background_wait_timeout: std::time::Duration::from_secs(0),
        memory_flush: false,
        memory_enabled_override: None,
    }
}

#[tokio::test]
async fn headless_missing_sidecar_uses_the_interactive_error() {
    let _guard = crate::acp::a3s_spawn::ACP_AGENT_BIN_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let restore = RestoreAgentBin(std::env::var_os("A3S_ACP_AGENT_BIN"));
    unsafe { std::env::remove_var("A3S_ACP_AGENT_BIN") };
    if crate::acp::a3s_spawn::a3s_acp_enabled() {
        let sibling = crate::acp::a3s_spawn::a3s_acp_executable();
        drop(restore);
        panic!(
            "sibling a3s-code-acp is present; missing-sidecar case is not this layout: {sibling:?}"
        );
    }
    let error = match super::spawn_agent(
        AgentConfig::default(),
        &CancellationToken::new(),
        None,
        &blank_options(),
    )
    .await
    {
        Err(error) => error,
        Ok(_spawned) => panic!("a missing sidecar fails closed"),
    };
    assert_eq!(
        error.to_string(),
        crate::acp::a3s_spawn::MISSING_AGENT_BACKEND
    );
    drop(restore);
}

#[tokio::test]
async fn headless_unusable_override_uses_the_interactive_error() {
    let _guard = crate::acp::a3s_spawn::ACP_AGENT_BIN_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let restore = RestoreAgentBin(std::env::var_os("A3S_ACP_AGENT_BIN"));
    let missing = std::env::temp_dir().join(format!(
        "pager-missing-acp-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    unsafe { std::env::set_var("A3S_ACP_AGENT_BIN", &missing) };
    let error = match super::spawn_agent(
        AgentConfig::default(),
        &CancellationToken::new(),
        None,
        &blank_options(),
    )
    .await
    {
        Err(error) => error,
        Ok(_spawned) => panic!("an unusable override fails closed"),
    };
    assert_eq!(
        error.to_string(),
        crate::acp::a3s_spawn::MISSING_AGENT_BACKEND
    );
    drop(restore);
}
