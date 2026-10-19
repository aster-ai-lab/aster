#![cfg(test)]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::extract::{Json, Path, Query, State};
use axum::http::HeaderMap;
use axum::routing::{get, post};
use axum::{Router, response::IntoResponse};
use serde_json::Value;

use super::*;

const ASSET: &str = "11111111-1111-4111-8111-111111111111";
const RELEASE: &str = "22222222-2222-4222-8222-222222222222";
const CONVERSATION: &str = "44444444-4444-4444-8444-444444444444";
const PACKAGE: &str = "use/a3s/weather";
const SLUG: &str = "weather";
const LOG_TEXT: &str = "22°C";
const PROMPT: &str = "上海天气";
const EXECUTION: &str = "33333333-3333-4333-8333-333333333333";
const MEMBER_TOKEN: &str = "a3s_ms_member-token";

struct SeenCreate {
    key: String,
    body: Value,
    conversation_id: String,
    authorization: String,
}

#[derive(Clone)]
struct SeenStop {
    key: String,
    authorization: String,
}

struct StandInner {
    creates: Mutex<Vec<SeenCreate>>,
    stops: Mutex<Vec<SeenStop>>,
    log_reads: AtomicUsize,
    log_cursors: Mutex<Vec<Option<String>>>,
    log_authorizations: Mutex<Vec<String>>,
    hang_logs: bool,
    text: String,
}

#[derive(serde::Deserialize)]
struct LogsQuery {
    cursor: Option<String>,
}

struct CloudHarness {
    turns: Arc<TurnService>,
    store: Arc<super::super::store::PlaneStore>,
    hub: Arc<EventHub>,
    session_id: String,
    dir: std::path::PathBuf,
}

fn digest() -> String {
    format!("sha256:{}", "ab".repeat(32))
}

fn binding(asset: &str, with_digest: bool) -> Value {
    let mut value = json!({
        "packageId": PACKAGE,
        "agentAssetId": asset,
        "agentAssetReleaseId": RELEASE,
    });
    if with_digest {
        value["digest"] = json!(digest());
    }
    value
}

fn cloud_body(asset: &str, with_digest: bool, conversation: Option<&str>) -> Value {
    let mut turn = serde_json::Map::new();
    turn.insert("directives".into(), json!([]));
    turn.insert("research".into(), json!(false));
    turn.insert("agentId".into(), json!(SLUG));
    turn.insert(
        "agentRuntime".into(),
        json!({
            "runtime": "cloud",
            "harness": "package",
            "entry": "agent/run.mjs",
            "resources": { "cpuMillis": 500, "memoryBytes": 268435456 }
        }),
    );
    turn.insert("cloudBinding".into(), binding(asset, with_digest));
    if let Some(conversation) = conversation {
        turn.insert("conversationId".into(), json!(conversation));
    }
    json!({
        "userMessageParts": [{ "type": "text", "text": PROMPT }],
        "aixOutput": "forged-renderer-text",
        "turnContext": Value::Object(turn),
    })
}

fn node_children() -> Vec<String> {
    let me = std::process::id().to_string();
    let Ok(output) = std::process::Command::new("ps")
        .args(["-axww", "-o", "ppid=,command="])
        .output()
    else {
        return Vec::new();
    };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let (ppid, command) = line.split_once(char::is_whitespace)?;
            if ppid != me {
                return None;
            }
            let command = command.trim();
            let is_node =
                command == "node" || command.starts_with("node ") || command.contains("/node ");
            is_node.then(|| command.to_owned())
        })
        .collect()
}

fn cloud_harness(name: &str, base: Option<&str>) -> CloudHarness {
    cloud_harness_with(name, base, Some(MEMBER_TOKEN), "")
}

fn cloud_harness_with(
    name: &str,
    base: Option<&str>,
    bearer: Option<&str>,
    slug: &str,
) -> CloudHarness {
    let dir = std::env::temp_dir().join(format!("a3s-cloud-aix-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let binary = dir.join("fake-acp");
    std::fs::write(&binary, super::super::fake_acp::SCRIPT).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let store = Arc::new(super::super::store::PlaneStore::open(Some(
        dir.join("state"),
    )));
    let agents = Arc::new(super::super::catalog::AgentCatalog::new(Arc::clone(&store)));
    let sessions = Arc::new(super::super::catalog::SessionCatalog::new(Arc::clone(
        &store,
    )));
    let hub = Arc::new(EventHub::new());
    let config = Arc::new(super::super::PlaneConfig {
        root: Some(dir.join("state")),
        credentials: Arc::new(|_| None),
        binary: Some(binary),
        workspace_documents: None,
        schedule_mcp: None,
        mail_mcp: None,
        slug: slug.to_owned(),
        personal_wiki: None,
        cloud_agent_runtime: base.map(|base| super::super::cloud_runtime::CloudAgentRuntime {
            base: base.to_owned(),
            bearer: bearer.map(str::to_owned),
        }),
    });
    let agent = agents
        .create(&json!({
            "type": "a3s-code",
            "name": "Helper",
            "model": "openai::gpt-test",
            "configuration": { "permission_mode": "default" },
        }))
        .unwrap();
    let session = sessions
        .create(&json!({ "agentId": agent.id, "name": "", "workspace": { "type": "system" } }))
        .unwrap();
    let turns = Arc::new(TurnService::new(
        Arc::clone(&store),
        agents,
        sessions,
        config,
        Arc::clone(&hub),
    ));
    CloudHarness {
        turns,
        store,
        hub,
        session_id: session.id,
        dir,
    }
}

async fn stand_in(hang_logs: bool) -> (String, Arc<StandInner>) {
    let inner = Arc::new(StandInner {
        creates: Mutex::new(Vec::new()),
        stops: Mutex::new(Vec::new()),
        log_reads: AtomicUsize::new(0),
        log_cursors: Mutex::new(Vec::new()),
        log_authorizations: Mutex::new(Vec::new()),
        hang_logs,
        text: LOG_TEXT.to_owned(),
    });
    let app = Router::new()
        .route(
            "/agent-conversations/{conversation_id}/agent-runtimes",
            post(create_route),
        )
        .route("/agent-runtimes/{execution_id}/logs", get(logs_route))
        .route("/agent-runtimes/{execution_id}/stop", post(stop_route))
        .with_state(Arc::clone(&inner));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    (format!("http://{address}"), inner)
}

async fn create_route(
    State(state): State<Arc<StandInner>>,
    Path(conversation_id): Path<String>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Json<Value> {
    let key = headers
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_owned();
    let authorization = header_value(&headers, "authorization");
    let mut creates = state
        .creates
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let replayed = !key.is_empty() && creates.iter().any(|row| row.key == key);
    creates.push(SeenCreate {
        key,
        body,
        conversation_id: conversation_id.clone(),
        authorization,
    });
    Json(json!({
        "projection": {
            "agentExecutionId": EXECUTION,
            "conversationId": conversation_id,
        },
        "replayed": replayed,
    }))
}

async fn logs_route(
    State(state): State<Arc<StandInner>>,
    Query(query): Query<LogsQuery>,
    headers: HeaderMap,
) -> axum::response::Response {
    let reads = state.log_reads.fetch_add(1, Ordering::SeqCst) + 1;
    state
        .log_cursors
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .push(query.cursor.clone());
    state
        .log_authorizations
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .push(header_value(&headers, "authorization"));
    if state.hang_logs && reads > 1 {
        std::future::pending::<axum::response::Response>().await
    } else if reads == 1 {
        Json(json!({
            "executionId": EXECUTION,
            "records": [{
                "kind": "model_output",
                "sequence": 1,
                "content": { "text": state.text }
            }],
            "nextCursor": null,
        }))
        .into_response()
    } else {
        Json(json!({
            "executionId": EXECUTION,
            "records": [
                {
                    "kind": "model_output",
                    "sequence": 1,
                    "content": { "text": state.text }
                },
                {
                    "kind": "execution_completed",
                    "sequence": 2,
                    "content": { "reason": "stop" }
                }
            ],
            "nextCursor": null,
        }))
        .into_response()
    }
}

async fn stop_route(State(state): State<Arc<StandInner>>, headers: HeaderMap) -> Json<Value> {
    state
        .stops
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .push(SeenStop {
            key: header_value(&headers, "idempotency-key"),
            authorization: header_value(&headers, "authorization"),
        });
    Json(json!({}))
}

fn header_value(headers: &HeaderMap, name: &str) -> String {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_owned()
}

fn creates(state: &StandInner) -> Vec<SeenCreate> {
    state
        .creates
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .drain(..)
        .collect()
}

fn stops(state: &StandInner) -> Vec<SeenStop> {
    state
        .stops
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .clone()
}

async fn wait_for(mut ready: impl FnMut() -> bool) {
    for _ in 0..100 {
        if ready() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("timed out");
}

fn error_text(error: PlaneError) -> String {
    match error {
        PlaneError::BadRequest(message)
        | PlaneError::NotFound(message)
        | PlaneError::Conflict(message)
        | PlaneError::Internal(message) => message,
    }
}

fn assistants(store: &super::super::store::PlaneStore, session_id: &str) -> Vec<Message> {
    store
        .messages(session_id)
        .into_iter()
        .filter(|message| message.role == "assistant")
        .collect()
}

fn acp_started(dir: &std::path::Path) -> bool {
    dir.join("turn-trace.json").exists() || dir.join("session-new.json").exists()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_cloud_declaration_does_not_run_as_a_local_package() {
    let before = node_children();
    let mut body = cloud_body(ASSET, true, Some(CONVERSATION));
    body.as_object_mut().expect("object").remove("aixOutput");
    let prepared = super::super::controllers::prepare_edge_turn(body, None)
        .await
        .expect("cloud prepare");
    assert!(prepared.get("aixOutput").is_none());
    let missing = cloud_body(ASSET, false, Some(CONVERSATION));
    let dir = std::env::temp_dir().join(format!("a3s-cloud-caps-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let refused =
        super::super::controllers::declared_agent_output(&missing, Some(&dir), None, None)
            .await
            .expect_err("missing binding");
    let message = refused.to_string();
    assert!(
        message.contains("not bound") || message.contains("install name"),
        "{message}"
    );
    assert!(!message.contains("package entry"), "{message}");
    assert_eq!(node_children(), before);
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_bound_cloud_turn_stores_one_projection_log_without_acp_or_stop() {
    let (base, stand) = stand_in(false).await;
    let harness = cloud_harness("success", Some(&base));
    let mut events = harness.hub.subscribe();
    let ack = harness
        .turns
        .open(
            &harness.session_id,
            &cloud_body(ASSET, true, Some(CONVERSATION)),
        )
        .expect("open");
    assert_eq!(ack["mode"], "started");
    let seen = super::tests::until_done(&mut events, |_| {}).await;
    assert_eq!(seen.last().unwrap()["payload"]["status"], "success");
    let created = creates(&stand);
    assert_eq!(created.len(), 1);
    assert_eq!(created[0].conversation_id, CONVERSATION);
    assert!(!created[0].key.is_empty());
    assert_eq!(created[0].authorization, format!("Bearer {MEMBER_TOKEN}"));
    assert_eq!(created[0].body["agentAssetId"], ASSET);
    assert_eq!(created[0].body["agentAssetReleaseId"], RELEASE);
    assert_eq!(created[0].body["input"]["packageId"], PACKAGE);
    assert_eq!(created[0].body["input"]["digest"], digest());
    assert_eq!(created[0].body["input"]["prompt"], PROMPT);
    assert_ne!(created[0].body["agentAssetId"], SLUG);
    assert_ne!(created[0].body["agentAssetId"], PACKAGE);
    assert!(stops(&stand).is_empty());
    assert_eq!(stand.log_reads.load(Ordering::SeqCst), 2);
    let cursors = stand
        .log_cursors
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .clone();
    assert_eq!(cursors, vec![None, None]);
    let log_auth = stand
        .log_authorizations
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .clone();
    assert_eq!(
        log_auth,
        vec![
            format!("Bearer {MEMBER_TOKEN}"),
            format!("Bearer {MEMBER_TOKEN}")
        ]
    );
    let rows = assistants(&harness.store, &harness.session_id);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, "success");
    assert!(rows[0].searchable_text.contains(LOG_TEXT));
    assert!(
        !rows[0]
            .searchable_text
            .contains(&format!("{LOG_TEXT}{LOG_TEXT}"))
    );
    assert!(!rows[0].searchable_text.contains("forged-renderer-text"));
    assert!(!harness.turns.is_live(&harness.session_id));
    assert!(!acp_started(&harness.dir));
    assert!(node_children().is_empty());
    let _ = std::fs::remove_dir_all(&harness.dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn an_aborted_cloud_turn_stops_once_and_does_not_succeed() {
    let (base, stand) = stand_in(true).await;
    let harness = cloud_harness("abort", Some(&base));
    harness
        .turns
        .open(
            &harness.session_id,
            &cloud_body(ASSET, true, Some(CONVERSATION)),
        )
        .expect("open");
    wait_for(|| stand.log_reads.load(Ordering::SeqCst) >= 2).await;
    let cursors = stand
        .log_cursors
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .clone();
    assert_eq!(cursors, vec![None, None]);
    harness.turns.abort(&harness.session_id);
    wait_for(|| !harness.turns.is_live(&harness.session_id)).await;
    let stopped = stops(&stand);
    assert_eq!(stopped.len(), 1);
    assert!(!stopped[0].key.is_empty());
    assert_eq!(stopped[0].authorization, format!("Bearer {MEMBER_TOKEN}"));
    let rows = assistants(&harness.store, &harness.session_id);
    assert!(rows.iter().all(|row| row.status != "success"));
    assert!(
        rows.iter()
            .all(|row| !row.searchable_text.contains(LOG_TEXT))
    );
    assert!(!acp_started(&harness.dir));
    assert!(node_children().is_empty());
    let _ = std::fs::remove_dir_all(&harness.dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_cloud_turn_without_binding_facts_does_not_create_or_spawn() {
    let (base, stand) = stand_in(false).await;
    let harness = cloud_harness("refuse", Some(&base));
    let before = node_children();
    let cases = [
        cloud_body(ASSET, false, Some(CONVERSATION)),
        cloud_body(SLUG, true, Some(CONVERSATION)),
        cloud_body(PACKAGE, true, Some(CONVERSATION)),
        cloud_body(ASSET, true, None),
    ];
    for body in cases {
        let error = harness
            .turns
            .open(&harness.session_id, &body)
            .expect_err("refused");
        let message = error_text(error);
        assert!(
            message.contains("not bound") || message.contains("install name"),
            "{message}"
        );
        assert!(!message.contains("package entry"), "{message}");
        assert!(!harness.turns.is_live(&harness.session_id));
    }
    assert!(creates(&stand).is_empty());
    assert!(stops(&stand).is_empty());
    assert_eq!(stand.log_reads.load(Ordering::SeqCst), 0);
    assert!(assistants(&harness.store, &harness.session_id).is_empty());
    assert!(!acp_started(&harness.dir));
    assert_eq!(node_children(), before);
    let Err(slug_error) =
        super::super::cloud_runtime::admit(&cloud_body(SLUG, true, Some(CONVERSATION)))
    else {
        panic!("slug asset id was admitted");
    };
    let slug_message = error_text(slug_error);
    assert!(slug_message.contains("install name"), "{slug_message}");
    let Err(package_error) =
        super::super::cloud_runtime::admit(&cloud_body(PACKAGE, true, Some(CONVERSATION)))
    else {
        panic!("package id was admitted as an asset id");
    };
    let package_message = error_text(package_error);
    assert!(
        package_message.contains("install name"),
        "{package_message}"
    );
    let _ = std::fs::remove_dir_all(&harness.dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_replayed_cloud_projection_keeps_one_assistant_result() {
    let (base, stand) = stand_in(false).await;
    let harness = cloud_harness("replay", Some(&base));
    let body = cloud_body(ASSET, true, Some(CONVERSATION));
    let mut first = harness.hub.subscribe();
    harness
        .turns
        .open(&harness.session_id, &body)
        .expect("first");
    let first_done = super::tests::until_done(&mut first, |_| {}).await;
    assert_eq!(first_done.last().unwrap()["payload"]["status"], "success");
    let mut second = harness.hub.subscribe();
    harness
        .turns
        .open(&harness.session_id, &body)
        .expect("second");
    let second_done = super::tests::until_done(&mut second, |_| {}).await;
    assert_eq!(second_done.last().unwrap()["channel"], "ai.stream.error");
    let created = stand
        .creates
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    assert_eq!(created.len(), 2);
    assert_eq!(created[0].key, created[1].key);
    assert!(!created[0].key.is_empty());
    assert_eq!(created[0].authorization, format!("Bearer {MEMBER_TOKEN}"));
    assert_eq!(created[1].authorization, created[0].authorization);
    drop(created);
    assert_eq!(stand.log_reads.load(Ordering::SeqCst), 2);
    assert!(stops(&stand).is_empty());
    let rows = assistants(&harness.store, &harness.session_id);
    let successes: Vec<_> = rows.iter().filter(|row| row.status == "success").collect();
    assert_eq!(successes.len(), 1);
    assert!(successes[0].searchable_text.contains(LOG_TEXT));
    assert!(!acp_started(&harness.dir));
    assert!(node_children().is_empty());
    let _ = std::fs::remove_dir_all(&harness.dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_cloud_turn_without_a_member_session_does_not_create() {
    let (base, stand) = stand_in(false).await;
    let harness = cloud_harness_with("unsigned", Some(&base), None, "acme");
    let error = harness
        .turns
        .open(
            &harness.session_id,
            &cloud_body(ASSET, true, Some(CONVERSATION)),
        )
        .expect_err("missing member");
    let message = error_text(error);
    assert!(message.contains("member session"), "{message}");
    assert!(creates(&stand).is_empty());
    assert_eq!(stand.log_reads.load(Ordering::SeqCst), 0);
    assert!(assistants(&harness.store, &harness.session_id).is_empty());
    let _ = std::fs::remove_dir_all(&harness.dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_cloud_turn_with_a_user_plane_token_does_not_create() {
    let (base, stand) = stand_in(false).await;
    let harness = cloud_harness_with("user-plane", Some(&base), Some("a3s_us_member-token"), "");
    let error = harness
        .turns
        .open(
            &harness.session_id,
            &cloud_body(ASSET, true, Some(CONVERSATION)),
        )
        .expect_err("user plane");
    let message = error_text(error);
    assert!(message.contains("member token"), "{message}");
    assert!(creates(&stand).is_empty());
    assert_eq!(stand.log_reads.load(Ordering::SeqCst), 0);
    assert!(assistants(&harness.store, &harness.session_id).is_empty());
    let _ = std::fs::remove_dir_all(&harness.dir);
}
