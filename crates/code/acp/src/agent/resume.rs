//! Recovery admission precedes model, MCP, document and filesystem activation.

use std::path::{Path, PathBuf};

use agent_client_protocol as acp;

use super::{workspace_scope::canonical_workspace, A3sCodeAgent};

impl A3sCodeAgent {
    pub(super) async fn resume_snapshot_store(
        directory: &Path,
        session_id: &str,
        workspace: &Path,
    ) -> acp::Result<std::sync::Arc<dyn a3s_code_core::store::SessionStore>> {
        use a3s_code_core::store::{FileSessionStore, SessionStore};
        let failed = |error: anyhow::Error| {
            acp::Error::internal_error().data(format!("session snapshot recovery: {error}"))
        };
        if !directory
            .try_exists()
            .map_err(|error: std::io::Error| failed(error.into()))?
        {
            return Err(acp::Error::internal_error()
                .data("session snapshot recovery: snapshot store is missing"));
        }
        let store = std::sync::Arc::new(FileSessionStore::new(directory).await.map_err(failed)?);
        let snapshot = store
            .load_snapshot(session_id)
            .await
            .map_err(failed)?
            .ok_or_else(|| {
                acp::Error::internal_error().data(format!(
                    "session snapshot recovery: snapshot is missing for session {session_id}"
                ))
            })?;
        snapshot.validate_for_session(session_id).map_err(failed)?;
        canonical_workspace(
            workspace,
            Some(Path::new(&snapshot.session.config.workspace)),
        )
        .map_err(|_| {
            acp::Error::invalid_params().data("session snapshot recovery: foreign workspace")
        })?;
        Ok(store)
    }

    pub(super) async fn admit_resume(&self, session_id: &str, cwd: &Path) -> acp::Result<PathBuf> {
        if session_id.is_empty() || session_id.len() > 4096 {
            return Err(acp::Error::invalid_params().data("invalid session identity"));
        }
        let session_id = session_id.to_owned();
        let bound = self
            .sessions
            .lock()
            .await
            .get(&session_id)
            .map(|live| live.session.workspace().to_owned());
        let launch = self.launch.workspace.clone();
        let cwd = cwd.to_owned();
        let permit = self
            .catalog_queries
            .clone()
            .try_acquire_owned()
            .map_err(|_| {
                acp::Error::invalid_params().data("session catalog query already in progress")
            })?;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let workspace = if let Some(bound) = bound {
                let actual = cwd
                    .canonicalize()
                    .map_err(|_| acp::Error::resource_not_found(None))?;
                if actual != bound {
                    return Err(acp::Error::resource_not_found(None));
                }
                actual
            } else {
                canonical_workspace(&launch, Some(&cwd))?
            };
            let entry =
                a3s_code_core::session_folder::resolve_workspace_session(&workspace, &session_id)
                    .map_err(|_| acp::Error::resource_not_found(None))?;
            if entry.is_none() {
                return Err(acp::Error::resource_not_found(None));
            }
            Ok(workspace)
        })
        .await
        .map_err(|_| acp::Error::internal_error().data("session recovery admission failed"))?
    }
}
