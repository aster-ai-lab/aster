use super::*;

#[test]
fn unresolved_effect_keeps_the_failure_and_offers_review_before_a_new_session() {
    for (error, needs_review) in [
        (
            "tool_effect_unresolved: 1 invocation obligation(s) require identity-bound reconciliation; local scans, final text, and ordinary waivers cannot discharge them.",
            true,
        ),
        ("Connection reset by peer", false),
    ] {
        let mut app = test_app_with_agent();
        let id = AgentId(0);
        let agent = app.agents.get_mut(&id).unwrap();
        agent.session.state = AgentState::TurnRunning;
        agent.turn_started_at = Some(std::time::Instant::now());
        dispatch(
            Action::TaskComplete(TaskResult::PromptResponse {
                agent_id: id,
                result: Err(error.into()),
                http_status: None,
                prompt_id: None,
            }),
            &mut app,
        );
        let scrollback = &agent_ref(&app, id).scrollback;
        assert!((0..scrollback.len()).any(|index| matches!(
            scrollback.entry(index).map(|entry| &entry.block),
            Some(RenderBlock::SessionEvent(event))
                if matches!(&event.event, SessionEvent::TurnFailed { error: actual, .. } if actual == error)
        )));
        let review_hint = (0..scrollback.len()).any(|index| {
            matches!(
                scrollback.entry(index).map(|entry| &entry.block),
                Some(RenderBlock::System(message))
                    if message.text.contains("Review the operation and workspace changes")
                        && message.text.contains("/new")
                        && message.text.contains("keeps the unconfirmed effects")
            )
        });
        assert_eq!(review_hint, needs_review);
        assert_eq!(app.agents.len(), 1);
    }
}
