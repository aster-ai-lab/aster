//! One short prompt against the shipped `a3s-code-acp` process and a real model.
//!
//! Ignored by default. Verification runs it twice with `--ignored`. A scripted
//! conformance model does not satisfy this file. The child keeps the caller's
//! `HOME`; this test does not redirect it.

#![cfg(test)]

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use agent_client_protocol as acp;
use agent_client_protocol::{Client, ClientSideConnection};
use async_trait::async_trait;
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

const MODEL: &str = "deepseek/deepseek-flash";
const PROMPT: &str = "Reply with the single word pong. Do not use tools.";

fn selected_model() -> String {
    std::env::var("A3S_LIVE_MODEL")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| MODEL.to_string())
}

struct CaptureClient {
    agent_text: std::sync::Arc<std::sync::Mutex<String>>,
}

#[async_trait(?Send)]
impl Client for CaptureClient {
    async fn session_notification(
        &self,
        notification: acp::SessionNotification,
    ) -> Result<(), acp::Error> {
        if let acp::SessionUpdate::AgentMessageChunk(chunk) = notification.update {
            if let acp::ContentBlock::Text(text) = chunk.content {
                self.agent_text
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .push_str(text.text.as_str());
            }
        }
        Ok(())
    }

    async fn request_permission(
        &self,
        _args: acp::RequestPermissionRequest,
    ) -> Result<acp::RequestPermissionResponse, acp::Error> {
        Ok(acp::RequestPermissionResponse::new(
            acp::RequestPermissionOutcome::Cancelled,
        ))
    }
}

fn live_config_path() -> PathBuf {
    if let Some(value) = std::env::var_os("A3S_CONFIG_FILE") {
        let path = PathBuf::from(value);
        assert!(
            path.is_file(),
            "A3S_CONFIG_FILE is not a file: {}",
            path.display()
        );
        return path;
    }
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join(".a3s/config.acl");
    if repo.is_file() {
        return repo;
    }
    if let Some(home) = std::env::var_os("HOME") {
        let user = PathBuf::from(home).join(".a3s/config.acl");
        if user.is_file() {
            return user;
        }
    }
    panic!("no ACL: set A3S_CONFIG_FILE or provide .a3s/config.acl");
}

fn engineering_off() -> acp::Meta {
    let mut meta = acp::Meta::new();
    meta.insert(
        "a3sEngineering".to_string(),
        serde_json::json!({"policy": "none"}),
    );
    meta
}

fn redact(text: &str) -> String {
    let mut out = String::new();
    for line in text.lines() {
        let lower = line.to_ascii_lowercase();
        if lower.contains("sk-")
            || lower.contains("api_key")
            || lower.contains("apikey")
            || lower.contains("bearer ")
            || lower.contains("authorization")
        {
            out.push_str("[redacted]\n");
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

async fn spawn_agent(
    workspace: &Path,
    config: &Path,
    model: &str,
) -> (
    tokio::process::Child,
    std::rc::Rc<ClientSideConnection>,
    std::sync::Arc<std::sync::Mutex<String>>,
    PathBuf,
) {
    let bin = PathBuf::from(env!("CARGO_BIN_EXE_a3s-code-acp"));
    let agent_text = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
    let client_text = std::sync::Arc::clone(&agent_text);
    let stderr_path = workspace.join("acp.stderr");
    let stderr = std::fs::File::create(&stderr_path).expect("acp stderr");
    let mut child = tokio::process::Command::new(&bin)
        .arg("--cwd")
        .arg(workspace)
        .arg("--config")
        .arg(config)
        .arg("--model")
        .arg(model)
        .env("A3S_CONFIG", config)
        .env("A3S_DISABLE_CC_SWITCH", "1")
        .env("A3S_DISABLE_GROK_ACCOUNT", "1")
        .env("A3S_DISABLE_CURSOR_ACCOUNT", "1")
        .current_dir(workspace)
        .kill_on_drop(true)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::from(stderr))
        .spawn()
        .unwrap_or_else(|error| panic!("spawn {}: {error}", bin.display()));
    let stdout = child.stdout.take().expect("stdout").compat();
    let stdin = child.stdin.take().expect("stdin").compat_write();
    let conn = ClientSideConnection::new(
        CaptureClient {
            agent_text: client_text,
        },
        stdin,
        stdout,
        |fut| {
            tokio::task::spawn_local(fut);
        },
    );
    let (conn, io) = (std::rc::Rc::new(conn.0), conn.1);
    tokio::task::spawn_local(async move {
        let _ = io.await;
    });
    (child, conn, agent_text, stderr_path)
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "real deepseek/deepseek-flash via A3S ACL; not a scripted model"]
async fn shared_acp_live_prompt_returns_usage() {
    let config = live_config_path();
    let workspace = std::env::temp_dir().join(format!(
        "a3s-shared-acp-live-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or(0)
    ));
    let _ = std::fs::remove_dir_all(&workspace);
    std::fs::create_dir_all(&workspace).expect("workspace");
    let result = tokio::task::LocalSet::new()
        .run_until(async {
            tokio::time::timeout(
                Duration::from_secs(180),
                run_prompt(&workspace, &config, &selected_model()),
            )
            .await
        })
        .await;
    let _ = std::fs::remove_dir_all(&workspace);
    match result {
        Ok(Ok(())) => {}
        Ok(Err(error)) => panic!("{}", redact(&error)),
        Err(_) => panic!("live prompt exceeded 180s"),
    }
}

async fn run_prompt(workspace: &Path, config: &Path, model: &str) -> Result<(), String> {
    let (mut child, conn, agent_text, stderr_path) = spawn_agent(workspace, config, model).await;
    let result = dialogue(workspace, &conn, &agent_text, model).await;
    drop(conn);
    let _ = tokio::time::timeout(Duration::from_secs(5), child.kill()).await;
    let _ = child.wait().await;
    if let Err(error) = &result {
        let stderr = std::fs::read_to_string(&stderr_path).unwrap_or_default();
        let tail: String = redact(&stderr)
            .lines()
            .rev()
            .take(20)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n");
        return Err(format!("{error}\nstderr:\n{tail}"));
    }
    result
}

async fn dialogue(
    workspace: &Path,
    conn: &ClientSideConnection,
    agent_text: &std::sync::Arc<std::sync::Mutex<String>>,
    model_id: &str,
) -> Result<(), String> {
    acp::Agent::initialize(conn, acp::InitializeRequest::new(acp::ProtocolVersion::V1))
        .await
        .map_err(|error| format!("initialize: {error}"))?;
    let session = acp::Agent::new_session(
        conn,
        acp::NewSessionRequest::new(workspace.to_path_buf()).meta(Some(engineering_off())),
    )
    .await
    .map_err(|error| format!("new session: {error}"))?
    .session_id;
    let response = acp::Agent::prompt(
        conn,
        acp::PromptRequest::new(
            session.clone(),
            vec![acp::ContentBlock::Text(acp::TextContent::new(PROMPT))],
        ),
    )
    .await
    .map_err(|error| format!("prompt: {error}"))?;
    let reply = agent_text
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone();
    let reply = reply.trim().to_string();
    if reply.is_empty() {
        return Err(format!(
            "assistant reply was empty; stop={:?}",
            response.stop_reason
        ));
    }
    let params = serde_json::value::to_raw_value(&serde_json::json!({
        "sessionId": session.0.as_ref(),
    }))
    .map_err(|error| format!("session info params: {error}"))?;
    let info: serde_json::Value = serde_json::from_str(
        acp::Agent::ext_method(
            conn,
            acp::ExtRequest::new("a3s/session/info", std::sync::Arc::from(params)),
        )
        .await
        .map_err(|error| format!("session info: {error}"))?
        .0
        .get(),
    )
    .map_err(|error| format!("session info json: {error}"))?;
    let used = info["result"]["context"]["used"].as_u64().unwrap_or(0);
    if used == 0 {
        return Err(format!(
            "usage tokens were zero: {}",
            redact(&info.to_string())
        ));
    }
    let model = info["result"]["model"].as_str().unwrap_or("").to_string();
    if model != model_id {
        return Err(format!("model was {model}, expected {model_id}"));
    }
    let preview: String = reply.chars().take(80).collect();
    println!(
        "LIVE_LLM reply_nonempty=1 usage={used} model={model} reply_len={} preview={}",
        reply.chars().count(),
        redact(&preview).trim()
    );
    Ok(())
}
