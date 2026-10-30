//! Read-only Git batches use the Git provider without taking mutation snapshots.

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use a3s_code_core::tools::ToolExecutor;
use serde_json::{json, Value};

const FIXTURE_ENV: &str = "ASTER_GIT_READ_FIXTURE";

#[tokio::test]
async fn readonly_git_batch_does_not_rescan_workspace_for_mutations() {
    if let Some(workspace) = std::env::var_os(FIXTURE_ENV) {
        let executor = ToolExecutor::new(workspace.into_string().expect("UTF-8 fixture workspace"));
        let status = json!({"command": "status"});
        let branch = json!({"command": "branch"});
        let remote = json!({"command": "remote"});
        let (status, branch, remote) = tokio::join!(
            executor.execute("git", &status),
            executor.execute("git", &branch),
            executor.execute("git", &remote),
        );
        for result in [status, branch, remote] {
            let result = result.expect("Git query");
            assert_eq!(result.exit_code, 0, "{}", result.output);
        }
        return;
    }

    let root = tempfile::tempdir().expect("fixture root");
    let workspace = root.path().join("repository 空间");
    std::fs::create_dir(&workspace).expect("workspace");
    for arguments in [
        vec!["init", "-q"],
        vec!["config", "user.name", "Git Fixture"],
        vec!["config", "user.email", "fixture@example.invalid"],
        vec!["config", "commit.gpgsign", "false"],
    ] {
        git(&workspace, &arguments);
    }
    std::fs::write(workspace.join("tracked.txt"), "before\n").expect("tracked file");
    git(&workspace, &["add", "tracked.txt"]);
    git(&workspace, &["commit", "-q", "-m", "fixture"]);
    std::fs::write(workspace.join("tracked.txt"), "after\n").expect("dirty file");

    let trace = root.path().join("git-trace.jsonl");
    let child = tokio::process::Command::new(std::env::current_exe().expect("test binary"))
        .args([
            "--exact",
            "readonly_git_batch_does_not_rescan_workspace_for_mutations",
            "--nocapture",
        ])
        .env(FIXTURE_ENV, &workspace)
        .env("GIT_TRACE2_EVENT", &trace)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("isolated Git query process");
    let output = tokio::time::timeout(Duration::from_secs(30), child.wait_with_output())
        .await
        .expect("bounded Git query batch")
        .expect("wait for query batch");
    assert!(
        output.status.success(),
        "Git query child failed: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    // Provider Git strips inherited GIT_* variables. The old mutation
    // snapshots spawn extra Git processes that inherit this trace destination.
    let events = match std::fs::read_to_string(&trace) {
        Ok(events) => events,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => panic!("read Git trace: {error}"),
    };
    let scans: Vec<Value> = events
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("Git trace event"))
        .filter(|event| event["event"] == "start")
        .collect();
    assert!(
        scans.is_empty(),
        "read-only batch launched {} host mutation-observation Git processes",
        scans.len()
    );
    assert_eq!(
        std::fs::read_to_string(workspace.join("tracked.txt")).expect("dirty file preserved"),
        "after\n"
    );
}

fn git(workspace: &Path, arguments: &[&str]) {
    let mut command = std::process::Command::new("git");
    command.env_clear();
    for key in ["PATH", "SystemRoot", "SystemDrive", "TEMP", "TMP"] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    let output = command
        .env("HOME", workspace)
        .env("USERPROFILE", workspace)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .arg("-c")
        .arg(if cfg!(windows) {
            "core.hooksPath=NUL"
        } else {
            "core.hooksPath=/dev/null"
        })
        .arg("-C")
        .arg(workspace)
        .args(arguments)
        .output()
        .expect("fixture Git");
    assert!(
        output.status.success(),
        "fixture Git failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
