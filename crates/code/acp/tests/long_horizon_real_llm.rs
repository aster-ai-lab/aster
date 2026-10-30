#![cfg(all(test, unix))]

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
#[path = "long_horizon/native_fixture.rs"]
mod native_fixture;
#[path = "long_horizon/relay.rs"]
mod relay;
#[path = "long_horizon/report.rs"]
mod report;
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[path = "long_horizon/runtime.rs"]
mod runtime;
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[path = "long_horizon/rust_workflow.rs"]
mod rust_workflow;
#[path = "long_horizon/upstream.rs"]
mod upstream;

use agent_client_protocol as acp;
use serde_json::json;
use std::time::Instant;

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "R1 LH01 real-provider Rust repair; requires explicit trusted ACL, model, qualified compiler VM assets and evidence directory"]
async fn real_rust_multifile_repair_binds_current_tests_and_independent_guest_oracle() {
    tokio::task::LocalSet::new().run_until(async {
        let assets = std::path::PathBuf::from(std::env::var_os("ASTER_TEST_OCI_ASSETS").expect("explicit qualified assets"));
        assert!(assets.is_absolute());
        let runtime_before = runtime::identity(&assets).unwrap();
        let mut campaign = live::Campaign::prepare("rust-multifile-real-diagnostic").await;
        let task = fixture::Fixture::create(campaign.root.path()).unwrap();
        let baseline = task.check_in_guest(&assets).await.unwrap();
        assert!(baseline.compile_ok && !baseline.correct && !baseline.tests_ok);
        let driver = campaign.spawn(&task.workspace,Some(&assets),true);
        let run = rust_workflow::RustRun::execute(driver,&task,&assets,std::time::Duration::from_secs(1800)).await;
        let relay = campaign.relay.take().unwrap().close().await;
        let live_calls = relay.as_ref().is_ok_and(|stats|stats.upstream_calls>0 && stats.statuses.get(&200).copied().unwrap_or(0)>0 && stats.listener_closed);
        let runtime_after = runtime::identity(&assets).unwrap();
        let identity = campaign.identity_unchanged() && runtime_before == runtime_after;
        let pass = run.passed() && identity && live_calls && relay.is_ok();
        let verified_tokens = relay.as_ref().ok().and_then(|stats| stats.verified_token_total());
        let record = report::Record {campaign:campaign.name.clone(),case:"C01".into(),variant:"rust-multifile-host-contract-real-diagnostic".into(),seed:2401,attempt:1,started:true,status:if pass {report::Status::Pass} else {report::Status::Fail},failure_layer:(!pass).then_some(report::FailureLayer::Undetermined),checks:[(report::Check::Identity,identity),(report::Check::Oracle,run.oracle.as_ref().is_ok_and(|oracle|oracle.correct)),(report::Check::Terminal,run.work.is_ok()),(report::Check::Cleanup,run.cleanup.is_ok() && run.tools_terminal && relay.is_ok()),(report::Check::CurrentArtifact,run.oracle.as_ref().is_ok_and(|oracle|oracle.correct)),(report::Check::SideEffects,run.oracle.as_ref().is_ok_and(|oracle|oracle.protected_unchanged))].into_iter().collect(),tokens:verified_tokens};
        let definitions = [report::Definition {case:"C01",variant:"rust-multifile-host-contract-real-diagnostic",required:&[report::Check::CurrentArtifact,report::Check::SideEffects]}];
        let summary = report::summarize(&campaign.name,&definitions,std::slice::from_ref(&record)).unwrap();
        run.retain(&campaign.output,&task,pass,json!({"campaign":campaign.name,"scope":"LH01 R1 single-seed diagnostic; remaining journeys, seeds and platforms unqualified","model":campaign.model,"pair_build_id":campaign.build_id,"acp_sha256":campaign.acp_hash,"aster_sha256":campaign.aster_hash,"frozen_pair_directory":campaign.binary.parent(),"identity_unchanged":identity,"runtime_before":runtime_before,"runtime_after":runtime_after,"baseline":baseline,"relay":relay,"record":record,"summary":summary,"host_profile":"strict completion gate with exact host-declared Rust test command; no byte-only attestation or waiver","raw_usage_presence_verified":verified_tokens.is_some(),"limits":{"upstream_calls":40,"output_tokens_per_call":4096,"prompt_deadline_ms":600000,"workflow_deadline_ms":1800000},"fixture_seed":2401,"model_sampling_seed":"unsupported by this relay profile"}));
        println!("RUST_REAL_RESULT {}",json!({"campaign":campaign.name,"pass":pass,"prompt_ms":run.prompt_ms,"evidence":campaign.output}));
        assert!(pass,"real Rust ACP workflow failed; original sanitized evidence retained");
    }).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "bounded real-provider ACP diagnostic; requires explicit trusted ACL, model and evidence directory"]
async fn real_native_file_workflow_multiple_user_turns() {
    tokio::task::LocalSet::new().run_until(async {
        let mut campaign = live::Campaign::prepare("native-multiturn-diagnostic").await;
        let task = native_fixture::NativeFixture::create(campaign.root.path()).unwrap();
        let baseline = task.prepare(1).unwrap();
        assert!(!task.check(&baseline), "missing artifact must fail the independent oracle");
        let driver = campaign.spawn(&task.workspace, None, false);
        let started = Instant::now();
        let mut turns = Vec::new();
        let work = tokio::time::timeout(std::time::Duration::from_secs(1800), async {
            let host_meta = json!({"a3sHost":{"completionAttestation":"disk_content"}}).as_object().unwrap().clone();
            let session = driver.open(&task.workspace,Some(host_meta)).await?;
            for revision in 1..=6 {
                let expected = task.prepare(revision).map_err(|_| "fixture_prepare")?;
                let source_order = task.read("order.json").map_err(|_| "fixture_order")?;
                let source_policy = task.read("policy.json").map_err(|_| "fixture_policy")?;
                let event_start = driver.capture.lock().unwrap().events.len();
                let answer = driver.prompt(&session,&format!("Revision {revision}: Read README.md, current order.json and policy.json. Update report.json to satisfy the stated contract. Preserve all source files and unrelated user work. Use native read/edit tools only; do not run commands or code. Finish with a concise account of the current result.")).await?;
                let outcome = driver.capture.lock().unwrap().events.iter().skip(event_start).find(|event| event["method"] == "a3s/turn/outcome" && event["params"]["sessionId"] == session.0.as_ref()).map(|event| event["params"]["outcome"].clone());
                let attested = outcome.as_ref().is_some_and(|outcome| outcome["terminal"] == "verified" && outcome["status"] == "passed");
                let correct = task.check(&expected);
                let sources_preserved = task.read("order.json").ok().as_ref() == Some(&source_order) && task.read("policy.json").ok().as_ref() == Some(&source_policy);
                turns.push(json!({"revision":revision,"expected":expected,"semantic_oracle":correct,"sources_preserved":sources_preserved,"host_byte_attestation":attested,"outcome":outcome,"answer":answer,"report":task.read("report.json").ok().and_then(|bytes|serde_json::from_slice::<serde_json::Value>(&bytes).ok())}));
                if !correct || !sources_preserved || !attested {return Err("semantic_or_attestation_oracle_failed");}
            }
            Ok(())
        }).await.unwrap_or(Err("workflow_deadline"));
        let (events, tools_terminal, tools, permissions, overflow, rpc_errors) = {
            let capture = driver.capture.lock().unwrap();
            (capture.events.clone(),capture.tools.values().all(|status| matches!(status,acp::ToolCallStatus::Completed|acp::ToolCallStatus::Failed)),capture.tools.len(),capture.permissions,capture.overflow,capture.rpc_errors.clone())
        };
        let child_cleanup = driver.close().await;
        let relay_cleanup = campaign.relay.take().unwrap().close().await;
        let identity = campaign.identity_unchanged();
        let cleanup = child_cleanup.is_ok() && relay_cleanup.is_ok() && tools_terminal;
        let live_calls = relay_cleanup.as_ref().is_ok_and(|stats| stats.upstream_calls > 0 && stats.statuses.get(&200).copied().unwrap_or(0) > 0);
        let correct = turns.len() == 6 && turns.iter().all(|turn| turn["semantic_oracle"] == true && turn["sources_preserved"] == true && turn["host_byte_attestation"] == true);
        let pass = work.is_ok() && correct && identity && cleanup && live_calls && !overflow && tools > 0;
        let verified_tokens = relay_cleanup.as_ref().ok().and_then(|stats| stats.verified_token_total());
        let record = report::Record {
            campaign: campaign.name.clone(),case:"C01".into(),variant:"native-data-multiturn-disk-attested-real-diagnostic".into(),seed:2401,attempt:1,started:true,
            status: if pass { report::Status::Pass } else { report::Status::Fail },
            failure_layer: (!pass).then_some(report::FailureLayer::Undetermined),
            checks: [(report::Check::Identity,identity),(report::Check::Oracle,correct),(report::Check::Terminal,work.is_ok()),(report::Check::Cleanup,cleanup),(report::Check::CurrentArtifact,correct),(report::Check::SideEffects,turns.iter().all(|turn| turn["sources_preserved"] == true))].into_iter().collect(),tokens:verified_tokens,
        };
        let definitions = [report::Definition {case:"C01",variant:"native-data-multiturn-disk-attested-real-diagnostic",required:&[report::Check::CurrentArtifact,report::Check::SideEffects]}];
        let summary = report::summarize(&campaign.name,&definitions,std::slice::from_ref(&record)).unwrap();
        let result = json!({"campaign":campaign.name,"scope":"C01 native data six-turn diagnostic only; Rust compilation, LH01 and remaining variants stay unqualified","model":campaign.model,"pair_build_id":campaign.build_id,"acp_sha256":campaign.acp_hash,"aster_sha256":campaign.aster_hash,"frozen_pair_directory":campaign.binary.parent(),"elapsed_ms":started.elapsed().as_millis(),"record":record,"summary":summary,"prompt_result":work,"turns":turns,"baseline_missing_artifact_refused":true,"relay":relay_cleanup,"child_cleanup":child_cleanup,"tool_calls":tools,"permission_requests":permissions,"raw_usage_presence_verified":verified_tokens.is_some(),"fixture_seed":2401,"model_sampling_seed":"not supported by this relay profile","limits":{"upstream_calls":40,"output_tokens_per_call":4096,"prompt_deadline_ms":600000,"workflow_deadline_ms":1800000,"relay_deadline_ms":120000,"trace_bytes":8388608},"cleanup_scope":"owned ACP Unix group, settled tool statuses, wire task and relay connections; crash/separate-group variants remain unqualified"});
        std::fs::write(campaign.output.join("result.json"),serde_json::to_vec_pretty(&result).unwrap()).unwrap();
        std::fs::write(campaign.output.join("wire-notifications.json"),serde_json::to_vec_pretty(&events).unwrap()).unwrap();
        std::fs::write(campaign.output.join("rpc-errors.json"),serde_json::to_vec_pretty(&rpc_errors).unwrap()).unwrap();
        std::fs::write(campaign.output.join("host-profile.json"),serde_json::to_vec_pretty(&json!({"completionAttestation":"disk_content","scope":"byte digest binding only; independent parent oracle proves arithmetic","requirements_revision":2,"previous_default_profile_failures_retained":true})).unwrap()).unwrap();
        for name in ["order.json","policy.json","report.json","README.md","notes.txt"] {
            if let Ok(bytes) = task.read(name) { std::fs::write(campaign.output.join(name),bytes).unwrap(); }
        }
        println!("LONG_HORIZON_RESULT {}",json!({"campaign":campaign.name,"model":campaign.model,"case":"C01","status":record.status,"elapsed_ms":started.elapsed().as_millis(),"evidence":campaign.output}));
        assert!(pass,"real ACP diagnostic failed; retained sanitized evidence before assertion");
    }).await;
}
