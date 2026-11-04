#![cfg(all(test, unix))]

use super::*;
use acp::Client as _;
use tokio::io::{AsyncBufReadExt, BufReader};

#[test]
fn timing_observes_live_terminals_and_skips_replayed_execution() {
    let root = tempfile::tempdir().unwrap();
    let guard = timing::Guard::open(root.path()).unwrap();
    let mut capture = Capture::default();
    let update = |kind: &str, replay: bool| {
        serde_json::json!({
        "sessionId":"timing-session-private-id",
        "update":{"sessionUpdate":kind,"subagent_id":"timing-child-private-id","description":"private-task-content"},
        "_meta":{"isReplay":replay},
    }).to_string()
    };
    for kind in [
        "auto_compact_started",
        "subagent_spawned",
        "auto_compact_completed",
        "subagent_finished",
    ] {
        capture.time_session_update(&update(kind, true));
    }
    assert!(capture.timing_compactions.is_empty() && capture.timing_subagents.is_empty());
    capture.time_tool("timing-tool-private-id", &acp::ToolCallStatus::Pending);
    capture.time_tool("timing-tool-private-id", &acp::ToolCallStatus::InProgress);
    assert_eq!(capture.timing_tools.len(), 1);
    capture.time_tool("timing-tool-private-id", &acp::ToolCallStatus::Failed);
    for kind in ["auto_compact_started", "subagent_spawned"] {
        capture.time_session_update(&update(kind, false));
    }
    assert_eq!(capture.timing_compactions.len(), 1);
    assert_eq!(capture.timing_subagents.len(), 1);
    for kind in ["auto_compact_completed", "subagent_finished"] {
        capture.time_session_update(&update(kind, false));
    }
    assert!(capture.timing_compactions.is_empty() && capture.timing_subagents.is_empty());
    capture.time_tool("terminal-without-start", &acp::ToolCallStatus::Completed);
    capture.time_tool("unsettled", &acp::ToolCallStatus::InProgress);
    drop(capture);
    drop(guard);
    let text = std::fs::read_to_string(root.path().join("phase-timing.jsonl")).unwrap();
    for secret in [
        "timing-session-private-id",
        "timing-child-private-id",
        "timing-tool-private-id",
        "private-task-content",
    ] {
        assert!(!text.contains(secret));
    }
    let rows: Vec<Value> = text
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    for phase in ["tool", "compaction", "subagent"] {
        let id = match phase {
            "tool" => "timing-tool-private-id",
            "compaction" => "timing-session-private-id",
            _ => "timing-child-private-id",
        };
        let subject = serde_json::to_value(timing::opaque_id(id)).unwrap();
        let relevant: Vec<_> = rows
            .iter()
            .filter(|row| row["phase"] == phase && row["subject"] == subject)
            .collect();
        assert_eq!(relevant.len(), 2, "one begin and terminal for {phase}");
        assert_eq!(relevant[0]["event"], "begin");
        assert_eq!(relevant[1]["event"], "end");
    }
    assert!(rows.iter().any(|row| row["phase"] == "tool_missing_start"));
    assert!(rows.iter().any(|row| row["phase"] == "tool_unfinished"));
}

#[test]
fn undeclared_or_invalid_sandbox_assets_cannot_be_inherited() {
    let root = tempfile::tempdir().unwrap();
    assert!(qualified_assets(root.path(), None).unwrap().is_none());
    assert!(qualified_assets(root.path(), Some(Path::new("relative"))).is_err());
    assert!(qualified_assets(root.path(), Some(root.path())).is_err());
}

#[tokio::test]
async fn thought_content_is_omitted_and_extension_outcomes_are_bounded() {
    let capture = Arc::new(Mutex::new(Capture::default()));
    let client = Client {
        capture: capture.clone(),
        allow: false,
    };
    let thought: acp::SessionNotification = serde_json::from_value(serde_json::json!({"sessionId":"fixture","update":{"sessionUpdate":"agent_thought_chunk","content":{"type":"text","text":"private-fixture-reasoning-body"}}})).unwrap();
    client.session_notification(thought).await.unwrap();
    let text = serde_json::to_string(&capture.lock().unwrap().events).unwrap();
    assert!(!text.contains("private-fixture-reasoning-body"));
    assert!(text.contains("contentOmitted"));
    let params = serde_json::value::to_raw_value(&serde_json::json!({"sessionId":"fixture","outcome":{"terminal":"verified","status":"passed"}})).unwrap();
    client
        .ext_notification(acp::ExtNotification::new(
            "aster/turn/outcome",
            params.into(),
        ))
        .await
        .unwrap();
    assert_eq!(
        capture.lock().unwrap().events[1]["params"]["outcome"]["terminal"],
        "verified"
    );
    let unknown =
        serde_json::value::to_raw_value(&serde_json::json!({"private":"private-extension-value"}))
            .unwrap();
    client
        .ext_notification(acp::ExtNotification::new("fixture/unknown", unknown.into()))
        .await
        .unwrap();
    assert!(!serde_json::to_string(&capture.lock().unwrap().events)
        .unwrap()
        .contains("private-extension-value"));
    capture.lock().unwrap().bytes = CAPTURE_LIMIT;
    let params =
        serde_json::value::to_raw_value(&serde_json::json!({"sessionId":"fixture"})).unwrap();
    assert!(client
        .ext_notification(acp::ExtNotification::new(
            "aster/turn/outcome",
            params.into()
        ))
        .await
        .is_err());
    assert!(capture.lock().unwrap().overflow);
    assert_eq!(capture.lock().unwrap().events.len(), 3);
}

#[test]
fn rpc_diagnostics_preserve_the_reason_without_exposing_fixture_credentials() {
    let key = "fixture-private-value";
    let error = acp::Error::internal_error().data(serde_json::json!({"reason":"worker exited without terminal","url":format!("Bearer {key}")}));
    let safe = safe_rpc_error(&error, key);
    assert_eq!(safe["data"]["reason"], "worker exited without terminal");
    assert!(!safe.to_string().contains(key));
    let oversized = acp::Error::internal_error().data("x".repeat(32 * 1024 + 1));
    let safe = safe_rpc_error(&oversized, key);
    assert_eq!(safe["diagnostic_truncated"], true);
    assert!(safe.to_string().len() < 1024);
}

#[tokio::test]
async fn permission_decisions_use_kinds_and_refuse_missing_options() {
    for (allow, kind, id) in [
        (true, acp::PermissionOptionKind::AllowOnce, "opaque-1"),
        (
            false,
            acp::PermissionOptionKind::RejectOnce,
            "allow-looking-label",
        ),
    ] {
        let client = Client {
            capture: Arc::new(Mutex::new(Capture::default())),
            allow,
        };
        let args = acp::RequestPermissionRequest::new(
            "fixture-session",
            acp::ToolCallUpdate::new("fixture-call", acp::ToolCallUpdateFields::default()),
            vec![acp::PermissionOption::new(id, "fixture", kind)],
        );
        let response = client.request_permission(args.clone()).await.unwrap();
        assert_eq!(
            response.outcome,
            acp::RequestPermissionOutcome::Selected(acp::SelectedPermissionOutcome::new(
                acp::PermissionOptionId::new(id)
            ))
        );
        let mut absent = args;
        absent.options.clear();
        assert_eq!(
            client.request_permission(absent).await.unwrap().outcome,
            acp::RequestPermissionOutcome::Cancelled
        );
    }
}

#[tokio::test]
async fn an_already_exited_owned_child_can_be_cleaned_up() {
    let mut command = tokio::process::Command::new("/bin/sh");
    command
        .args(["-c", "printf 'ready\\n'"])
        .stdout(Stdio::piped());
    let mut child = OwnedChild::spawn(&mut command).unwrap();
    let mut line = String::new();
    let mut output = BufReader::new(child.child_mut().stdout.take().unwrap());
    tokio::time::timeout(Duration::from_secs(5), output.read_line(&mut line))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(line, "ready\n");
    // Reading EOF is the barrier: the shell has closed its only output.
    line.clear();
    assert_eq!(output.read_line(&mut line).await.unwrap(), 0);
    child.close().await.unwrap();
}

#[tokio::test]
async fn closing_a_live_owned_child_settles_its_output_and_reaps_it() {
    let mut command = tokio::process::Command::new("/bin/sh");
    command
        .args(["-c", "printf 'ready\\n'; exec sleep 60"])
        .stdout(Stdio::piped());
    let mut child = OwnedChild::spawn(&mut command).unwrap();
    let mut line = String::new();
    let mut output = BufReader::new(child.child_mut().stdout.take().unwrap());
    tokio::time::timeout(Duration::from_secs(5), output.read_line(&mut line))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(line, "ready\n");
    child.close().await.unwrap();
    line.clear();
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(5), output.read_line(&mut line))
            .await
            .unwrap()
            .unwrap(),
        0
    );
}
