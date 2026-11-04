use super::*;

#[test]
fn tui_trace_requires_the_selected_session_terminal_and_cleanup() {
    let good = vec![
        json!({"kind":"model-selected","model_id":"glm-01/glm-5.3","session_id":"s"}),
        json!({"kind":"prompt-request","rpc_id":7,"session_id":"s"}),
        json!({"kind":"prompt-response","rpc_id":7,"session_id":"s","stop_reason":"end_turn","error_code":null}),
        json!({"kind":"proxy-closed","pending_prompts":0}),
    ];
    assert!(trace_pass(&good, true, "glm-01/glm-5.3", true));
    assert!(!trace_pass(&good, false, "glm-01/glm-5.3", true));
    assert!(!trace_pass(&good, true, "glm-02/glm-5.3", true));
    for (row, key, value) in [
        (1, "session_id", json!("other")),
        (2, "rpc_id", json!(8)),
        (2, "stop_reason", json!("cancelled")),
        (2, "error_code", json!(-32603)),
        (3, "pending_prompts", json!(1)),
    ] {
        let mut bad = good.clone();
        bad[row][key] = value;
        assert!(!trace_pass(&bad, true, "glm-01/glm-5.3", true), "{key}");
    }
    assert!(!trace_pass(&good[..2], true, "glm-01/glm-5.3", true));
    assert!(!trace_pass(&good, true, "glm-01/glm-5.3", false));
    assert!(!trace_pass(&[], true, "glm-01/glm-5.3", true));
}

#[tokio::test]
async fn tui_proxy_captures_raw_extensions_and_reaps_its_child_on_term() {
    use std::os::unix::fs::PermissionsExt;
    use std::process::Stdio;
    use std::time::Duration;
    use tokio::io::AsyncBufReadExt;

    for reject_escalation in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let proxy = root.path().join("proxy.py");
        let peer = root.path().join("peer");
        let trace = root.path().join("trace.jsonl");
        let pid_file = root.path().join("peer.pid");
        std::fs::write(&proxy, tui_proxy_source()).unwrap();
        std::fs::write(&peer,format!(r##"#!{}
import json, os, sys
from pathlib import Path
Path(os.environ['ASTER_PROXY_TEST_PID']).write_text(str(os.getpid()))
for kind in ['subagent_spawned', 'subagent_finished']:
    print(json.dumps({{'jsonrpc':'2.0','method':'_aster/session_notification','params':{{'sessionId':'probe','update':{{'sessionUpdate':kind}}}}}}),flush=True)
print(json.dumps({{'jsonrpc':'2.0','method':'_aster/turn/outcome','params':{{'sessionId':'probe','outcome':{{'terminal':'verified','status':'passed'}}}}}}),flush=True)
if os.environ['ASTER_PROXY_TEST_ESCAPE'] == '0':
    print(json.dumps({{'jsonrpc':'2.0','id':8,'error':{{'code':-32603,'message':'controlled error','data':'fixture: synthetic-relay-secret'}}}}),flush=True)
if os.environ['ASTER_PROXY_TEST_ESCAPE'] == '1':
    print(json.dumps({{'jsonrpc':'2.0','id':99,'method':'session/request_permission','params':{{'toolCall':{{'rawInput':{{'sandbox_permissions':'require_escalated'}}}}}}}}),flush=True)
for line in sys.stdin:
    pass
"##,python::executable().display())).unwrap();
        std::fs::set_permissions(&peer, std::fs::Permissions::from_mode(0o700)).unwrap();
        let mut child = tokio::process::Command::new(python::executable())
            .arg(&proxy)
            .env("ASTER_TUI_TEST_ACP_REAL", &peer)
            .env("ASTER_TUI_TEST_ACP_TRACE", &trace)
            .env("ASTER_PROXY_TEST_PID", &pid_file)
            .env(config::RELAY_KEY_ENV, "synthetic-relay-secret")
            .env(
                "ASTER_PROXY_TEST_ESCAPE",
                if reject_escalation { "1" } else { "0" },
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let proxy_pid = child.id().unwrap();
        let mut lines = tokio::io::BufReader::new(child.stdout.take().unwrap()).lines();
        for expected in [
            "_aster/session_notification",
            "_aster/session_notification",
            "_aster/turn/outcome",
        ] {
            let line = tokio::time::timeout(Duration::from_secs(5), lines.next_line())
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            let packet: Value = serde_json::from_str(&line).unwrap();
            assert_eq!(packet["method"], expected);
        }
        if !reject_escalation {
            let line = tokio::time::timeout(Duration::from_secs(5), lines.next_line())
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            let packet: Value = serde_json::from_str(&line).unwrap();
            assert_eq!(packet["error"]["data"], "fixture: synthetic-relay-secret");
        }
        let peer_pid = std::fs::read_to_string(&pid_file).unwrap();
        if !reject_escalation {
            let terminated = tokio::process::Command::new("/bin/kill")
                .args(["-TERM", &child.id().unwrap().to_string()])
                .status()
                .await
                .unwrap();
            assert!(terminated.success());
        }
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(5), child.wait())
                .await
                .unwrap()
                .unwrap()
                .success(),
            !reject_escalation
        );
        assert!(
            tokio::time::timeout(Duration::from_secs(5), lines.next_line())
                .await
                .unwrap()
                .unwrap()
                .is_none()
        );
        let rows = std::fs::read_to_string(&trace)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .collect::<Vec<_>>();
        assert!(rows
            .windows(2)
            .all(|pair| pair[0]["elapsed_us"].as_u64().unwrap()
                <= pair[1]["elapsed_us"].as_u64().unwrap()));
        for row in &rows {
            assert_eq!(
                row["unix_time_us"].as_u64().unwrap(),
                row["clock_origin_unix_us"].as_u64().unwrap() + row["elapsed_us"].as_u64().unwrap()
            );
            assert_eq!(row["pid"], proxy_pid);
        }
        assert_eq!(rows.iter().filter(|row| row["kind"] == "packet").count(), 3);
        let errors = rows
            .iter()
            .filter(|v| v["kind"] == "rpc-error")
            .collect::<Vec<_>>();
        assert_eq!(errors.len(), usize::from(!reject_escalation));
        if !reject_escalation {
            assert_eq!(
                errors[0]["error"]["data"],
                "fixture: [fixture credential redacted]"
            );
        }
        assert!(!std::fs::read_to_string(&trace)
            .unwrap()
            .contains("synthetic-relay-secret"));
        assert!(proxy_processes_absent(&rows).await);
        assert!(!proxy_processes_absent(&[]).await);
        let mut live = rows.clone();
        live.iter_mut()
            .find(|v| v["kind"] == "proxy-started")
            .unwrap()["agent_pid"] = json!(std::process::id());
        assert!(!proxy_processes_absent(&live).await);
        assert!(rows
            .iter()
            .any(|row| row["kind"] == "proxy-closed" && row["pending_prompts"] == 0));
        assert!(!tokio::process::Command::new("/bin/kill")
            .args(["-0", &peer_pid])
            .output()
            .await
            .unwrap()
            .status
            .success());
    }
}

#[test]
fn goal_completion_wait_matches_the_actual_english_producer() {
    assert!(include_str!("../../src/agent/goal_ext.rs").contains(GOAL_FINISHED_TEXT));
}
