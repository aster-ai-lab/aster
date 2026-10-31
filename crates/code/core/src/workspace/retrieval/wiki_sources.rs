//! Read-only compile inputs from the host's existing manifest and scope receipt.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::UNIX_EPOCH;

use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt, OpenOptionsSyncExt};
use cap_std::fs::{Dir, OpenOptions};
use serde::Serialize;
use sha2::{Digest, Sha256};
use tokio_util::sync::CancellationToken;

use super::chunk::{chunk_file_with_strategy, ChunkFileRequest};
use super::{
    ChunkingConfig, WorkspaceChunk, WorkspaceChunkingStrategy, WorkspaceEligibilityPolicy,
};
use crate::workspace::{
    LocalWorkspaceBackend, LocalWorkspaceFile, LocalWorkspaceManifestSnapshot, WorkspacePath,
};

const MAX_FILES: usize = 512;
const MAX_BYTES: usize = 8 * 1024 * 1024;
const MAX_MANIFEST_FILES: usize = 100_000;
const MAX_SKIP_EXAMPLES: usize = 16;

macro_rules! source_enum {
    ($name:ident { $($variant:ident => $wire:literal),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize)]
        pub enum $name { $(#[serde(rename = $wire)] $variant),+ }
        impl $name {
            pub fn parse(value: &str) -> io::Result<Self> {
                match value {
                    $($wire => Ok(Self::$variant),)+
                    _ => Err(invalid(format!("unknown {}: {value}", stringify!($name)))),
                }
            }
            pub fn as_str(self) -> &'static str {
                match self { $(Self::$variant => $wire),+ }
            }
        }
    };
}

source_enum!(WorkspaceWikiSourceFormat { Text => "text", Document => "document" });
source_enum!(WorkspaceWikiSourceSkipReason {
    Excluded => "excluded", Unsupported => "unsupported", Empty => "empty",
    TooLarge => "too_large", Budget => "budget", Unavailable => "unavailable", Changed => "changed"
});

/// Host budgets may narrow these fixed compile ceilings, never expand them.
#[derive(Clone, Copy, Debug)]
pub struct WorkspaceWikiSourceLimits {
    pub max_files: usize,
    pub max_file_bytes: usize,
    pub max_total_bytes: usize,
}

impl Default for WorkspaceWikiSourceLimits {
    fn default() -> Self {
        Self {
            max_files: MAX_FILES,
            max_file_bytes: MAX_BYTES,
            max_total_bytes: MAX_BYTES,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct WorkspaceWikiSourceCoverage {
    pub included_files: usize,
    pub included_bytes: usize,
    pub skipped: BTreeMap<WorkspaceWikiSourceSkipReason, usize>,
    /// Bounded examples; counts above cover every entry in the supplied manifest.
    pub examples: Vec<WorkspaceWikiSourceSkip>,
}

#[derive(Clone, Debug, Serialize)]
pub struct WorkspaceWikiSourceSkip {
    pub path: String,
    pub reason: WorkspaceWikiSourceSkipReason,
}

impl WorkspaceWikiSourceCoverage {
    fn skip(&mut self, path: &str, reason: WorkspaceWikiSourceSkipReason) {
        *self.skipped.entry(reason).or_default() += 1;
        if self.examples.len() < MAX_SKIP_EXAMPLES {
            self.examples.push(WorkspaceWikiSourceSkip {
                path: path.into(),
                reason,
            });
        }
    }
}

/// Immutable admitted bytes. Debug output deliberately omits source contents.
#[derive(Clone)]
pub struct WorkspaceWikiSourceFile {
    path: String,
    generation: String,
    format: WorkspaceWikiSourceFormat,
    bytes: Vec<u8>,
    chunks: Vec<Arc<WorkspaceChunk>>,
}

impl std::fmt::Debug for WorkspaceWikiSourceFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkspaceWikiSourceFile")
            .field("path", &self.path)
            .field("generation", &self.generation)
            .field("format", &self.format)
            .field("bytes", &self.bytes.len())
            .field("chunks", &self.chunks.len())
            .finish()
    }
}

impl WorkspaceWikiSourceFile {
    pub fn path(&self) -> &str {
        &self.path
    }
    pub fn generation(&self) -> &str {
        &self.generation
    }
    pub fn format(&self) -> WorkspaceWikiSourceFormat {
        self.format
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    /// Original retrieval chunks retain exact UTF-8 byte and source-line anchors.
    pub fn chunks(&self) -> &[Arc<WorkspaceChunk>] {
        &self.chunks
    }
}

/// A compile-input projection, not a new scanner, index or permission authority.
#[derive(Debug)]
pub struct WorkspaceWikiSources {
    root: PathBuf,
    manifest_version: u64,
    generation: String,
    files: Vec<WorkspaceWikiSourceFile>,
    coverage: WorkspaceWikiSourceCoverage,
}

impl WorkspaceWikiSources {
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn manifest_version(&self) -> u64 {
        self.manifest_version
    }
    pub fn generation(&self) -> &str {
        &self.generation
    }
    pub fn files(&self) -> &[WorkspaceWikiSourceFile] {
        &self.files
    }
    pub fn coverage(&self) -> &WorkspaceWikiSourceCoverage {
        &self.coverage
    }
}

/// Capture only entries already enumerated by the host's manifest. The host must
/// supply its current binding/permission receipt and exact runtime/output exclusions.
/// Call on the host's blocking pool; cancellation never returns a partial snapshot.
pub fn capture_workspace_wiki_sources(
    root: &Path,
    manifest: &LocalWorkspaceManifestSnapshot,
    scope_revision: &str,
    excluded: &[WorkspacePath],
    limits: WorkspaceWikiSourceLimits,
    cancellation: &CancellationToken,
) -> io::Result<WorkspaceWikiSources> {
    check_cancelled(cancellation)?;
    if !root.is_absolute()
        || !manifest.root.is_absolute()
        || scope_revision.is_empty()
        || scope_revision.len() > 4096
        || scope_revision.chars().any(char::is_control)
        || limits.max_files == 0
        || limits.max_files > MAX_FILES
        || limits.max_file_bytes == 0
        || limits.max_file_bytes > MAX_BYTES
        || limits.max_total_bytes == 0
        || limits.max_total_bytes > MAX_BYTES
        || manifest.files.len() > MAX_MANIFEST_FILES
        || excluded.len() > MAX_FILES
    {
        return Err(invalid("workspace Wiki source scope or limits are invalid"));
    }
    let root = root.canonicalize()?;
    if root.parent().is_none() || manifest.root.canonicalize()? != root {
        return Err(invalid("workspace Wiki manifest root mismatch"));
    }
    let root_name = root
        .to_str()
        .filter(|s| s.len() <= 4096)
        .ok_or_else(|| invalid("workspace Wiki root is not bounded UTF-8"))?;
    let mut paths = BTreeSet::new();
    for file in &manifest.files {
        if !valid_path(&file.path) || !paths.insert(file.path.as_str()) {
            return Err(invalid(
                "workspace Wiki manifest paths must be unique and normalized",
            ));
        }
    }
    if excluded.iter().any(|path| !valid_path(path.as_str())) {
        return Err(invalid(
            "workspace Wiki exclusions must be normalized relative paths",
        ));
    }
    let excluded = excluded
        .iter()
        .map(|path| path.as_str())
        .collect::<BTreeSet<_>>();
    let directory = Dir::open_ambient_dir(&root, cap_std::ambient_authority())?;
    let backend = LocalWorkspaceBackend::new_with_source_egress_policy(root.clone());
    let policy = WorkspaceEligibilityPolicy {
        max_file_bytes: limits.max_file_bytes as u64,
    };
    let mut entries = manifest.files.iter().collect::<Vec<_>>();
    entries.sort_unstable_by(|a, b| a.path.cmp(&b.path));
    let mut files = Vec::new();
    let mut coverage = WorkspaceWikiSourceCoverage::default();
    for entry in entries {
        check_cancelled(cancellation)?;
        let path = Path::new(&entry.path);
        if excluded.iter().any(|prefix| path.starts_with(prefix))
            || path.components().any(|c| {
                c.as_os_str()
                    .to_str()
                    .is_some_and(|name| name.eq_ignore_ascii_case(".aster"))
            })
            || crate::workspace::source_egress::path_is_denied(path)
            || entry.generated
        {
            coverage.skip(&entry.path, WorkspaceWikiSourceSkipReason::Excluded);
            continue;
        }
        let format = if document_path(path) {
            WorkspaceWikiSourceFormat::Document
        } else if entry.binary {
            coverage.skip(&entry.path, WorkspaceWikiSourceSkipReason::Unsupported);
            continue;
        } else {
            WorkspaceWikiSourceFormat::Text
        };
        let mut admission = entry.clone();
        admission.binary = false;
        if !policy.admits(&admission) {
            coverage.skip(&entry.path, WorkspaceWikiSourceSkipReason::TooLarge);
            continue;
        }
        if files.len() >= limits.max_files
            || entry.size > (limits.max_total_bytes - coverage.included_bytes) as u64
        {
            coverage.skip(&entry.path, WorkspaceWikiSourceSkipReason::Budget);
            continue;
        }
        let bytes = match read_source(&directory, &backend, entry, limits.max_file_bytes) {
            Ok(bytes) => bytes,
            Err(reason) => {
                coverage.skip(&entry.path, reason);
                continue;
            }
        };
        check_cancelled(cancellation)?;
        if bytes.is_empty() {
            coverage.skip(&entry.path, WorkspaceWikiSourceSkipReason::Empty);
            continue;
        }
        let chunks = if format == WorkspaceWikiSourceFormat::Text {
            let Ok(text) = std::str::from_utf8(&bytes) else {
                coverage.skip(&entry.path, WorkspaceWikiSourceSkipReason::Unsupported);
                continue;
            };
            if text.contains('\0') {
                coverage.skip(&entry.path, WorkspaceWikiSourceSkipReason::Unsupported);
                continue;
            }
            match chunk_file_with_strategy(
                ChunkFileRequest {
                    path: &entry.path,
                    language: entry.language.as_deref(),
                    source_revision: manifest.version,
                    content: text,
                },
                ChunkingConfig::default(),
                &WorkspaceChunkingStrategy::Lines,
            ) {
                Ok(chunked) => chunked.chunks,
                Err(_) => {
                    coverage.skip(&entry.path, WorkspaceWikiSourceSkipReason::Budget);
                    continue;
                }
            }
        } else {
            Vec::new()
        };
        coverage.included_bytes += bytes.len();
        files.push(WorkspaceWikiSourceFile {
            path: entry.path.clone(),
            generation: digest(&bytes),
            format,
            bytes,
            chunks,
        });
    }
    coverage.included_files = files.len();
    check_cancelled(cancellation)?;
    let identity = serde_json::to_vec(&(
        "workspace-wiki-sources-v2",
        root_name,
        scope_revision,
        excluded,
        files
            .iter()
            .map(|f| (&f.path, &f.generation, f.format))
            .collect::<Vec<_>>(),
    ))
    .map_err(io::Error::other)?;
    Ok(WorkspaceWikiSources {
        generation: digest(&identity),
        root,
        manifest_version: manifest.version,
        files,
        coverage,
    })
}

fn read_source(
    directory: &Dir,
    backend: &LocalWorkspaceBackend,
    entry: &LocalWorkspaceFile,
    limit: usize,
) -> Result<Vec<u8>, WorkspaceWikiSourceSkipReason> {
    use WorkspaceWikiSourceSkipReason::{Changed, TooLarge, Unavailable};
    let path = Path::new(&entry.path);
    let mut parent = directory.try_clone().map_err(|_| Unavailable)?;
    if let Some(parts) = path.parent() {
        for part in parts.components() {
            parent = parent
                .open_dir_nofollow(part.as_os_str())
                .map_err(|_| Unavailable)?;
        }
    }
    let name = path.file_name().ok_or(Unavailable)?;
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No).nonblock(true);
    let mut file: File = parent
        .open_with(name, &options)
        .map_err(|_| Unavailable)?
        .into_std();
    let before = file.metadata().map_err(|_| Unavailable)?;
    if !before.is_file() {
        return Err(Unavailable);
    }
    if before.len() > limit as u64 {
        return Err(TooLarge);
    }
    backend
        .refuse_direct_read(&WorkspacePath::from_normalized(&entry.path), &file)
        .map_err(|_| Unavailable)?;
    let modified = before
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .and_then(|time| u64::try_from(time.as_millis()).ok());
    if before.len() != entry.size || modified != entry.modified_ms {
        return Err(Changed);
    }
    let mut bytes = Vec::new();
    (&mut file)
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Unavailable)?;
    let after = file.metadata().map_err(|_| Unavailable)?;
    if bytes.len() > limit {
        return Err(TooLarge);
    }
    if bytes.len() as u64 != before.len()
        || after.len() != before.len()
        || after.modified().ok() != before.modified().ok()
    {
        return Err(Changed);
    }
    Ok(bytes)
}

fn valid_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 4096
        && !path.contains(['\\', ':'])
        && !path.chars().any(char::is_control)
        && path
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
        && Path::new(path)
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
}

fn document_path(path: &Path) -> bool {
    path.extension()
        .and_then(|s| s.to_str())
        .is_some_and(|ext| {
            matches!(
                ext.to_ascii_lowercase().as_str(),
                "pdf"
                    | "doc"
                    | "docx"
                    | "xls"
                    | "xlsx"
                    | "ppt"
                    | "pptx"
                    | "odt"
                    | "ods"
                    | "odp"
                    | "png"
                    | "jpg"
                    | "jpeg"
                    | "webp"
                    | "gif"
                    | "bmp"
                    | "tif"
                    | "tiff"
            )
        })
}

fn digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}
fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}
fn check_cancelled(cancellation: &CancellationToken) -> io::Result<()> {
    if cancellation.is_cancelled() {
        Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "workspace Wiki source capture cancelled",
        ))
    } else {
        Ok(())
    }
}

#[cfg(test)]
#[path = "wiki_sources_tests.rs"]
mod tests;
