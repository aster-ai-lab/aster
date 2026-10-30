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
#[path = "long_horizon/scripted.rs"]
mod scripted;
#[path = "long_horizon/upstream.rs"]
mod upstream;

use a3s_code_core::config::CodeConfig;
use serde_json::json;
use std::path::PathBuf;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "H1 actual ACP and compiler VM; requires explicit qualified assets and evidence directory, no real provider"]
async fn scripted_rust_multifile_repair_binds_real_vm_verification() {
    rust_workflow(false).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "H1 exact-contract repair through actual ACP and compiler VM; explicit qualified assets/evidence required"]
async fn masked_verification_is_repaired_with_the_admitted_exact_command() {
    rust_workflow(true).await;
}

async fn rust_workflow(repair_masked_check: bool) {
    tokio::task::LocalSet::new().run_until(async {
        let assets = PathBuf::from(std::env::var_os("ASTER_TEST_OCI_ASSETS").expect("explicit qualified assets"));
        let evidence = PathBuf::from(std::env::var_os("A3S_LH_EVIDENCE_DIR").expect("explicit evidence directory"));
        assert!(assets.is_absolute() && evidence.is_absolute());
        let runtime_before = runtime::identity(&assets).unwrap();
        let root = tempfile::tempdir().unwrap();
        let task = fixture::Fixture::create(root.path()).unwrap();
        let baseline = task.check_in_guest(&assets).await.unwrap();
        assert!(baseline.compile_ok && !baseline.correct && !baseline.tests_ok);
        let server = scripted::Scripted::start(repair_masked_check).await;
        let meter = relay::Relay::start(upstream::Route {
            endpoint:server.endpoint.clone(),headers:vec![("Authorization".into(),format!("Bearer {}",scripted::KEY))],model:scripted::MODEL.into(),protected_values:vec![scripted::KEY.as_bytes().to_vec()],
        },24,4096).await.unwrap();
        let home = root.path().join("home");
        std::fs::create_dir_all(home.join("tmp")).unwrap();
        let catalog = CodeConfig::from_acl("providers \"openai\" { models \"fixture-model\" { limit = { context = 32768, output = 4096 } } }").unwrap();
        let provider = &catalog.providers[0];
        let acl = config::isolated_acl(provider,&provider.models[0],meter.endpoint(),&home.join("state"),0).unwrap();
        let acl = config::with_rust_verification(&acl,&task.workspace).unwrap();
        let acl_path = root.path().join("isolated.acl");
        std::fs::write(&acl_path,acl).unwrap();
        let output = evidence.join(format!("rust-scripted-{}",uuid::Uuid::new_v4()));
        std::fs::create_dir(&output).unwrap();
        let frozen = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(3).unwrap().join("target/qualification").join(output.file_name().unwrap());
        let (binary,aster) = live::freeze_pair(&frozen);
        let before = fixture::digest(&binary).unwrap();
        let aster_before = fixture::digest(&aster).unwrap();
        let driver = driver::Driver::spawn(&binary,&task.workspace,&home,&acl_path,meter.key(),true,Some(&assets)).unwrap();
        std::fs::write(output.join("process-launch.json"), serde_json::to_vec_pretty(&json!({
            "pid":driver.process_id(), "parent_pid":std::process::id(), "workspace":task.workspace,
            "binary":binary, "scope":"actual Driver child; excludes build-id probes",
        })).unwrap()).unwrap();
        let run = rust_workflow::RustRun::execute(driver,&task,&assets,std::time::Duration::from_secs(180)).await;
        let meter = meter.close().await.unwrap();
        let server = server.close().await;
        let runtime_after = runtime::identity(&assets).unwrap();
        let identity = fixture::digest(&binary).unwrap() == before && fixture::digest(&aster).unwrap() == aster_before && runtime_before == runtime_after;
        let usage_bound = meter.upstream_calls == server.requests && meter.verified_token_total() == Some(server.requests as u64 * 2);
        let initial_exit_code_bound = server.saw_initial_exit_code && server.initial_exit_code.map(i64::from) == run.wire_checks["exact_checks"][0]["exit_code"].as_i64();
        let pass = run.passed() && identity && usage_bound && initial_exit_code_bound && (!repair_masked_check || server.saw_exact_contract_advice);
        run.retain(&output,&task,pass,json!({"scope":"LH01 H1 controlled model; no real LLM qualification","build_id":env!("ASTER_PAIR_BUILD_ID"),"acp_sha256":before,"aster_sha256":aster_before,"frozen_pair_directory":frozen,"identity_unchanged":identity,"runtime_before":runtime_before,"runtime_after":runtime_after,"baseline":baseline,"scripted":server,"relay":meter,"usage_bound":usage_bound,"initial_exit_code_bound":initial_exit_code_bound,"usage_fixture":"each actual controlled generation reports 1 input and 1 output; includes auxiliary requests; not a tokenizer or real cost measurement","repair_mode":"four independent write calls in one completion; no fixed per-module repair order"}));
        println!("RUST_SCRIPTED_RESULT {}",json!({"pass":pass,"identity":identity,"evidence":output,"model_requests":server.requests,"main_steps":server.main_steps,"prompt_ms":run.prompt_ms}));
        assert!(pass,"scripted Rust workflow failed; original evidence retained");
    }).await;
}
