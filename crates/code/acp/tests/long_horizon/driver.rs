#![cfg(all(test, unix))]

use agent_client_protocol as acp;
use async_trait::async_trait;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Stdio;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

const CAPTURE_LIMIT: usize = 8 * 1024 * 1024;

#[derive(Default)]
pub struct Capture {
    pub events: Vec<Value>,
    pub text: String,
    pub permissions: usize,
    pub overflow: bool,
    pub tools: BTreeMap<String, acp::ToolCallStatus>,
    pub rpc_errors: Vec<Value>,
    bytes: usize,
}

impl Capture {
    fn record(&mut self, value: Value) -> acp::Result<()> {
        self.bytes = self.bytes.saturating_add(value.to_string().len());
        if self.bytes > CAPTURE_LIMIT {
            self.overflow = true;
            return Err(acp::Error::internal_error());
        }
        self.events.push(value);
        Ok(())
    }
}

struct Client {
    capture: Arc<Mutex<Capture>>,
    allow: bool,
}

fn safe_rpc_error(error: &acp::Error, credential: &str) -> Value {
    let text = serde_json::to_string(error)
        .unwrap_or_default()
        .replace(credential, "[fixture credential redacted]");
    if text.len() > 32 * 1024 {
        return serde_json::json!({"code":error.code,"diagnostic_truncated":true});
    }
    serde_json::from_str(&text)
        .unwrap_or_else(|_| serde_json::json!({"code":error.code,"diagnostic_unavailable":true}))
}

#[async_trait(?Send)]
impl acp::Client for Client {
    async fn session_notification(
        &self,
        notification: acp::SessionNotification,
    ) -> acp::Result<()> {
        let value = if let acp::SessionUpdate::AgentThoughtChunk(chunk) = &notification.update {
            let text_bytes = match &chunk.content {
                acp::ContentBlock::Text(text) => Some(text.text.len()),
                _ => None,
            };
            serde_json::json!({"sessionId":notification.session_id,"update":{"sessionUpdate":"agent_thought_chunk","contentOmitted":true,"textBytes":text_bytes}})
        } else {
            serde_json::to_value(&notification).map_err(|_| acp::Error::internal_error())?
        };
        let mut capture = self.capture.lock().unwrap();
        capture.record(value)?;
        match notification.update {
            acp::SessionUpdate::AgentMessageChunk(chunk) => {
                if let acp::ContentBlock::Text(text) = chunk.content {
                    capture.text.push_str(&text.text);
                }
            }
            acp::SessionUpdate::ToolCall(call) => {
                capture
                    .tools
                    .insert(call.tool_call_id.0.to_string(), call.status);
            }
            acp::SessionUpdate::ToolCallUpdate(update) => {
                if let Some(status) = update.fields.status {
                    capture
                        .tools
                        .insert(update.tool_call_id.0.to_string(), status);
                }
            }
            _ => {}
        }
        Ok(())
    }

    async fn ext_notification(&self, args: acp::ExtNotification) -> acp::Result<()> {
        let value = if args.method.as_ref() == "a3s/turn/outcome" {
            serde_json::json!({"method":args.method,"params":args})
        } else {
            serde_json::json!({"method":args.method,"paramsOmitted":true})
        };
        self.capture.lock().unwrap().record(value)
    }

    async fn request_permission(
        &self,
        args: acp::RequestPermissionRequest,
    ) -> acp::Result<acp::RequestPermissionResponse> {
        self.capture.lock().unwrap().permissions += 1;
        let kind = if self.allow {
            acp::PermissionOptionKind::AllowOnce
        } else {
            acp::PermissionOptionKind::RejectOnce
        };
        let outcome = match args.options.iter().find(|option| option.kind == kind) {
            Some(option) => acp::RequestPermissionOutcome::Selected(
                acp::SelectedPermissionOutcome::new(option.option_id.clone()),
            ),
            None => acp::RequestPermissionOutcome::Cancelled,
        };
        Ok(acp::RequestPermissionResponse::new(outcome))
    }
}

// The child is a new Unix process group; keep its unreaped leader until cleanup
// so a recycled PID can never name an unrelated group.
pub struct OwnedChild {
    child: tokio::process::Child,
    group: Option<i32>,
}

unsafe extern "C" {
    fn kill(pid: i32, signal: i32) -> i32;
}

// shortcut: Unix process groups cover this stdio driver; Windows Job ownership
// and tools that deliberately create another group need their own cleanup proof.

impl OwnedChild {
    pub fn spawn(command: &mut tokio::process::Command) -> Result<Self, &'static str> {
        use std::os::unix::process::CommandExt;
        command.as_std_mut().process_group(0);
        let child = command
            .kill_on_drop(true)
            .spawn()
            .map_err(|_| "child_spawn")?;
        let group = child.id().and_then(|pid| i32::try_from(pid).ok());
        Ok(Self { child, group })
    }

    pub fn child_mut(&mut self) -> &mut tokio::process::Child {
        &mut self.child
    }

    fn stop_group(&mut self) -> Result<(), &'static str> {
        if let Some(group) = self.group.take() {
            // POSIX SIGKILL is 9 on the supported macOS/Linux test platforms.
            if unsafe { kill(-group, 9) } != 0 {
                let error = std::io::Error::last_os_error();
                if error.raw_os_error() != Some(3) {
                    return Err("child_group_cleanup");
                }
            }
        }
        Ok(())
    }

    pub async fn close(mut self) -> Result<(), &'static str> {
        let group_id = self.group;
        let stopped = self.stop_group();
        let reaped = tokio::time::timeout(Duration::from_secs(5), self.child.wait()).await;
        reaped
            .map_err(|_| "child_reap_timeout")?
            .map_err(|_| "child_reap")?;
        if stopped.is_err() {
            // Darwin may refuse a group containing only an unreaped zombie.
            // After reaping, only probe absence; never signal a recycled ID.
            let absent = group_id.is_some_and(|group| unsafe { kill(-group, 0) } != 0
                && std::io::Error::last_os_error().raw_os_error() == Some(3));
            if !absent {
                return stopped;
            }
        }
        Ok(())
    }
}

#[path = "driver_tests.rs"]
mod tests;

impl Drop for OwnedChild {
    fn drop(&mut self) {
        let _ = self.stop_group();
        let _ = self.child.start_kill();
    }
}

pub struct Driver {
    pub connection: Rc<acp::ClientSideConnection>,
    pub capture: Arc<Mutex<Capture>>,
    child: OwnedChild,
    io: tokio::task::JoinHandle<()>,
    credential: String,
}

impl Driver {
    pub fn process_id(&self) -> Option<u32> {
        self.child.child.id()
    }

    pub fn spawn(
        binary: &Path,
        workspace: &Path,
        home: &Path,
        acl: &Path,
        relay_key: &str,
        allow: bool,
        sandbox_assets: Option<&Path>,
    ) -> Result<Self, &'static str> {
        let sandbox_assets = qualified_assets(workspace, sandbox_assets)?;
        let mut command = tokio::process::Command::new(binary);
        let mut paths = vec![home.join("bin")];
        paths.extend(std::env::split_paths(
            &std::env::var_os("PATH").ok_or("missing_tool_path")?,
        ));
        command
            .args([
                "--cwd",
                workspace.to_str().ok_or("workspace_encoding")?,
                "--config",
                acl.to_str().ok_or("acl_encoding")?,
            ])
            .current_dir(workspace)
            .env_clear()
            .env(
                "PATH",
                std::env::join_paths(paths).map_err(|_| "invalid_tool_path")?,
            )
            .env("HOME", home)
            .env("TMPDIR", home.join("tmp"))
            .env("LANG", "en_US.UTF-8")
            .env(super::config::RELAY_KEY_ENV, relay_key)
            .env("A3S_DISABLE_CC_SWITCH", "1")
            .env("A3S_DISABLE_GROK_ACCOUNT", "1")
            .env("A3S_DISABLE_CURSOR_ACCOUNT", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            // No unbounded or potentially credential-bearing diagnostic file.
            .stderr(Stdio::null());
        if let Some(directory) = sandbox_assets {
            command.env("ASTER_SANDBOX_DIR", directory);
        }
        let mut child = OwnedChild::spawn(&mut command)?;
        let output = child
            .child_mut()
            .stdout
            .take()
            .ok_or("child_stdout")?
            .compat();
        let input = child
            .child_mut()
            .stdin
            .take()
            .ok_or("child_stdin")?
            .compat_write();
        let capture = Arc::new(Mutex::new(Capture::default()));
        let (connection, io) = acp::ClientSideConnection::new(
            Client {
                capture: capture.clone(),
                allow,
            },
            input,
            output,
            |future| {
                tokio::task::spawn_local(future);
            },
        );
        let io = tokio::task::spawn_local(async move {
            let _ = io.await;
        });
        Ok(Self {
            connection: Rc::new(connection),
            capture,
            child,
            io,
            credential: relay_key.to_owned(),
        })
    }

    pub async fn open(
        &self,
        workspace: &Path,
        host_meta: Option<serde_json::Map<String, Value>>,
    ) -> Result<acp::SessionId, &'static str> {
        tokio::time::timeout(Duration::from_secs(30), async {
            let initialization = acp::Agent::initialize(
                self.connection.as_ref(),
                acp::InitializeRequest::new(acp::ProtocolVersion::V1),
            )
            .await
            .map_err(|_| "initialize_error")?;
            if initialization.protocol_version != acp::ProtocolVersion::V1 {
                return Err("unexpected_protocol_version");
            }
            acp::Agent::new_session(
                self.connection.as_ref(),
                acp::NewSessionRequest::new(workspace.to_owned()).meta(host_meta),
            )
            .await
            .map(|session| session.session_id)
            .map_err(|_| "new_session_error")
        })
        .await
        .map_err(|_| "session_open_timeout")?
    }

    pub async fn prompt(
        &self,
        session: &acp::SessionId,
        text: &str,
    ) -> Result<String, &'static str> {
        self.capture.lock().unwrap().text.clear();
        let response = tokio::time::timeout(
            Duration::from_secs(600),
            acp::Agent::prompt(
                self.connection.as_ref(),
                acp::PromptRequest::new(
                    session.clone(),
                    vec![acp::ContentBlock::Text(acp::TextContent::new(text))],
                ),
            ),
        )
        .await
        .map_err(|_| "prompt_timeout")?
        .map_err(|error| {
            let mut capture = self.capture.lock().unwrap();
            if capture.rpc_errors.len() < 16 {
                capture
                    .rpc_errors
                    .push(safe_rpc_error(&error, &self.credential));
            }
            "prompt_error"
        })?;
        if response.stop_reason != acp::StopReason::EndTurn {
            return Err("unexpected_prompt_terminal");
        }
        let capture = self.capture.lock().unwrap();
        if capture.overflow {
            return Err("capture_overflow");
        }
        if capture.text.trim().is_empty() {
            return Err("missing_final_message");
        }
        Ok(capture.text.clone())
    }

    pub async fn close(self) -> Result<(), &'static str> {
        let child = self.child.close().await;
        let mut io = self.io;
        let settled = match tokio::time::timeout(Duration::from_secs(5), &mut io).await {
            Ok(result) => result.map_err(|_| "wire_task_cleanup"),
            Err(_) => {
                io.abort();
                let _ = io.await;
                Err("wire_cleanup_timeout")
            }
        };
        child.and(settled)
    }
}

fn qualified_assets(
    workspace: &Path,
    assets: Option<&Path>,
) -> Result<Option<std::path::PathBuf>, &'static str> {
    let Some(assets) = assets else {
        return Ok(None);
    };
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        if !assets.is_absolute() {
            return Err("sandbox_assets_must_be_absolute");
        }
        let assets = assets
            .canonicalize()
            .map_err(|_| "sandbox_assets_unavailable")?;
        let workspace = workspace
            .canonicalize()
            .map_err(|_| "sandbox_workspace_unavailable")?;
        if assets.starts_with(&workspace) || workspace.starts_with(&assets) {
            return Err("sandbox_assets_overlap_workspace");
        }
        a3s_code_core::sandbox::oci::OciSandboxAssets::from_directory(&assets)
            .map_err(|_| "sandbox_assets_invalid")?;
        Ok(Some(assets))
    }
    #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
    {
        let _ = (workspace, assets);
        Err("sandbox_profile_unsupported")
    }
}
