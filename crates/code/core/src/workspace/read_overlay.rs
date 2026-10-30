//! Client-held text that takes precedence over the workspace backend on reads.
//!
//! An editor host (for example an ACP client) can hold unsaved buffer text
//! for a file. The overlay lets text reads see that text while writes,
//! listings, search, and git keep using the session's own backend.

use super::{
    WorkspaceDirEntry, WorkspaceError, WorkspaceFileSystem, WorkspacePath, WorkspaceResult,
    WorkspaceTextRange, WorkspaceTextReader, WorkspaceWriteOutcome,
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

/// Closed set of overlay outcomes that determines whether backend fallback is safe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceReadOverlayErrorKind {
    NotFound,
    Unsupported,
    Conflict,
    Timeout,
    Foreign,
    Transport,
    Invalid,
}

impl WorkspaceReadOverlayErrorKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotFound => "not_found",
            Self::Unsupported => "unsupported",
            Self::Conflict => "conflict",
            Self::Timeout => "timeout",
            Self::Foreign => "foreign",
            Self::Transport => "transport",
            Self::Invalid => "invalid",
        }
    }

    pub const fn allows_backend_fallback(self) -> bool {
        matches!(self, Self::NotFound | Self::Unsupported)
    }
}

impl std::fmt::Display for WorkspaceReadOverlayErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for WorkspaceReadOverlayErrorKind {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "not_found" => Ok(Self::NotFound),
            "unsupported" => Ok(Self::Unsupported),
            "conflict" => Ok(Self::Conflict),
            "timeout" => Ok(Self::Timeout),
            "foreign" => Ok(Self::Foreign),
            "transport" => Ok(Self::Transport),
            "invalid" => Ok(Self::Invalid),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("workspace read overlay {kind}: {message}")]
pub struct WorkspaceReadOverlayError {
    kind: WorkspaceReadOverlayErrorKind,
    message: String,
}

impl WorkspaceReadOverlayError {
    pub fn new(kind: WorkspaceReadOverlayErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub const fn kind(&self) -> WorkspaceReadOverlayErrorKind {
        self.kind
    }
}

impl From<WorkspaceReadOverlayError> for WorkspaceError {
    fn from(error: WorkspaceReadOverlayError) -> Self {
        Self::Backend(anyhow::Error::new(error))
    }
}

/// Immutable client text supplied when an SDK session is created.
/// Dynamic editor hosts can implement [`WorkspaceReadOverlay`] directly.
pub struct WorkspaceReadOverlaySnapshot(HashMap<String, String>);

impl WorkspaceReadOverlaySnapshot {
    /// Validate workspace-relative paths and the same per-file text bound as local reads.
    pub fn new(files: HashMap<String, String>) -> anyhow::Result<Self> {
        use super::WorkspacePathResolver;
        let mut normalized = HashMap::with_capacity(files.len());
        for (path, text) in files {
            let path = super::VirtualPathResolver.normalize(&path)?;
            anyhow::ensure!(
                !path.is_root(),
                "workspace read overlay requires a file path"
            );
            anyhow::ensure!(
                text.len() as u64 <= super::local::MAX_TEXT_FILE_BYTES,
                "workspace read overlay text exceeds the 16 MiB file limit"
            );
            anyhow::ensure!(
                normalized.insert(path.as_str().to_owned(), text).is_none(),
                "workspace read overlay contains duplicate normalized paths"
            );
        }
        Ok(Self(normalized))
    }
}

#[async_trait]
impl WorkspaceReadOverlay for WorkspaceReadOverlaySnapshot {
    async fn read_text(&self, path: &WorkspacePath) -> Option<String> {
        self.0.get(path.as_str()).cloned()
    }
}

/// Source of client-held file text for a session.
#[async_trait]
pub trait WorkspaceReadOverlay: Send + Sync {
    /// Compatibility adapter for existing overlay implementations and callers.
    /// Production workspace reads use [`Self::read_text_result`].
    async fn read_text(&self, path: &WorkspacePath) -> Option<String>;

    /// Return client text, an explicit fallback outcome, or a typed failure.
    /// Only `NotFound` and `Unsupported` may fall back to the backend.
    async fn read_text_result(
        &self,
        path: &WorkspacePath,
    ) -> Result<Option<String>, WorkspaceReadOverlayError> {
        Ok(self.read_text(path).await)
    }
}

impl WorkspaceTextRange {
    /// Slice complete text into the range a streaming reader would return.
    pub fn from_text(content: &str, offset: usize, limit: usize) -> Self {
        let lines = content.lines().collect::<Vec<_>>();
        if offset >= lines.len() {
            return Self {
                lines: Vec::new(),
                next_offset: None,
                eof: true,
                total_lines: Some(lines.len()),
            };
        }
        let end = offset.saturating_add(limit).min(lines.len());
        Self {
            lines: lines[offset..end]
                .iter()
                .map(|line| (*line).to_string())
                .collect(),
            next_offset: (end < lines.len()).then_some(end),
            eof: end == lines.len(),
            total_lines: Some(lines.len()),
        }
    }
}

pub(super) struct OverlayFileSystem {
    pub(super) overlay: Arc<dyn WorkspaceReadOverlay>,
    pub(super) inner: Arc<dyn WorkspaceFileSystem>,
}

#[async_trait]
impl WorkspaceFileSystem for OverlayFileSystem {
    async fn read_text(&self, path: &WorkspacePath) -> WorkspaceResult<String> {
        match self.overlay.read_text_result(path).await {
            Ok(Some(content)) => Ok(content),
            Ok(None) => self.inner.read_text(path).await,
            Err(error) if error.kind().allows_backend_fallback() => {
                self.inner.read_text(path).await
            }
            Err(error) => Err(error.into()),
        }
    }

    async fn write_text(
        &self,
        path: &WorkspacePath,
        content: &str,
    ) -> WorkspaceResult<WorkspaceWriteOutcome> {
        self.inner.write_text(path, content).await
    }

    async fn list_dir(&self, path: &WorkspacePath) -> WorkspaceResult<Vec<WorkspaceDirEntry>> {
        self.inner.list_dir(path).await
    }
}

pub(super) struct OverlayTextReader {
    pub(super) overlay: Arc<dyn WorkspaceReadOverlay>,
    pub(super) inner: Option<Arc<dyn WorkspaceTextReader>>,
    pub(super) fs: Arc<dyn WorkspaceFileSystem>,
}

#[async_trait]
impl WorkspaceTextReader for OverlayTextReader {
    async fn read_text_range(
        &self,
        path: &WorkspacePath,
        offset: usize,
        limit: usize,
    ) -> WorkspaceResult<WorkspaceTextRange> {
        match self.overlay.read_text_result(path).await {
            Ok(Some(content)) => Ok(WorkspaceTextRange::from_text(&content, offset, limit)),
            Ok(None) => self.read_backend_range(path, offset, limit).await,
            Err(error) if error.kind().allows_backend_fallback() => {
                self.read_backend_range(path, offset, limit).await
            }
            Err(error) => Err(error.into()),
        }
    }
}

impl OverlayTextReader {
    async fn read_backend_range(
        &self,
        path: &WorkspacePath,
        offset: usize,
        limit: usize,
    ) -> WorkspaceResult<WorkspaceTextRange> {
        match &self.inner {
            Some(reader) => reader.read_text_range(path, offset, limit).await,
            None => {
                let content = self.fs.read_text(path).await?;
                Ok(WorkspaceTextRange::from_text(&content, offset, limit))
            }
        }
    }
}

#[cfg(test)]
#[path = "read_overlay/tests.rs"]
mod tests;
