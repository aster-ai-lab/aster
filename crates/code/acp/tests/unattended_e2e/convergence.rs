use super::*;

#[derive(Default)]
pub(super) struct ObservationScript {
    pub(super) source: Option<PathBuf>,
    pub(super) compactions: usize,
    requests: usize,
    fresh: bool,
}

fn source_text(version: usize) -> String {
    format!("version {version}: {}\n", "x".repeat(192)).repeat(128)
}

pub(super) fn observation_reply(script: &mut ObservationScript, streaming: bool) -> String {
    if script.fresh && script.requests == 7 {
        return reply_text("All seven external observations are recorded.", streaming);
    }
    script.requests += 1;
    if script.fresh {
        std::fs::write(
            script.source.as_ref().unwrap(),
            source_text(script.requests),
        )
        .unwrap();
    }
    let reply = reply_tool(
        "read",
        serde_json::json!({"file_path":"source.txt"}),
        script.requests as u64,
        streaming,
    );
    let usage =
        serde_json::json!({"prompt_tokens":14_000,"completion_tokens":1,"total_tokens":14_001});
    if streaming {
        // The usage frame precedes finish_reason so the client sees it before completion.
        let frame = serde_json::json!({"choices":[],"usage":usage});
        format!("data: {frame}\n\n{reply}")
    } else {
        let mut body: serde_json::Value = serde_json::from_str(&reply).unwrap();
        body["usage"] = usage;
        body.to_string()
    }
}

#[tokio::test(flavor = "current_thread")]
async fn actual_acp_compaction_keeps_the_original_repeat_allowance() {
    observation_run(false).await;
}

#[tokio::test(flavor = "current_thread")]
async fn actual_acp_compaction_allows_new_external_observations() {
    observation_run(true).await;
}

async fn observation_run(fresh: bool) {
    tokio::task::LocalSet::new().run_until(async {
        let workspace = isolate("convergence 观察-ws");
        let outside = isolate("convergence-state");
        let home = isolate("convergence-home");
        let tmp = isolate("convergence-tmp");
        let source = workspace.join("source.txt");
        std::fs::write(&source, source_text(0)).unwrap();
        let (addr, script) = spawn_mock();
        script.lock().unwrap().convergence = ObservationScript { source: Some(source.clone()), fresh, ..Default::default() };
        let config = outside.join("config.acl");
        std::fs::write(&config, format!(r#"
providers "mock" {{
  base_url = "http://{addr}/v1"
  api_key = "test"
  models "gpt" {{ limit = {{ context = 16384, output = 1024 }} }}
}}
default_model = "{MODEL_ID}"
"#)).unwrap();
        let agent = spawn_agent(&workspace, &config, &home, &tmp, &[]).await;
        let session = open(&agent, &workspace).await;
        let result = tokio::time::timeout(Duration::from_secs(15), say(
            &agent, &session, "UNATTENDED-TASK Observe source.txt and report the external observations.",
        )).await.expect("the controlled run must settle before the watchdog");
        let state = script.lock().unwrap();
        let observations = &state.convergence;
        let facts = private_fixture_facts(&workspace);
        let compacted: Vec<_> = facts.iter().filter(|fact| fact["type"] == "compaction.done").collect();
        assert!(observations.compactions >= 2, "actual model compaction was not exercised");
        assert!(compacted.len() >= 2, "original compaction facts must be retained");
        assert!(compacted.iter().any(|fact| fact["payload"]["kept"].as_array().is_some_and(|kept|
            kept.iter().filter_map(serde_json::Value::as_str).filter(|line| line.starts_with("assistant\ntool ")).count() == 1)),
            "compaction must actually discard prior tool-call messages");
        let succeeded = facts.iter().filter(|fact| fact["type"] == "tool.result"
            && fact["payload"]["receipt"]["outcome"] == "succeeded").count();
        let denied = facts.iter().filter(|fact| fact["type"] == "tool.result"
            && fact["payload"]["receipt"]["outcome"] == "denied").count();
        if fresh {
            assert!(result.is_ok(), "new external observations must remain eligible: {result:?}");
            assert_eq!(result.unwrap().stop_reason, acp::StopReason::EndTurn);
            assert_eq!(observations.requests, 7);
            assert_eq!((succeeded, denied), (7, 0));
            assert_eq!(std::fs::read_to_string(&source).unwrap(), source_text(7));
        } else {
            let error = result.expect_err("an unchanged observation cycle is not successful completion");
            assert!(format!("{error:?}").contains("run.tool_refusal_cycle:"), "{error:?}");
            assert_eq!(observations.requests, 5);
            assert_eq!((succeeded, denied), (3, 2));
            assert_eq!(std::fs::read_to_string(&source).unwrap(), source_text(0));
        }
        assert!(!facts.iter().any(|fact| fact["type"] == "aster.goal.completed"));
        eprintln!("ACP_CONVERGENCE {}", serde_json::json!({"fresh":fresh,"requests":observations.requests,
            "successfulReceipts":succeeded,"deniedReceipts":denied,"compactions":observations.compactions,"workspace":workspace}));
    }).await;
}
