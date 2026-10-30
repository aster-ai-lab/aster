#![cfg(all(test, unix))]

use super::*;
use acp::Client as _;
use tokio::io::{AsyncBufReadExt, BufReader};

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
        .ext_notification(acp::ExtNotification::new("a3s/turn/outcome", params.into()))
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
        .ext_notification(acp::ExtNotification::new("a3s/turn/outcome", params.into()))
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
