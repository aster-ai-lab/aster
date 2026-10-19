use super::*;

const PEER: &str = r#"#!/usr/bin/env node
const fs=require('fs'),path=require('path');
const root=path.dirname(process.argv[1]), trace=path.join(root,'reasoning-trace.jsonl');
let effort='high',mode='unset';
const send=m=>process.stdout.write(JSON.stringify(m)+'\n');
require('readline').createInterface({input:process.stdin}).on('line',line=>{
 const m=JSON.parse(line),reply=result=>send({jsonrpc:'2.0',id:m.id,result});
 fs.appendFileSync(trace,JSON.stringify({method:m.method,params:m.params,effort,mode})+'\n');
 if(m.method==='initialize')return reply({protocolVersion:1});
 if(m.method==='session/new')return reply({sessionId:'reasoning-session'});
 if(m.method==='session/set_mode'){mode=m.params.modeId;return reply({});}
 if(m.method==='session/set_model'){
  if(fs.existsSync(path.join(root,'reject-effort')))return send({jsonrpc:'2.0',id:m.id,error:{code:-32602,message:'Unsupported reasoning effort'}});
  effort=m.params._meta.reasoningEffort;mode='unset';return reply({});
 }
 if(m.method==='session/prompt'){
  if(m.params.prompt[0].text.includes('retry-effort')){
   const file=path.join(root,'retry-count');let count=0;
   try{count=Number(fs.readFileSync(file,'utf8'))}catch(_){}
   fs.writeFileSync(file,String(++count));
   if(count<=2)return send({jsonrpc:'2.0',id:m.id,error:{code:-32603,message:'Internal error',data:'context length exceeded'}});
  }
  send({jsonrpc:'2.0',method:'session/update',params:{sessionId:'reasoning-session',update:{sessionUpdate:'agent_message_chunk',content:{type:'text',text:effort+'/'+mode}}}});
  return reply({stopReason:'end_turn'});
 }
});
"#;

fn fixture(name: &str) -> tests::Harness {
    let h = tests::harness(name, "auto").expect("node is required for the ACP fixture");
    std::fs::write(h.dir.join("fake-acp"), PEER).unwrap();
    h
}

fn trace(h: &tests::Harness) -> Vec<Value> {
    std::fs::read_to_string(h.dir.join("reasoning-trace.jsonl"))
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

async fn submit(h: &tests::Harness, effort: Option<Value>) -> Vec<Value> {
    let mut events = h.hub.subscribe();
    let mut body = tests::message("hello");
    if let Some(effort) = effort {
        body["reasoningEffort"] = effort;
    }
    h.turns.open(&h.session_id, &body).unwrap();
    tests::until_done(&mut events, |_| {}).await
}

#[tokio::test(flavor = "multi_thread")]
async fn explicit_effort_reaches_acp_before_the_prompt_and_reapplies_mode() {
    let h = fixture("effort-wire");
    for effort in [
        "low", "low", "minimal", "medium", "high", "xhigh", "max", "none", "default",
    ] {
        let events = submit(&h, Some(json!(effort))).await;
        assert!(
            !events
                .iter()
                .any(|event| event["channel"] == "ai.stream.error")
        );
        let text = events
            .iter()
            .filter_map(|event| {
                event
                    .pointer("/payload/chunk/delta")
                    .and_then(Value::as_str)
            })
            .collect::<String>();
        assert!(text.contains(&format!("{effort}/auto")), "{text:?}");
    }
    let calls = trace(&h);
    let changes: Vec<_> = calls
        .iter()
        .filter(|call| call["method"] == "session/set_model")
        .collect();
    assert_eq!(
        changes.len(),
        8,
        "an unchanged selection must not rebuild the session"
    );
    assert_eq!(
        changes
            .iter()
            .map(|call| call["params"]["_meta"]["reasoningEffort"].as_str().unwrap())
            .collect::<Vec<_>>(),
        [
            "low", "minimal", "medium", "high", "xhigh", "max", "none", "default"
        ]
    );
    assert!(
        changes
            .iter()
            .all(|call| call["params"]["modelId"] == "openai/gpt-test")
    );
    assert_eq!(
        calls
            .iter()
            .filter(|call| call["method"] == "initialize")
            .count(),
        1
    );
    h.turns.close_test_runtimes().await;
    let _ = std::fs::remove_dir_all(h.dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn absent_or_null_effort_keeps_the_existing_default() {
    let h = fixture("effort-absent");
    for effort in [None, Some(Value::Null)] {
        let events = submit(&h, effort).await;
        assert!(
            !events
                .iter()
                .any(|event| event["channel"] == "ai.stream.error")
        );
    }
    assert!(
        !trace(&h)
            .iter()
            .any(|call| call["method"] == "session/set_model")
    );
    submit(&h, Some(json!("low"))).await;
    for effort in [None, Some(Value::Null)] {
        submit(&h, effort).await;
    }
    let calls = trace(&h);
    assert_eq!(
        calls
            .iter()
            .filter(|call| call["method"] == "session/set_model")
            .count(),
        1
    );
    let prompts: Vec<_> = calls
        .iter()
        .filter(|call| call["method"] == "session/prompt")
        .collect();
    assert!(prompts[2..].iter().all(|call| call["effort"] == "low"));
    h.turns.close_test_runtimes().await;
    let _ = std::fs::remove_dir_all(h.dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_rejected_effort_prevents_the_model_prompt() {
    let h = fixture("effort-rejected");
    std::fs::write(h.dir.join("reject-effort"), b"1").unwrap();
    let events = submit(&h, Some(json!("low"))).await;
    assert!(
        events
            .iter()
            .any(|event| event["channel"] == "ai.stream.error")
    );
    assert!(
        !trace(&h)
            .iter()
            .any(|call| call["method"] == "session/prompt")
    );
    h.turns.close_test_runtimes().await;
    let _ = std::fs::remove_dir_all(h.dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn unsupported_effort_is_refused_before_storing_or_launching() {
    let h = fixture("effort-invalid");
    let before = h.store.messages(&h.session_id).len();
    for effort in [
        json!("auto"),
        json!("ultra"),
        json!("wrong"),
        json!(""),
        json!(42),
    ] {
        let mut body = tests::message("hello");
        body["reasoningEffort"] = effort;
        let error = h
            .turns
            .open(&h.session_id, &body)
            .expect_err("unsupported local effort");
        assert!(format!("{error:?}").contains("reasoning effort"));
    }
    assert_eq!(h.store.messages(&h.session_id).len(), before);
    assert!(trace(&h).is_empty());
    h.turns.close_test_runtimes().await;
    let _ = std::fs::remove_dir_all(h.dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_failed_switch_cannot_leave_an_old_effort_or_mode_cache() {
    let h = fixture("effort-rejection-cache");
    submit(&h, Some(json!("low"))).await;
    std::fs::write(h.dir.join("reject-effort"), b"1").unwrap();
    let failed = submit(&h, Some(json!("high"))).await;
    assert!(
        failed
            .iter()
            .any(|event| event["channel"] == "ai.stream.error")
    );
    std::fs::remove_file(h.dir.join("reject-effort")).unwrap();
    let completed = submit(&h, Some(json!("low"))).await;
    assert!(
        !completed
            .iter()
            .any(|event| event["channel"] == "ai.stream.error")
    );
    let calls = trace(&h);
    assert_eq!(
        calls
            .iter()
            .filter(|call| call["method"] == "session/set_model")
            .count(),
        3
    );
    assert_eq!(
        calls
            .iter()
            .filter(|call| call["method"] == "initialize")
            .count(),
        1
    );
    let prompts: Vec<_> = calls
        .iter()
        .filter(|call| call["method"] == "session/prompt")
        .collect();
    assert_eq!(prompts.len(), 2);
    assert!(
        prompts
            .iter()
            .all(|call| call["effort"] == "low" && call["mode"] == "auto")
    );
    h.turns.close_test_runtimes().await;
    let _ = std::fs::remove_dir_all(h.dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_restarted_retry_receives_the_captured_effort_before_its_prompt() {
    let h = fixture("effort-retry");
    let mut events = h.hub.subscribe();
    let mut body = tests::message("retry-effort");
    body["reasoningEffort"] = json!("low");
    h.turns.open(&h.session_id, &body).unwrap();
    let completed = tests::until_done(&mut events, |_| {}).await;
    assert!(
        !completed
            .iter()
            .any(|event| event["channel"] == "ai.stream.error")
    );
    let calls = trace(&h);
    assert_eq!(
        calls
            .iter()
            .filter(|call| call["method"] == "session/set_model")
            .count(),
        2
    );
    assert_eq!(
        calls
            .iter()
            .filter(|call| call["method"] == "initialize")
            .count(),
        2
    );
    let prompts: Vec<_> = calls
        .iter()
        .filter(|call| call["method"] == "session/prompt")
        .collect();
    assert_eq!(prompts.len(), 3);
    assert!(
        prompts
            .iter()
            .all(|call| call["effort"] == "low" && call["mode"] == "auto")
    );
    h.turns.close_test_runtimes().await;
    let _ = std::fs::remove_dir_all(h.dir);
}
