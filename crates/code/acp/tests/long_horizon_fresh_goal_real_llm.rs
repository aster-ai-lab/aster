//! A fresh Goal binds its stored objective; it has no earlier conversation facts.

#![cfg(all(test, target_os = "macos", target_arch = "aarch64"))]

#[path = "long_horizon/artifact.rs"]
mod artifact;
#[path = "long_horizon/candidate.rs"]
mod candidate;
#[path = "long_horizon/config.rs"]
mod config;
#[path = "long_horizon/driver.rs"]
mod driver;
#[path = "long_horizon/fixture.rs"]
mod fixture;
#[path = "long_horizon/live.rs"]
mod live;
#[path = "long_horizon/relay.rs"]
mod relay;
#[path = "long_horizon/runtime.rs"]
mod runtime;
#[path = "long_horizon/rust_workflow.rs"]
mod rust_workflow;
#[path = "long_horizon/upstream.rs"]
mod upstream;

use serde_json::{json, Value};

const OBJECTIVE_PREFIX: &str = "Complete the following task in at least two dependency-ordered stages. Accept each stage only with its current evidence. Preserve every requirement below.\n\n";

fn fresh_goal_complete(view: &Value, session_id: &str, objective: &str) -> bool {
    let definition = &view["definition"];
    definition["state"] == "completed"
        && definition["goal"]["description"] == objective
        && definition["binding"]["session_id"] == session_id
        && definition["plan_accepted"] == true
        && definition["plan_fact_sequence"].is_u64()
        && view["work_policy_digest"].is_string()
        && definition["binding"]["work_policy_digest"] == view["work_policy_digest"]
        && definition["binding"]["scope_fact_sequences"]
            .as_array()
            .is_some_and(|facts| facts.is_empty())
        && definition["plan"]["steps"].as_array().is_some_and(|steps| {
            steps.len() >= 2
                && steps.iter().all(|step| step["status"] == "completed")
                && steps.iter().any(|step| {
                    step["dependencies"]
                        .as_array()
                        .is_some_and(|deps| !deps.is_empty())
                })
        })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "registered fresh GLM5.3 Goal; original C01 prompt/VM oracle, completed dependent stages and new-process restore"]
async fn glm_fresh_staged_goal_repairs_rust_and_survives_new_acp_process() {
    let seed = fixture::FixtureSeed::from_environment().expect("registered fixture seed");
    tokio::task::LocalSet::new().run_until(async {
        let assets = std::path::PathBuf::from(std::env::var_os("ASTER_TEST_OCI_ASSETS").expect("qualified assets"));
        let runtime_before = runtime::identity(&assets).unwrap();
        let mut campaign = live::Campaign::prepare("rust-fresh-staged-goal-glm-diagnostic").await;
        assert_eq!(campaign.model, "glm-01/glm-5.3");
        let task = fixture::Fixture::create_seeded(campaign.root.path(), seed).unwrap();
        let objective = format!("{OBJECTIVE_PREFIX}{}", task.request());
        let baseline = task.check_in_guest(&assets).await.unwrap();
        assert!(baseline.compile_ok && !baseline.correct && !baseline.tests_ok);
        let run = rust_workflow::RustRun::execute_goal(campaign.spawn(&task.workspace, Some(&assets), true), &task, &assets, std::time::Duration::from_secs(1800)).await;
        let completed = run.session_id.as_ref().zip(run.goal_state.as_ref()).is_some_and(|(id, state)| fresh_goal_complete(state, id.0.as_ref(), &objective));
        let next = campaign.spawn(&task.workspace, Some(&assets), true);
        let restored = if let Some(id) = &run.session_id {
            match next.load(id, &task.workspace).await {
                Ok(()) => next.goal_state(id).await,
                Err(error) => Err(error),
            }
        } else { Err("original_session_not_opened") };
        let restored_same = run.session_id.as_ref().zip(run.goal_state.as_ref()).zip(restored.as_ref().ok()).is_some_and(|((id, before), after)| {
            fresh_goal_complete(after, id.0.as_ref(), &objective) && before["definition"] == after["definition"] && before["work_policy_digest"] == after["work_policy_digest"]
        });
        let reload_cleanup = next.close().await;
        let relay = campaign.relay.take().unwrap().close().await;
        let runtime_after = runtime::identity(&assets).unwrap();
        let identity = campaign.identity_unchanged() && runtime_before == runtime_after;
        let live_calls = relay.as_ref().is_ok_and(|stats| stats.upstream_calls > 0 && stats.statuses.get(&200).copied().unwrap_or(0) > 0 && stats.listener_closed);
        let pass = run.passed() && completed && restored_same && reload_cleanup.is_ok() && relay.is_ok() && live_calls && identity;
        run.retain(&campaign.output, &task, pass, json!({
            "campaign":campaign.name,"case":"C01","variant":"rust-multifile-fresh-staged-goal-real-diagnostic","fixture_seed":seed.number(),
            "model":campaign.model,"pair_build_id":campaign.build_id,"acp_sha256":campaign.acp_hash,"aster_sha256":campaign.aster_hash,
            "identity_unchanged":identity,"runtime_before":runtime_before,"runtime_after":runtime_after,"baseline":baseline,
            "goal_state":run.goal_state,"goal_completed_with_at_least_two_accepted_stages":completed,"restored_goal_state":restored,
            "same_definition_after_new_acp_process":restored_same,"reload_cleanup":reload_cleanup,"relay":relay,
            "scope":"fresh session: exact stored objective/session/policy binding, empty prior history; original Rust artifact and current-byte verification oracles unchanged",
            "limits":{"upstream_calls":40,"output_tokens_per_call":4096,"prompt_deadline_ms":600000,"workflow_deadline_ms":1800000},
            "not_claimed":"regrading the earlier history-required variant or original five-seed/full-catalog qualification"
        }));
        println!("RUST_FRESH_STAGED_GOAL_RESULT {}", json!({"pass":pass,"evidence":campaign.output}));
        assert!(pass, "fresh staged GLM Goal or current artifact/reload proof failed; evidence retained");
    }).await;
}

#[path = "long_horizon/fresh_goal_oracle_tests.rs"]
mod oracle_tests;
