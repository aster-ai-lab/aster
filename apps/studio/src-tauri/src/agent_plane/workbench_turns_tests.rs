#![cfg(test)]

use super::*;
use crate::agent_plane::{
    PlaneConfig,
    catalog::{AgentCatalog, SessionCatalog},
    events::EventHub,
    store::PlaneStore,
};
use std::io::{Read, Write};

#[test]
fn workbench_tool_evidence_keeps_literal_text_for_exact_review_quotes() {
    let output = "line one\n\"quoted line\"";
    let evidence = tools(&[
        json!({"type":"dynamic-tool","toolCallId":"text","toolName":"read","state":"output-available","output":output}),
        json!({"type":"dynamic-tool","toolCallId":"error","toolName":"write","state":"output-error","errorText":output}),
        json!({"type":"dynamic-tool","toolCallId":"object","toolName":"bash","state":"output-available","output":{"exit_code":0}}),
    ]);
    assert_eq!(evidence[0].output, output);
    assert_eq!(evidence[1].output, output);
    assert_eq!(evidence[2].output, "{\"exit_code\":0}");
}

const ACP: &str = r#"#!/usr/bin/env node
const fs=require('fs'),path=require('path'); let cwd='',count=0,pendingWrite=null;
const send=m=>process.stdout.write(JSON.stringify(m)+'\n');
require('readline').createInterface({input:process.stdin}).on('line',line=>{
const m=JSON.parse(line),reply=result=>send({jsonrpc:'2.0',id:m.id,result});
if(m.method==='initialize')return reply({protocolVersion:1});
if(m.method==='session/new'){cwd=m.params.cwd; fs.mkdirSync(cwd,{recursive:true}); return reply({sessionId:'proof-session'});}
if(m.method==='session/set_mode')return reply({});
if(m.id===901 && pendingWrite){ const finish=pendingWrite;pendingWrite=null;if(m.result.outcome.optionId==='allow-once')finish();return;}
if(m.method==='session/prompt'){
const body=++count===1?'draft':'corrected', file=path.join(cwd,'report.txt');
const update=u=>send({jsonrpc:'2.0',method:'session/update',params:{sessionId:'proof-session',update:u}});
update({sessionUpdate:'agent_message_chunk',content:{type:'text',text:body}});
update({sessionUpdate:'tool_call',toolCallId:'write-'+count,title:'Write report',kind:'edit',status:'in_progress',rawInput:{file_path:file,content:body}});
const finish=()=>{fs.writeFileSync(file,body);update({sessionUpdate:'tool_call_update',toolCallId:'write-'+count,status:'completed',content:[{type:'diff',path:file,oldText:'',newText:body}]});reply({stopReason:fs.existsSync(path.join(path.dirname(process.argv[1]),'truncate'))?'max_tokens':'end_turn'});};
if(count>1 && fs.existsSync(path.join(path.dirname(process.argv[1]),'ask-repair'))){pendingWrite=finish;send({jsonrpc:'2.0',id:901,method:'session/request_permission',params:{sessionId:'proof-session',toolCall:{toolCallId:'write-'+count,title:'write',kind:'edit',rawInput:{file_path:file,content:body}},options:[{optionId:'allow-once',kind:'allow_once'},{optionId:'reject-once',kind:'reject_once'}]}});return;}
return finish();
}
});
"#;

struct Fixture {
    turns: Arc<TurnService>,
    session: String,
    root: PathBuf,
    calls: Arc<std::sync::atomic::AtomicUsize>,
    invalid: Arc<std::sync::atomic::AtomicBool>,
    hold: Arc<std::sync::atomic::AtomicBool>,
    resolved: Arc<std::sync::atomic::AtomicBool>,
    stop: Arc<std::sync::atomic::AtomicBool>,
    server: Option<std::thread::JoinHandle<()>>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(server) = self.server.take() {
            let result = server.join();
            if !std::thread::panicking() {
                assert!(result.is_ok());
            }
        }
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn fixture(repair: bool, invalid: bool) -> Fixture {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let base = format!("http://{}/v1", listener.local_addr().unwrap());
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stopped = Arc::clone(&stop);
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let called = Arc::clone(&calls);
    let invalid = Arc::new(std::sync::atomic::AtomicBool::new(invalid));
    let invalid_response = Arc::clone(&invalid);
    let hold = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let held = Arc::clone(&hold);
    let resolved = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let verified = Arc::clone(&resolved);
    let server = std::thread::spawn(move || {
        while !stopped.load(Ordering::SeqCst) {
            let Ok((mut socket, _)) = listener.accept() else {
                std::thread::sleep(Duration::from_millis(5));
                continue;
            };
            socket.set_nonblocking(false).unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut bytes = Vec::new();
            let mut chunk = [0; 4096];
            loop {
                let n = socket.read(&mut chunk).unwrap();
                assert_ne!(n, 0);
                bytes.extend_from_slice(&chunk[..n]);
                let request = String::from_utf8_lossy(&bytes);
                if let Some(end) = request.find("\r\n\r\n") {
                    let length: usize = request[..end]
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .and_then(|length| length.trim().parse().ok())
                        })
                        .unwrap();
                    if bytes.len() >= end + 4 + length {
                        break;
                    }
                }
            }
            let request = String::from_utf8(bytes).unwrap();
            let body: Value =
                serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
            assert!(body.get("tools").is_none());
            assert!(!body["messages"].to_string().contains("test-secret"));
            let evidence: Value =
                serde_json::from_str(body["messages"][1]["content"][0]["text"].as_str().unwrap())
                    .unwrap();
            called.fetch_add(1, Ordering::SeqCst);
            while held.load(Ordering::SeqCst) && !stopped.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(5));
            }
            let result = if invalid_response.load(Ordering::SeqCst) {
                "invalid output".to_owned()
            } else if repair && evidence["repair"].is_null() {
                json!({"findings":[{"artifactId":evidence["original"]["artifacts"][0]["id"],"severity":"warning","category":"change","claim":"The file is a draft","evidenceQuote":"draft","guidance":"Correct it"}],"resolutions":[]}).to_string()
            } else {
                if repair {
                    assert_eq!(evidence["artifactContents"][1]["text"], "corrected");
                }
                let resolutions = if evidence["repair"].is_null() {
                    vec![]
                } else {
                    evidence["original"]["review"]["findings"].as_array().unwrap().iter().filter(|finding|finding["status"]=="pending").map(|finding|json!({"findingId":finding["id"],"resolved":verified.load(Ordering::SeqCst),"reason":"Independent resolution evidence"})).collect()
                };
                json!({"findings":[],"resolutions":resolutions}).to_string()
            };
            let response =
                json!({"choices":[{"finish_reason":"stop","message":{"content":result}}]})
                    .to_string();
            let _ = write!(
                socket,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response.len(),
                response
            );
        }
    });
    let root = std::env::temp_dir().join(format!("a3s-proof-e2e-{}", new_id()));
    std::fs::create_dir_all(&root).unwrap();
    let binary = root.join("agent");
    std::fs::write(&binary, ACP).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let store = Arc::new(PlaneStore::open(Some(root.join("state"))));
    let agents = Arc::new(AgentCatalog::new(Arc::clone(&store)));
    let sessions = Arc::new(SessionCatalog::new(Arc::clone(&store)));
    let agent=agents.create(&json!({"type":"a3s-code","name":"Proof test","model":"openai::model","configuration":{"permission_mode":"auto"}})).unwrap();
    let session = sessions
        .create(&json!({"agentId":agent.id,"name":"Proof test","workspace":{"type":"system"}}))
        .unwrap();
    let config = Arc::new(PlaneConfig {
        root: Some(root.join("state")),
        credentials: Arc::new(move |_| {
            Some(crate::providers::ProviderCredentials {
                base_url: base.clone(),
                api_key: "test-secret".into(),
            })
        }),
        binary: Some(binary),
        workspace_documents: None,
        schedule_mcp: None,
        mail_mcp: None,
        slug: String::new(),
        personal_wiki: None,
        cloud_agent_runtime: None,
    });
    Fixture {
        turns: Arc::new(TurnService::new(
            store,
            agents,
            sessions,
            config,
            Arc::new(EventHub::new()),
        )),
        session: session.id,
        root,
        calls,
        invalid,
        hold,
        resolved,
        stop,
        server: Some(server),
    }
}

async fn send(f: &Fixture, review: bool, trace: bool) -> Proof {
    let mut body = super::super::tests::message("Write the report");
    body["turnContext"] = json!({"workbench":{"reviewMode":review,"artifactTrace":trace}});
    f.turns.open(&f.session, &body).unwrap();
    settled(f).await;
    f.turns
        .stored_proofs(&f.session)
        .into_iter()
        .next()
        .unwrap()
}

async fn settled(f: &Fixture) {
    tokio::time::timeout(Duration::from_secs(12), async {
        while f.turns.lock_live().contains_key(&f.session) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("native turn settled");
}

#[tokio::test(flavor = "multi_thread")]
async fn workbench_four_policies_bind_to_native_messages_and_actual_bytes() {
    for review in [false, true] {
        for trace in [false, true] {
            let f = fixture(false, false);
            let proof = send(&f, review, trace).await;
            assert_eq!(
                proof.policy,
                Policy {
                    review_mode: review,
                    artifact_trace: trace
                }
            );
            assert_eq!(proof.source_text, "draft");
            assert_eq!(
                proof.review.status,
                if review {
                    ReviewStatus::Completed
                } else {
                    ReviewStatus::Off
                }
            );
            assert_eq!(f.calls.load(Ordering::SeqCst), usize::from(review));
            assert_eq!(proof.artifacts.len(), usize::from(review || trace));
            if let Some(version) = proof.artifacts.first() {
                assert_eq!(
                    f.turns.proof_artifacts.read(&f.session, version).unwrap(),
                    b"draft"
                );
                assert!(f.turns.workbench_artifact("other", &version.id).is_err());
                assert_eq!(
                    f.turns.workbench_artifact(&f.session, &version.id).unwrap()["contentDigest"],
                    version.content_digest
                );
            }
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn workbench_repair_appends_versions_then_requires_explicit_acceptance() {
    let f = fixture(true, false);
    let proof = send(&f, true, true).await;
    assert_eq!(proof.review.status, ReviewStatus::OpenQueue);
    assert_eq!(proof.review.findings.len(), 1);
    let finding = &proof.review.findings[0];
    assert_eq!(finding.status, FindingStatus::Addressed);
    assert_eq!(f.calls.load(Ordering::SeqCst), 2);
    let proofs = f.turns.stored_proofs(&f.session);
    assert_eq!(proofs.len(), 2);
    assert_eq!(proofs[0].source_text, "draft");
    assert_eq!(proofs[1].source_text, "corrected");
    assert_eq!(proofs[1].artifacts[0].ordinal, 2);
    assert_eq!(
        f.turns
            .proof_artifacts
            .read(&f.session, &proofs[0].artifacts[0])
            .unwrap(),
        b"draft"
    );
    assert_eq!(
        f.turns
            .proof_artifacts
            .read(&f.session, &proofs[1].artifacts[0])
            .unwrap(),
        b"corrected"
    );
    assert!(
        f.turns
            .workbench_proof_action(
                &f.session,
                &proof.message_id,
                json!({"action":"ignore","findingId":finding.id})
            )
            .is_err()
    );
    let accepted = f
        .turns
        .workbench_proof_action(
            &f.session,
            &proof.message_id,
            json!({"action":"accept","findingId":finding.id}),
        )
        .unwrap();
    assert_eq!(accepted["review"]["findings"][0]["status"], "accepted");
    assert_eq!(accepted["review"]["status"], "completed");
    assert!(
        f.turns
            .workbench_proof_action(
                "other",
                &proof.message_id,
                json!({"action":"accept","findingId":finding.id})
            )
            .is_err()
    );
    let restored = PlaneStore::open(Some(f.root.join("state")));
    let saved = restored.messages(&f.session);
    let original = saved.iter().find(|row| row.id == proof.message_id).unwrap();
    assert_eq!(
        Proof::from_parts(original.parts()).unwrap().review.findings[0].status,
        FindingStatus::Accepted
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn workbench_invalid_independent_response_is_failed_not_clean() {
    let f = fixture(false, true);
    let proof = send(&f, true, true).await;
    assert_eq!(proof.review.status, ReviewStatus::Failed);
    assert!(proof.review.error.is_some());
    assert_eq!(proof.source_text, "draft");
    assert_eq!(proof.artifacts.len(), 1);
    assert!(
        f.turns
            .workbench_proof_action(
                &f.session,
                &proof.message_id,
                json!({"action":"accept","findingId":"forged"})
            )
            .is_err()
    );
    f.invalid.store(false, Ordering::SeqCst);
    f.turns
        .workbench_proof_action(&f.session, &proof.message_id, json!({"action":"retry"}))
        .unwrap();
    assert!(
        f.turns.store.session_executions()[&f.session]
            .status
            .is_live()
    );
    settled(&f).await;
    let retried = f
        .turns
        .stored_proofs(&f.session)
        .into_iter()
        .next()
        .unwrap();
    assert_eq!(retried.review.status, ReviewStatus::Completed);
    assert_eq!(retried.artifacts[0].id, proof.artifacts[0].id);
    assert_eq!(f.turns.store.messages(&f.session).len(), 2);
    assert_eq!(
        f.turns.store.session_executions()[&f.session].status,
        ExecutionStatus::Done
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn workbench_review_cancel_preserves_source_and_refuses_interjection() {
    let f = fixture(false, false);
    f.hold.store(true, Ordering::SeqCst);
    let mut body = super::super::tests::message("Write report");
    body["turnContext"] = json!({"workbench":{"reviewMode":true,"artifactTrace":true}});
    f.turns.open(&f.session, &body).unwrap();
    tokio::time::timeout(Duration::from_secs(8), async {
        while f.calls.load(Ordering::SeqCst) == 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let proof = f
        .turns
        .stored_proofs(&f.session)
        .into_iter()
        .next()
        .unwrap();
    assert_eq!(proof.review.status, ReviewStatus::Running);
    assert!(
        f.turns
            .insert_into_live(&f.session, &super::super::tests::message("Change the task"))
            .await
            .is_err()
    );
    f.turns
        .workbench_proof_action(&f.session, &proof.message_id, json!({"action":"cancel"}))
        .unwrap();
    settled(&f).await;
    f.hold.store(false, Ordering::SeqCst);
    let proof = f
        .turns
        .stored_proofs(&f.session)
        .into_iter()
        .next()
        .unwrap();
    assert_eq!(proof.review.status, ReviewStatus::Cancelled);
    assert_eq!(proof.source_text, "draft");
    assert_eq!(proof.artifacts.len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn workbench_http_proofs_artifacts_and_dispositions_enforce_identity_and_auth() {
    let f = fixture(true, false);
    let proof = send(&f, true, true).await;
    let config = PlaneConfig {
        root: Some(f.root.join("state")),
        credentials: Arc::clone(&f.turns.config.credentials),
        binary: f.turns.config.binary.clone(),
        workspace_documents: None,
        schedule_mcp: None,
        mail_mcp: None,
        slug: String::new(),
        personal_wiki: None,
        cloud_agent_runtime: None,
    };
    let app = crate::agent_plane::application(config, "local-proof-test".into()).unwrap();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let base = format!(
        "http://{}/api/agent-sessions/{}",
        listener.local_addr().unwrap(),
        f.session
    );
    let server = tokio::spawn(crate::agent_plane::serve(app, listener));
    let client = reqwest::Client::new();
    let endpoint = format!("{base}/proofs");
    let response = client.get(&endpoint).send().await.unwrap();
    assert!(!response.status().is_success());
    let response = client
        .get(&endpoint)
        .bearer_auth("local-proof-test")
        .send()
        .await
        .unwrap();
    assert!(response.status().is_success());
    let rows: Value = response.json().await.unwrap();
    assert_eq!(rows.as_array().unwrap().len(), 2);
    let version = &proof.artifacts[0];
    let response = client
        .get(format!("{base}/proof-artifacts/{}", version.id))
        .bearer_auth("local-proof-test")
        .send()
        .await
        .unwrap();
    assert!(response.status().is_success());
    assert_eq!(response.json::<Value>().await.unwrap()["data"], "ZHJhZnQ=");
    let response = client
        .post(format!("{base}/messages/{}/proof", proof.message_id))
        .bearer_auth("local-proof-test")
        .json(&json!({"action":"accept","findingId":proof.review.findings[0].id}))
        .send()
        .await
        .unwrap();
    assert!(response.status().is_success());
    assert_eq!(
        response.json::<Value>().await.unwrap()["review"]["findings"][0]["status"],
        "accepted"
    );
    let response = client
        .post(format!("{base}/messages/{}/proof", proof.message_id))
        .bearer_auth("local-proof-test")
        .json(&json!({"action":"accept","findingId":"forged","sessionId":"other"}))
        .send()
        .await
        .unwrap();
    assert!(!response.status().is_success());
    let response=client.post(format!("{base}/turns")).bearer_auth("local-proof-test").json(&json!({"userMessageParts":[{"type":"text","text":"Must not execute an external runtime"}],"turnContext":{"workbench":{"reviewMode":true,"artifactTrace":true},"agentRuntime":{"runtime":"edge","entry":"never-run"}}})).send().await.unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);
    assert_eq!(f.calls.load(Ordering::SeqCst), 2);
    server.abort();
    let _ = server.await;
}

#[test]
fn workbench_terminal_dispositions_survive_later_progress_publication() {
    let mut proof = Proof::new(
        "session",
        "message",
        "run",
        "openai::model",
        "request",
        Policy {
            review_mode: true,
            artifact_trace: true,
        },
    );
    proof.review.status = ReviewStatus::Remediating;
    proof.review.findings.push(Finding {
        id: "finding".into(),
        severity: super::super::super::workbench_proof::Severity::Warning,
        category: super::super::super::workbench_proof::Category::Change,
        claim: "Claim".into(),
        evidence_quote: "Quote".into(),
        guidance: "Check".into(),
        status: FindingStatus::Addressed,
        reply: None,
        receipt_digest: None,
    });
    let mut accepted = proof.clone();
    accepted.review.findings[0].status = FindingStatus::Accepted;
    merge_dispositions(&mut proof, Some(&accepted));
    assert_eq!(proof.review.findings[0].status, FindingStatus::Accepted);
    assert_eq!(proof.review.status, ReviewStatus::Remediating);
    let mut foreign = accepted.clone();
    foreign.session_id = "foreign".into();
    proof.review.findings[0].status = FindingStatus::Pending;
    merge_dispositions(&mut proof, Some(&foreign));
    assert_eq!(proof.review.findings[0].status, FindingStatus::Pending);
}

#[tokio::test(flavor = "multi_thread")]
async fn workbench_restart_exposes_interrupted_review_as_retryable_without_rewriting_source() {
    let mut f = fixture(false, false);
    let mut proof = send(&f, true, true).await;
    proof.review.status = ReviewStatus::Running;
    let mut message = f
        .turns
        .store
        .messages(&f.session)
        .into_iter()
        .find(|row| row.id == proof.message_id)
        .unwrap();
    message.data["parts"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|part| part["type"] == "data-workbench-proof")
        .unwrap()["data"] = json!(proof);
    f.turns.store.save_message(message).unwrap();
    let store = Arc::new(PlaneStore::open(Some(f.root.join("state"))));
    let agents = Arc::new(AgentCatalog::new(Arc::clone(&store)));
    let sessions = Arc::new(SessionCatalog::new(Arc::clone(&store)));
    f.turns = Arc::new(TurnService::new(
        store,
        agents,
        sessions,
        Arc::clone(&f.turns.config),
        Arc::new(EventHub::new()),
    ));
    f.turns.settle_orphan_pending(&f.session);
    let interrupted = f
        .turns
        .stored_proofs(&f.session)
        .into_iter()
        .next()
        .unwrap();
    assert_eq!(interrupted.review.status, ReviewStatus::Failed);
    assert_eq!(interrupted.producer_status, ProducerStatus::Completed);
    assert_eq!(interrupted.source_text, "draft");
    f.turns
        .workbench_proof_action(&f.session, &proof.message_id, json!({"action":"retry"}))
        .unwrap();
    settled(&f).await;
    let retried = f
        .turns
        .stored_proofs(&f.session)
        .into_iter()
        .next()
        .unwrap();
    assert_eq!(retried.review.status, ReviewStatus::Completed);
    assert_eq!(retried.artifacts[0].id, proof.artifacts[0].id);
    assert_eq!(retried.source_text, "draft");
}

#[tokio::test(flavor = "multi_thread")]
async fn workbench_simultaneous_submits_reserve_one_owner() {
    let f = fixture(false, false);
    f.hold.store(true, Ordering::SeqCst);
    let barrier = Arc::new(std::sync::Barrier::new(2));
    let turns = Arc::clone(&f.turns);
    let session = f.session.clone();
    let wait = Arc::clone(&barrier);
    let handle = tokio::runtime::Handle::current();
    let worker = std::thread::spawn(move || {
        let _runtime = handle.enter();
        wait.wait();
        turns.open(&session, &super::super::tests::message("first"))
    });
    barrier.wait();
    let second = f
        .turns
        .open(&f.session, &super::super::tests::message("second"));
    let first = worker.join().unwrap();
    assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);
    settled(&f).await;
    assert_eq!(f.turns.store.messages(&f.session).len(), 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn workbench_repair_permission_is_visible_on_its_own_message_and_keeps_the_original_owner() {
    let f = fixture(true, false);
    std::fs::write(f.root.join("ask-repair"), "").unwrap();
    let session = f.turns.sessions.get(&f.session).unwrap();
    let agent = session.agent_id.as_ref().unwrap();
    f.turns
        .agents
        .update(
            agent,
            &json!({"configuration":{"permission_mode":"default"}}),
        )
        .unwrap();
    let mut body = super::super::tests::message("Write report");
    body["turnContext"] = json!({"workbench":{"reviewMode":true,"artifactTrace":true}});
    f.turns.open(&f.session, &body).unwrap();
    let approval = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let id = f
                .turns
                .lock_live()
                .get(&f.session)
                .and_then(|live| live.approvals.keys().next().cloned());
            if let Some(id) = id {
                break id;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let rows = f.turns.store.messages(&f.session);
    assert_eq!(rows.len(), 3);
    let parent = Proof::from_parts(rows[1].parts()).unwrap();
    assert_eq!(parent.review.status, ReviewStatus::Remediating);
    assert!(rows[2].parts().iter().any(|part|part["state"]=="approval-requested" && part["approval"]["id"]==approval));
    assert_eq!(f.turns.lock_live()[&f.session].owner, parent.message_id);
    assert_eq!(
        std::fs::read(PathBuf::from(session.workspace.path).join("report.txt")).unwrap(),
        b"draft"
    );
    assert!(f.turns.respond_approval(&approval, true));
    settled(&f).await;
    let proofs = f.turns.stored_proofs(&f.session);
    assert_eq!(proofs[0].source_text, "draft");
    assert_eq!(proofs[1].source_text, "corrected");
    assert_eq!(
        proofs[0].review.findings[0].status,
        FindingStatus::Addressed
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn workbench_unverified_repairs_remain_pending_until_explicit_ignore() {
    let f = fixture(true, false);
    f.resolved.store(false, Ordering::SeqCst);
    let proof = send(&f, true, true).await;
    assert_eq!(proof.review.status, ReviewStatus::OpenQueue);
    assert_eq!(proof.review.findings[0].status, FindingStatus::Pending);
    assert_eq!(f.calls.load(Ordering::SeqCst), 4);
    assert_eq!(f.turns.stored_proofs(&f.session).len(), 4);
    let id = &proof.review.findings[0].id;
    assert!(
        f.turns
            .workbench_proof_action(
                &f.session,
                &proof.message_id,
                json!({"action":"accept","findingId":id})
            )
            .is_err()
    );
    for _ in 0..2 {
        let ignored = f
            .turns
            .workbench_proof_action(
                &f.session,
                &proof.message_id,
                json!({"action":"ignore","findingId":id}),
            )
            .unwrap();
        assert_eq!(ignored["review"]["findings"][0]["status"], "waived");
        assert_eq!(ignored["review"]["status"], "completed");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn workbench_truncated_producer_preserves_incomplete_bytes_and_never_enters_review() {
    let f = fixture(false, false);
    std::fs::write(f.root.join("truncate"), "").unwrap();
    let proof = send(&f, true, true).await;
    assert_eq!(proof.producer_status, ProducerStatus::Failed);
    assert_eq!(proof.review.status, ReviewStatus::Failed);
    assert_eq!(f.calls.load(Ordering::SeqCst), 0);
    assert_eq!(proof.artifacts.len(), 1);
    assert_eq!(proof.artifacts[0].status.as_str(), "incomplete");
    assert_eq!(
        f.turns
            .proof_artifacts
            .read(&f.session, &proof.artifacts[0])
            .unwrap(),
        b"draft"
    );
    assert!(
        f.turns
            .workbench_proof_action(&f.session, &proof.message_id, json!({"action":"retry"}))
            .is_err()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn workbench_acceptance_does_not_erase_failed_or_cancelled_review() {
    let f = fixture(true, false);
    let original = send(&f, true, true).await;
    for status in [ReviewStatus::Failed, ReviewStatus::Cancelled] {
        let mut proof = original.clone();
        proof.review.status = status.clone();
        let mut message = f
            .turns
            .store
            .messages(&f.session)
            .into_iter()
            .find(|message| message.id == proof.message_id)
            .unwrap();
        message.data["parts"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|part| part["type"] == "data-workbench-proof")
            .unwrap()["data"] = json!(proof);
        f.turns.store.save_message(message).unwrap();
        let result = f
            .turns
            .workbench_proof_action(
                &f.session,
                &proof.message_id,
                json!({"action":"accept","findingId":proof.review.findings[0].id}),
            )
            .unwrap();
        assert_eq!(result["review"]["findings"][0]["status"], "accepted");
        assert_eq!(result["review"]["status"], status.as_str());
    }
}
