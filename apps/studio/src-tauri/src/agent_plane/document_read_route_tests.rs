#![cfg(test)]

use super::tests::{harness, message, until_done};
use super::*;

const PEER: &str = r#"#!/usr/bin/env node
const rl = require('readline').createInterface({ input: process.stdin });
const path = require('path');
const cwd = process.argv[process.argv.indexOf('--cwd') + 1];
let prompt;
const send = value => process.stdout.write(JSON.stringify(value) + '\n');
rl.on('line', line => {
  const m = JSON.parse(line);
  if (m.method === 'initialize') return send({jsonrpc:'2.0',id:m.id,result:{protocolVersion:1}});
  if (m.method === 'session/new') return send({jsonrpc:'2.0',id:m.id,result:{sessionId:'S'}});
  if (m.method === 'session/set_mode') return send({jsonrpc:'2.0',id:m.id,result:{}});
  if (m.method === 'session/prompt') {
    prompt = m.id;
    const text = JSON.stringify(m.params);
    const file = text.includes('read-unsupported') ? 'binary.xlsx' : text.includes('read-oversize') ? 'large.txt' : 'note.txt';
    return send({jsonrpc:'2.0',id:900,method:'_a3s/document/read',params:{sessionId:text.includes('wrong-session') ? 'foreign' : 'S',path:path.join(cwd,file)}});
  }
  if (m.id === 900) {
    send({jsonrpc:'2.0',method:'session/update',params:{sessionId:'S',update:{sessionUpdate:'agent_message_chunk',content:{type:'text',text:JSON.stringify(m.result || m.error)}}}});
    return send({jsonrpc:'2.0',id:prompt,result:{stopReason:'end_turn'}});
  }
});
"#;

#[tokio::test(flavor = "multi_thread")]
async fn native_turn_pump_reads_the_shell_model_and_refuses_foreign_session() {
    let mut h = harness("document-read-route", "always-approve").expect("Node is required");
    std::fs::write(h.dir.join("fake-acp"), PEER).unwrap();
    let root = PathBuf::from(h.store.session(&h.session_id).unwrap().workspace.path);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("note.txt"), "disk text").unwrap();
    let documents = Arc::new(crate::workspace_document::WorkspaceDocuments::new(
        Arc::new(crate::workspace_snapshot::WorkspaceSnapshots::default()),
    ));
    let editor = documents
        .open(
            root.to_str().unwrap(),
            root.join("note.txt").to_str().unwrap(),
        )
        .unwrap();
    documents
        .edit(
            &editor.lease,
            1,
            editor.snapshot.version,
            "Studio native unsaved text",
        )
        .unwrap();
    let old = &h.turns;
    let config = Arc::new(PlaneConfig {
        root: old.config.root.clone(),
        credentials: old.config.credentials.clone(),
        binary: old.config.binary.clone(),
        workspace_documents: Some(documents.clone()),
        schedule_mcp: None,
        mail_mcp: None,
        slug: String::new(),
        personal_wiki: None,
        cloud_agent_runtime: None,
    });
    h.turns = Arc::new(TurnService::new(
        h.store.clone(),
        old.agents.clone(),
        old.sessions.clone(),
        config,
        h.hub.clone(),
    ));
    let mut events = h.hub.subscribe();
    h.turns
        .open(&h.session_id, &message("read document"))
        .unwrap();
    let seen = until_done(&mut events, |_| {}).await;
    assert_eq!(seen.last().unwrap()["payload"]["status"], "success");
    let rows = h.store.messages(&h.session_id);
    let value: Value = serde_json::from_str(&rows.last().unwrap().searchable_text).unwrap();
    assert_eq!(value["snapshot"]["text"], "Studio native unsaved text");
    assert_eq!(value["snapshot"]["documentId"], editor.snapshot.document_id);
    assert_eq!(value["snapshot"]["version"], 2);
    assert_eq!(value["released"], true);
    h.turns
        .open(&h.session_id, &message("wrong-session"))
        .unwrap();
    until_done(&mut events, |_| {}).await;
    let rows = h.store.messages(&h.session_id);
    let value: Value = serde_json::from_str(&rows.last().unwrap().searchable_text).unwrap();
    assert_eq!(value["message"], "document not found");
    assert!(value.get("snapshot").is_none());
    std::fs::write(root.join("binary.xlsx"), b"PK\x03\x04\0binary").unwrap();
    std::fs::write(root.join("large.txt"), "x".repeat(80 * 1024)).unwrap();
    for (prompt, error) in [
        (
            "read-unsupported",
            crate::workspace_document::DocumentError::Unsupported,
        ),
        (
            "read-oversize",
            crate::workspace_document::DocumentError::TooLarge,
        ),
    ] {
        h.turns.open(&h.session_id, &message(prompt)).unwrap();
        until_done(&mut events, |_| {}).await;
        let rows = h.store.messages(&h.session_id);
        let value: Value = serde_json::from_str(&rows.last().unwrap().searchable_text).unwrap();
        assert_eq!(value["message"], error.to_string());
        assert!(value.get("snapshot").is_none());
    }
    assert_eq!(
        std::fs::read_to_string(root.join("note.txt")).unwrap(),
        "disk text"
    );
    documents.close(&editor.lease).unwrap();
    h.turns.discard_runtime(&h.session_id).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn native_turn_without_document_host_refuses_read_request() {
    let h = harness("document-host-absent", "always-approve").expect("Node is required");
    std::fs::write(h.dir.join("fake-acp"), PEER).unwrap();
    let mut events = h.hub.subscribe();
    h.turns
        .open(&h.session_id, &message("read document"))
        .unwrap();
    until_done(&mut events, |_| {}).await;
    let rows = h.store.messages(&h.session_id);
    let value: Value = serde_json::from_str(&rows.last().unwrap().searchable_text).unwrap();
    assert_eq!(value["message"], "document not found");
    assert!(value.get("snapshot").is_none());
    h.turns.discard_runtime(&h.session_id).await;
}

const PIN_CANCEL_PEER: &str = r#"#!/usr/bin/env node
const rl = require('readline').createInterface({ input: process.stdin });
const path = require('path'); const fs = require('fs');
const cwd = process.argv[process.argv.indexOf('--cwd') + 1];
const runId = '00112233-4455-4677-8899-aabbccddeeff';
let prompt;
const send = value => process.stdout.write(JSON.stringify(value) + '\n');
rl.on('line', line => {
  const m = JSON.parse(line);
  if (m.method === 'initialize') return send({jsonrpc:'2.0',id:m.id,result:{protocolVersion:1}});
  if (m.method === 'session/new') return send({jsonrpc:'2.0',id:m.id,result:{sessionId:'S'}});
  if (m.method === 'session/set_mode') return send({jsonrpc:'2.0',id:m.id,result:{}});
  if (m.method === 'session/prompt') {
    prompt=m.id;
    return send({jsonrpc:'2.0',id:900,method:'_a3s/document/read',params:{sessionId:'S',runId,path:path.join(cwd,'note.txt')}});
  }
  if (m.id === 900) {
    if (!m.result || m.result.released !== false) throw new Error('pin not acknowledged');
    return send({jsonrpc:'2.0',method:'session/update',params:{sessionId:'S',update:{sessionUpdate:'agent_thought_chunk',content:{type:'text',text:'pinned-ready'}}}});
  }
  if (m.method === 'session/cancel') return send({jsonrpc:'2.0',id:901,method:'_a3s/document/release_run',params:{sessionId:'S',runId}});
  if (m.id === 901) {
    if (!m.result || !m.result.released) throw new Error('cancelled release not acknowledged');
    return send({jsonrpc:'2.0',id:902,method:'_a3s/document/read',params:{sessionId:'S',runId,path:path.join(cwd,'note.txt')}});
  }
  if (m.id === 902) {
    fs.writeFileSync(path.join(cwd,'native-cleanup-receipt'),JSON.stringify({denied:!!m.error,released:true}));
    return send({jsonrpc:'2.0',id:prompt,result:{stopReason:'cancelled'}});
  }
});
"#;

#[tokio::test(flavor = "multi_thread")]
async fn cancelled_native_prompt_allows_run_release_and_refuses_new_document_reads() {
    let mut h = harness("document-pin-cancel-route", "always-approve").expect("Node is required");
    std::fs::write(h.dir.join("fake-acp"), PIN_CANCEL_PEER).unwrap();
    let root = PathBuf::from(h.store.session(&h.session_id).unwrap().workspace.path);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("note.txt"), "clean text").unwrap();
    let documents = Arc::new(crate::workspace_document::WorkspaceDocuments::new(
        Arc::new(crate::workspace_snapshot::WorkspaceSnapshots::default()),
    ));
    let editor = documents
        .open(
            root.to_str().unwrap(),
            root.join("note.txt").to_str().unwrap(),
        )
        .unwrap();
    let identity = editor.snapshot.document_id.clone();
    let old = &h.turns;
    let config = Arc::new(PlaneConfig {
        root: old.config.root.clone(),
        credentials: old.config.credentials.clone(),
        binary: old.config.binary.clone(),
        workspace_documents: Some(documents.clone()),
        schedule_mcp: None,
        mail_mcp: None,
        slug: String::new(),
        personal_wiki: None,
        cloud_agent_runtime: None,
    });
    h.turns = Arc::new(TurnService::new(
        h.store.clone(),
        old.agents.clone(),
        old.sessions.clone(),
        config,
        h.hub.clone(),
    ));
    let mut events = h.hub.subscribe();
    h.turns
        .open(&h.session_id, &message("pin and cancel"))
        .unwrap();
    let turns = h.turns.clone();
    let session = h.session_id.clone();
    let seen = until_done(&mut events, |event| {
        if event["payload"]["chunk"]["type"] == "reasoning-delta" {
            documents.close(&editor.lease).unwrap();
            turns.abort(&session);
        }
    })
    .await;
    assert_eq!(seen.last().unwrap()["payload"]["status"], "paused");
    let receipt: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("native-cleanup-receipt")).unwrap(),
    )
    .unwrap();
    assert_eq!(receipt, json!({"released":true,"denied":true}));
    let next = documents
        .open(
            root.to_str().unwrap(),
            root.join("note.txt").to_str().unwrap(),
        )
        .unwrap();
    assert_ne!(next.snapshot.document_id, identity);
    documents.close(&next.lease).unwrap();
    h.turns.discard_runtime(&h.session_id).await;
}
