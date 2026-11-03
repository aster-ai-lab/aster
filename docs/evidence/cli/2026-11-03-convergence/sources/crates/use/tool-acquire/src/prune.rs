//! Explicit declaration tracking and offline unused-version planning.

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::lock::InstallLock;
use crate::spec::parse_tool_name;
use crate::store::{json_bytes, read_json_bounded, write_atomic};
use crate::{
    read_env_declarations, resolve_env_tool, user_env_path, ToolAcquireError, ToolReceipt,
    ToolStore, VersionRequest, VersionRequestKind,
};

const TRACKING: &str = "env-sources-v1.json";
const MAX_TRACKING_BYTES: u64 = 4 * 1024 * 1024;
const MAX_SOURCES: usize = 4096;
mod process;
#[cfg(unix)]
const TRACKING_SCHEMA: &str = "aster.use.env-sources.unix.v1";
#[cfg(windows)]
const TRACKING_SCHEMA: &str = "aster.use.env-sources.windows.v1";
#[cfg(not(any(unix, windows)))]
const TRACKING_SCHEMA: &str = "aster.use.env-sources.unsupported.v1";

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TrackedSources {
    schema: String,
    paths: Vec<Vec<u8>>,
}

/// Read-only cleanup candidates. Current pointers are not evidence of use.
#[derive(Debug)]
pub struct ToolPrunePlan {
    tools: Vec<ToolReceipt>,
    stale_configs: Vec<PathBuf>,
}

impl ToolPrunePlan {
    pub fn tools(&self) -> &[ToolReceipt] {
        &self.tools
    }
    pub fn stale_configs(&self) -> &[PathBuf] {
        &self.stale_configs
    }
}

impl ToolStore {
    /// Plan deletion using current global/tracked declarations and native running
    /// executable paths. Never downloads, changes tracking, or removes files.
    /// This is blocking I/O. Invalid declarations/records and unsupported native
    /// process inspection refuse the entire plan before any removal.
    pub fn plan_prune(
        &self,
        aster_home: &Path,
        names: &[String],
    ) -> Result<ToolPrunePlan, ToolAcquireError> {
        for name in names {
            parse_tool_name(name)?;
        }
        let tracked = read_sources(self)?;
        let mut paths = tracked.clone();
        paths.insert(user_env_path(aster_home));
        let mut needed = BTreeSet::new();
        let mut stale_configs = Vec::new();
        for path in paths {
            let Some(config) = read_env_declarations(&path)? else {
                if tracked.contains(&path) {
                    stale_configs.push(path);
                }
                continue;
            };
            for (name, requests) in config.tools {
                if !names.is_empty() && !names.contains(&name) {
                    continue;
                }
                for raw in requests {
                    let request = VersionRequest::parse(&raw)?;
                    if matches!(
                        request.kind(),
                        VersionRequestKind::Path(_) | VersionRequestKind::System
                    ) {
                        continue;
                    }
                    let tool = resolve_env_tool(&name, &raw, None, None)?;
                    if let Some(receipt) = self.uninstall_candidate_for(tool.tool())? {
                        needed.insert((receipt.name, receipt.version));
                    }
                }
            }
        }
        let running = process::running_paths()?;
        let tools = self
            .all_uninstall_candidates()?
            .into_iter()
            .filter(|receipt| {
                (names.is_empty() || names.contains(&receipt.name))
                    && !needed.contains(&(receipt.name.clone(), receipt.version.clone()))
                    && !running.iter().any(|path| {
                        path.starts_with(
                            self.root_path()
                                .join("installs")
                                .join(&receipt.name)
                                .join(&receipt.version),
                        )
                    })
            })
            .collect();
        Ok(ToolPrunePlan {
            tools,
            stale_configs,
        })
    }

    /// Remove only missing tracked declaration paths. Existing declarations and
    /// installed tools are untouched. Returns the number of forgotten paths.
    /// Uses blocking bounded I/O and the store's existing installation lock.
    pub fn prune_env_sources(&self) -> Result<usize, ToolAcquireError> {
        let _lock = InstallLock::acquire(self.root_path())?;
        let mut paths = read_sources(self)?;
        let before = paths.len();
        for path in missing_sources(&paths)? {
            paths.remove(&path);
        }
        if paths.len() != before {
            write_sources(self, &paths)?;
        }
        Ok(before - paths.len())
    }

    /// Inspect missing tracked files without parsing declarations or writing state.
    /// Bounded blocking reads preserve native paths and return filesystem errors.
    pub fn stale_env_sources(&self) -> Result<Vec<PathBuf>, ToolAcquireError> {
        missing_sources(&read_sources(self)?)
    }

    pub(crate) fn track_env_sources(
        &self,
        sources: &BTreeSet<PathBuf>,
    ) -> Result<(), ToolAcquireError> {
        if sources.is_empty() {
            return Ok(());
        }
        let _lock = InstallLock::acquire(self.root_path())?;
        let mut paths = read_sources(self)?;
        let before = paths.len();
        paths.extend(sources.iter().cloned());
        if paths.len() != before {
            write_sources(self, &paths)?;
        }
        Ok(())
    }
}

fn missing_sources(paths: &BTreeSet<PathBuf>) -> Result<Vec<PathBuf>, ToolAcquireError> {
    let mut stale = Vec::new();
    for path in paths {
        match fs::metadata(path) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => stale.push(path.clone()),
            Err(error) => {
                return Err(ToolAcquireError::io(
                    "inspect tracked declaration",
                    path,
                    error,
                ))
            }
        }
    }
    Ok(stale)
}

fn read_sources(store: &ToolStore) -> Result<BTreeSet<PathBuf>, ToolAcquireError> {
    let path = store.root_path().join(TRACKING);
    match fs::symlink_metadata(&path) {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeSet::new()),
        Err(error) => {
            return Err(ToolAcquireError::io(
                "inspect environment tracking",
                path,
                error,
            ))
        }
    }
    let data: TrackedSources = read_json_bounded(&path, MAX_TRACKING_BYTES)?;
    if data.schema != TRACKING_SCHEMA || data.paths.len() > MAX_SOURCES {
        return Err(ToolAcquireError::InvalidEnvTracking(
            "unsupported schema or source count".into(),
        ));
    }
    data.paths
        .into_iter()
        .map(|bytes| {
            let path = decode_path(bytes)?;
            if !path.is_absolute() {
                return Err(ToolAcquireError::InvalidEnvTracking(
                    "source must be an absolute path".into(),
                ));
            }
            Ok(path)
        })
        .collect()
}

fn write_sources(store: &ToolStore, paths: &BTreeSet<PathBuf>) -> Result<(), ToolAcquireError> {
    if paths.len() > MAX_SOURCES {
        return Err(ToolAcquireError::InvalidEnvTracking(
            "source count exceeds 4096".into(),
        ));
    }
    let paths = paths
        .iter()
        .map(|path| encode_path(path))
        .collect::<Result<_, _>>()?;
    let bytes = json_bytes(&TrackedSources {
        schema: TRACKING_SCHEMA.into(),
        paths,
    })?;
    if bytes.len() as u64 > MAX_TRACKING_BYTES {
        return Err(ToolAcquireError::InvalidEnvTracking(
            "tracking exceeds its byte limit".into(),
        ));
    }
    write_atomic(&store.root_path().join(TRACKING), &bytes)
}

#[cfg(unix)]
fn encode_path(path: &Path) -> Result<Vec<u8>, ToolAcquireError> {
    use std::os::unix::ffi::OsStrExt;
    Ok(path.as_os_str().as_bytes().to_vec())
}
#[cfg(unix)]
fn decode_path(bytes: Vec<u8>) -> Result<PathBuf, ToolAcquireError> {
    use std::os::unix::ffi::OsStringExt;
    Ok(PathBuf::from(OsString::from_vec(bytes)))
}
#[cfg(windows)]
fn encode_path(path: &Path) -> Result<Vec<u8>, ToolAcquireError> {
    use std::os::windows::ffi::OsStrExt;
    Ok(path
        .as_os_str()
        .encode_wide()
        .flat_map(u16::to_le_bytes)
        .collect())
}
#[cfg(windows)]
fn decode_path(bytes: Vec<u8>) -> Result<PathBuf, ToolAcquireError> {
    use std::os::windows::ffi::OsStringExt;
    if bytes.len() % 2 != 0 {
        return Err(ToolAcquireError::InvalidEnvTracking(
            "invalid Windows path units".into(),
        ));
    }
    let units: Vec<_> = bytes
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect();
    Ok(PathBuf::from(OsString::from_wide(&units)))
}
#[cfg(not(any(unix, windows)))]
fn encode_path(_: &Path) -> Result<Vec<u8>, ToolAcquireError> {
    Err(ToolAcquireError::InvalidEnvTracking(
        "unsupported native path format".into(),
    ))
}
#[cfg(not(any(unix, windows)))]
fn decode_path(_: Vec<u8>) -> Result<PathBuf, ToolAcquireError> {
    Err(ToolAcquireError::InvalidEnvTracking(
        "unsupported native path format".into(),
    ))
}

#[cfg(test)]
mod tests;
