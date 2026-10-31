use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use aster_core::capability::{WorkspaceOperation, WorkspaceOperationAccess};
use aster_core::tools::{Tool, ToolCapabilities, ToolContext, ToolOutput};
use aster_core::workspace::{WorkspaceReadOverlayError, WorkspaceReadOverlayErrorKind};
use agent_client_protocol::{AgentSideConnection, Client, ExtRequest};
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::{mpsc, oneshot};

const TEXT_LIMIT: usize = 64 * 1024;
const CALL_TIMEOUT: Duration = Duration::from_secs(5);

fn read_error(
    kind: WorkspaceReadOverlayErrorKind,
    message: impl Into<String>,
) -> WorkspaceReadOverlayError {
    WorkspaceReadOverlayError::new(kind, message)
}

fn transport_error(message: impl Into<String>) -> WorkspaceReadOverlayError {
    read_error(WorkspaceReadOverlayErrorKind::Transport, message)
}

fn invalid_observation(message: impl Into<String>) -> WorkspaceReadOverlayError {
    read_error(WorkspaceReadOverlayErrorKind::Invalid, message)
}

fn map_acp_read_error(error: agent_client_protocol::Error) -> WorkspaceReadOverlayError {
    let structured = error
        .data
        .as_ref()
        .and_then(|data| data.pointer("/asterDocument/kind"))
        .and_then(Value::as_str)
        .and_then(|kind| kind.parse().ok());
    let kind = structured.unwrap_or(match error.code {
        agent_client_protocol::ErrorCode::ResourceNotFound => {
            WorkspaceReadOverlayErrorKind::NotFound
        }
        agent_client_protocol::ErrorCode::MethodNotFound => {
            WorkspaceReadOverlayErrorKind::Unsupported
        }
        _ => WorkspaceReadOverlayErrorKind::Transport,
    });
    read_error(kind, error.to_string())
}

fn map_read_error(error: anyhow::Error) -> WorkspaceReadOverlayError {
    match error.downcast::<WorkspaceReadOverlayError>() {
        Ok(error) => error,
        Err(error) => transport_error(error.to_string()),
    }
}

fn validate_document_observation(
    document: &Value,
    path: &Path,
    run_id: Option<&str>,
) -> Result<(), WorkspaceReadOverlayError> {
    if document.pointer("/snapshot/diskConflict") == Some(&Value::Bool(true)) {
        return Err(read_error(
            WorkspaceReadOverlayErrorKind::Conflict,
            "document conflict",
        ));
    }
    let invalid = || invalid_observation("invalid document observation");
    let foreign = || {
        read_error(
            WorkspaceReadOverlayErrorKind::Foreign,
            "document observation binding mismatch",
        )
    };
    let path = path.to_str().ok_or_else(invalid)?;
    if document.get("path").and_then(Value::as_str) != Some(path) {
        return Err(foreign());
    }
    let released = document
        .get("released")
        .and_then(Value::as_bool)
        .ok_or_else(invalid)?;
    let actual_run = match document.get("runId") {
        None | Some(Value::Null) => None,
        Some(Value::String(run)) => Some(run.as_str()),
        _ => return Err(invalid()),
    };
    if released != run_id.is_none() || actual_run != run_id {
        return Err(foreign());
    }
    let snapshot = document
        .get("snapshot")
        .and_then(Value::as_object)
        .ok_or_else(invalid)?;
    let document_id = snapshot
        .get("documentId")
        .and_then(Value::as_str)
        .ok_or_else(invalid)?;
    let version = snapshot
        .get("version")
        .and_then(Value::as_u64)
        .ok_or_else(invalid)?;
    let uri = snapshot
        .get("uri")
        .and_then(Value::as_str)
        .ok_or_else(invalid)?;
    let expected_uri = url::Url::from_file_path(path).map_err(|()| invalid())?;
    if uri != expected_uri.as_str() {
        return Err(foreign());
    }
    if document_id.is_empty()
        || document_id.len() > 128
        || version == 0
        || version > 9_007_199_254_740_991
        || !snapshot.get("dirty").is_some_and(Value::is_boolean)
        || !snapshot.get("diskConflict").is_some_and(Value::is_boolean)
    {
        return Err(invalid());
    }
    Ok(())
}

pub(crate) enum CallKind {
    Read {
        session: agent_client_protocol::SessionId,
        path: PathBuf,
        run_id: Option<String>,
    },
    Ext {
        method: &'static str,
        params: Value,
        timeout: Duration,
    },
}

pub(crate) struct Call {
    pub(crate) access: Option<WorkspaceOperationAccess>,
    pub(crate) kind: CallKind,
    pub(crate) reply: oneshot::Sender<Result<HostResponse>>,
}

pub(crate) enum HostResponse {
    Read {
        content: String,
        document: Option<Value>,
    },
    Ext(Value),
}

pub(crate) struct ClientRead {
    pub content: String,
    pub document: Option<Value>,
}

pub(crate) struct DocumentBridge {
    pub(crate) sender: mpsc::Sender<Call>,
    receiver: Mutex<Option<mpsc::Receiver<Call>>>,
    worker: Mutex<Option<tokio::task::JoinHandle<()>>>,
    runs: Mutex<HashMap<String, document_run::RetainedRun>>,
}

impl DocumentBridge {
    pub fn new() -> Arc<Self> {
        let (sender, receiver) = mpsc::channel(16);
        Arc::new(Self {
            sender,
            receiver: Mutex::new(Some(receiver)),
            worker: Mutex::new(None),
            runs: Mutex::new(HashMap::new()),
        })
    }

    pub fn connect(&self, client: Rc<AgentSideConnection>) {
        let receiver = self
            .receiver
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        if let Some(mut receiver) = receiver {
            let worker = tokio::task::spawn_local(async move {
                while let Some(call) = receiver.recv().await {
                    let result = async {
                        if let Some(access) = &call.access {
                            access.check()?;
                        }
                        let response = match call.kind {
                            CallKind::Read {
                                session,
                                path,
                                run_id,
                            } => {
                                let expected_path = path.clone();
                                let expected_run = run_id.clone();
                                let mut request =
                                    agent_client_protocol::ReadTextFileRequest::new(session, path);
                                if let Some(run_id) = run_id {
                                    request = request.meta(serde_json::Map::from_iter([(
                                        "asterDocument".to_owned(),
                                        json!({"runId":run_id}),
                                    )]));
                                }
                                let response = tokio::time::timeout(
                                    CALL_TIMEOUT,
                                    client.read_text_file(request),
                                )
                                .await
                                .map_err(|_| {
                                    read_error(
                                        WorkspaceReadOverlayErrorKind::Timeout,
                                        "document host timeout",
                                    )
                                })?
                                .map_err(map_acp_read_error)?;
                                if response.content.len() > TEXT_LIMIT
                                    || response.content.contains('\0')
                                {
                                    return Err(invalid_observation(
                                        "invalid document observation",
                                    )
                                    .into());
                                }
                                let document = response
                                    .meta
                                    .and_then(|mut meta| meta.remove("asterDocument"));
                                if let Some(document) = &document {
                                    let size = serde_json::to_vec(document)
                                        .map_err(|error| invalid_observation(error.to_string()))?
                                        .len();
                                    if size > TEXT_LIMIT * 8 + 32768 {
                                        return Err(invalid_observation(
                                            "document host response exceeds limit",
                                        )
                                        .into());
                                    }
                                    validate_document_observation(
                                        document,
                                        &expected_path,
                                        expected_run.as_deref(),
                                    )?;
                                }
                                HostResponse::Read {
                                    content: response.content,
                                    document,
                                }
                            }
                            CallKind::Ext {
                                method,
                                params,
                                timeout,
                            } => {
                                let params = serde_json::value::to_raw_value(&params)?;
                                let response = tokio::time::timeout(
                                    timeout,
                                    client.ext_method(ExtRequest::new(method, Arc::from(params))),
                                )
                                .await
                                .context("document host timeout")??;
                                anyhow::ensure!(
                                    response.0.get().len() <= TEXT_LIMIT * 8 + 32768,
                                    "document host response exceeds limit"
                                );
                                HostResponse::Ext(serde_json::from_str(response.0.get())?)
                            }
                        };
                        if let Some(access) = &call.access {
                            access.check()?;
                        }
                        Ok(response)
                    }
                    .await;
                    let _ = call.reply.send(result);
                }
            });
            *self
                .worker
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(worker);
        }
    }

    pub fn stop(&self) {
        if let Some(worker) = self
            .worker
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
        {
            worker.abort();
        }
        self.receiver
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
    }

    pub(crate) async fn read_text(
        &self,
        access: Option<WorkspaceOperationAccess>,
        session: &str,
        path: PathBuf,
        run_id: Option<String>,
    ) -> Result<ClientRead, WorkspaceReadOverlayError> {
        let cancellation = access
            .as_ref()
            .map(WorkspaceOperationAccess::cancellation)
            .transpose()
            .map_err(|error| transport_error(error.to_string()))?;
        let (reply, result) = oneshot::channel();
        let work = async {
            self.sender
                .send(Call {
                    access: access.clone(),
                    kind: CallKind::Read {
                        session: agent_client_protocol::SessionId::new(session.to_owned()),
                        path,
                        run_id,
                    },
                    reply,
                })
                .await
                .map_err(|_| transport_error("document bridge closed"))?;
            let response = result
                .await
                .map_err(|_| transport_error("document host unavailable"))?
                .map_err(map_read_error)?;
            if let Some(access) = &access {
                access
                    .check()
                    .map_err(|error| transport_error(error.to_string()))?;
            }
            let HostResponse::Read { content, document } = response else {
                return Err(invalid_observation(
                    "document host returned the wrong response",
                ));
            };
            Ok(ClientRead { content, document })
        };
        let timeout = || {
            read_error(
                WorkspaceReadOverlayErrorKind::Timeout,
                "document read timeout",
            )
        };
        match cancellation {
            Some(cancellation) => tokio::select! {
                biased;
                _ = cancellation.cancelled() => Err(transport_error("document read cancelled")),
                result = tokio::time::timeout(CALL_TIMEOUT, work) => result.map_err(|_| timeout())?,
            },
            None => tokio::time::timeout(CALL_TIMEOUT, work)
                .await
                .map_err(|_| timeout())?,
        }
    }

    async fn release_run(&self, session: &str, run: &str) -> Result<()> {
        let (reply, result) = oneshot::channel();
        tokio::time::timeout(CALL_TIMEOUT, async {
            self.sender
                .send(Call {
                    access: None,
                    kind: CallKind::Ext {
                        method: "aster/document/release_run",
                        params: json!({"sessionId":session,"runId":run}),
                        timeout: CALL_TIMEOUT,
                    },
                    reply,
                })
                .await
                .context("document bridge closed")?;
            let response = result.await.context("document host unavailable")??;
            let HostResponse::Ext(value) = response else {
                anyhow::bail!("document host returned the wrong response")
            };
            anyhow::ensure!(
                value["released"] == true && value["runId"] == run,
                "document host did not acknowledge Run release"
            );
            Ok(())
        })
        .await
        .context("document Run release timeout")?
    }

    pub fn tool(self: &Arc<Self>, workspace: &Path, session: &str) -> Result<Arc<dyn Tool>> {
        Ok(Arc::new(DocumentReadTool {
            bridge: self.clone(),
            workspace: crate::plugin_workspace::WorkspaceBinding::new(workspace)?,
            session: session.to_owned(),
        }))
    }

    pub(crate) async fn process_source(
        &self,
        access: WorkspaceOperationAccess,
        session: &str,
        path: &Path,
        digest: &str,
        first: usize,
        count: usize,
    ) -> Result<Value> {
        let cancellation = access.cancellation()?;
        let (reply, result) = oneshot::channel();
        let work = async {
            access.check()?;
            self.sender.send(Call { access: Some(access.clone()), kind: CallKind::Ext {
                method: "aster/document/process",
                params: json!({"sessionId":session,"path":path,"sourceSha256":digest,"startPage":first,"pageCount":count}),
                timeout: Duration::from_secs(125),
            }, reply }).await.context("document bridge closed")?;
            let HostResponse::Ext(value) = result.await.context("document host unavailable")??
            else {
                anyhow::bail!("invalid document processing response");
            };
            access.check()?;
            anyhow::ensure!(
                value["source_sha256"] == digest
                    && value["local_only"] == true
                    && value["entries"].is_array(),
                "document processing source mismatch"
            );
            anyhow::ensure!(
                serde_json::to_vec(&value)?.len() <= TEXT_LIMIT,
                "document processing result exceeds 64 KiB"
            );
            Ok(value)
        };
        tokio::select! {
            biased;
            _ = cancellation.cancelled() => anyhow::bail!("document processing cancelled"),
            result = tokio::time::timeout(Duration::from_secs(125), work) => result.context("document processing timed out")?,
        }
    }

    pub(crate) async fn search_workspace_wikis(
        &self,
        access: WorkspaceOperationAccess,
        session: &str,
        query: &str,
        scope: &str,
        cursor: Option<&str>,
    ) -> Result<Value> {
        self.search_saved_workspaces(
            "aster/wiki/search-workspaces",
            access,
            session,
            query,
            scope,
            cursor,
        )
        .await
    }

    pub(crate) async fn search_workspace_memories(
        &self,
        access: WorkspaceOperationAccess,
        session: &str,
        query: &str,
        scope: &str,
        cursor: Option<&str>,
    ) -> Result<Value> {
        self.search_saved_workspaces(
            "aster/memory/search-workspaces",
            access,
            session,
            query,
            scope,
            cursor,
        )
        .await
    }

    async fn search_saved_workspaces(
        &self,
        method: &'static str,
        access: WorkspaceOperationAccess,
        session: &str,
        query: &str,
        scope: &str,
        cursor: Option<&str>,
    ) -> Result<Value> {
        let cancellation = access.cancellation()?;
        let (reply, result) = oneshot::channel();
        let work = async {
            access.check()?;
            self.sender.send(Call {access:Some(access.clone()),kind:CallKind::Ext {
                method,
                params:json!({"sessionId":session,"query":query,"scope":scope,"cursor":cursor}),
                timeout:CALL_TIMEOUT,
            },reply}).await.context("workspace search host unavailable")?;
            let HostResponse::Ext(value) = result
                .await
                .context("workspace search host unavailable")??
            else {
                anyhow::bail!("invalid workspace search response")
            };
            access.check()?;
            anyhow::ensure!(
                serde_json::to_vec(&value)?.len() <= TEXT_LIMIT,
                "workspace search response exceeds 64 KiB"
            );
            Ok(value)
        };
        tokio::select! {
            biased;
            _=cancellation.cancelled()=>anyhow::bail!("workspace search cancelled"),
            value=tokio::time::timeout(CALL_TIMEOUT,work)=>value.context("workspace search timed out")?,
        }
    }
}

struct DocumentReadTool {
    bridge: Arc<DocumentBridge>,
    workspace: crate::plugin_workspace::WorkspaceBinding,
    session: String,
}

impl Drop for DocumentBridge {
    fn drop(&mut self) {
        self.stop();
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    file_path: String,
    #[serde(default)]
    retain_for_run: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Response {
    path: String,
    snapshot: Snapshot,
    released: bool,
    #[serde(default, rename = "runId")]
    run_id: Option<String>,
    #[serde(default)]
    diagnostics: Vec<document_diagnostics::Publication>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Snapshot {
    document_id: String,
    uri: String,
    version: u64,
    text: String,
    dirty: bool,
    disk_conflict: bool,
}

#[async_trait]
impl Tool for DocumentReadTool {
    fn name(&self) -> &str {
        "studio_document_read"
    }
    fn description(&self) -> &str {
        "Read UTF-8 plain text, including unsaved text, version and advisory diagnostics from the shared editor document model. This tool does not parse Office or PDF files; use office_read when the host exposes it, with an absolute file path or workspace-relative path for persisted DOCX/XLSX/PPTX sources. Report unavailable Office support when it is absent. PDF text, page-image and OCR capabilities require a separately admitted tool that explicitly provides them. An empty published set does not verify a document. Set retain_for_run to keep its model identity until Run cleanup."
    }
    fn parameters(&self) -> Value {
        json!({"type":"object","properties":{"file_path":{"type":"string","description":"UTF-8 text file path, absolute or relative to the current workspace; not a document ID."},"retain_for_run":{"type":"boolean","description":"Keep this model pinned until the current Run releases its resources."}},"required":["file_path"],"additionalProperties":false})
    }
    fn capabilities(&self, _: &Value) -> ToolCapabilities {
        // Fencing a cancelled RPC does not certify settlement of native blocking I/O.
        ToolCapabilities {
            read_only: true,
            idempotent: true,
            ..ToolCapabilities::conservative()
        }
    }
    async fn execute(&self, args: &Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let result: Result<ToolOutput> = async {
            self.workspace.validate(&ctx.workspace)?;
            let access = ctx.workspace_operation(WorkspaceOperation::Read)?;
            let input: Input = serde_json::from_value(args.clone())?;
            anyhow::ensure!(!input.file_path.is_empty() && input.file_path.len() <= 4096 && !input.file_path.contains('\0'), "invalid document path");
            let retain_for_run = input.retain_for_run;
            let input = PathBuf::from(input.file_path);
            anyhow::ensure!(!input.components().any(|part| part == Component::ParentDir), "document not found");
            let path = if input.is_absolute() { input } else { self.workspace.root().join(input) };
            anyhow::ensure!(path.starts_with(self.workspace.root()), "document not found");
            let path = path.to_str().context("invalid document path")?;
            // A replaced root must stop this Turn before another model observes its result.
            ctx.register_capability_effect(self.workspace.clone())?;
            let run = if retain_for_run { Some(self.bridge.retain_run(ctx, &self.session)?) } else { None };
            let read = self.bridge.read_text(Some(access.clone()), &self.session, PathBuf::from(path), run.clone()).await?;
            self.workspace.validate(&ctx.workspace)?;
            let mut value = read.document.context("document host omitted document metadata")?;
            let snapshot = value.get_mut("snapshot").and_then(Value::as_object_mut).context("invalid document observation")?;
            snapshot.insert("text".to_owned(), json!(read.content));
            let response: Response = serde_json::from_value(value)?;
            anyhow::ensure!(response.released == run.is_none() && response.run_id == run && response.path == path, "document host did not acknowledge release or binding");
            let snapshot = response.snapshot;
            anyhow::ensure!(!snapshot.disk_conflict, "document conflict");
            let canonical = std::fs::canonicalize(path).context("document not found")?;
            anyhow::ensure!(canonical.starts_with(self.workspace.root()), "document not found");
            let uri = url::Url::from_file_path(canonical).map_err(|()| anyhow::anyhow!("invalid document URI"))?;
            anyhow::ensure!(snapshot.uri == uri.as_str(), "document URI binding mismatch");
            anyhow::ensure!(!snapshot.document_id.is_empty() && snapshot.document_id.len() <= 128 && snapshot.uri.starts_with("file:") && snapshot.uri.len() <= 16384 && snapshot.version > 0 && snapshot.version <= 9_007_199_254_740_991 && snapshot.text.len() <= TEXT_LIMIT && !snapshot.text.contains('\0'), "invalid document observation");
            document_diagnostics::validate(&response.diagnostics, &snapshot)?;
            access.check()?;
            Ok(ToolOutput::success(json!({"documentId":snapshot.document_id,"uri":snapshot.uri,"version":snapshot.version,"text":snapshot.text,"dirty":snapshot.dirty,"diskConflict":snapshot.disk_conflict,"retainedForRun":retain_for_run,"diagnostics":response.diagnostics,"diagnosticsComplete":false}).to_string()))
        }.await;
        match result {
            Ok(output) => Ok(output),
            Err(error) => Ok(ToolOutput::error(
                error.to_string().chars().take(1024).collect::<String>(),
            )),
        }
    }
}

#[cfg(test)]
#[path = "document_read_tests.rs"]
mod tests;

#[path = "document_run.rs"]
mod document_run;

#[path = "document_diagnostics.rs"]
mod document_diagnostics;

#[cfg(test)]
#[path = "document_run_tests.rs"]
mod run_tests;
