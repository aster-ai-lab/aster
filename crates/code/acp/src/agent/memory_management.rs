use super::*;
use aster_core::durable_memory_vec::management::{
    ManagedMemory, MemoryInspectionLayer, MemoryManagementRequest,
};

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    workspace_root: PathBuf,
    session_id: Option<String>,
    request: MemoryManagementRequest,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SnapshotRequest {
    workspace_root: PathBuf,
    session_id: Option<String>,
    layer: MemoryInspectionLayer,
    #[serde(default)]
    count_only: bool,
}

impl AsterCodeAgent {
    async fn memory_scope(
        &self,
        workspace_root: &std::path::Path,
        session_id: Option<&str>,
    ) -> acp::Result<(
        PathBuf,
        Arc<aster_core::durable_memory_vec::LazyFileMemoryStore>,
    )> {
        let missing = || acp::Error::resource_not_found(None);
        let workspace = if let Some(id) = session_id {
            if id.is_empty() || id.len() > 4096 {
                return Err(missing());
            }
            self.sessions
                .lock()
                .await
                .get(id)
                .map(|live| live.workspace.clone())
                .ok_or_else(missing)?
        } else {
            self.launch.workspace.clone()
        };
        if session_id.is_some() && workspace_root != workspace {
            return Err(missing());
        }
        let workspace = workspace.canonicalize().map_err(|_| missing())?;
        if workspace_root != workspace {
            return Err(missing());
        }
        let store = if self.launch.workspace.canonicalize().ok().as_ref() == Some(&workspace) {
            Arc::clone(&self.managed_memory)
        } else {
            Arc::new(aster_core::durable_memory_vec::LazyFileMemoryStore::new(
                session_memory_dir(&workspace, &self.launch.config),
            ))
        };
        Ok((workspace, store))
    }

    pub(super) async fn handle_memory_snapshot(
        &self,
        args: &acp::ExtRequest,
    ) -> acp::Result<acp::ExtResponse> {
        let invalid = || acp::Error::invalid_params().data("invalid live memory snapshot request");
        if args.params.get().len() > 8192 {
            return Err(invalid());
        }
        let request: SnapshotRequest =
            serde_json::from_str(args.params.get()).map_err(|_| invalid())?;
        let (workspace, store) = self
            .memory_scope(&request.workspace_root, request.session_id.as_deref())
            .await
            .map_err(|_| invalid())?;
        if request.layer == MemoryInspectionLayer::Durable && !request.count_only {
            return Err(invalid());
        }
        if request.count_only && request.layer == MemoryInspectionLayer::Durable {
            use aster_durable_memory::MemoryStore as _;
            let count = store
                .count()
                .await
                .map_err(|error| acp::Error::internal_error().data(error.to_string()))?;
            let body = serde_json::json!({"workspace_root":workspace,"memory_directory":session_memory_dir(&workspace, &self.launch.config),"layer":request.layer,"count":count});
            return Ok(acp::ExtResponse::new(
                serde_json::value::to_raw_value(&body)
                    .map_err(|_| invalid())?
                    .into(),
            ));
        }
        let sessions: Vec<_> = self
            .sessions
            .lock()
            .await
            .iter()
            .filter(|(id, live)| {
                live.workspace == workspace
                    && request
                        .session_id
                        .as_ref()
                        .is_none_or(|requested| requested == *id)
            })
            .filter_map(|(id, live)| {
                live.session
                    .memory()
                    .map(|memory| (id.clone(), Arc::clone(memory)))
            })
            .collect();
        let mut items = Vec::new();
        let mut bytes = 0;
        let mut count: usize = 0;
        for (id, memory) in sessions {
            if request.count_only {
                count = count
                    .checked_add(match request.layer {
                        MemoryInspectionLayer::Working => memory.working_count().await,
                        MemoryInspectionLayer::Session => memory.short_term_count().await,
                        MemoryInspectionLayer::Durable => return Err(invalid()),
                    })
                    .ok_or_else(invalid)?;
                continue;
            }
            let records = match request.layer {
                MemoryInspectionLayer::Working => memory.get_working().await,
                MemoryInspectionLayer::Session => memory.get_short_term().await,
                MemoryInspectionLayer::Durable => return Err(invalid()),
            };
            for record in records {
                let item = ManagedMemory::from_session_item(&workspace, &id, request.layer, record)
                    .map_err(|error| acp::Error::internal_error().data(error.to_string()))?;
                bytes += serde_json::to_vec(&item).map_err(|_| invalid())?.len();
                if items.len() >= 10_000 || bytes > 16 * 1024 * 1024 {
                    return Err(acp::Error::internal_error()
                        .data("live memory snapshot exceeds inspection bounds"));
                }
                items.push(item);
            }
        }
        if request.count_only {
            let body = serde_json::json!({"workspace_root":workspace,"memory_directory":session_memory_dir(&workspace, &self.launch.config),"layer":request.layer,"count":count});
            return Ok(acp::ExtResponse::new(
                serde_json::value::to_raw_value(&body)
                    .map_err(|_| invalid())?
                    .into(),
            ));
        }
        items.sort_by(|a, b| a.id.cmp(&b.id));
        let body = serde_json::json!({"workspace_root": workspace, "memory_directory": session_memory_dir(&workspace, &self.launch.config), "layer": request.layer, "items": items});
        let raw = serde_json::value::to_raw_value(&body).map_err(|_| invalid())?;
        Ok(acp::ExtResponse::new(raw.into()))
    }

    pub(super) async fn handle_memory_management(
        &self,
        args: &acp::ExtRequest,
    ) -> acp::Result<acp::ExtResponse> {
        let invalid = |message: String| acp::Error::invalid_params().data(message);
        if args.params.get().len() > 512 * 1024 {
            return Err(invalid("memory request exceeds its envelope".into()));
        }
        let request: Request =
            serde_json::from_str(args.params.get()).map_err(|error| invalid(error.to_string()))?;
        request
            .request
            .validate()
            .map_err(|error| invalid(error.to_string()))?;
        let (workspace, store) = self
            .memory_scope(&request.workspace_root, request.session_id.as_deref())
            .await?;
        let runtime = tokio::runtime::Handle::current();
        let result =
            tokio::task::spawn_blocking(move || runtime.block_on(store.manage(request.request)))
                .await
                .map_err(|error| acp::Error::internal_error().data(error.to_string()))?;
        let body = serde_json::json!({ "workspace_root": workspace, "memory_directory": session_memory_dir(&workspace, &self.launch.config), "outcome": result });
        let raw = serde_json::value::to_raw_value(&body)
            .map_err(|error| acp::Error::internal_error().data(error.to_string()))?;
        Ok(acp::ExtResponse::new(raw.into()))
    }

    /// `aster/memory/flush`: pager `/flush`. Core durable memory is already
    /// file-backed; acknowledge the session and report a flushed disposition
    /// compatible with the pager's `MemoryFlushResponse`.
    pub(super) async fn handle_memory_flush(
        &self,
        args: &acp::ExtRequest,
    ) -> acp::Result<acp::ExtResponse> {
        let session_id = memory_command_session_id(args)?;
        self.require_live_session(&session_id).await?;
        ext_json(serde_json::json!({
            "flushed": true,
            "disposition": "flushed",
        }))
    }

    /// `aster/memory/dream`: pager `/dream`. Aster Core has no observation
    /// consolidation queue; report `no_work` so the pager shows a typed outcome.
    pub(super) async fn handle_memory_dream(
        &self,
        args: &acp::ExtRequest,
    ) -> acp::Result<acp::ExtResponse> {
        let session_id = memory_command_session_id(args)?;
        self.require_live_session(&session_id).await?;
        ext_json(serde_json::json!({
            "disposition": "no_work",
            "observation_count": 0,
            "topics_affected": 0,
        }))
    }

    async fn require_live_session(&self, session_id: &str) -> acp::Result<()> {
        if session_id.is_empty() || session_id.len() > 4096 {
            return Err(acp::Error::invalid_params().data("memory command: session_id"));
        }
        let sessions = self.sessions.lock().await;
        if sessions.contains_key(session_id) {
            Ok(())
        } else {
            Err(acp::Error::invalid_params().data("unknown session id"))
        }
    }
}

#[derive(serde::Deserialize)]
struct MemoryCommandParams {
    session_id: String,
}

fn memory_command_session_id(args: &acp::ExtRequest) -> acp::Result<String> {
    if args.params.get().len() > 8192 {
        return Err(acp::Error::invalid_params().data("memory command params too large"));
    }
    let params: MemoryCommandParams = serde_json::from_str(args.params.get()).map_err(|error| {
        acp::Error::invalid_params().data(format!("memory command params: {error}"))
    })?;
    Ok(params.session_id)
}

fn ext_json(value: serde_json::Value) -> acp::Result<acp::ExtResponse> {
    let raw = serde_json::value::to_raw_value(&value)
        .map_err(|error| acp::Error::internal_error().data(error.to_string()))?;
    Ok(acp::ExtResponse::new(raw.into()))
}

#[cfg(test)]
#[path = "memory_management_tests.rs"]
mod tests;
