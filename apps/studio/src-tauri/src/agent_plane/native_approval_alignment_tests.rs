#![cfg(test)]

use super::tests::{harness, message, until_done};
use super::*;
use futures_util::{FutureExt, StreamExt};

async fn waiting(events: &mut broadcast::Receiver<Value>) -> Value {
    loop {
        let event = tokio::time::timeout(Duration::from_secs(10), events.recv())
            .await
            .expect("approval deadline")
            .unwrap();
        if event["channel"] == "topic.status"
            && event["payload"]["status"]["status"] == "awaiting-approval"
        {
            return event["payload"]["status"].clone();
        }
    }
}

async fn snapshot(turns: &TurnService) -> Value {
    let mut stream = super::super::events::stream(turns.hub.clone());
    let frame = stream.next().await.unwrap().unwrap();
    let encoded = String::from_utf8(frame.encode()).unwrap();
    serde_json::from_str::<Value>(encoded.trim().strip_prefix("data: ").unwrap()).unwrap()["payload"]["liveStatuses"][0]["status"].clone()
}

#[tokio::test]
async fn native_alignment_reconnect_recovers_the_live_approval_identity() {
    let h = harness("native-alignment-reconnect", "default").expect("node fixture");
    let result = std::panic::AssertUnwindSafe(async {
        let mut events = h.hub.subscribe();
        h.turns.open(&h.session_id, &message("ask-twice")).unwrap();
        let status = waiting(&mut events).await;
        let attached = h.turns.attach(&h.session_id);
        let pending = attached["streamStatus"]["pendingApprovals"]
            .as_array()
            .expect("authoritative pending snapshot");
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0]["toolCallId"], "t1");
        assert_eq!(pending[0]["messageId"], pending[0]["anchorMessageId"]);
        assert_eq!(
            pending[0]["executionId"],
            attached["streamStatus"]["turnId"]
        );
        assert_eq!(status["pendingApprovals"], json!(pending));
        assert_eq!(snapshot(&h.turns).await["pendingApprovals"], json!(pending));
        h.turns.abort(&h.session_id);
        until_done(&mut events, |_| {}).await;
    })
    .catch_unwind()
    .await;
    h.turns.close_test_runtimes().await;
    let _ = std::fs::remove_dir_all(&h.dir);
    if let Err(error) = result {
        std::panic::resume_unwind(error);
    }
}

#[tokio::test]
async fn native_alignment_abort_revokes_approval_before_settlement() {
    let h = harness("native-alignment-abort", "default").expect("node fixture");
    let result = std::panic::AssertUnwindSafe(async {
        let mut events = h.hub.subscribe();
        h.turns.open(&h.session_id, &message("ask first")).unwrap();
        waiting(&mut events).await;
        let approval = h
            .store
            .messages(&h.session_id)
            .last()
            .unwrap()
            .parts()
            .iter()
            .find(|part| part["state"] == "approval-requested")
            .unwrap()["approval"]["id"]
            .as_str()
            .unwrap()
            .to_owned();
        h.turns.abort(&h.session_id);
        assert!(
            !h.turns.respond_approval(&approval, true),
            "abort synchronously revokes authority"
        );
        assert_eq!(snapshot(&h.turns).await["pendingApprovals"], json!([]));
        let seen = until_done(&mut events, |_| {}).await;
        assert_eq!(seen.last().unwrap()["payload"]["status"], "paused");
        assert_eq!(h.turns.attach(&h.session_id)["status"], "not-found");
    })
    .catch_unwind()
    .await;
    h.turns.close_test_runtimes().await;
    let _ = std::fs::remove_dir_all(&h.dir);
    if let Err(error) = result {
        std::panic::resume_unwind(error);
    }
}

#[test]
fn native_alignment_plan_first_is_explicit_prompt_metadata() {
    assert_eq!(
        acp_turn(&json!({"turnContext":{"planFirst":true}})).unwrap()["planFirst"],
        true
    );
    assert!(acp_turn(&json!({"turnContext":{"planFirst":false}})).is_none());
}

#[tokio::test]
async fn native_alignment_invalid_plan_first_is_refused_before_admission() {
    let h = harness("native-alignment-invalid-plan", "default").expect("node fixture");
    let result = std::panic::AssertUnwindSafe(async {
        for value in [json!("true"), json!(1), Value::Null, json!({})] {
            let mut body = message("not admitted");
            body["turnContext"] = json!({"planFirst": value});
            assert!(h.turns.open(&h.session_id, &body).is_err());
        }
        assert!(h.store.messages(&h.session_id).is_empty());
        assert_eq!(h.turns.attach(&h.session_id)["status"], "not-found");
    })
    .catch_unwind()
    .await;
    h.turns.close_test_runtimes().await;
    let _ = std::fs::remove_dir_all(h.dir);
    if let Err(error) = result {
        std::panic::resume_unwind(error);
    }
}

#[tokio::test]
async fn native_alignment_plan_first_preserves_permission_posture_on_the_wire() {
    let h = harness("native-alignment-plan-wire", "default").expect("node fixture");
    let result = std::panic::AssertUnwindSafe(async {
        let mut events = h.hub.subscribe();
        let mut body = message("plan this synthetic task");
        body["turnContext"] = json!({"planFirst":true});
        h.turns.open(&h.session_id, &body).unwrap();
        until_done(&mut events, |_| {}).await;
        h.turns
            .open(&h.session_id, &message("normal task"))
            .unwrap();
        until_done(&mut events, |_| {}).await;
        let trace: Value =
            serde_json::from_slice(&std::fs::read(h.dir.join("turn-trace.json")).unwrap()).unwrap();
        assert_eq!(trace["promptMeta"][0]["a3sTurn"]["planFirst"], true);
        assert_eq!(trace["promptMeta"][0]["permissionPosture"], "default");
        assert!(trace["promptMeta"][1].get("a3sTurn").is_none());
        assert_eq!(trace["promptMeta"][1]["permissionPosture"], "default");
    })
    .catch_unwind()
    .await;
    h.turns.close_test_runtimes().await;
    let _ = std::fs::remove_dir_all(h.dir);
    if let Err(error) = result {
        std::panic::resume_unwind(error);
    }
}

#[tokio::test]
async fn native_alignment_incomplete_stop_is_not_reported_as_success() {
    const PEER: &str = r#"#!/usr/bin/env node
const send=value=>process.stdout.write(JSON.stringify(value)+'\n');
require('readline').createInterface({input:process.stdin}).on('line', line=>{
 const m=JSON.parse(line),reply=result=>send({jsonrpc:'2.0',id:m.id,result});
 if(m.method==='initialize')return reply({protocolVersion:1});
 if(m.method==='session/new')return reply({sessionId:'S'});
 if(m.method==='session/set_mode'||m.method==='session/set_model')return reply({});
 if(m.method==='session/prompt'){
  send({jsonrpc:'2.0',method:'session/update',params:{sessionId:'S',update:{sessionUpdate:'agent_message_chunk',content:{type:'text',text:'partial evidence'}}}});
  const text=m.params.prompt[0].text;
  const reason=['max_tokens','max_turn_requests','refusal','unknown','missing'].find(reason=>text.includes(reason));
  return reply(reason==='missing'?{}:{stopReason:reason});
 }
});
"#;
    for reason in [
        "max_tokens",
        "max_turn_requests",
        "refusal",
        "unknown",
        "missing",
    ] {
        let h = harness(&format!("native-alignment-stop-{reason}"), "auto").expect("node fixture");
        std::fs::write(h.dir.join("fake-acp"), PEER).unwrap();
        let result = std::panic::AssertUnwindSafe(async {
            let mut events = h.hub.subscribe();
            h.turns.open(&h.session_id, &message(reason)).unwrap();
            let seen = until_done(&mut events, |_| {}).await;
            assert_eq!(
                seen.last().unwrap()["channel"],
                "ai.stream.error",
                "{reason}"
            );
            assert_eq!(
                h.store.messages(&h.session_id).last().unwrap().status,
                "error"
            );
            assert!(
                h.store
                    .messages(&h.session_id)
                    .last()
                    .unwrap()
                    .searchable_text
                    .contains("partial evidence")
            );
        })
        .catch_unwind()
        .await;
        h.turns.close_test_runtimes().await;
        let _ = std::fs::remove_dir_all(h.dir);
        if let Err(error) = result {
            std::panic::resume_unwind(error);
        }
    }
}

#[tokio::test]
async fn native_alignment_bound_approval_rejects_every_wrong_identity_and_is_single_use() {
    let h = harness("native-alignment-bound", "default").expect("node fixture");
    let result = std::panic::AssertUnwindSafe(async {
        let mut events = h.hub.subscribe();
        h.turns.open(&h.session_id, &message("ask first")).unwrap();
        waiting(&mut events).await;
        let pending = h.turns.attach(&h.session_id)["streamStatus"]["pendingApprovals"][0].clone();
        let binding: ApprovalBinding = serde_json::from_value(pending.clone()).unwrap();
        for key in [
            "executionId",
            "anchorMessageId",
            "messageId",
            "approvalId",
            "toolCallId",
        ] {
            let mut wrong = pending.clone();
            wrong[key] = json!("foreign");
            let wrong = serde_json::from_value(wrong).unwrap();
            assert!(
                !h.turns.respond_bound_approval(&h.session_id, &wrong, true),
                "{key}"
            );
            assert_eq!(
                h.turns.attach(&h.session_id)["streamStatus"]["pendingApprovals"],
                json!([pending.clone()])
            );
        }
        assert!(
            !h.turns
                .respond_bound_approval("foreign-session", &binding, true)
        );
        assert!(
            h.turns
                .respond_bound_approval(&h.session_id, &binding, true)
        );
        assert!(
            !h.turns
                .respond_bound_approval(&h.session_id, &binding, true)
        );
        assert_eq!(snapshot(&h.turns).await["pendingApprovals"], json!([]));
        until_done(&mut events, |_| {}).await;
    })
    .catch_unwind()
    .await;
    h.turns.close_test_runtimes().await;
    let _ = std::fs::remove_dir_all(h.dir);
    if let Err(error) = result {
        std::panic::resume_unwind(error)
    }
}

#[test]
fn native_alignment_unknown_forced_stop_never_upgrades_to_success() {
    for forced in [json!("future_unrecognized_stop"), json!(42), json!({})] {
        let mut stop = PromptStop {
            session: "S".into(),
            prompt: "P".into(),
            epoch: 1,
            forced: None,
        };
        stop.accept(
            &json!({"sessionId":"S","promptId":"P","outcome":{"forced":forced}}),
            1,
        );
        assert!(matches!(
            PromptEnd::from_response(&json!({"stopReason":"end_turn"}), false, stop.forced),
            PromptEnd::Failed(_)
        ));
    }
    for forced in [json!({}), json!({"forced":null})] {
        let mut stop = PromptStop {
            session: "S".into(),
            prompt: "P".into(),
            epoch: 1,
            forced: None,
        };
        stop.accept(&json!({"sessionId":"S","promptId":"P","outcome":forced}), 1);
        assert!(matches!(
            PromptEnd::from_response(&json!({"stopReason":"end_turn"}), false, stop.forced),
            PromptEnd::Finished(_)
        ));
    }
}

#[tokio::test]
async fn native_alignment_permission_and_form_requests_require_the_active_acp_session() {
    const PEER: &str = r#"#!/usr/bin/env node
const send=v=>process.stdout.write(JSON.stringify(v)+'\n');let prompt;
require('readline').createInterface({input:process.stdin}).on('line',line=>{const m=JSON.parse(line);const reply=result=>send({jsonrpc:'2.0',id:m.id,result});
if(m.id===900){require('fs').writeFileSync(require('path').join(__dirname,'foreign-response.json'),JSON.stringify(m));send({jsonrpc:'2.0',id:prompt,result:{stopReason:'end_turn'}});return;}
if(m.method==='initialize')return reply({protocolVersion:1});if(m.method==='session/new')return reply({sessionId:'S'});if(m.method==='_a3s/goal/state')return reply({definition:null,tick:null});if(m.method==='session/set_model'||m.method==='session/set_mode')return reply({});
if(m.method==='session/cancel'){send({jsonrpc:'2.0',id:prompt,result:{stopReason:'cancelled'}});return;}
if(m.method==='session/prompt'){prompt=m.id;const fixture=JSON.parse(m.params.prompt[0].text);let params=fixture.method==='session/request_permission'?{toolCall:{toolCallId:'foreign-call',title:'foreign',kind:'edit'},options:[{optionId:'allow-once',name:'Allow',kind:'allow_once'}]}:{formId:'foreign-form',format:'openui',spec:'root = Card("foreign", [])'};if(fixture.session!==null)params.sessionId=fixture.session;send({jsonrpc:'2.0',id:900,method:fixture.method,params});}
});
"#;
    for (method, index) in ["session/request_permission", "_a3s/form/request"]
        .into_iter()
        .enumerate()
        .map(|(i, m)| (m, i))
    {
        for (session, suffix) in [(json!("foreign"), "foreign"), (Value::Null, "missing")] {
            let h = harness(&format!("native-session-scope-{index}-{suffix}"), "default")
                .expect("node fixture");
            std::fs::write(h.dir.join("fake-acp"), PEER).unwrap();
            let result = std::panic::AssertUnwindSafe(async {
                let mut events = h.hub.subscribe();
                let mut surfaced = false;
                h.turns
                    .open(
                        &h.session_id,
                        &message(&json!({"method":method,"session":session}).to_string()),
                    )
                    .unwrap();
                let seen = until_done(&mut events, |event| {
                    if matches!(
                        event
                            .pointer("/payload/status/status")
                            .and_then(Value::as_str),
                        Some("awaiting-approval" | "awaiting-form")
                    ) {
                        surfaced = true;
                        h.turns.abort(&h.session_id);
                    }
                })
                .await;
                assert!(
                    !surfaced,
                    "{method}/{suffix} must not create current execution authority"
                );
                assert_eq!(seen.last().unwrap()["payload"]["status"], "success");
                let reply: Value = serde_json::from_slice(
                    &std::fs::read(h.dir.join("foreign-response.json")).unwrap(),
                )
                .unwrap();
                assert_eq!(reply["error"]["code"], -32602);
                assert_eq!(h.turns.attach(&h.session_id)["status"], "not-found");
            })
            .catch_unwind()
            .await;
            h.turns.close_test_runtimes().await;
            let _ = std::fs::remove_dir_all(h.dir);
            if let Err(error) = result {
                std::panic::resume_unwind(error)
            }
        }
    }
}
