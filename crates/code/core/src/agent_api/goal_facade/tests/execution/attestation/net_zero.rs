#![cfg(test)]

use super::*;
use crate::agent_api::{AgentRunSpawn, GoalTickAdmission};
use crate::goal_control::GoalTickDecisionV1;

struct CurrentBytes(std::path::PathBuf);

impl crate::CompletionAttestor for CurrentBytes {
    fn attest(
        &self,
        digest: &str,
        paths: &[crate::MutatedPathRecord],
    ) -> Option<VerificationReport> {
        if paths.len() != 1 || paths[0].path != "source.txt" {
            return None;
        }
        let bytes = std::fs::read(self.0.join("source.txt")).ok()?;
        if paths[0].content_digest != sha256::digest(&bytes) {
            return None;
        }
        Some(
            VerificationReport::new(
                "host:current-byte-identity",
                vec![VerificationCheck::required(
                    "inspect",
                    "current-bytes",
                    "Original source bytes match their receipt",
                )
                .with_status(VerificationStatus::Passed)],
            )
            .with_effect_digest(digest),
        )
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn repeated_workspace_states_stop_while_legal_v2_history_replays_unchanged() {
    for file in [false, true] {
        for legacy in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let storage = tempfile::tempdir().unwrap();
            std::fs::write(root.path().join("source.txt"), "a").unwrap();
            let store: Arc<dyn SessionStore> = if file {
                Arc::new(FileSessionStore::new(storage.path()).await.unwrap())
            } else {
                Arc::new(MemorySessionStore::new())
            };
            let limits = automatic::policy(true, 8, 1, automatic::now_ms() + 600_000);
            let (agent, session, client) = fixture_with(
                root.path(),
                true,
                options("goal-run")
                    .with_session_store(store.clone())
                    .with_assistant_final_response(true)
                    .with_completion_attestor(Arc::new(CurrentBytes(root.path().into())))
                    .with_permission_policy(
                        crate::permissions::PermissionPolicy::new().allow("write(*)"),
                    ),
                Some(limits.clone()),
            )
            .await;
            let rounds = if legacy { 5 } else { 3 };
            for index in 0..rounds {
                script(&client, true);
                {
                    let mut responses = client.scripted.lock().unwrap();
                    if let crate::llm::ContentBlock::ToolUse { input, .. } =
                        &mut responses.front_mut().unwrap().message.content[0]
                    {
                        input["content"] =
                            serde_json::json!(if index % 2 == 0 { "b" } else { "a" });
                    }
                    responses.back_mut().unwrap().message = Message::assistant(
                        r#"{"achieved":false,"progress":0.0,"remaining_criteria":["more work"]}"#,
                    );
                }
                let tick = automatic::request(&session, &format!("state-cycle-{index}"));
                let admitted = if legacy && index < 4 {
                    let source = serde_json::from_value(serde_json::json!({
                        "policy":limits, "observed_unix_ms":automatic::now_ms(),
                        "progress_basis":"turn_progress_delta_v2",
                    }))
                    .unwrap();
                    crate::agent_api::conversation_runtime::spawn_goal_tick(
                        &session, tick, source, "inspect",
                    )
                    .await
                    .unwrap()
                } else {
                    let GoalTickAdmission::Admitted(admitted) =
                        session.tick_goal(&tick).await.unwrap()
                    else {
                        panic!(
                            "the finite historical prefix and current source must admit their intended rounds"
                        )
                    };
                    admitted
                };
                let AgentRunSpawn::Started { worker, .. } = admitted else {
                    panic!("no replay in a new original round")
                };
                tokio::time::timeout(std::time::Duration::from_secs(10), worker)
                    .await
                    .unwrap()
                    .unwrap();
            }
            let decision = session
                .preview_goal_tick(&automatic::request(&session, "cycle-stop"))
                .await
                .unwrap();
            let facts = settlement::facts(root.path());
            let bases: Vec<_> = facts
                .iter()
                .filter(|fact| fact.kind == "a3s.goal.slice.admitted")
                .map(|fact| {
                    fact.payload["body"]["continuation"]["progress_basis"]
                        .as_str()
                        .unwrap()
                        .to_owned()
                })
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
                .preview_goal_tick(&automatic::request(&restored, "cycle-reopened"))
                .await
                .unwrap();
            restored.close().await;
            agent.close().await;
            assert_eq!(client.calls.load(Ordering::SeqCst), rounds * 3);
            assert_eq!(decision, GoalTickDecisionV1::NeedsInput);
            assert_eq!(reopened, GoalTickDecisionV1::NeedsInput);
            assert_eq!(
                settlement::facts(root.path()),
                facts,
                "replay cannot rewrite accepted history"
            );
            assert_eq!(
                bases,
                if legacy {
                    vec!["turn_progress_delta_v2"; 4]
                        .into_iter()
                        .chain(["turn_progress_delta_v3"])
                        .collect::<Vec<_>>()
                } else {
                    vec!["turn_progress_delta_v3"; 3]
                }
            );
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn net_zero_write_cycles_cannot_refresh_automatic_progress_after_reopen() {
    for file in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let storage = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join("source.txt"),
            "independent expected source",
        )
        .unwrap();
        let store: Arc<dyn SessionStore> = if file {
            Arc::new(FileSessionStore::new(storage.path()).await.unwrap())
        } else {
            Arc::new(MemorySessionStore::new())
        };
        let limits = automatic::policy(true, 8, 1, automatic::now_ms() + 600_000);
        let opts = options("goal-run")
            .with_session_store(store.clone())
            .with_assistant_final_response(true)
            .with_completion_attestor(Arc::new(ByteAttestor {
                root: root.path().into(),
                calls: AtomicUsize::new(0),
            }))
            .with_permission_policy(crate::permissions::PermissionPolicy::new().allow("write(*)"));
        let (agent, session, client) =
            fixture_with(root.path(), true, opts, Some(limits.clone())).await;
        for index in 0..2 {
            script(&client, true);
            {
                let mut responses = client.scripted.lock().unwrap();
                let mut restore = responses.front().unwrap().clone();
                if let crate::llm::ContentBlock::ToolUse { id, .. } =
                    &mut restore.message.content[0]
                {
                    *id = "restore-original".into();
                }
                if let crate::llm::ContentBlock::ToolUse { input, .. } =
                    &mut responses.front_mut().unwrap().message.content[0]
                {
                    input["content"] = serde_json::json!("temporary bytes");
                }
                responses.insert(1, restore);
                responses.back_mut().unwrap().message = Message::assistant(
                    r#"{"achieved":false,"progress":0.0,"remaining_criteria":["more work"]}"#,
                );
            }
            let GoalTickAdmission::Admitted(AgentRunSpawn::Started { worker, .. }) = session
                .tick_goal(&automatic::request(&session, &format!("net-zero-{index}")))
                .await
                .unwrap()
            else {
                panic!("the first observation and one stalled slice must be admitted")
            };
            tokio::time::timeout(std::time::Duration::from_secs(10), worker)
                .await
                .unwrap()
                .unwrap();
        }
        let decision = session
            .preview_goal_tick(&automatic::request(&session, "net-zero-stop"))
            .await
            .unwrap();
        let facts = settlement::facts(root.path());
        let deltas: Vec<_> = facts
            .iter()
            .filter(|fact| fact.kind == "a3s.goal.slice.settled")
            .map(|fact| {
                fact.payload["body"]["progress"]["information_delta"]
                    .as_u64()
                    .unwrap()
            })
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
            .preview_goal_tick(&automatic::request(&restored, "net-zero-reopened"))
            .await
            .unwrap();
        restored.close().await;
        agent.close().await;
        assert_eq!(
            std::fs::read(root.path().join("source.txt")).unwrap(),
            b"independent expected source"
        );
        assert_eq!(client.calls.load(Ordering::SeqCst), 8);
        assert_eq!(deltas, vec![2, 0]);
        assert_eq!(
            decision,
            GoalTickDecisionV1::NeedsInput,
            "intermediate writes are not net workspace progress"
        );
        assert_eq!(reopened, GoalTickDecisionV1::NeedsInput);
    }
}
