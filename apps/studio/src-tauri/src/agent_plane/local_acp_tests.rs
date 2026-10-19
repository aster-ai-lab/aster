//! Ignored checks that a Studio task process is this checkout's a3s-code-acp.
//!
//! Rows R1–R13 stop before `session/prompt`.
//! `studio_task_local_acp_live` sends M1–M14 only when `A3S_STUDIO_TASK_LIVE=1`.
//! Without that variable those rows are not counted.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tokio::sync::broadcast;

use super::{
    AgentCatalog, EventHub, PlaneConfig, PlaneStore, SessionCatalog, TEST_BINARY_LOOKUP,
    TEST_INITIALIZE, TEST_SESSION_NEW, TurnService, apply_mode, cancel,
};

fn workspace_acp() -> PathBuf {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../target/debug/a3s-code-acp");
    std::fs::canonicalize(&path).unwrap_or_else(|_| {
        panic!(
            "build target/debug/a3s-code-acp before this test ({})",
            path.display()
        )
    })
}

fn require_env_matches(expected: &Path) {
    let Some(raw) = std::env::var_os("A3S_ACP_AGENT_BIN") else {
        return;
    };
    let given = PathBuf::from(&raw);
    let canonical = std::fs::canonicalize(&given).unwrap_or(given);
    assert_eq!(
        canonical,
        expected,
        "A3S_ACP_AGENT_BIN points at {}",
        canonical.display()
    );
}

fn mapped_executable(pid: u32) -> Option<PathBuf> {
    let output = std::process::Command::new("lsof")
        .args(["-a", "-p", &pid.to_string(), "-d", "txt", "-Fn"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    text.lines()
        .filter_map(|line| line.strip_prefix('n'))
        .find_map(|path| {
            path.ends_with("a3s-code-acp")
                .then(|| std::fs::canonicalize(path).unwrap_or_else(|_| PathBuf::from(path)))
        })
}

fn wait_mapped(pid: u32) -> PathBuf {
    for _ in 0..30 {
        if let Some(path) = mapped_executable(pid) {
            return path;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!("pid {pid} did not map a3s-code-acp");
}

fn process_alive(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn wait_until_dead(pid: u32) {
    for _ in 0..50 {
        if !process_alive(pid) {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!("pid {pid} was still running");
}

fn acp_child_pids() -> Vec<u32> {
    let output = std::process::Command::new("pgrep")
        .args(["-P", &std::process::id().to_string(), "-l"])
        .output();
    let Ok(output) = output else {
        return Vec::new();
    };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let pid = parts.next()?.parse::<u32>().ok()?;
            let name = parts.next().unwrap_or("");
            name.contains("a3s-code-acp").then_some(pid)
        })
        .collect()
}

fn assert_no_new_acp(before: &[u32]) {
    let extra: Vec<u32> = acp_child_pids()
        .into_iter()
        .filter(|pid| !before.contains(pid))
        .collect();
    assert!(
        extra.is_empty(),
        "a3s-code-acp started despite the rejection: {extra:?}"
    );
}

/// Puts binary lookup back when a missing-binary row returns or panics.
struct BinaryLookupGuard;

impl Drop for BinaryLookupGuard {
    fn drop(&mut self) {
        *TEST_BINARY_LOOKUP
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = true;
    }
}

enum Creds {
    Fixture,
    Missing,
    Live,
}

struct LocalHarness {
    turns: Arc<TurnService>,
    store: Arc<PlaneStore>,
    hub: Arc<EventHub>,
    sessions: Arc<SessionCatalog>,
    agents: Arc<AgentCatalog>,
    session_id: String,
    agent_id: String,
    dir: PathBuf,
}

impl Drop for LocalHarness {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn local_harness(name: &str, binary: Option<&Path>, creds: Creds) -> LocalHarness {
    let dir = std::env::temp_dir().join(format!("a3s-studio-task-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let store = Arc::new(PlaneStore::open(Some(dir.join("state"))));
    let agents = Arc::new(AgentCatalog::new(Arc::clone(&store)));
    let sessions = Arc::new(SessionCatalog::new(Arc::clone(&store)));
    let hub = Arc::new(EventHub::new());
    let credentials: crate::agent_plane::CredentialLookup = match creds {
        Creds::Live => Arc::new(|provider: &str| {
            let token = signed_in_token();
            crate::providers::signed_in_gateway_credentials(provider, token.as_deref())
        }),
        Creds::Fixture => Arc::new(|_provider: &str| {
            Some(crate::providers::ProviderCredentials {
                base_url: "https://example.test/v1".into(),
                api_key: "sk-test".into(),
            })
        }),
        Creds::Missing => Arc::new(|_provider: &str| None),
    };
    let config = Arc::new(PlaneConfig {
        root: Some(dir.join("state")),
        credentials,
        binary: binary.map(Path::to_path_buf),
        workspace_documents: None,
        schedule_mcp: None,
        mail_mcp: None,
        slug: String::new(),
        personal_wiki: None,
        cloud_agent_runtime: None,
    });
    let model = if matches!(creds, Creds::Live) {
        let token = signed_in_token().expect("no signed-in session");
        crate::providers::first_chat_model_for_key(&token).expect("no gateway model")
    } else {
        "openai::gpt-test".to_owned()
    };
    let agent = agents
        .create(&json!({
            "type": "a3s-code",
            "name": "Helper",
            "model": model,
            "configuration": { "permission_mode": "default" },
        }))
        .unwrap();
    let session = sessions
        .create(
            &json!({ "agentId": agent.id, "name": "local-acp", "workspace": { "type": "system" } }),
        )
        .unwrap();
    let turns = Arc::new(TurnService::new(
        Arc::clone(&store),
        Arc::clone(&agents),
        Arc::clone(&sessions),
        config,
        Arc::clone(&hub),
    ));
    LocalHarness {
        turns,
        store,
        hub,
        sessions,
        agents,
        session_id: session.id,
        agent_id: agent.id,
        dir,
    }
}

fn workspace_files(dir: &Path) -> Vec<String> {
    let mut names = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(current) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            // The agent writes its session log under `.a3s` when the turn
            // starts, before any command is allowed. Command rows judge the
            // other workspace files.
            if path.file_name().and_then(|name| name.to_str()) == Some(".a3s") {
                continue;
            }
            if path.is_dir() {
                pending.push(path);
            } else {
                names.push(path.to_string_lossy().into_owned());
            }
        }
    }
    names.sort();
    names
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "starts target/debug/a3s-code-acp"]
async fn studio_task_local_acp_handshake() {
    let expected = workspace_acp();
    require_env_matches(&expected);
    *TEST_INITIALIZE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = None;

    let harness = local_harness("handshake", Some(&expected), Creds::Fixture);
    let session = harness.sessions.get(&harness.session_id).unwrap();
    let agent = harness.agents.get(&harness.agent_id).unwrap();
    let started = Instant::now();
    let (runtime, fresh) = harness
        .turns
        .runtime(&session, &agent, "openai::gpt-test", None, &[])
        .await
        .unwrap_or_else(|error| panic!("S1 handshake failed: {error}"));
    let handshake_ms = started.elapsed().as_millis();
    assert!(fresh, "S1 starts a process");
    assert!(!runtime.acp_session.is_empty(), "S1 session id");
    let initialized = TEST_INITIALIZE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
        .expect("S1 initialize result");
    let version = &initialized["protocolVersion"];
    assert!(
        version.as_u64() == Some(1) || version.as_str() == Some("1"),
        "S1 protocolVersion is {version}"
    );
    assert!(
        harness.store.messages(&harness.session_id).is_empty(),
        "S1 does not send session/prompt"
    );

    let pid = runtime.client.process_id().await.expect("S0 process id");
    assert_eq!(wait_mapped(pid), expected, "S0 canonical executable");

    let mut applied_modes = Vec::new();
    for (studio_mode, acp_mode) in [
        ("default", "default"),
        ("plan", "plan"),
        ("auto", "auto"),
        ("bypassPermissions", "yolo"),
    ] {
        apply_mode(&runtime, studio_mode)
            .await
            .unwrap_or_else(|error| panic!("S2 {studio_mode}: {error}"));
        assert_eq!(
            runtime.mode.lock().await.as_str(),
            acp_mode,
            "S2 {acp_mode}"
        );
        applied_modes.push(acp_mode);
    }

    cancel(&runtime, &runtime.acp_session).await;
    assert!(process_alive(pid), "S3 process stays up after cancel");

    let acp_session = runtime.acp_session.clone();
    runtime.client.kill().await;
    wait_until_dead(pid);
    drop(runtime);
    let dir = harness.dir.clone();
    drop(harness);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(!dir.exists(), "S4 temp dir removed");
    let protocol_version = version
        .as_u64()
        .map(|number| number.to_string())
        .or_else(|| version.as_str().map(str::to_owned))
        .expect("S1 protocolVersion text");
    let modes = applied_modes.join(",");
    eprintln!(
        "STUDIO_TASK_ACP_PASS path={} session={acp_session} handshake_ms={handshake_ms} prompt=false protocolVersion={protocol_version} modes={modes}",
        expected.display()
    );
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "starts target/debug/a3s-code-acp"]
async fn studio_task_local_acp_acl_and_cwd() {
    let expected = workspace_acp();
    require_env_matches(&expected);
    *TEST_SESSION_NEW
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
    let harness = local_harness("acl", Some(&expected), Creds::Fixture);
    let session = harness.sessions.get(&harness.session_id).unwrap();
    let agent = harness.agents.get(&harness.agent_id).unwrap();
    let (runtime, fresh) = harness
        .turns
        .runtime(&session, &agent, "openai::gpt-test", None, &[])
        .await
        .unwrap_or_else(|error| panic!("R5 launch failed: {error}"));
    assert!(fresh, "R5 starts a process");
    let acl = harness
        .dir
        .join("state")
        .join("runtime")
        .join(format!("{}.acl", harness.session_id));
    let text = std::fs::read_to_string(&acl).unwrap_or_else(|error| panic!("R5 acl: {error}"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&acl)
            .unwrap_or_else(|error| panic!("R5 acl metadata: {error}"))
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600, "R5 acl mode");
    }
    assert!(
        text.contains("openai/gpt-test"),
        "R5 acl names the fixture model"
    );
    // `MemberSessionStore::open` is empty under cfg(test). The on-disk ACL is
    // checked against the Application Support session from `open_app`.
    let token = signed_in_token().expect("R5 member session");
    assert!(!token.is_empty(), "R5 member session empty");
    assert!(
        !text.contains(token.as_str()),
        "R5 acl contains the member token"
    );
    let params = TEST_SESSION_NEW
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
        .expect("R6 session/new params");
    assert_eq!(
        params["cwd"].as_str(),
        Some(session.workspace.path.as_str()),
        "R6 cwd"
    );
    let pid = runtime.client.process_id().await.expect("R5 pid");
    runtime.client.kill().await;
    wait_until_dead(pid);
    drop(runtime);
    eprintln!("STUDIO_TASK_ACP_R5 mode=0600 model=openai/gpt-test token=absent");
    eprintln!("STUDIO_TASK_ACP_R6 cwd=session-workspace");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "rejects a missing a3s-code-acp without spawning"]
async fn studio_task_local_acp_missing_binary() {
    let before = acp_child_pids();
    *TEST_BINARY_LOOKUP
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = false;
    let _restore = BinaryLookupGuard;
    let harness = local_harness("missing-binary", None, Creds::Fixture);
    let session = harness.sessions.get(&harness.session_id).unwrap();
    let agent = harness.agents.get(&harness.agent_id).unwrap();
    let Err(error) = harness
        .turns
        .runtime(&session, &agent, "openai::gpt-test", None, &[])
        .await
    else {
        panic!("R7 must refuse");
    };
    assert_eq!(
        error,
        "a3s-code ACP agent was not found. Build target/debug/a3s-code-acp, bundle agent/a3s-code-acp with the app, install ~/.a3s/bin/a3s-code-acp, or set A3S_ACP_AGENT_BIN."
    );
    assert!(harness.turns.runtimes.lock().await.is_empty(), "R7 runtime");
    assert_no_new_acp(&before);
    eprintln!("STUDIO_TASK_ACP_R7 child=false");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "rejects a missing workspace without spawning"]
async fn studio_task_local_acp_missing_workspace() {
    let before = acp_child_pids();
    let expected = workspace_acp();
    require_env_matches(&expected);
    let harness = local_harness("missing-workspace", Some(&expected), Creds::Fixture);
    let session = harness.sessions.get(&harness.session_id).unwrap();
    let agent = harness.agents.get(&harness.agent_id).unwrap();
    std::fs::remove_dir_all(&session.workspace.path)
        .unwrap_or_else(|error| panic!("R8 remove workspace: {error}"));
    let Err(error) = harness
        .turns
        .runtime(&session, &agent, "openai::gpt-test", None, &[])
        .await
    else {
        panic!("R8 must refuse");
    };
    let folder = Path::new(&session.workspace.path);
    assert_eq!(
        error,
        format!("Workspace folder {} does not exist.", folder.display())
    );
    assert!(harness.turns.runtimes.lock().await.is_empty(), "R8 runtime");
    assert_no_new_acp(&before);
    eprintln!("STUDIO_TASK_ACP_R8 child=false");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "rejects an empty task message without spawning"]
async fn studio_task_local_acp_empty_message() {
    let before = acp_child_pids();
    let expected = workspace_acp();
    require_env_matches(&expected);
    let harness = local_harness("empty-message", Some(&expected), Creds::Fixture);
    let error = harness
        .turns
        .open(
            &harness.session_id,
            &json!({ "userMessageParts": [{ "type": "text", "text": "  " }] }),
        )
        .expect_err("R9 must refuse");
    assert_eq!(error.to_string(), "message is empty");
    assert!(
        harness.store.messages(&harness.session_id).is_empty(),
        "R9 stores nothing"
    );
    assert!(harness.turns.runtimes.lock().await.is_empty(), "R9 runtime");
    assert_no_new_acp(&before);
    eprintln!("STUDIO_TASK_ACP_R9 child=false");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "rejects a model id that is not provider::id without spawning"]
async fn studio_task_local_acp_bad_model() {
    let before = acp_child_pids();
    let expected = workspace_acp();
    require_env_matches(&expected);
    let harness = local_harness("bad-model", Some(&expected), Creds::Fixture);
    let error = harness
        .turns
        .open(
            &harness.session_id,
            &json!({
                "model": "gpt-test",
                "userMessageParts": [{ "type": "text", "text": "ping" }],
            }),
        )
        .expect_err("R10 must refuse");
    assert_eq!(error.to_string(), "model must be providerId::modelId");
    assert!(
        harness.store.messages(&harness.session_id).is_empty(),
        "R10 stores nothing"
    );
    assert!(
        harness.turns.runtimes.lock().await.is_empty(),
        "R10 runtime"
    );
    assert_no_new_acp(&before);
    eprintln!("STUDIO_TASK_ACP_R10 child=false");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "fails closed when the credential callback returns nothing"]
async fn studio_task_local_acp_missing_credentials() {
    let before = acp_child_pids();
    let expected = workspace_acp();
    require_env_matches(&expected);
    let harness = local_harness("missing-credentials", Some(&expected), Creds::Missing);
    let session = harness.sessions.get(&harness.session_id).unwrap();
    let agent = harness.agents.get(&harness.agent_id).unwrap();
    let Err(error) = harness
        .turns
        .runtime(&session, &agent, "openai::gpt-test", None, &[])
        .await
    else {
        panic!("R11 must refuse");
    };
    assert_eq!(error, "登录后通过统一网关使用模型。");
    assert!(
        harness.turns.runtimes.lock().await.is_empty(),
        "R11 runtime"
    );
    assert_no_new_acp(&before);
    eprintln!("STUDIO_TASK_ACP_R11 child=false");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "starts target/debug/a3s-code-acp"]
async fn studio_task_local_acp_reuses_process() {
    let expected = workspace_acp();
    require_env_matches(&expected);
    let harness = local_harness("reuse", Some(&expected), Creds::Fixture);
    let session = harness.sessions.get(&harness.session_id).unwrap();
    let agent = harness.agents.get(&harness.agent_id).unwrap();
    let (first, fresh) = harness
        .turns
        .runtime(&session, &agent, "openai::gpt-test", None, &[])
        .await
        .unwrap_or_else(|error| panic!("R12 launch failed: {error}"));
    assert!(fresh, "R12 starts a process");
    let pid = first.client.process_id().await.expect("R12 pid");
    let (second, again) = harness
        .turns
        .runtime(&session, &agent, "openai::gpt-test", None, &[])
        .await
        .unwrap_or_else(|error| panic!("R12 reuse failed: {error}"));
    assert!(!again, "R12 keeps the process");
    assert_eq!(second.client.process_id().await, Some(pid), "R12 pid");
    assert!(process_alive(pid), "R12 process stays up");
    assert_eq!(wait_mapped(pid), expected, "R12 executable");
    second.client.kill().await;
    wait_until_dead(pid);
    drop(second);
    drop(first);
    eprintln!("STUDIO_TASK_ACP_R12 reused=true");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "starts target/debug/a3s-code-acp"]
async fn studio_task_local_acp_replaces_process_when_the_model_changes() {
    let expected = workspace_acp();
    require_env_matches(&expected);
    let harness = local_harness("replace", Some(&expected), Creds::Fixture);
    let session = harness.sessions.get(&harness.session_id).unwrap();
    let agent = harness.agents.get(&harness.agent_id).unwrap();
    let (first, _) = harness
        .turns
        .runtime(&session, &agent, "openai::gpt-test", None, &[])
        .await
        .unwrap_or_else(|error| panic!("R13 first launch failed: {error}"));
    let old = first.client.process_id().await.expect("R13 old pid");
    let (second, fresh) = harness
        .turns
        .runtime(&session, &agent, "openai::gpt-other", None, &[])
        .await
        .unwrap_or_else(|error| panic!("R13 second launch failed: {error}"));
    assert!(fresh, "R13 starts a new process");
    let new = second.client.process_id().await.expect("R13 new pid");
    assert_ne!(new, old, "R13 pid");
    wait_until_dead(old);
    assert_eq!(wait_mapped(new), expected, "R13 executable");
    assert!(process_alive(new), "R13 new process stays up");
    second.client.kill().await;
    wait_until_dead(new);
    drop(second);
    drop(first);
    eprintln!("STUDIO_TASK_ACP_R13 replaced=true");
}

/// The signed-in session file. `MemberSessionStore::open` stays empty under
/// `cfg(test)` so library tests do not touch Application Support.
fn signed_in_token() -> Option<String> {
    let slug = std::env::var("A3S_TENANT_SLUG").unwrap_or_else(|_| "toyou".to_owned());
    let home = std::env::var_os("HOME")?;
    let root = PathBuf::from(home).join("Library/Application Support/dev.a3s.studio");
    crate::member_session::MemberSessionStore::open_app(&root, &slug).token()
}

fn m10_parts_path() -> PathBuf {
    std::env::temp_dir().join("a3s-studio-task-m10-parts.json")
}

/// Default effort is high. Later turns then stream reasoning until the wait
/// ends. The level is changed on this same process.
async fn set_reasoning(harness: &LocalHarness, effort: &str) {
    let runtime = harness
        .turns
        .runtimes
        .lock()
        .await
        .get(&harness.session_id)
        .cloned()
        .expect("runtime");
    let model_id = runtime.model.replacen("::", "/", 1);
    runtime
        .client
        .request(
            "session/set_model",
            json!({
                "sessionId": runtime.acp_session,
                "modelId": model_id,
                "_meta": { "reasoningEffort": effort }
            }),
        )
        .await
        .unwrap_or_else(|error| panic!("set reasoning effort: {error}"));
}

fn set_mode(harness: &LocalHarness, mode: &str) {
    harness
        .agents
        .update(
            &harness.agent_id,
            &json!({ "configuration": { "permission_mode": mode } }),
        )
        .unwrap_or_else(|error| panic!("set mode {mode}: {error}"));
}

/// One bash call. The host permission card is the confirmation, so the
/// prompt must not ask the model to stop and question the user.
fn bash_only(command: &str) -> String {
    format!(
        "只调用一次 bash。command 参数只能是 {command}。不要填写其他参数。不要提问，不要调用 ask_user。不要创建或修改任何文件。"
    )
}

fn open_text(harness: &LocalHarness, text: &str) {
    harness
        .turns
        .open(
            &harness.session_id,
            &json!({ "userMessageParts": [{ "type": "text", "text": text }] }),
        )
        .unwrap_or_else(|error| panic!("open: {error}"));
}

fn latest_assistant(harness: &LocalHarness) -> Value {
    harness
        .store
        .messages(&harness.session_id)
        .into_iter()
        .rev()
        .find(|message| message.role == "assistant")
        .expect("assistant row")
        .data
}

fn tool_parts(data: &Value) -> Vec<&Value> {
    data["parts"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|part| part["type"] == "dynamic-tool")
        .collect()
}

/// Short tool ids only. A question title can carry the run token, so it is `other`.
fn tool_label(name: &str) -> String {
    if !name.is_empty()
        && name.len() <= 32
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        name.to_owned()
    } else {
        "other".to_owned()
    }
}

/// Tool name, state, and input keys only. Values stay out of the log.
fn tool_shape(data: &Value) -> String {
    let shapes: Vec<String> = tool_parts(data)
        .into_iter()
        .map(|part| {
            let name = tool_label(part["toolName"].as_str().unwrap_or(""));
            let state = part["state"].as_str().unwrap_or("?");
            let keys = part["input"]
                .as_object()
                .map(|object| object.keys().cloned().collect::<Vec<_>>().join(","))
                .unwrap_or_default();
            format!("{name}:{state}:{keys}")
        })
        .collect();
    if shapes.is_empty() {
        "no-tool".to_owned()
    } else {
        shapes.join(" ")
    }
}

/// Class of each bash command. The command text can hold the row token, so the
/// log keeps only whether it is the existence check and how long it is.
fn bash_command_class(data: &Value) -> String {
    let classes: Vec<String> = tool_parts(data)
        .into_iter()
        .filter(|part| part["toolName"] == "bash")
        .map(|part| {
            let command = part["input"]["command"].as_str().unwrap_or("");
            let kind = if command == "test -f out.txt" {
                "exact-test-f"
            } else if command.contains("test -f") || command.contains("test -e") {
                "has-test-f"
            } else {
                "other"
            };
            format!("{kind}:len={}", command.chars().count())
        })
        .collect();
    if classes.is_empty() {
        "no-bash".to_owned()
    } else {
        classes.join(" ")
    }
}

fn input_text(data: &Value) -> String {
    tool_parts(data)
        .into_iter()
        .map(|part| part["input"].to_string())
        .collect()
}

fn output_text(data: &Value) -> String {
    tool_parts(data)
        .into_iter()
        .map(|part| part["output"].to_string())
        .collect()
}

fn text_parts(data: &Value) -> String {
    data["parts"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|part| part["type"] == "text")
        .filter_map(|part| part["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Shape of a missed command. Counts and names only, so the log stays free of
/// the assistant body.
fn markup_hint(text: &str) -> String {
    let doubled = text.contains("\u{FF5C}\u{FF5C}DSML\u{FF5C}\u{FF5C}");
    let single = text.contains("\u{FF5C}DSML\u{FF5C}");
    let ascii = text.contains("||DSML||");
    let kind = if doubled {
        "doubled-bar"
    } else if single {
        "single-bar"
    } else if ascii {
        "ascii-dsml"
    } else if text.contains("DSML") {
        "word-dsml"
    } else if text.contains("sleep") {
        "mentions-sleep"
    } else {
        "no-sleep-word"
    };
    let bars = text.chars().filter(|ch| *ch == '\u{FF5C}').count();
    let pipes = text.chars().filter(|ch| *ch == '|').count();
    format!(
        "{kind} bars={bars} pipes={pipes} chars={}",
        text.chars().count()
    )
}

fn workspace_contains(dir: &Path, token: &str) -> bool {
    workspace_files(dir)
        .iter()
        .any(|path| std::fs::read_to_string(path).is_ok_and(|text| text.contains(token)))
}

fn process_rows() -> Vec<(u32, u32, String)> {
    let Ok(output) = std::process::Command::new("ps")
        .args(["-ax", "-o", "pid=,ppid=,command="])
        .output()
    else {
        return Vec::new();
    };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let pid = parts.next()?.parse().ok()?;
            let ppid = parts.next()?.parse().ok()?;
            let command = parts.collect::<Vec<_>>().join(" ");
            Some((pid, ppid, command))
        })
        .collect()
}

fn sleep_descendant(root: u32) -> Option<u32> {
    let rows = process_rows();
    let mut queue = vec![root];
    let mut seen = Vec::new();
    while let Some(parent) = queue.pop() {
        for (pid, ppid, command) in &rows {
            if *ppid != parent || seen.contains(pid) {
                continue;
            }
            let word = command.split_whitespace().next().unwrap_or("");
            if word == "sleep" || word.ends_with("/sleep") {
                return Some(*pid);
            }
            seen.push(*pid);
            queue.push(*pid);
        }
    }
    None
}

fn interject_method_missing(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("method not found")
        || lower.contains("method not supported")
        || lower.contains("unknown method")
        || lower.contains("-32601")
        || message.contains("_x.ai/interject")
}

struct Approval {
    id: String,
    tool_call_id: String,
}

async fn next_event(events: &mut broadcast::Receiver<Value>, label: &str) -> Value {
    events
        .recv()
        .await
        .unwrap_or_else(|error| panic!("{label} stream closed: {error}"))
}

/// Returns when this wait has seen exactly one approval request and that
/// card's own command body contains `needle` once. A different card or a
/// second request fails the row. This wait does not answer either card.
async fn wait_exact_command(
    harness: &LocalHarness,
    events: &mut broadcast::Receiver<Value>,
    label: &str,
    needle: &str,
) -> Approval {
    let mut seen = 0usize;
    let mut approvals = 0usize;
    let mut last = "none".to_owned();
    tokio::time::timeout(Duration::from_secs(180), async {
        loop {
            let event = next_event(events, label).await;
            seen += 1;
            let chunk = &event["payload"]["chunk"];
            if let Some(kind) = chunk["type"].as_str() {
                last = kind.to_owned();
            } else if let Some(channel) = event["channel"].as_str() {
                last = channel.to_owned();
            }
            if chunk["type"] == "tool-approval-request" {
                approvals += 1;
                let tool_call_id = chunk["toolCallId"].as_str().unwrap_or("").to_owned();
                let data = latest_assistant(harness);
                let input = tool_parts(&data)
                    .into_iter()
                    .find(|part| part["toolCallId"].as_str() == Some(tool_call_id.as_str()))
                    .map(|part| part["input"].to_string())
                    .unwrap_or_default();
                let command = input.matches(needle).count() == 1;
                if approvals != 1 || !command {
                    panic!(
                        "{label} saw {approvals} approval requests command={command} {}",
                        tool_shape(&data)
                    );
                }
                let approval = Approval {
                    id: chunk["approvalId"].as_str().unwrap_or("").to_owned(),
                    tool_call_id,
                };
                loop {
                    match events.try_recv() {
                        Ok(extra) => {
                            if extra["payload"]["chunk"]["type"] == "tool-approval-request" {
                                panic!(
                                    "{label} saw 2 approval requests {}",
                                    tool_shape(&latest_assistant(harness))
                                );
                            }
                        }
                        Err(broadcast::error::TryRecvError::Empty) => break,
                        Err(broadcast::error::TryRecvError::Lagged(_)) => {
                            panic!("{label} lagged while counting approval requests");
                        }
                        Err(broadcast::error::TryRecvError::Closed) => {
                            panic!("{label} stream closed while counting approval requests");
                        }
                    }
                }
                return approval;
            }
            let channel = event["channel"].as_str().unwrap_or("");
            if channel == "ai.stream.done" || channel == "ai.stream.error" {
                let status = event["payload"]["status"].as_str().unwrap_or("");
                let data = latest_assistant(harness);
                let text = text_parts(&data);
                let hint = markup_hint(&text);
                panic!(
                    "{label} ended before confirmation status={status} approvals={approvals} {} {hint}",
                    tool_shape(&data)
                );
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("{label} timed out approvals={approvals} events={seen} last={last}"))
}

async fn wait_done(events: &mut broadcast::Receiver<Value>, label: &str) -> String {
    wait_until_done(events, label, None).await
}

/// Later confirmation cards are refused so one extra ask cannot pin the turn.
/// The row still requires its own command, output, and status.
async fn wait_done_refusing(
    harness: &LocalHarness,
    events: &mut broadcast::Receiver<Value>,
    label: &str,
) -> String {
    wait_until_done(events, label, Some(harness)).await
}

async fn wait_until_done(
    events: &mut broadcast::Receiver<Value>,
    label: &str,
    refuse: Option<&LocalHarness>,
) -> String {
    let mut seen = 0usize;
    let mut cards = 0usize;
    let mut last = "none".to_owned();
    tokio::time::timeout(Duration::from_secs(180), async {
        loop {
            let event = next_event(events, label).await;
            seen += 1;
            if let Some(kind) = event["payload"]["chunk"]["type"].as_str() {
                last = kind.to_owned();
            } else if let Some(channel) = event["channel"].as_str() {
                last = channel.to_owned();
            }
            if event["payload"]["chunk"]["type"] == "tool-approval-request" {
                cards += 1;
                if let Some(harness) = refuse {
                    let id = event["payload"]["chunk"]["approvalId"]
                        .as_str()
                        .unwrap_or("");
                    if !id.is_empty() {
                        let _ = harness.turns.respond_approval(id, false);
                    }
                }
            }
            if event["channel"] == "ai.stream.done" {
                return event["payload"]["status"].as_str().unwrap_or("").to_owned();
            }
            if event["channel"] == "ai.stream.error" {
                let message = event["payload"]["error"]["message"].as_str().unwrap_or("");
                let (shape, bash) = match refuse {
                    Some(harness) => {
                        let data = latest_assistant(harness);
                        (tool_shape(&data), bash_command_class(&data))
                    }
                    None => ("no-harness".to_owned(), "no-bash".to_owned()),
                };
                panic!(
                    "{label} failed cards={cards} {shape} {bash} {}",
                    stream_error_class(message)
                );
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("{label} timed out cards={cards} events={seen} last={last}"))
}

/// A short class for a live stream failure. The provider text can echo the
/// prompt, so the panic keeps the class, the length, and alphabetic words.
fn stream_error_class(message: &str) -> String {
    let lower = message.to_ascii_lowercase();
    let tag = if lower.contains("context") {
        "context"
    } else if lower.contains("rate") || lower.contains("429") {
        "rate"
    } else if lower.contains("unauthorized") || lower.contains("401") || lower.contains("api key") {
        "auth"
    } else if lower.contains("execution mode") {
        "mode"
    } else if lower.contains("exited") || lower.contains("broken pipe") {
        "process"
    } else if lower.contains("timeout") || lower.contains("timed out") {
        "timeout"
    } else if lower.contains("payload") {
        "payload"
    } else if lower.contains("500") || lower.contains("502") || lower.contains("503") {
        "server"
    } else {
        "other"
    };
    let mut words = Vec::new();
    let mut word = String::new();
    for ch in message.chars() {
        if ch.is_ascii_alphabetic() {
            word.push(ch.to_ascii_lowercase());
        } else if !word.is_empty() {
            if (4..=20).contains(&word.len()) && !words.contains(&word) {
                words.push(word.clone());
            }
            word.clear();
            if words.len() == 6 {
                break;
            }
        }
    }
    if (4..=20).contains(&word.len()) && !words.contains(&word) && words.len() < 6 {
        words.push(word);
    }
    let paths = message
        .split_once(" (")
        .and_then(|(_, rest)| rest.split_once(')').map(|(body, _)| body))
        .map(redact_gate_paths)
        .unwrap_or_default();
    format!(
        "{tag} len={} paths={paths} words={}",
        message.chars().count(),
        words.join("-")
    )
}

/// Keep the gate's path list and drop digit runs, which can be a process id.
fn redact_gate_paths(body: &str) -> String {
    let mut out = String::new();
    let mut digits = 0usize;
    for ch in body.chars() {
        if ch.is_ascii_digit() {
            digits += 1;
            continue;
        }
        if digits > 0 {
            if digits >= 3 {
                out.push('#');
            } else {
                out.extend(std::iter::repeat('0').take(digits));
            }
            digits = 0;
        }
        if ch.is_ascii() && !ch.is_control() {
            out.push(ch);
        }
    }
    if digits >= 3 {
        out.push('#');
    } else if digits > 0 {
        out.extend(std::iter::repeat('0').take(digits));
    }
    out
}

/// Counts confirmation cards while waiting for the turn to settle.
async fn wait_done_counting(
    events: &mut broadcast::Receiver<Value>,
    label: &str,
) -> (String, usize) {
    let mut cards = 0usize;
    let mut seen = 0usize;
    let mut last = "none".to_owned();
    let status = tokio::time::timeout(Duration::from_secs(180), async {
        loop {
            let event = next_event(events, label).await;
            seen += 1;
            if let Some(kind) = event["payload"]["chunk"]["type"].as_str() {
                last = kind.to_owned();
            } else if let Some(channel) = event["channel"].as_str() {
                last = channel.to_owned();
            }
            if event["payload"]["chunk"]["type"] == "tool-approval-request" {
                cards += 1;
            }
            if event["channel"] == "ai.stream.done" {
                return event["payload"]["status"].as_str().unwrap_or("").to_owned();
            }
            if event["channel"] == "ai.stream.error" {
                panic!("{label} failed");
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("{label} timed out cards={cards} events={seen} last={last}"));
    (status, cards)
}

/// A read turn must not confirm the read. Any other card is refused so the turn can settle.
async fn wait_read_turn(
    harness: &LocalHarness,
    events: &mut broadcast::Receiver<Value>,
    label: &str,
) -> String {
    let mut seen = 0usize;
    let mut last = "none".to_owned();
    tokio::time::timeout(Duration::from_secs(180), async {
        loop {
            let event = next_event(events, label).await;
            seen += 1;
            if let Some(kind) = event["payload"]["chunk"]["type"].as_str() {
                last = kind.to_owned();
            } else if let Some(channel) = event["channel"].as_str() {
                last = channel.to_owned();
            }
            if event["payload"]["chunk"]["type"] == "tool-approval-request" {
                let data = latest_assistant(harness);
                let tool_call_id = event["payload"]["chunk"]["toolCallId"]
                    .as_str()
                    .unwrap_or("");
                let input = tool_parts(&data)
                    .into_iter()
                    .find(|part| part["toolCallId"].as_str() == Some(tool_call_id))
                    .map(|part| part["input"].to_string())
                    .unwrap_or_default();
                if input.contains("notes.txt") {
                    panic!("{label} asked for a read card {}", tool_shape(&data));
                }
                let id = event["payload"]["chunk"]["approvalId"]
                    .as_str()
                    .unwrap_or("");
                if !id.is_empty() {
                    let _ = harness.turns.respond_approval(id, false);
                }
            }
            if event["channel"] == "ai.stream.done" {
                return event["payload"]["status"].as_str().unwrap_or("").to_owned();
            }
            if event["channel"] == "ai.stream.error" {
                panic!("{label} failed");
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("{label} timed out events={seen} last={last}"))
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "set A3S_STUDIO_TASK_LIVE=1 to send prompts"]
async fn studio_task_local_acp_live() {
    if std::env::var_os("A3S_STUDIO_TASK_LIVE").is_none() {
        eprintln!(
            "studio_task_local_acp_live skipped: A3S_STUDIO_TASK_LIVE is unset, so M1-M14 are not counted in 28"
        );
        return;
    }
    let expected = workspace_acp();
    require_env_matches(&expected);
    let parts_path = m10_parts_path();
    let _ = std::fs::remove_file(&parts_path);
    let harness = local_harness("live", Some(&expected), Creds::Live);
    let session = harness.sessions.get(&harness.session_id).unwrap();
    let workspace = PathBuf::from(&session.workspace.path);
    let note = format!("NOTES-{}", std::process::id());
    std::fs::write(workspace.join("notes.txt"), &note).unwrap();
    let agent = harness.agents.get(&harness.agent_id).unwrap();
    let model = agent.model.clone().expect("live model");
    let (runtime, _) = harness
        .turns
        .runtime(
            &session,
            &agent,
            &model,
            None,
            &super::super::harness_assemble::default_components(),
        )
        .await
        .unwrap_or_else(|error| panic!("live session failed: {error}"));
    let pid = runtime.client.process_id().await.expect("live pid");
    assert_eq!(wait_mapped(pid), expected, "live executable");
    drop(runtime);
    // Startup may write its own files. These rows judge the command, so the
    // baseline is the workspace after the process is up and before the prompt.
    let baseline = workspace_files(&workspace);
    let mut events = harness.hub.subscribe();
    let run = std::process::id();
    set_reasoning(&harness, "none").await;
    while events.try_recv().is_ok() {}

    // Bypass runs before the rejection turns. After those turns the default
    // high-effort session keeps streaming reasoning and this printf never settles.
    set_mode(&harness, "bypassPermissions");
    let m7 = format!("M7-{run}");
    open_text(&harness, &bash_only(&format!("printf {m7}")));
    let (status, cards) = wait_done_counting(&mut events, "M7").await;
    assert_eq!(cards, 0, "M7 no card");
    assert_eq!(status, "success", "M7");
    assert!(
        output_text(&latest_assistant(&harness)).contains(&m7),
        "M7 stdout"
    );
    assert!(!workspace_contains(&workspace, &m7), "M7 no token file");
    set_mode(&harness, "default");

    let m1 = format!("M1-{run}");
    open_text(&harness, &bash_only(&format!("printf {m1}")));
    let approval = wait_exact_command(&harness, &mut events, "M1", &m1).await;
    assert!(!approval.id.is_empty(), "M1 approval");
    let waiting = latest_assistant(&harness);
    assert_eq!(
        input_text(&waiting).matches(&m1).count(),
        1,
        "M1 command once {}",
        tool_shape(&waiting)
    );
    assert!(
        waiting.to_string().contains("approval-requested"),
        "M1 waits"
    );
    assert_eq!(workspace_files(&workspace), baseline, "M1 files");

    let attached = harness.turns.attach(&harness.session_id);
    assert_ne!(attached["status"], "not-found", "M13");
    assert_eq!(attached["status"], "attached", "M13");
    let buffered = attached["bufferedChunks"].as_array().expect("M13 chunks");
    assert!(
        buffered
            .iter()
            .any(|chunk| chunk["chunk"]["type"] == "tool-approval-request"),
        "M13 buffered the card"
    );

    let conflict = harness
        .turns
        .open(
            &harness.session_id,
            &json!({ "userMessageParts": [{ "type": "text", "text": "再开一回合" }] }),
        )
        .expect_err("M14 must conflict");
    assert_eq!(conflict.to_string(), "a turn is already running", "M14");
    let same = harness
        .turns
        .runtimes
        .lock()
        .await
        .get(&harness.session_id)
        .cloned()
        .expect("M14 runtime")
        .client
        .process_id()
        .await;
    assert_eq!(same, Some(pid), "M14 pid");
    assert_eq!(acp_child_pids(), vec![pid], "M14 children");

    harness.turns.abort(&harness.session_id);
    assert_eq!(wait_done(&mut events, "M2").await, "paused", "M2");
    assert_eq!(workspace_files(&workspace), baseline, "M2 files");
    assert!(
        !output_text(&latest_assistant(&harness)).contains(&m1),
        "M2 command did not run"
    );

    let m3 = format!("M3-{run}");
    open_text(&harness, &bash_only(&format!("printf {m3}")));
    let approval = wait_exact_command(&harness, &mut events, "M3", &m3).await;
    let waiting = latest_assistant(&harness);
    assert_eq!(
        input_text(&waiting).matches(&m3).count(),
        1,
        "M3 command once {}",
        tool_shape(&waiting)
    );
    assert!(
        harness.turns.respond_approval(&approval.id, true),
        "M3 allow"
    );
    assert_eq!(
        wait_done_refusing(&harness, &mut events, "M3").await,
        "success",
        "M3"
    );
    let done = latest_assistant(&harness);
    assert!(output_text(&done).contains(&m3), "M3 stdout");
    assert!(
        tool_parts(&done)
            .iter()
            .any(|part| part["state"] == "output-available"),
        "M3 tool success"
    );

    let m4 = format!("M4-{run}");
    open_text(&harness, &bash_only(&format!("printf {m4}")));
    let approval = wait_exact_command(&harness, &mut events, "M4", &m4).await;
    let waiting = latest_assistant(&harness);
    assert_eq!(
        input_text(&waiting).matches(&m4).count(),
        1,
        "M4 command once {}",
        tool_shape(&waiting)
    );
    assert!(
        harness.turns.respond_approval(&approval.id, false),
        "M4 reject"
    );
    let _ = wait_done_refusing(&harness, &mut events, "M4").await;
    let rejected = latest_assistant(&harness);
    assert!(
        !tool_parts(&rejected).iter().any(|part| {
            part["state"] == "output-available" && part["output"].to_string().contains(&m4)
        }),
        "M4 no success output"
    );
    assert!(!workspace_contains(&workspace, &m4), "M4 no token file");

    set_mode(&harness, "auto");
    let m5 = format!("M5-{run}");
    open_text(
        &harness,
        "只调用一次 read，file_path 只能是 notes.txt。不要提问，不要调用 ask_user，不要调用 bash，不要创建或修改任何文件。",
    );
    let read_status = wait_read_turn(&harness, &mut events, "M5 read").await;
    assert_eq!(read_status, "success", "M5 read");
    let read_data = latest_assistant(&harness);
    assert!(
        tool_parts(&read_data)
            .iter()
            .any(|part| part["input"].to_string().contains("notes.txt")),
        "M5 read {}",
        tool_shape(&read_data)
    );

    open_text(&harness, &bash_only(&format!("printf {m5}")));
    let approval = wait_exact_command(&harness, &mut events, "M5", &m5).await;
    let waiting = latest_assistant(&harness);
    assert!(
        input_text(&waiting).contains(&m5),
        "M5 command {}",
        tool_shape(&waiting)
    );
    assert!(
        harness.turns.respond_approval(&approval.id, false),
        "M5 reject"
    );
    let _ = wait_done_refusing(&harness, &mut events, "M5").await;
    assert!(
        !workspace_contains(&workspace, &m5),
        "M5 command did not write"
    );

    set_mode(&harness, "plan");
    let m6 = format!("M6-{run}");
    open_text(
        &harness,
        "计划模式会拒绝 bash。不要调用 bash。必须调用一次 read，file_path 只能是 notes.txt，并且这次读取要成功。禁止只用文字回答。不要提问，不要调用 ask_user，不要创建或修改任何文件。",
    );
    let read_status = wait_read_turn(&harness, &mut events, "M6 read").await;
    assert_eq!(read_status, "success", "M6 read status");
    let planned = latest_assistant(&harness);
    assert!(
        tool_parts(&planned).iter().any(|part| {
            part["state"] == "output-available"
                && (part["input"].to_string().contains("notes.txt")
                    || part["output"].to_string().contains(&note))
        }),
        "M6 read {}",
        tool_shape(&planned)
    );

    // Plan mode refuses the command. A long reasoning stream never reaches
    // `ai.stream.done`, so this turn asks the model to stop once it is refused.
    open_text(
        &harness,
        &format!(
            "计划模式会拒绝这条命令。不要写长篇推理。不能执行时立刻结束。{}",
            bash_only(&format!("printf {m6}"))
        ),
    );
    let (_status, cards) = wait_done_counting(&mut events, "M6 command").await;
    assert_eq!(cards, 0, "M6 no card");
    let command = latest_assistant(&harness);
    assert!(
        !tool_parts(&command).iter().any(|part| {
            part["state"] == "output-available" && part["output"].to_string().contains(&m6)
        }),
        "M6 command did not run"
    );
    assert!(!workspace_contains(&workspace, &m6), "M6 no token file");

    set_mode(&harness, "default");
    // `none` answers this sleep in prose and never calls bash.
    set_reasoning(&harness, "low").await;
    while events.try_recv().is_ok() {}
    open_text(
        &harness,
        "使用结构化工具调用，不要把调用写进正文，不要输出 DSML。只调用一次 bash。command 只能是 sleep 30。不要填写 timeout 或其他参数。不要提问，不要调用 ask_user。不要创建或修改任何文件。",
    );
    let approval = wait_exact_command(&harness, &mut events, "M8", "sleep 30").await;
    assert!(
        harness.turns.respond_approval(&approval.id, true),
        "M8 allow"
    );
    let sleep_pid = tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            if let Some(pid) = sleep_descendant(pid) {
                return pid;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("M8 sleep did not start"));
    let inserted = tokio::time::timeout(Duration::from_secs(30), async {
        harness
            .turns
            .insert_into_live(
                &harness.session_id,
                &json!({
                    "userMessageParts": [{
                        "type": "text",
                        "text": "继续等待，不要额外执行命令。"
                    }]
                }),
            )
            .await
    })
    .await;
    let m9_covered = match inserted {
        Ok(Ok(value)) => {
            assert_eq!(value["mode"], "injected", "M9");
            true
        }
        Ok(Err(error)) if interject_method_missing(&error.to_string()) => {
            eprintln!("STUDIO_TASK_ACP_M9 uncovered: interject method is missing");
            false
        }
        Ok(Err(error)) => panic!("M9 failed: {error}"),
        Err(_) => panic!("M9 interject timed out"),
    };
    harness.turns.abort(&harness.session_id);
    assert_eq!(wait_done(&mut events, "M8").await, "paused", "M8");
    wait_until_dead(sleep_pid);
    assert!(process_alive(pid), "M8 agent stays up");

    set_mode(&harness, "bypassPermissions");
    let m10 = format!("M10-{run}");
    // Yolo still leaves an unverified write behind the completion gate.
    // `test -f` binds that mutation. The token stays out of the shell command.
    open_text(
        &harness,
        &format!(
            "只调用一次写入文件工具。file_path 只能是 out.txt，内容只能是下面的令牌。然后只调用一次 bash，command 必须正好是 test -f out.txt，不能改成其他命令。不要把令牌写进 bash。这两次调用完成前不要结束。令牌：{m10}"
        ),
    );
    assert_eq!(
        wait_done_refusing(&harness, &mut events, "M10").await,
        "success",
        "M10"
    );
    let written = std::fs::read_to_string(workspace.join("out.txt")).expect("M10 out.txt");
    assert_eq!(written.trim(), m10, "M10 file");
    let saved = latest_assistant(&harness);
    assert!(
        saved.to_string().contains("out.txt"),
        "M10 parts name the file"
    );
    std::fs::write(
        &parts_path,
        serde_json::to_vec(&json!({ "parts": saved["parts"], "token": m10 })).expect("M10 json"),
    )
    .expect("M10 parts file");

    let before_false = workspace_files(&workspace);
    open_text(&harness, &bash_only("false"));
    assert_eq!(
        wait_done_refusing(&harness, &mut events, "M11").await,
        "success",
        "M11 turn"
    );
    let failed = latest_assistant(&harness);
    assert!(
        tool_parts(&failed)
            .iter()
            .any(|part| part["state"] == "output-error"),
        "M11 tool failed {}",
        tool_shape(&failed)
    );
    assert_eq!(workspace_files(&workspace), before_false, "M11 files");

    open_text(
        &harness,
        "请读取 out.txt，只回复文件里的令牌，不要执行命令。",
    );
    assert_eq!(
        wait_done_refusing(&harness, &mut events, "M12").await,
        "success",
        "M12"
    );
    assert!(
        text_parts(&latest_assistant(&harness)).contains(&m10),
        "M12 reply"
    );

    runtime_kill(&harness, pid).await;
    assert!(!process_alive(sleep_pid), "M8 sleep stays dead");
    if m9_covered {
        eprintln!("STUDIO_TASK_ACP_LIVE rows=M1-M14");
    } else {
        eprintln!("STUDIO_TASK_ACP_LIVE rows=M1-M8,M10-M14 M9=uncovered");
    }
}

async fn runtime_kill(harness: &LocalHarness, pid: u32) {
    if let Some(runtime) = harness
        .turns
        .runtimes
        .lock()
        .await
        .get(&harness.session_id)
        .cloned()
    {
        runtime.client.kill().await;
    }
    wait_until_dead(pid);
}
