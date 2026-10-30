#![cfg(test)]

use super::automatic::{now_ms, policy, request};
use super::*;
use crate::agent_api::{AgentRunSpawn, GoalTickAdmission};
use crate::goal_control::GoalTickDecisionV1;

mod productive;

const NOT_ACHIEVED: &str =
    r#"{"achieved":false,"progress":0.0,"remaining_criteria":["Original source is checked"]}"#;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn repeated_observations_under_new_run_identities_stop_and_stay_stalled_after_reopen() {
    for file in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let storage = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("source.txt"), "current source").unwrap();
        let store: Arc<dyn SessionStore> = if file {
            Arc::new(FileSessionStore::new(storage.path()).await.unwrap())
        } else {
            Arc::new(MemorySessionStore::new())
        };
        let limits = policy(true, 8, 2, now_ms() + 600_000);
        let (agent, session, client) = fixture_with(
            root.path(),
            true,
            options("goal-run")
                .with_session_store(store.clone())
                .with_permission_policy(
                    crate::permissions::PermissionPolicy::new().allow("verify_source(*)"),
                ),
            Some(limits.clone()),
        )
        .await;
        session
            .register_dynamic_tool(Arc::new(acceptance::VerifySource("current source".into())))
            .unwrap();
        for index in 0..3 {
            acceptance::script(&client, NOT_ACHIEVED);
            {
                let mut responses = client.scripted.lock().unwrap();
                let mut repeated = responses.front().unwrap().clone();
                if let crate::llm::ContentBlock::ToolUse { id, .. } =
                    &mut repeated.message.content[0]
                {
                    *id = "verify-repeated".into();
                }
                responses.insert(1, repeated);
            }
            let GoalTickAdmission::Admitted(AgentRunSpawn::Started { worker, .. }) = session
                .tick_goal(&request(&session, &format!("novelty-{index}")))
                .await
                .unwrap()
            else {
                panic!(
                    "slice {index}: the first observation and two bounded stalled attempts must be admitted"
                )
            };
            tokio::time::timeout(std::time::Duration::from_secs(10), worker)
                .await
                .unwrap()
                .unwrap();
            let facts = settlement::facts(root.path());
            eprintln!(
                "NOVELTY slice={index} calls={} progress={}",
                client.calls.load(Ordering::SeqCst),
                facts
                    .iter()
                    .rfind(|fact| fact.kind == "a3s.goal.slice.settled")
                    .unwrap()
                    .payload["body"]["progress"]
            );
        }
        let decision = session
            .preview_goal_tick(&request(&session, "must-stop"))
            .await
            .unwrap();
        let all = settlement::facts(root.path());
        assert!(
            client
                .inputs
                .lock()
                .unwrap()
                .iter()
                .all(|input| !input.contains("observation_digest")),
            "internal metering hashes must not consume main or assessment model context"
        );
        let progress: Vec<_> = all
            .iter()
            .filter(|fact| fact.kind == "a3s.goal.slice.settled")
            .map(|fact| {
                fact.payload["body"]["progress"]["information_delta"]
                    .as_u64()
                    .unwrap()
            })
            .collect();
        let identities: std::collections::BTreeSet<_> = all
            .iter()
            .filter(|fact| fact.kind == "tool.result")
            .filter_map(|fact| {
                serde_json::from_value::<a3s_effect::ToolExecutionResultV1>(
                    fact.payload["receipt"].clone(),
                )
                .ok()
            })
            .map(|receipt| receipt.identity.key().unwrap())
            .collect();
        session.save().await.unwrap();
        session.close().await;
        agent.close().await;

        let agent = Agent::from_config(fixture_config(Some(limits)))
            .await
            .unwrap();
        let store: Arc<dyn SessionStore> = if file {
            Arc::new(FileSessionStore::new(storage.path()).await.unwrap())
        } else {
            store
        };
        let restored = agent
            .resume_session_async("goal-run", options("goal-run").with_session_store(store))
            .await
            .unwrap();
        let reopened = restored
            .preview_goal_tick(&request(&restored, "must-stay-stopped"))
            .await
            .unwrap();
        restored.close().await;
        agent.close().await;
        assert_eq!(client.calls.load(Ordering::SeqCst), 12);
        assert_eq!(
            identities.len(),
            6,
            "each repeated observation has a distinct original receipt"
        );
        assert_eq!(decision, GoalTickDecisionV1::NeedsInput);
        assert_eq!(reopened, GoalTickDecisionV1::NeedsInput);
        assert_eq!(
            progress,
            vec![1, 0, 0],
            "new receipt identity is not new tool information"
        );
    }
}
