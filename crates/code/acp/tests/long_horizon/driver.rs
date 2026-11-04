#![cfg(all(test, unix))]

use super::artifact::timing::{self, Phase, Subject};
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
    timing_tools: BTreeMap<String, timing::Span>,
    timing_compactions: BTreeMap<String, timing::Span>,
    timing_subagents: BTreeMap<String, timing::Span>,
}

impl Capture {
    fn time_tool(&mut self, id: &str, status: &acp::ToolCallStatus) {
        match status {
            acp::ToolCallStatus::Pending | acp::ToolCallStatus::InProgress => {
                self.timing_tools
                    .entry(id.to_owned())
                    .or_insert_with(|| timing::span_for(Phase::Tool, Some(timing::opaque_id(id))));
            }
            acp::ToolCallStatus::Completed | acp::ToolCallStatus::Failed => {
                if self.timing_tools.remove(id).is_none() {
                    timing::point(Phase::ToolMissingStart, Some(timing::opaque_id(id)));
                }
            }
            _ => {}
        }
    }

    fn time_session_update(&mut self, raw: &str) {
        #[derive(serde::Deserialize)]
        struct Notification {
            #[serde(rename = "sessionId")]
            session_id: String,
            update: Update,
            #[serde(default, rename = "_meta")]
            meta: Metadata,
        }
        #[derive(Default, serde::Deserialize)]
        struct Metadata {
            #[serde(default, rename = "isReplay")]
            replay: bool,
        }
        #[derive(serde::Deserialize)]
        struct Update {
            #[serde(rename = "sessionUpdate")]
            kind: String,
            subagent_id: Option<String>,
        }
        let Ok(notification) = serde_json::from_str::<Notification>(raw) else {
            return;
        };
        if notification.meta.replay {
            let phase = match notification.update.kind.as_str() {
                "auto_compact_started" | "auto_compact_completed" => Phase::CompactionReplay,
                "subagent_spawned" | "subagent_finished" => Phase::SubagentReplay,
                _ => return,
            };
            timing::point(
                phase,
                Some(timing::opaque_id(
                    notification
                        .update
                        .subagent_id
                        .as_deref()
                        .unwrap_or(&notification.session_id),
                )),
            );
            return;
        }
        match notification.update.kind.as_str() {
            "auto_compact_started" => {
                self.timing_compactions
                    .entry(notification.session_id.clone())
                    .or_insert_with(|| {
                        timing::span_for(
                            Phase::Compaction,
                            Some(timing::opaque_id(&notification.session_id)),
                        )
                    });
            }
            "auto_compact_completed" => {
                if self
                    .timing_compactions
                    .remove(&notification.session_id)
                    .is_none()
                {
                    timing::point(
                        Phase::CompactionMissingStart,
                        Some(timing::opaque_id(&notification.session_id)),
                    );
                }
            }
            "subagent_spawned" => {
                if let Some(id) = notification.update.subagent_id {
                    self.timing_subagents.entry(id.clone()).or_insert_with(|| {
                        timing::span_for(Phase::Subagent, Some(timing::opaque_id(&id)))
                    });
                }
            }
            "subagent_finished" => {
                if let Some(id) = notification.update.subagent_id {
                    if self.timing_subagents.remove(&id).is_none() {
                        timing::point(Phase::SubagentMissingStart, Some(timing::opaque_id(&id)));
                    }
                }
            }
            _ => {}
        }
    }

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

impl Drop for Capture {
    fn drop(&mut self) {
        for (phase, ids) in [
            (Phase::ToolUnfinished, &self.timing_tools),
            (Phase::CompactionUnfinished, &self.timing_compactions),
            (Phase::SubagentUnfinished, &self.timing_subagents),
        ] {
            for id in ids.keys() {
                timing::point(phase, Some(timing::opaque_id(id)));
            }
        }
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
        let replay = value
            .get("_meta")
            .and_then(|meta| meta.get("isReplay"))
            .and_then(Value::as_bool)
            == Some(true);
        let mut capture = self.capture.lock().unwrap();
        capture.record(value)?;
        match notification.update {
            acp::SessionUpdate::AgentMessageChunk(chunk) => {
                if let acp::ContentBlock::Text(text) = chunk.content {
                    capture.text.push_str(&text.text);
                }
            }
            acp::SessionUpdate::ToolCall(call) => {
                if replay {
                    timing::point(
                        Phase::ToolReplay,
                        Some(timing::opaque_id(call.tool_call_id.0.as_ref())),
                    );
                } else {
                    capture.time_tool(call.tool_call_id.0.as_ref(), &call.status);
                }
                capture
                    .tools
                    .insert(call.tool_call_id.0.to_string(), call.status);
            }
            acp::SessionUpdate::ToolCallUpdate(update) => {
                if let Some(status) = update.fields.status {
                    if replay {
                        timing::point(
                            Phase::ToolReplay,
                            Some(timing::opaque_id(update.tool_call_id.0.as_ref())),
                        );
                    } else {
                        capture.time_tool(update.tool_call_id.0.as_ref(), &status);
                    }
                    capture
                        .tools
                        .insert(update.tool_call_id.0.to_string(), status);
                }
            }
            acp::SessionUpdate::UsageUpdate(_) => {
                timing::point(
                    Phase::ModelUsage,
                    Some(timing::opaque_id(notification.session_id.0.as_ref())),
                );
            }
            _ => {}
        }
        Ok(())
    }

    async fn ext_notification(&self, args: acp::ExtNotification) -> acp::Result<()> {
        if args.method.as_ref() == "aster/session_notification" {
            self.capture
                .lock()
                .unwrap()
                .time_session_update(args.params.get());
        }
        let value = if args.method.as_ref() == "aster/turn/outcome" {
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
        let _timing = timing::span(Phase::Permission);
        self.capture.lock().unwrap().permissions += 1;
        let host_escalation = args
            .tool_call
            .fields
            .raw_input
            .as_ref()
            .and_then(|input| input.get("sandbox_permissions"))
            .and_then(Value::as_str)
            == Some("require_escalated");
        let kind = if self.allow && !host_escalation {
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
        let _timing = timing::span(Phase::ChildSpawn);
        use std::os::unix::process::CommandExt;
        command.as_std_mut().process_group(0);
        let child = command
            .kill_on_drop(true)
            .spawn()
            .map_err(|_| "child_spawn")?;
        let group = child.id().and_then(|pid| i32::try_from(pid).ok());
        timing::point(Phase::ChildSpawn, child.id().map(Subject::Process));
        Ok(Self { child, group })
    }

    pub fn child_mut(&mut self) -> &mut tokio::process::Child {
        &mut self.child
    }

    fn stop_group(&mut self) -> Result<(), &'static str> {
        let _timing = timing::span(Phase::ChildStop);
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
        let reaped = {
            let _timing = timing::span(Phase::ChildReap);
            tokio::time::timeout(Duration::from_secs(5), self.child.wait()).await
        };
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
        let _timing = timing::span(Phase::DriverSpawn);
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
            .env("ASTER_DISABLE_GROK_ACCOUNT", "1")
            .env("ASTER_DISABLE_CURSOR_ACCOUNT", "1")
            .env("ASTER_DISABLE_CODEX_ACCOUNT", "1")
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
            let initialization_timing = timing::span(Phase::Initialize);
            let initialization = acp::Agent::initialize(
                self.connection.as_ref(),
                acp::InitializeRequest::new(acp::ProtocolVersion::V1),
            )
            .await
            .map_err(|_| "initialize_error")?;
            drop(initialization_timing);
            if initialization.protocol_version != acp::ProtocolVersion::V1 {
                return Err("unexpected_protocol_version");
            }
            let _timing = timing::span(Phase::SessionNew);
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
        let _timing = timing::span_for(Phase::Prompt, Some(timing::opaque_id(session.0.as_ref())));
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

    pub async fn load(
        &self,
        session: &acp::SessionId,
        workspace: &Path,
    ) -> Result<(), &'static str> {
        let _timing = timing::span(Phase::SessionLoad);
        tokio::time::timeout(Duration::from_secs(30), async {
            acp::Agent::initialize(
                self.connection.as_ref(),
                acp::InitializeRequest::new(acp::ProtocolVersion::V1),
            )
            .await
            .map_err(|_| "initialize_error")?;
            acp::Agent::load_session(
                self.connection.as_ref(),
                acp::LoadSessionRequest::new(session.clone(), workspace.to_owned()),
            )
            .await
            .map_err(|_| "load_session_error")?;
            Ok(())
        })
        .await
        .map_err(|_| "session_load_timeout")?
    }

    pub async fn goal_state(&self, session: &acp::SessionId) -> Result<Value, &'static str> {
        let _timing = timing::span(Phase::GoalState);
        let params =
            serde_json::value::to_raw_value(&serde_json::json!({"session_id":session.0.as_ref()}))
                .map_err(|_| "goal_state_params")?;
        let response = tokio::time::timeout(
            Duration::from_secs(30),
            acp::Agent::ext_method(
                self.connection.as_ref(),
                acp::ExtRequest::new("aster/goal/state", std::sync::Arc::from(params)),
            ),
        )
        .await
        .map_err(|_| "goal_state_timeout")?
        .map_err(|_| "goal_state_error")?;
        serde_json::from_str(response.0.get()).map_err(|_| "goal_state_json")
    }

    pub async fn close(self) -> Result<(), &'static str> {
        let _timing = timing::span(Phase::DriverClose);
        let child = self.child.close().await;
        let mut io = self.io;
        let wire_timing = timing::span(Phase::WireClose);
        let settled = match tokio::time::timeout(Duration::from_secs(5), &mut io).await {
            Ok(result) => result.map_err(|_| "wire_task_cleanup"),
            Err(_) => {
                io.abort();
                let _ = io.await;
                Err("wire_cleanup_timeout")
            }
        };
        drop(wire_timing);
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
        aster_core::sandbox::oci::OciSandboxAssets::from_directory(&assets)
            .map_err(|_| "sandbox_assets_invalid")?;
        Ok(Some(assets))
    }
    #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
    {
        let _ = (workspace, assets);
        Err("sandbox_profile_unsupported")
    }
}

#[tokio::test]
async fn model_workflow_permission_client_refuses_host_escalation() {
    let client = Client {
        capture: Arc::new(Mutex::new(Capture::default())),
        allow: true,
    };
    for (mode, expected) in [("use_default", "allow"), ("require_escalated", "reject")] {
        let request = acp::RequestPermissionRequest::new(
            acp::SessionId::new("permission-fixture"),
            acp::ToolCallUpdate::new(
                acp::ToolCallId::new("bash"),
                acp::ToolCallUpdateFields::new().raw_input(
                    serde_json::json!({"command":"rustc --version","sandbox_permissions":mode}),
                ),
            ),
            vec![
                acp::PermissionOption::new(
                    acp::PermissionOptionId::new("allow"),
                    "Allow",
                    acp::PermissionOptionKind::AllowOnce,
                ),
                acp::PermissionOption::new(
                    "reject",
                    "Reject",
                    acp::PermissionOptionKind::RejectOnce,
                ),
            ],
        );
        let response = acp::Client::request_permission(&client, request)
            .await
            .unwrap();
        match response.outcome {
            acp::RequestPermissionOutcome::Selected(selected) => {
                assert_eq!(selected.option_id.0.as_ref(), expected)
            }
            _ => panic!("fixture must select its exact existing option"),
        }
    }
}
