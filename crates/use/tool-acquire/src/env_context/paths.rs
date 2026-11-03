//! Default external layouts follow mise v2026.10.25 (MIT); the GitHub discovery
//! rules are adapted with native paths and bounded work. See licenses/mise-MIT.txt.

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use crate::installation::MAX_TREE_ENTRIES;
use crate::{parse_tool_spec, CoreRuntime, ToolAcquireError, ToolBackend, VersionRequest};

pub(super) fn external_bin_dirs(
    spec: &str,
    root: &Path,
    request: &VersionRequest,
) -> Result<Vec<PathBuf>, ToolAcquireError> {
    match parse_tool_spec(spec)?.backend() {
        ToolBackend::Core(CoreRuntime::Node) => {
            #[cfg(windows)]
            let directory = root.to_path_buf();
            #[cfg(not(windows))]
            let directory = root.join("bin");
            Ok(vec![directory])
        }
        ToolBackend::Npm { .. } => {
            let mut paths = Vec::new();
            for path in [root.join("node_modules").join(".bin"), root.join("bin")] {
                if is_directory(&path)? {
                    paths.push(path);
                }
            }
            if paths.is_empty() {
                paths.push(root.to_path_buf());
            }
            Ok(paths)
        }
        ToolBackend::Pipx { .. } => {
            #[cfg(windows)]
            let directory = root.join("Scripts");
            #[cfg(not(windows))]
            let directory = root.join("bin");
            Ok(vec![directory])
        }
        ToolBackend::Github { .. } => github_bin_dirs(root),
        // shortcut: Aqua registry-described path layouts require the native backend metadata in T008.
        ToolBackend::Aqua { .. } | ToolBackend::Catalog => Err(
            ToolAcquireError::UnsupportedVersionRequest(request.canonical()),
        ),
    }
}

fn github_bin_dirs(root: &Path) -> Result<Vec<PathBuf>, ToolAcquireError> {
    for path in [root.join("bin"), root.join("Contents").join("MacOS")] {
        if is_directory(&path)? {
            return Ok(vec![path]);
        }
    }
    let mut remaining = MAX_TREE_ENTRIES;
    let children = entries(root, &mut remaining)?;
    if children.iter().any(|path| executable(path, root)) {
        return Ok(vec![root.to_path_buf()]);
    }
    let mut paths = Vec::new();
    for path in children {
        if !is_directory(&path)? {
            continue;
        }
        if path.extension() == Some(OsStr::new("app")) {
            let macos = path.join("Contents").join("MacOS");
            if is_directory(&macos)? {
                paths.push(macos);
                continue;
            }
        }
        let bin = path.join("bin");
        if is_directory(&bin)? {
            paths.push(bin);
        } else if entries(&path, &mut remaining)?
            .iter()
            .any(|entry| executable(entry, &path))
        {
            paths.push(path);
        }
    }
    if paths.is_empty() {
        paths.push(root.to_path_buf());
    }
    paths.sort();
    Ok(paths)
}

fn is_directory(path: &Path) -> Result<bool, ToolAcquireError> {
    match fs::metadata(path) {
        Ok(metadata) => Ok(metadata.is_dir()),
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
            ) =>
        {
            Ok(false)
        }
        Err(error) => Err(ToolAcquireError::io(
            "inspect external executable directory",
            path,
            error,
        )),
    }
}

fn entries(path: &Path, remaining: &mut usize) -> Result<Vec<PathBuf>, ToolAcquireError> {
    let mut paths = Vec::new();
    for entry in fs::read_dir(path)
        .map_err(|error| ToolAcquireError::io("read external executable directory", path, error))?
    {
        *remaining = remaining.checked_sub(1).ok_or_else(|| {
            ToolAcquireError::InvalidInstallation(
                "external executable discovery exceeds its entry limit".into(),
            )
        })?;
        paths.push(
            entry
                .map_err(|error| {
                    ToolAcquireError::io("read external executable entry", path, error)
                })?
                .path(),
        );
    }
    Ok(paths)
}

fn executable(path: &Path, cwd: &Path) -> bool {
    which::which_in(path, None::<&OsStr>, cwd).is_ok()
}
