#![cfg(test)]

use super::*;

async fn run_slice(session: &AgentSession, operation: &str) {
    let GoalTickAdmission::Admitted(AgentRunSpawn::Started { worker, .. }) = session
        .tick_goal(&request(session, operation))
        .await
        .unwrap()
    else {
        panic!("the productive or bounded stalled slice must be admitted")
    };
    tokio::time::timeout(std::time::Duration::from_secs(10), worker)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn accepted_step_progress_continues_with_identical_tool_observations() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("source.txt"), "current source").unwrap();
    let (agent, session, client) = fixture_with(
        root.path(),
        true,
        settlement::opts(),
        Some(policy(true, 8, 1, now_ms() + 600_000)),
    )
    .await;
    session
        .register_dynamic_tool(Arc::new(acceptance::VerifySource("current source".into())))
        .unwrap();
    for (index, judgment) in [NOT_ACHIEVED, automatic::ACHIEVED, automatic::ACHIEVED]
        .into_iter()
        .enumerate()
    {
        acceptance::script(&client, judgment);
        run_slice(&session, &format!("accepted-progress-{index}")).await;
    }
    let goal = session.goal_definition().unwrap().unwrap();
    let deltas: Vec<_> = settlement::facts(root.path())
        .iter()
        .filter(|fact| fact.kind == "a3s.goal.slice.settled")
        .map(|fact| {
            (
                fact.payload["body"]["progress"]["information_delta"]
                    .as_u64()
                    .unwrap(),
                fact.payload["body"]["progress"]["plan_delta"]
                    .as_u64()
                    .unwrap(),
            )
        })
        .collect();
    session.close().await;
    agent.close().await;
    assert_eq!(goal.state, GoalState::Completed);
    assert_eq!(deltas, vec![(1, 0), (0, 1), (0, 1)]);
    assert_eq!(client.calls.load(Ordering::SeqCst), 9);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn genuinely_changed_observations_extend_progress_but_revisited_bytes_do_not() {
    let root = tempfile::tempdir().unwrap();
    let (agent, session, client) = fixture_with(
        root.path(),
        true,
        settlement::opts(),
        Some(policy(true, 8, 2, now_ms() + 600_000)),
    )
    .await;
    for (index, source) in ["first", "second", "first", "second"]
        .into_iter()
        .enumerate()
    {
        std::fs::write(root.path().join("source.txt"), source).unwrap();
        session
            .register_dynamic_tool(Arc::new(acceptance::VerifySource(source.into())))
            .unwrap();
        acceptance::script(&client, NOT_ACHIEVED);
        run_slice(&session, &format!("changed-progress-{index}")).await;
    }
    let decision = session
        .preview_goal_tick(&request(&session, "changed-must-stop"))
        .await
        .unwrap();
    let deltas: Vec<_> = settlement::facts(root.path())
        .iter()
        .filter(|fact| fact.kind == "a3s.goal.slice.settled")
        .map(|fact| {
            fact.payload["body"]["progress"]["information_delta"]
                .as_u64()
                .unwrap()
        })
        .collect();
    session.close().await;
    agent.close().await;
    assert_eq!(deltas, vec![1, 1, 0, 0]);
    assert_eq!(decision, GoalTickDecisionV1::NeedsInput);
    assert_eq!(client.calls.load(Ordering::SeqCst), 12);
}
