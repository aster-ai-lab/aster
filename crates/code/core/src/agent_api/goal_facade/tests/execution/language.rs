use super::*;

#[tokio::test]
async fn authored_goal_updates_reply_language_without_a_model_call_or_replay_reset() {
    let root = tempfile::tempdir().unwrap();
    let (agent, session, client) = fixture(root.path(), false, options("goal-language")).await;
    let before = session.goal_definition().unwrap().unwrap();
    let mut revision = command(&session, None);
    revision.kind = GoalCommandKindV1::Revise;
    revision.operation_id = "chinese-goal".into();
    revision.expected_goal_revision = Some(before.binding.goal_revision);
    revision.expected_plan_revision = before.plan_revision;
    revision.expected_goal_sequence = Some(before.goal_sequence);
    revision.goal.description = "准备好 agent-desktop 环境".into();
    session.apply_goal_definition(revision.clone()).unwrap();
    assert_eq!(session.reply_language().as_deref(), Some("zh-CN"));
    assert_eq!(client.calls.load(Ordering::SeqCst), 0);
    session.tool_context.set_reply_language(Some("en"));
    let sequence = session.goal_definition().unwrap().unwrap().goal_sequence;
    session.apply_goal_definition(revision).unwrap();
    assert_eq!(session.reply_language().as_deref(), Some("en"));
    assert_eq!(
        session.goal_definition().unwrap().unwrap().goal_sequence,
        sequence
    );
    assert_eq!(client.calls.load(Ordering::SeqCst), 0);
    session.close().await;
    agent.close().await;
}

#[tokio::test]
async fn automatic_goal_tick_preserves_user_language_through_english_prompts_and_results() {
    use super::automatic::{now_ms, policy, request, ACHIEVED};
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("source.txt"), "current source").unwrap();
    let permission = crate::permissions::PermissionPolicy::new().allow("verify_source(*)");
    let (agent, session, client) = fixture_with(
        root.path(),
        true,
        options("goal-run").with_permission_policy(permission),
        Some(policy(true, 1, 4, now_ms() + 600_000)),
    )
    .await;
    session.tool_context.set_reply_language(Some("zh-CN"));
    session
        .register_dynamic_tool(Arc::new(acceptance::VerifySource("current source".into())))
        .unwrap();
    acceptance::script(&client, ACHIEVED);
    let tick = request(&session, "language-tick");
    let crate::agent_api::GoalTickStream::Started(stream) =
        session.stream_goal_tick(&tick).await.unwrap()
    else {
        panic!("ready goal must start one stream");
    };
    let mut events = stream.events;
    while events.recv().await.is_some() {}
    stream.worker.await.unwrap();
    let systems = client.systems.lock().unwrap();
    assert!(systems.len() >= 2);
    let contract = crate::prompts::turn_output_language_override("zh-CN");
    assert!(systems[..2].iter().all(|system| system.contains(&contract)));
    assert_eq!(session.reply_language().as_deref(), Some("zh-CN"));
    drop(systems);
    session.close().await;
    agent.close().await;
}
