#![cfg(test)]

use super::acceptance::{script, VerifySource};
use super::automatic::{now_ms, policy, request, ACHIEVED};
use super::*;
use crate::agent_api::{AgentRunSpawn, GoalTickAdmission};

async fn execute(session: &AgentSession, tick: &GoalControlCommandV1) -> String {
    match session.tick_goal(tick).await.unwrap() {
        GoalTickAdmission::Admitted(AgentRunSpawn::Started { worker, .. }) => {
            tokio::time::timeout(std::time::Duration::from_secs(10), worker)
                .await
                .unwrap()
                .unwrap();
        }
        _ => panic!("expected one new admitted Run"),
    }
    let run = session.run_snapshot(&tick.operation_id).await.unwrap();
    assert_eq!(run.status, crate::run::RunStatus::Completed);
    run.prompt
}

#[tokio::test]
async fn rejected_goal_feedback_survives_reopen_and_informs_only_its_bound_step() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("source.txt"), "current source").unwrap();
    let limits = policy(true, 8, 8, now_ms() + 600_000);
    let opts = options("goal-run").with_permission_policy(
        crate::permissions::PermissionPolicy::new().allow("verify_source(*)"),
    );
    let (agent, session, client) =
        fixture_with(root.path(), true, opts.clone(), Some(limits.clone())).await;
    session
        .register_dynamic_tool(Arc::new(VerifySource("current source".into())))
        .unwrap();
    let criteria = vec![
        "Required deliverable /app/output/mutation.report.json does not exist",
        "中文验收\nKeep the exact order and `quotes`",
        "Required deliverable /app/output/mutation.report.json does not exist",
    ];
    script(
        &client,
        &serde_json::json!({"achieved": false, "progress": 0.5, "remaining_criteria": criteria})
            .to_string(),
    );
    let first = request(&session, "feedback-first");
    let original = execute(&session, &first).await;
    assert_eq!(
        original,
        "Execute only the current admitted step `inspect` using fresh evidence."
    );
    let view = session.goal_definition().unwrap().unwrap();
    assert_ne!(
        view.plan.unwrap().steps[0].status,
        crate::planning::TaskStatus::Completed
    );
    assert!(
        session
            .goal_tick_feedback(&first, "inspect")
            .unwrap()
            .is_none(),
        "replay excludes its own later judgment"
    );
    let facts = crate::fact_control::read_workspace_facts(root.path(), "goal-run").unwrap();
    let closeout = facts.last().unwrap();
    let bad = crate::goal_control::GoalStepFeedback {
        step_id: "verify".into(),
        remaining_criteria: vec!["wrong step".into()],
    };
    assert!(crate::goal_control::prepare_settlement(
        &facts[..facts.len() - 1],
        "goal-run",
        &first.workspace,
        first.work_policy_digest.as_deref(),
        &first.operation_id,
        Some(Default::default()),
        None,
        Some(bad)
    )
    .is_err());
    assert_eq!(closeout.payload["body"]["feedback"]["step_id"], "inspect");
    let next = request(&session, "feedback-next");
    for mismatch in 0..4 {
        let mut other = next.clone();
        match mismatch {
            0 => other.goal_id = "another-goal".into(),
            1 => other.expected_goal_revision += 1,
            2 => other.expected_plan_revision = Some(999),
            _ => other.work_policy_digest = Some("another-policy".into()),
        }
        assert!(session
            .goal_tick_feedback(&other, "inspect")
            .unwrap()
            .is_none());
    }
    assert!(session
        .goal_tick_feedback(&next, "verify")
        .unwrap()
        .is_none());
    session.save().await.unwrap();
    session.close().await;
    agent.close().await;

    let agent = Agent::from_config(fixture_config(Some(limits)))
        .await
        .unwrap();
    let client = Arc::new(GoalClient::default());
    let session = agent
        .build_session(
            root.path().display().to_string(),
            client.clone(),
            &opts
                .with_planning_mode(crate::PlanningMode::Enabled)
                .with_goal_tracking(true),
        )
        .unwrap();
    session
        .register_dynamic_tool(Arc::new(VerifySource("current source".into())))
        .unwrap();
    assert_eq!(
        session
            .goal_tick_feedback(&next, "inspect")
            .unwrap()
            .unwrap()
            .remaining_criteria,
        criteria
    );
    script(&client, ACHIEVED);
    let prompt = execute(&session, &next).await;
    let data: serde_json::Value = serde_json::from_str(prompt.lines().last().unwrap()).unwrap();
    assert_eq!(data["remaining_criteria"], serde_json::json!(criteria));
    assert_eq!(data["goal_id"], next.goal_id);
    assert_eq!(data["goal_revision"], next.expected_goal_revision);
    assert_eq!(
        data["plan_revision"],
        serde_json::json!(next.expected_plan_revision)
    );
    assert_eq!(data["step_id"], "inspect");
    assert!(
        client.inputs.lock().unwrap()[0].contains(&prompt),
        "the actual execution model receives feedback"
    );
    assert_eq!(
        session
            .goal_definition()
            .unwrap()
            .unwrap()
            .plan
            .unwrap()
            .steps[0]
            .status,
        crate::planning::TaskStatus::Completed
    );
    assert_eq!(
        client.calls.load(Ordering::SeqCst),
        3,
        "acceptance requires a fresh tool observation and another judgment"
    );
    assert_eq!(
        session
            .goal_tick_feedback(&next, "inspect")
            .unwrap()
            .unwrap()
            .remaining_criteria,
        criteria,
        "replaying this operation ignores its own successful judgment"
    );
    let subsequent = request(&session, "feedback-after-success");
    assert!(
        session
            .goal_tick_feedback(&subsequent, "inspect")
            .unwrap()
            .unwrap()
            .remaining_criteria
            .is_empty(),
        "an empty newer judgment clears old feedback"
    );
    session.close().await;
    agent.close().await;
}

#[tokio::test]
async fn empty_remaining_criteria_preserves_the_original_automatic_tick_input() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("source.txt"), "current source").unwrap();
    let (agent, session, client) = fixture_with(
        root.path(),
        true,
        options("goal-run").with_permission_policy(
            crate::permissions::PermissionPolicy::new().allow("verify_source(*)"),
        ),
        Some(policy(true, 4, 4, now_ms() + 600_000)),
    )
    .await;
    session
        .register_dynamic_tool(Arc::new(VerifySource("current source".into())))
        .unwrap();
    script(
        &client,
        r#"{"achieved":false,"progress":0.5,"remaining_criteria":[]}"#,
    );
    execute(&session, &request(&session, "empty-first")).await;
    script(&client, ACHIEVED);
    let prompt = execute(&session, &request(&session, "empty-next")).await;
    assert_eq!(
        prompt,
        "Execute only the current admitted step `inspect` using fresh evidence."
    );
    session.close().await;
    agent.close().await;
}
