#![cfg(test)]

use super::*;

fn code_app() -> AppView {
    let mut app = test_app_with_agent();
    // Set the per-connection contract without changing process-global flags.
    app.is_a3s_code = true;
    let names = app.a3s_unavailable_command_names();
    for agent in app.agents.values_mut() {
        agent.set_backend_unavailable_commands(&names);
    }
    app
}

#[test]
fn code_backend_unavailable_commands_never_become_model_prompts() {
    for input in ["/dashboard", "/sessions"] {
        let mut app = code_app();
        let effects = dispatch(Action::SendPrompt(input.into()), &mut app);
        assert!(
            effects.is_empty(),
            "{input} escaped local command dispatch: {effects:?}"
        );
        assert!(agent_ref(&app, AgentId(0)).prompt.text().is_empty());
        assert!(scrollback_has_system_text(
            &app,
            AgentId(0),
            "is unavailable with the Code backend. Use /help for available commands."
        ));
    }

    let mut app = code_app();
    let effects = dispatch(Action::SendPrompt("/fork new-branch".into()), &mut app);
    assert!(
        matches!(effects.as_slice(), [Effect::ForkSession { .. }]),
        "the enabled prefix fork must stay a local action: {effects:?}"
    );
    assert!(agent_ref(&app, AgentId(0)).prompt.text().is_empty());
}

#[test]
fn code_unknown_command_never_becomes_a_model_prompt() {
    let mut app = code_app();
    let effects = dispatch(Action::SendPrompt("/unknown-cmd arg1".into()), &mut app);
    assert!(
        effects.is_empty(),
        "unknown command escaped local dispatch: {effects:?}"
    );
    assert!(scrollback_has_system_text(
        &app,
        AgentId(0),
        "Unknown TUI command: /unknown-cmd. Use /help for available commands."
    ));
}

#[test]
fn code_bare_slash_never_becomes_a_model_prompt() {
    let mut app = code_app();
    let effects = dispatch(Action::SendPrompt("/".into()), &mut app);
    assert!(
        effects.is_empty(),
        "bare slash escaped local dispatch: {effects:?}"
    );
    assert!(scrollback_has_system_text(
        &app,
        AgentId(0),
        "Enter a TUI command after /. Use /help for available commands."
    ));
}

#[test]
fn code_ordinary_and_literal_prompts_keep_their_original_text() {
    let mut app = code_app();
    let effects = dispatch(Action::SendPrompt("explain /dashboard".into()), &mut app);
    assert!(
        matches!(effects.as_slice(), [Effect::SendPrompt { text, .. }] if text == "explain /dashboard")
    );

    let mut app = code_app();
    let effects = crate::app::dispatch::prompt::dispatch_send_prompt_inner(
        &mut app,
        "/unknown-cmd literal text".into(),
        true,
        true,
        false,
    );
    assert!(
        matches!(effects.as_slice(), [Effect::SendPrompt { text, .. }] if text == "/unknown-cmd literal text")
    );
}

#[test]
fn code_direct_shell_refusal_preserves_draft_without_queue_or_turn() {
    for bound in [true, false] {
        let mut app = code_app();
        let id = AgentId(0);
        let agent = app.agents.get_mut(&id).unwrap();
        if !bound {
            agent.session.session_id = None;
        }
        agent.prompt_input_mode = crate::app::agent_view::PromptInputMode::Bash;
        agent.prompt.set_text("printf retained-command");
        let effects = dispatch(
            Action::SendBashCommand("printf retained-command".into()),
            &mut app,
        );
        let agent = agent_ref(&app, id);
        assert!(effects.is_empty());
        assert_eq!(agent.session.queue_len(), 0);
        assert!(!agent.session.state.is_turn_running());
        assert_eq!(agent.prompt.text(), "printf retained-command");
        assert!(agent.session.prompt_history.is_empty());
        assert_eq!(
            agent.toast.as_ref().map(|(message, _)| message.as_str()),
            Some("Direct shell commands are unavailable with the Code backend.")
        );
    }
}
