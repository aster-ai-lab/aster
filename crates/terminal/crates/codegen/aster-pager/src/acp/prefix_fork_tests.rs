use super::*;
use aster_acp_lib::{AcpAgentMessage, AcpArgs};
use serde_json::Value;
use tokio::sync::mpsc::UnboundedReceiver;

async fn receive(rx: &mut UnboundedReceiver<AcpAgentMessage>) -> AcpArgs<acp::ExtRequest> {
    match tokio::time::timeout(std::time::Duration::from_secs(5), rx.recv())
        .await
        .unwrap()
        .unwrap()
    {
        AcpAgentMessage::ExtMethod(args) => args,
        other => panic!("unexpected request: {other:?}"),
    }
}

fn reply(args: AcpArgs<acp::ExtRequest>, response: Value) {
    args.response_tx
        .send(Ok(acp::ExtResponse::new(
            serde_json::value::to_raw_value(&response).unwrap().into(),
        )))
        .unwrap();
}

fn page(params: &Value, first: bool) -> Value {
    let cut = json!({"seq": 8, "digest": "source-digest"});
    json!({
        "schema": PAGE_SCHEMA, "sessionId": "parent", "operationId": params["operationId"],
        "page": {"session_id": "parent", "source_cut": cut,
            "entries": [{"entry_id": if first {"user-1"} else {"complete-2"},
                "kind": if first {"user"} else {"turn_complete"},
                "fact_seq": if first {1} else {7}}],
            "next_cursor": if first {json!({"session_id": "parent",
                "after_entry_id": "user-1", "source_cut": cut})} else {Value::Null}},
    })
}

#[tokio::test]
async fn prefix_fork_paginates_one_log_cut_and_preserves_backend_admission() {
    let (client, mut agent) = aster_acp_lib::acp_channels();
    let task = tokio::spawn(async move {
        fork(
            &client.tx,
            "parent",
            Path::new("/workspace"),
            Some("child"),
            None,
        )
        .await
    });
    for first in [true, false] {
        let args = receive(&mut agent.rx).await;
        assert_eq!(
            args.request.method.as_ref(),
            "aster/session/transcript/page/v1"
        );
        let params: Value = serde_json::from_str(args.request.params.get()).unwrap();
        assert_eq!(params["cwd"], "/workspace");
        assert_eq!(params["limit"], 256);
        assert_eq!(params["cursor"].is_null(), first);
        reply(args, page(&params, first));
    }
    let args = receive(&mut agent.rx).await;
    assert_eq!(args.request.method.as_ref(), "aster/session/fork/prefix/v1");
    let params: Value = serde_json::from_str(args.request.params.get()).unwrap();
    assert_eq!(params["request"]["expected_workspace"], "/workspace");
    assert_eq!(params["request"]["boundary_entry_id"], "complete-2");
    assert_eq!(params["request"]["target_session_id"], "child");
    // A stale or active source is rejected by Core; the pager must not retry at a newer cut.
    args.response_tx
        .send(Err(acp::Error::invalid_params().data("source changed")))
        .unwrap();
    assert!(task.await.unwrap().is_err());
    assert!(agent.rx.try_recv().is_err());
}

#[tokio::test]
async fn prefix_fork_accepts_only_the_requested_child_and_lineage() {
    for wrong_child in [false, true] {
        let (client, mut agent) = aster_acp_lib::acp_channels();
        let task = tokio::spawn(async move {
            let mut meta = acp::Meta::new();
            meta.insert("noReplay".into(), json!(true));
            fork(
                &client.tx,
                "parent",
                Path::new("/workspace"),
                Some("child"),
                Some(
                    acp::LoadSessionRequest::new(acp::SessionId::new("parent"), "/workspace")
                        .mcp_servers(vec![acp::McpServer::Stdio(acp::McpServerStdio::new(
                            "fixture",
                            "/fixture/mcp",
                        ))])
                        .meta(Some(meta)),
                ),
            )
            .await
        });
        let load = agent.rx.recv().await.unwrap();
        let AcpAgentMessage::LoadSession(load) = load else {
            panic!("startup must load its parent")
        };
        assert_eq!(load.request.session_id.0.as_ref(), "parent");
        assert_eq!(load.request.meta.as_ref().unwrap()["noReplay"], true);
        let acp::McpServer::Stdio(mcp) = &load.request.mcp_servers[0] else {
            panic!("MCP config changed")
        };
        assert_eq!(mcp.name, "fixture");
        assert_eq!(mcp.command, Path::new("/fixture/mcp"));
        load.response_tx
            .send(Ok(acp::LoadSessionResponse::new()))
            .unwrap();
        let args = receive(&mut agent.rx).await;
        let params: Value = serde_json::from_str(args.request.params.get()).unwrap();
        reply(args, page(&params, false));
        let args = receive(&mut agent.rx).await;
        let params: Value = serde_json::from_str(args.request.params.get()).unwrap();
        reply(
            args,
            json!({"schema": FORK_SCHEMA, "operationId": params["operationId"],
            "childId": if wrong_child {"foreign"} else {"child"},
            "sourceCut": params["request"]["expected_cut"],
            "lineage": {"sourceSessionId": "parent", "childSessionId": "child",
                "boundaryEntryId": "complete-2"}}),
        );
        let result = task.await.unwrap();
        if wrong_child {
            assert!(result.is_err());
        } else {
            assert_eq!(result.unwrap(), "child");
        }
        assert!(agent.rx.try_recv().is_err());
    }
}

#[tokio::test]
async fn prefix_fork_refuses_malformed_or_incomplete_pages_before_any_fork() {
    for case in [
        "foreign-session",
        "foreign-operation",
        "schema",
        "empty",
        "unfinished",
        "cursor",
        "cut",
        "repeated",
    ] {
        let (client, mut agent) = aster_acp_lib::acp_channels();
        let task = tokio::spawn(async move {
            fork(
                &client.tx,
                "parent",
                Path::new("/workspace"),
                Some("child"),
                None,
            )
            .await
        });
        let args = receive(&mut agent.rx).await;
        let params: Value = serde_json::from_str(args.request.params.get()).unwrap();
        let mut response = page(&params, false);
        match case {
            "foreign-session" => response["page"]["session_id"] = json!("foreign"),
            "foreign-operation" => response["operationId"] = json!("foreign"),
            "schema" => response["schema"] = json!("v0"),
            "empty" => response["page"]["entries"] = json!([]),
            "unfinished" => response["page"]["entries"][0]["kind"] = json!("tool"),
            "cursor" => {
                response = page(&params, true);
                response["page"]["next_cursor"]["session_id"] = json!("foreign");
            }
            "cut" | "repeated" => response = page(&params, true),
            _ => unreachable!(),
        }
        reply(args, response);
        if matches!(case, "cut" | "repeated") {
            let args = receive(&mut agent.rx).await;
            let mut response = page(&params, false);
            if case == "cut" {
                response["page"]["source_cut"]["seq"] = json!(9);
            } else {
                response["page"]["entries"][0]["fact_seq"] = json!(1);
            }
            reply(args, response);
        }
        assert!(task.await.unwrap().is_err(), "{case}");
        assert!(agent.rx.try_recv().is_err(), "{case} attempted a fork");
    }
}

#[tokio::test]
async fn prefix_fork_refuses_a_mismatched_source_load_before_any_request() {
    let (client, mut agent) = aster_acp_lib::acp_channels();
    let result = fork(
        &client.tx,
        "parent",
        Path::new("/workspace"),
        Some("child"),
        Some(acp::LoadSessionRequest::new(
            acp::SessionId::new("other"),
            "/workspace",
        )),
    )
    .await;
    assert!(result.is_err());
    assert!(agent.rx.try_recv().is_err());
}
