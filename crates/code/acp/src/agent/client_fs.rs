//! `fs/read_text_file` overlay: Core and rich document reads share one transport.

use super::AsterCodeAgent;
use aster_core::workspace::{WorkspacePath, WorkspaceReadOverlay, WorkspaceReadOverlayError};
use std::path::PathBuf;
use std::sync::Arc;

struct ClientReadOverlay {
    root: PathBuf,
    session: String,
    bridge: Arc<crate::document_read::DocumentBridge>,
}

#[async_trait::async_trait]
impl WorkspaceReadOverlay for ClientReadOverlay {
    async fn read_text(&self, path: &WorkspacePath) -> Option<String> {
        self.read_text_result(path).await.ok().flatten()
    }

    async fn read_text_result(
        &self,
        path: &WorkspacePath,
    ) -> Result<Option<String>, WorkspaceReadOverlayError> {
        self.bridge
            .read_text(None, &self.session, self.root.join(path.as_str()), None)
            .await
            .map(|read| Some(read.content))
    }
}

impl AsterCodeAgent {
    /// Enable the overlay only for a registered Studio document host that
    /// declared `fs.readTextFile`. Must run on the ACP `LocalSet`.
    pub(super) fn with_client_reads(
        &self,
        options: aster_core::SessionOptions,
        session_id: &str,
        workspace: &std::path::Path,
    ) -> aster_core::SessionOptions {
        if !self.client_reads_files.get() || self.client.borrow().is_none() {
            return options;
        }
        let Some(bridge) = self.document_bridge() else {
            return options;
        };
        options.with_workspace_read_overlay(Arc::new(ClientReadOverlay {
            root: workspace.to_path_buf(),
            session: session_id.to_owned(),
            bridge,
        }))
    }
}
