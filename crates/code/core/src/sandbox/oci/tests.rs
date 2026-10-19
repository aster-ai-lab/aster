#![cfg(test)]

use super::*;
use crate::workspace::{CommandOutputObserver, CommandOutputSummary};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Observer {
    output: Mutex<String>,
    summary: Mutex<Option<CommandOutputSummary>>,
}

#[async_trait]
impl CommandOutputObserver for Observer {
    async fn on_output_delta(&self, text: &str) {
        self.output.lock().unwrap().push_str(text);
    }
    async fn on_output_complete(&self, summary: &CommandOutputSummary) {
        *self.summary.lock().unwrap() = Some(*summary);
    }
}

fn assets() -> OciSandboxAssets {
    OciSandboxAssets::from_directory(Path::new(
        &std::env::var_os("ASTER_TEST_OCI_ASSETS").expect("explicit real OCI assets"),
    ))
    .unwrap()
}

fn request(
    command: String,
    observer: Option<Arc<dyn CommandOutputObserver>>,
    timeout_ms: u64,
) -> SandboxCommandRequest {
    SandboxCommandRequest {
        command,
        guest_workspace: "/workspace".into(),
        timeout_ms,
        output_observer: observer,
        env: None,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires signed HVF assets; run explicitly on Apple Silicon"]
async fn real_command_phase_timing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter("a3s_code_core::sandbox::oci=debug")
        .with_target(true)
        .json()
        .without_time()
        .with_ansi(false)
        .try_init()
        .expect("timing probe requires its own tracing subscriber");
    let workspace = tempfile::tempdir_in("/private/var/tmp").unwrap();
    let root = workspace.path().canonicalize().unwrap();
    let sandbox = OciBashSandbox::from_artifacts(&root, assets()).unwrap();
    for sample in 0..19 {
        println!(
            "OCI_TIMING_BEGIN {}",
            serde_json::json!({"sample": sample, "timeout_ms": 5_000})
        );
        let began = Instant::now();
        let output = sandbox
            .exec(request("printf phase-probe".into(), None, 5_000))
            .await;
        println!(
            "OCI_TIMING {}",
            serde_json::json!({
                "sample": sample,
                "total_ms": began.elapsed().as_secs_f64() * 1000.0,
                "exit_code": output.as_ref().ok().map(|output| output.exit_code),
                "stdout": output.as_ref().ok().map(|output| &output.stdout),
                "stderr": output.as_ref().ok().map(|output| &output.stderr),
                "timed_out": output.as_ref().ok().map(|output| output.timed_out),
                "error": output.as_ref().err().map(|error| format!("{error:#}")),
            })
        );
        let output = output.expect("timed command must produce a settled receipt");
        assert_eq!(output.exit_code, 0);
        assert_eq!(output.stdout, "phase-probe");
        assert_eq!(output.stderr, "");
        assert!(!output.timed_out);
    }
    sandbox.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires signed HVF assets; run explicitly on Apple Silicon"]
async fn real_bash_streaming_receipt_workspace_and_credential_contract() {
    let directory = tempfile::Builder::new()
        .prefix("aster-中文-\"-\\-")
        .tempdir_in("/private/var/tmp")
        .unwrap();
    let root = directory.path().canonicalize().unwrap();
    std::fs::write(root.join("input.txt"), "input").unwrap();
    std::fs::write(root.join(".env"), "credential").unwrap();
    std::fs::create_dir_all(root.join("node_modules/pkg")).unwrap();
    std::fs::hard_link(root.join(".env"), root.join("node_modules/pkg/config")).unwrap();
    std::fs::create_dir(root.join(".git")).unwrap();
    std::fs::write(root.join(".git/config"), "control").unwrap();
    let sandbox = OciBashSandbox::from_artifacts(&root, assets()).unwrap();
    let observer = Arc::new(Observer::default());
    let command = concat!(
        "set -e; [[ $PATH != :* ]]; items=(alpha 中文); [[ ${items[1]} == 中文 ]]; ",
        "[[ $(cat input.txt) == input ]]; printf 'pwd=%s\\n' \"$PWD\"; ",
        "cd /workspace; printf '中文🌟\\n'; printf written > 中文.txt; ",
        "if cat .env >/dev/null 2>&1; then exit 91; fi; ",
        "if cat node_modules/pkg/config >/dev/null 2>&1; then exit 92; fi; ",
        "if printf denied > .git/config 2>/dev/null; then exit 93; fi; ",
        "printf loop > .a3s/loops/state; exit 7"
    );
    let output = sandbox
        .exec(request(command.into(), Some(observer.clone()), 20_000))
        .await
        .unwrap();
    assert_eq!(output.exit_code, 7, "{}", output.stderr);
    assert!(!output.timed_out);
    assert!(output.stdout.contains(&format!("pwd={}\n", root.display())));
    assert!(output.stdout.contains("中文🌟"));
    assert!(observer.output.lock().unwrap().contains("中文🌟"));
    assert_eq!(std::fs::read(root.join("中文.txt")).unwrap(), b"written");
    assert_eq!(std::fs::read(root.join(".env")).unwrap(), b"credential");
    assert_eq!(std::fs::read(root.join(".git/config")).unwrap(), b"control");
    assert_eq!(
        std::fs::read(root.join(".a3s/loops/state")).unwrap(),
        b"loop"
    );
    assert!(!observer.summary.lock().unwrap().unwrap().timed_out);
    sandbox.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires signed HVF assets; run explicitly on Apple Silicon"]
async fn real_deadline_reaps_a_setsid_descendant_before_returning() {
    let workspace = tempfile::tempdir_in("/private/var/tmp").unwrap();
    let root = workspace.path().canonicalize().unwrap();
    let sandbox = OciBashSandbox::from_artifacts(&root, assets()).unwrap();
    let output=sandbox.exec(request("/bin/busybox setsid /bin/bash -c 'while :; do printf x >> counter; sleep 0.05; done' >/dev/null 2>&1 & wait".into(),None,1000)).await.unwrap();
    assert!(output.timed_out);
    assert_eq!(output.exit_code, 124);
    let length = std::fs::metadata(root.join("counter")).unwrap().len();
    assert!(length > 0);
    tokio::time::sleep(Duration::from_millis(250)).await;
    assert_eq!(
        std::fs::metadata(root.join("counter")).unwrap().len(),
        length
    );
    sandbox.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires signed HVF assets; run explicitly on Apple Silicon"]
async fn real_cancel_settles_the_owned_worker_and_closes_its_host() {
    let workspace = tempfile::tempdir_in("/private/var/tmp").unwrap();
    let root = workspace.path().canonicalize().unwrap();
    let sandbox = Arc::new(OciBashSandbox::from_artifacts(&root, assets()).unwrap());
    let running = sandbox.clone();
    let task = tokio::spawn(async move {
        running.exec(request("/bin/busybox setsid /bin/bash -c 'while :; do printf x >> counter; sleep 0.05; done' >/dev/null 2>&1 & wait".into(),None,60_000)).await
    });
    tokio::time::timeout(Duration::from_secs(30), async {
        while !root.join("counter").exists() {
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .unwrap();
    task.abort();
    assert!(matches!(task.await, Err(error) if error.is_cancelled()));
    tokio::time::timeout(Duration::from_secs(35), sandbox.shutdown())
        .await
        .unwrap();
    let length = std::fs::metadata(root.join("counter")).unwrap().len();
    tokio::time::sleep(Duration::from_millis(250)).await;
    assert_eq!(
        std::fs::metadata(root.join("counter")).unwrap().len(),
        length
    );
    assert!(sandbox.validate_execution_available().is_err());
}
