#![cfg(test)]

use super::*;
use futures_util::FutureExt;

#[test]
fn native_alignment_typed_reconciliation_cannot_be_retried_as_an_internal_error() {
    let message = acp_error_text(
        &json!({"code":-32603,"message":"Internal error","data":{"kind":"tool_effect_unresolved","message":"Internal error"}}),
    );
    assert!(message.contains("reconciliation"));
    assert!(message.contains("Do not repeat"));
    assert!(!message.contains("Internal error"));
}

#[tokio::test]
async fn native_alignment_timed_out_readonly_rpc_drops_its_pending_callback() {
    let root = tempfile::tempdir().unwrap();
    let binary = root.path().join("silent-peer");
    std::fs::write(&binary, "#!/usr/bin/env node\nprocess.stdin.resume();\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let (client, _) = AcpClient::spawn_fixture(
        &binary,
        &[],
        root.path(),
        (
            "http://127.0.0.1:9".into(),
            Some("synthetic-invalid".into()),
        ),
    )
    .unwrap();
    let result = std::panic::AssertUnwindSafe(async {
        for _ in 0..3 {
            assert!(
                tokio::time::timeout(
                    std::time::Duration::from_millis(50),
                    client.request("_a3s/goal/state", json!({"session_id":"S"}))
                )
                .await
                .is_err()
            );
            assert!(
                client.pending.lock().unwrap().is_empty(),
                "cancelled read must release callback ownership"
            );
        }
    })
    .catch_unwind()
    .await;
    client.kill().await;
    assert!(client.process_id().await.is_none());
    if let Err(error) = result {
        std::panic::resume_unwind(error);
    }
}

#[tokio::test]
async fn native_alignment_raw_extension_notifications_decode_exactly_one_prefix() {
    let root = tempfile::tempdir().unwrap();
    let binary = root.path().join("notification-peer");
    std::fs::write(&binary,r#"#!/usr/bin/env node
for (const [index,method] of ['session/update','_x.ai/turn/outcome','__a3s/execution/observe','x.ai/turn/outcome','_session/update','__x.ai/turn/outcome','_other.extension','session/update'].entries()) process.stdout.write(JSON.stringify({jsonrpc:'2.0',method,params:{index}})+'\n');
process.stdin.resume();
"#).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let (client, mut inbound) = AcpClient::spawn_fixture(
        &binary,
        &[],
        root.path(),
        ("http://127.0.0.1:9".into(), None),
    )
    .unwrap();
    let result = std::panic::AssertUnwindSafe(async {
        let mut methods = vec![];
        loop {
            let message = tokio::time::timeout(std::time::Duration::from_secs(1), inbound.recv())
                .await
                .unwrap()
                .unwrap();
            if let Inbound::Notification { method, params } = message {
                let done = params["index"] == 7;
                methods.push(method);
                if done {
                    break;
                }
            }
        }
        assert_eq!(
            methods,
            [
                "session/update",
                "x.ai/turn/outcome",
                "_a3s/execution/observe",
                "_x.ai/turn/outcome",
                "other.extension",
                "session/update"
            ]
        );
    })
    .catch_unwind()
    .await;
    client.kill().await;
    if let Err(error) = result {
        std::panic::resume_unwind(error)
    }
}

#[tokio::test]
async fn native_alignment_incomplete_or_conflicting_replies_are_not_acknowledgements() {
    let root = tempfile::tempdir().unwrap();
    let binary = root.path().join("reply-peer");
    std::fs::write(&binary,r#"#!/usr/bin/env node
require('readline').createInterface({input:process.stdin}).on('line',line=>{const m=JSON.parse(line);let reply={jsonrpc:'2.0',id:m.id};if(m.method==='both'){reply.result=null;reply.error={code:-32603,message:'denied'}}if(m.method==='null')reply.result=null;if(m.method==='bad-version'){reply.jsonrpc='1.0';reply.result={}};process.stdout.write(JSON.stringify(reply)+'\n')});
"#).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let (client, _) = AcpClient::spawn_fixture(
        &binary,
        &[],
        root.path(),
        ("http://127.0.0.1:9".into(), None),
    )
    .unwrap();
    let result = std::panic::AssertUnwindSafe(async {
        for method in ["missing", "both", "bad-version"] {
            let result = tokio::time::timeout(
                std::time::Duration::from_secs(1),
                client.request(method, json!({})),
            )
            .await
            .unwrap();
            assert!(
                result.is_err(),
                "malformed {method} response is not an acknowledgement: {result:?}"
            );
        }
        assert_eq!(
            client.request("null", json!({})).await.unwrap(),
            Value::Null
        );
    })
    .catch_unwind()
    .await;
    client.kill().await;
    if let Err(error) = result {
        std::panic::resume_unwind(error)
    }
}
