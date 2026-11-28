//! Core types and traits for Aster tool system

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Tool definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// Tool execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
}

/// Resolve a path relative to workspace, ensuring it's within bounds
pub fn resolve_path(workspace: &Path, path: &str) -> Result<PathBuf, String> {
    resolve_read_path(workspace, path).map_err(|error| error.to_string())
}

/// A read path could not be resolved or resolved outside the workspace.
#[derive(Debug, thiserror::Error)]
pub enum ResolveReadPathError {
    #[error("Failed to resolve path: {0}")]
    Canonicalize(#[source] std::io::Error),
    #[error("Path escapes workspace: {0}")]
    OutsideWorkspace(String),
}

impl ResolveReadPathError {
    /// Whether the OS found no file at the requested address.
    pub fn is_not_found(&self) -> bool {
        matches!(self, Self::Canonicalize(error) if error.kind() == std::io::ErrorKind::NotFound)
    }
}

/// Resolve an existing workspace path without erasing the I/O error kind.
///
/// The workspace root must already be canonical. A path resolving outside it
/// is rejected, regardless of how the caller spelled the path.
pub fn resolve_read_path(workspace: &Path, path: &str) -> Result<PathBuf, ResolveReadPathError> {
    let resolved = workspace.join(path);

    // Canonicalize to resolve .. and symlinks
    let canonical = resolved
        .canonicalize()
        .map_err(ResolveReadPathError::Canonicalize)?;

    // Ensure the path is within workspace
    if !canonical.starts_with(workspace) {
        return Err(ResolveReadPathError::OutsideWorkspace(path.to_owned()));
    }

    Ok(canonical)
}

/// Resolve a path for writing (allows non-existent files)
pub fn resolve_path_for_write(workspace: &Path, path: &str) -> Result<PathBuf, String> {
    let resolved = workspace.join(path);

    // Check parent directory exists and is within workspace
    if let Some(parent) = resolved.parent() {
        if parent.exists() {
            let canonical = parent
                .canonicalize()
                .map_err(|e| format!("Failed to resolve parent: {}", e))?;

            if !canonical.starts_with(workspace) {
                return Err(format!("Path escapes workspace: {}", path));
            }

            // Return the resolved path with the filename
            if let Some(filename) = resolved.file_name() {
                return Ok(canonical.join(filename));
            }
        }
    }

    // If parent doesn't exist or path is invalid, return error
    Err(format!(
        "Invalid path or parent directory doesn't exist: {}",
        path
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_tool_creation() {
        let tool = Tool {
            name: "test".to_string(),
            description: "Test tool".to_string(),
            parameters: serde_json::json!({}),
        };
        assert_eq!(tool.name, "test");
    }

    #[test]
    fn test_resolve_path() {
        let temp = std::env::temp_dir()
            .canonicalize()
            .expect("temp dir should be canonicalizable");
        let workspace = temp.join("test_workspace_common");
        fs::create_dir_all(&workspace).unwrap();

        let result = resolve_path(&workspace, ".");
        assert!(result.is_ok());

        fs::remove_dir_all(&workspace).ok();
    }
}
