//! Workspace boundary helpers used by Aster Core local access checks.
//!
#![allow(dead_code)]
//! Path / hard-link / sensitive-file helpers shared with the ksandbox-backed
//! native Bash adapter (formerly part of `aster-cmd-sandbox`).

use anyhow::{bail, Context, Result};
use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

const MAX_WORKSPACE_SCAN_ENTRIES: usize = 1_000_000;
const MAX_WORKSPACE_SCAN_DEPTH: usize = 64;
const MAX_CREDENTIAL_ALIAS_SCAN_ENTRIES: usize = 5_000_000;

pub const PROTECTED_WORKSPACE_DIRECTORIES: &[&str] = &[
    ".git", ".aster", ".agents", ".codex", ".claude", ".vscode", ".idea",
];

/// Workspace-relative files that can alter command discovery or repository
/// behavior without living in a protected directory.
pub const PROTECTED_WORKSPACE_FILES: &[&str] = &[
    ".gitmodules",
    ".mcp.json",
    ".ripgreprc",
    ".bashrc",
    ".bash_profile",
    ".zshrc",
    ".zprofile",
    ".profile",
];

/// Return whether a normalized workspace-relative path targets protected
/// control metadata.
///
/// Durable `/goal` loop artifacts under `.aster/loops/` are intentionally
/// agent-writable (ACCEPTANCE/STATE/RUN_LOG). Other `.aster` control-plane
/// siblings stay protected.
pub fn is_protected_workspace_path(path: &str) -> bool {
    let normalized = path.replace('\\', "/");
    let mut components = normalized
        .split('/')
        .filter(|component| !component.is_empty() && *component != ".");
    let Some(first) = components.next() else {
        return false;
    };
    if first == ".." || components.clone().any(|component| component == "..") {
        return false;
    }

    if first.eq_ignore_ascii_case(".aster") {
        return !components
            .next()
            .is_some_and(|second| second.eq_ignore_ascii_case("loops"));
    }

    PROTECTED_WORKSPACE_DIRECTORIES
        .iter()
        .any(|protected| first.eq_ignore_ascii_case(protected))
        || PROTECTED_WORKSPACE_FILES
            .iter()
            .any(|protected| first.eq_ignore_ascii_case(protected))
}

pub fn sensitive_paths() -> Vec<PathBuf> {
    let mut paths = dirs::home_dir()
        .map(|home| default_sensitive_paths(&home))
        .unwrap_or_default();

    extend_configured_secret(&mut paths, "CODEX_HOME", Some("auth.json"));
    extend_configured_secret(&mut paths, "CLAUDE_CONFIG_DIR", Some(".credentials.json"));
    extend_configured_secret(&mut paths, "CARGO_HOME", Some("credentials"));
    extend_configured_secret(&mut paths, "CARGO_HOME", Some("credentials.toml"));
    for variable in ["ASTER_KIMI_HOME", "KIMI_CODE_HOME", "KIMI_SHARE_DIR"] {
        extend_configured_secret(&mut paths, variable, Some("credentials/kimi-code.json"));
    }
    for variable in [
        "ASTER_KIMI_DESKTOP_HOME",
        "KIMI_DESKTOP_HOME",
        "WORKBUDDY_CONFIG_DIR",
        "CODEBUDDY_CONFIG_DIR",
    ] {
        extend_configured_secret(&mut paths, variable, None);
    }
    paths
}

fn read_denied_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(home) = dirs::home_dir() {
        roots.push(home.canonicalize().unwrap_or(home));
    }
    let temp = std::env::temp_dir();
    roots.push(temp.canonicalize().unwrap_or(temp));
    roots
}

fn readable_tool_paths(workspace: &Path, scratch: &Path) -> Vec<PathBuf> {
    const TOOLCHAIN_ROOTS: &[&str] = &[
        "CARGO_HOME",
        "RUSTUP_HOME",
        "GOPATH",
        "GOROOT",
        "GOMODCACHE",
        "NVM_DIR",
        "FNM_DIR",
        "VOLTA_HOME",
        "BUN_INSTALL",
        "DENO_DIR",
        "PNPM_HOME",
        "JAVA_HOME",
        "GRADLE_USER_HOME",
        "MAVEN_HOME",
        "SDKROOT",
        "DEVELOPER_DIR",
    ];

    let mut paths = vec![workspace.to_path_buf(), scratch.to_path_buf()];
    for variable in TOOLCHAIN_ROOTS {
        let Some(path) = std::env::var_os(variable).filter(|value| !value.is_empty()) else {
            continue;
        };
        let path = PathBuf::from(path);
        if path.is_absolute() && path.exists() {
            paths.push(path.canonicalize().unwrap_or(path));
        }
    }
    if let Some(path) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&path).filter_map(|path| {
            if !path.is_absolute() || !path.exists() {
                return None;
            }
            path.canonicalize().ok()
        }));
    }
    paths
}

fn default_sensitive_paths(home: &Path) -> Vec<PathBuf> {
    [
        ".ssh",
        ".gnupg",
        ".aws",
        ".azure",
        ".kube",
        ".docker",
        ".config/gcloud",
        ".config/gh",
        ".netrc",
        ".npmrc",
        ".pypirc",
        ".cargo/credentials",
        ".cargo/credentials.toml",
        ".codex/auth.json",
        ".claude/.credentials.json",
        ".claude.json",
        ".git-credentials",
        ".config/git/credentials",
        ".workbuddy",
        ".workbuddy-ai",
        "credentials/kimi-code.json",
        ".kimi-code/credentials/kimi-code.json",
        ".kimi/credentials/kimi-code.json",
        ".config/kimi-desktop/daimon-share",
        "Library/Application Support/kimi-desktop/daimon-share",
        ".config/opencode/auth.json",
        ".local/share/opencode/auth.json",
        ".gemini/oauth_creds.json",
        ".terraform.d/credentials.tfrc.json",
        ".local/share/keyrings",
        ".password-store",
        ".aster/os-auth.json",
        "Library/Keychains",
    ]
    .into_iter()
    .map(|path| home.join(path))
    .collect()
}

const FIXED_WORKSPACE_SECRET_FILES: &[&str] = &[
    ".env",
    ".env.local",
    ".env.development",
    ".env.production",
    ".env.test",
    ".netrc",
    ".npmrc",
    ".pypirc",
    ".git-credentials",
    ".aster/os-auth.json",
    ".codex/auth.json",
    ".claude/.credentials.json",
    ".claude.json",
];

fn fixed_workspace_secret_paths(workspace: &Path) -> Vec<PathBuf> {
    FIXED_WORKSPACE_SECRET_FILES
        .iter()
        .map(|path| workspace.join(path))
        .collect()
}

/// Discover credential-like files inside a workspace.
pub fn workspace_sensitive_paths(workspace: &Path) -> Result<Vec<PathBuf>> {
    let mut paths = fixed_workspace_secret_paths(workspace);
    paths.extend(scan_workspace_security(workspace)?.nested_env);
    Ok(paths)
}

/// Discover source-tree files that still have a hard-link name outside the
/// workspace.
///
/// A link whose every directory entry is inside the workspace, including
/// names under `node_modules` or `target`, is an internal duplicate. Those
/// paths stay out of the Seatbelt deny set. A path stays denied when its
/// link count is greater than the number of names found in the workspace,
/// or when that count cannot be proven. Credential aliases inside package
/// stores are recovered separately via
/// [`workspace_credential_hardlink_aliases`].
pub fn workspace_hardlink_paths(workspace: &Path) -> Result<Vec<PathBuf>> {
    let mut hardlinks = scan_workspace_security(workspace)?.source_hardlinks;
    deduplicate_paths(&mut hardlinks);
    Ok(hardlinks)
}

#[derive(Debug, Default)]
struct WorkspaceSecurityScan {
    nested_env: Vec<PathBuf>,
    source_hardlinks: Vec<PathBuf>,
}

struct HardlinkAccount {
    nlink: u64,
    names_found: u64,
    source_paths: Vec<PathBuf>,
    /// Every in-workspace name found on this scan. Empty when a remembered
    /// proof was reused.
    names: Vec<PathBuf>,
    /// `names_found` was set from a remembered proof. Do not overwrite it.
    remembered: bool,
}

/// Collect nested `.env*` paths and source files whose hard-link names are
/// not all inside the workspace.
///
/// The source walk skips package stores and does not count names inside
/// protected directories. Those names are counted afterwards, and only while
/// a source link is still short of its link count. A count that cannot be
/// finished fails the scan so the profile is not compiled from a partial set.
fn scan_workspace_security(workspace: &Path) -> Result<WorkspaceSecurityScan> {
    let mut pending = vec![(workspace.to_path_buf(), 0usize, true)];
    let mut scanned = 0usize;
    let mut scan = WorkspaceSecurityScan::default();
    let mut accounts: HashMap<FileIdentity, HardlinkAccount> = HashMap::new();
    let mut node_module_roots = Vec::new();
    let mut target_roots = Vec::new();
    let mut later_name_roots = Vec::new();

    while let Some((directory, depth, collect_hardlinks)) = pending.pop() {
        let Some(entries) = workspace_scan_result(std::fs::read_dir(&directory), || {
            format!("failed to scan workspace {}", directory.display())
        })?
        else {
            continue;
        };
        for entry in entries {
            let Some(entry) = workspace_scan_result(entry, || {
                format!("failed to enumerate workspace {}", directory.display())
            })?
            else {
                continue;
            };
            scanned = next_workspace_scan_entry(scanned, MAX_WORKSPACE_SCAN_ENTRIES)?;
            let path = entry.path();
            let file_name = entry.file_name();
            let Some(file_type) = workspace_scan_result(entry.file_type(), || {
                format!("failed to inspect workspace path {}", path.display())
            })?
            else {
                continue;
            };

            if file_name.to_str().is_some_and(|name| {
                name.get(..4)
                    .is_some_and(|prefix| prefix.eq_ignore_ascii_case(".env"))
            }) {
                scan.nested_env.push(path.clone());
            }

            if file_type.is_symlink() {
                continue;
            }
            if file_type.is_dir() {
                // A store under a protected directory is counted with that
                // directory. Recording it again would count the same names twice.
                if is_git_directory(&file_name) {
                    if collect_hardlinks {
                        later_name_roots.push(path);
                    }
                    continue;
                }
                if is_package_or_build_store_directory(&file_name) {
                    if collect_hardlinks {
                        if file_name
                            .to_str()
                            .is_some_and(|name| name.eq_ignore_ascii_case("target"))
                        {
                            target_roots.push(path);
                        } else {
                            node_module_roots.push(path);
                        }
                    }
                    continue;
                }
                ensure_workspace_scan_depth(depth, &path)?;
                if collect_hardlinks && is_protected_workspace_directory(&file_name) {
                    later_name_roots.push(path.clone());
                }
                let child_collect_hardlinks =
                    collect_hardlinks && !is_protected_workspace_directory(&file_name);
                pending.push((path, depth + 1, child_collect_hardlinks));
                continue;
            }
            if !collect_hardlinks || !file_type.is_file() {
                continue;
            }
            let Some(metadata) = workspace_scan_result(std::fs::symlink_metadata(&path), || {
                format!("failed to inspect workspace path {}", path.display())
            })?
            else {
                continue;
            };
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                continue;
            }
            let nlink = hard_link_count(&path, &metadata);
            if nlink <= 1 {
                continue;
            }
            let Some(identity) = FileIdentity::from_path(&path, &metadata) else {
                scan.source_hardlinks.push(path);
                continue;
            };
            let account = accounts.entry(identity).or_insert(HardlinkAccount {
                nlink: 0,
                names_found: 0,
                source_paths: Vec::new(),
                names: Vec::new(),
                remembered: false,
            });
            account.nlink = account.nlink.max(nlink);
            account.names_found = account.names_found.saturating_add(1);
            account.names.push(path.clone());
            account.source_paths.push(path);
        }
    }

    // A remembered proof is reused only when every recorded name still has
    // this link count. Replacing an in-workspace twin with an outside name
    // fails that check and forces a fresh count.
    apply_remembered_hardlinks(&mut accounts);
    // Protected trees are small. Count them before package stores so a link
    // that lives there does not force a walk of every `target` directory.
    later_name_roots.sort_by_key(|path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case(".git"))
    });
    later_name_roots.reverse();
    count_workspace_hardlink_names(&later_name_roots, &mut accounts)?;
    // Pop walks the last root first. Reverse the shallow-first sort so the
    // workspace-root `node_modules` is searched before nested stores.
    node_module_roots.sort_by_key(|path| path.components().count());
    node_module_roots.reverse();
    count_workspace_hardlink_names(&node_module_roots, &mut accounts)?;
    if source_hardlink_accounts_unproven(&accounts) {
        count_workspace_hardlink_names(&target_roots, &mut accounts)?;
    }
    remember_proven_hardlinks(&accounts);
    for account in accounts.into_values() {
        if account.names_found < account.nlink {
            scan.source_hardlinks.extend(account.source_paths);
        }
    }
    Ok(scan)
}

fn source_hardlink_accounts_unproven(accounts: &HashMap<FileIdentity, HardlinkAccount>) -> bool {
    accounts
        .values()
        .any(|account| account.names_found < account.nlink)
}

/// Count directory entries under `roots` for inodes already recorded in
/// `accounts`. Returns when every recorded link count is accounted for.
/// A walk that hits its entry or depth limit fails closed.
fn count_workspace_hardlink_names(
    roots: &[PathBuf],
    accounts: &mut HashMap<FileIdentity, HardlinkAccount>,
) -> Result<()> {
    if roots.is_empty() || !source_hardlink_accounts_unproven(accounts) {
        return Ok(());
    }
    let mut pending: Vec<(PathBuf, usize)> =
        roots.iter().map(|root| (root.clone(), 0usize)).collect();
    let mut scanned = 0usize;
    while let Some((directory, depth)) = pending.pop() {
        if !source_hardlink_accounts_unproven(accounts) {
            return Ok(());
        }
        let Some(entries) = workspace_scan_result(std::fs::read_dir(&directory), || {
            format!(
                "failed to count hard-link names under {}",
                directory.display()
            )
        })?
        else {
            continue;
        };
        for entry in entries {
            let Some(entry) = workspace_scan_result(entry, || {
                format!(
                    "failed to enumerate hard-link names under {}",
                    directory.display()
                )
            })?
            else {
                continue;
            };
            scanned = next_workspace_scan_entry(scanned, MAX_CREDENTIAL_ALIAS_SCAN_ENTRIES)?;
            let path = entry.path();
            let Some(file_type) = workspace_scan_result(entry.file_type(), || {
                format!("failed to inspect hard-link name {}", path.display())
            })?
            else {
                continue;
            };
            if file_type.is_symlink() {
                continue;
            }
            if file_type.is_dir() {
                ensure_workspace_scan_depth(depth, &path)?;
                pending.push((path, depth + 1));
                continue;
            }
            if !file_type.is_file() {
                continue;
            }
            let Some(metadata) = workspace_scan_result(std::fs::symlink_metadata(&path), || {
                format!("failed to inspect hard-link name {}", path.display())
            })?
            else {
                continue;
            };
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                continue;
            }
            let nlink = hard_link_count(&path, &metadata);
            if nlink <= 1 {
                continue;
            }
            let Some(identity) = FileIdentity::from_path(&path, &metadata) else {
                continue;
            };
            if let Some(account) = accounts.get_mut(&identity) {
                if account.remembered {
                    continue;
                }
                account.names_found = account.names_found.saturating_add(1);
                account.nlink = account.nlink.max(nlink);
                account.names.push(path);
            }
        }
    }
    Ok(())
}

fn hardlink_name_cache() -> &'static std::sync::Mutex<HashMap<FileIdentity, Vec<PathBuf>>> {
    static CACHE: std::sync::OnceLock<std::sync::Mutex<HashMap<FileIdentity, Vec<PathBuf>>>> =
        std::sync::OnceLock::new();
    CACHE.get_or_init(|| std::sync::Mutex::new(HashMap::new()))
}

fn lock_hardlink_name_cache() -> std::sync::MutexGuard<'static, HashMap<FileIdentity, Vec<PathBuf>>>
{
    match hardlink_name_cache().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn apply_remembered_hardlinks(accounts: &mut HashMap<FileIdentity, HardlinkAccount>) {
    if accounts.is_empty() {
        return;
    }
    let snapshots: Vec<(FileIdentity, Vec<PathBuf>)> = {
        let cache = lock_hardlink_name_cache();
        accounts
            .keys()
            .filter_map(|identity| cache.get(identity).map(|names| (*identity, names.clone())))
            .collect()
    };
    let mut stale = Vec::new();
    for (identity, names) in snapshots {
        let Some(account) = accounts.get(&identity) else {
            continue;
        };
        if remembered_hardlink_names_match(identity, account.nlink, &names) {
            if let Some(account) = accounts.get_mut(&identity) {
                account.names_found = account.nlink;
                account.remembered = true;
            }
        } else {
            stale.push(identity);
        }
    }
    if stale.is_empty() {
        return;
    }
    let mut cache = lock_hardlink_name_cache();
    for identity in stale {
        cache.remove(&identity);
    }
}

fn remembered_hardlink_names_match(identity: FileIdentity, nlink: u64, names: &[PathBuf]) -> bool {
    if nlink == 0 || names.len() as u64 != nlink {
        return false;
    }
    for path in names {
        let Ok(metadata) = std::fs::symlink_metadata(path) else {
            return false;
        };
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return false;
        }
        if hard_link_count(path, &metadata) != nlink {
            return false;
        }
        if FileIdentity::from_path(path, &metadata) != Some(identity) {
            return false;
        }
    }
    true
}

fn remember_proven_hardlinks(accounts: &HashMap<FileIdentity, HardlinkAccount>) {
    let mut cache = lock_hardlink_name_cache();
    for (identity, account) in accounts {
        if account.names_found < account.nlink {
            cache.remove(identity);
            continue;
        }
        if account.remembered {
            continue;
        }
        if account.names.len() as u64 == account.nlink {
            cache.insert(*identity, account.names.clone());
        }
    }
}

/// Find workspace paths that hard-link to already-discovered credential files.
///
/// Ordinary package/build multi-link artifacts are ignored. Only inodes that
/// already belong to a sensitive path are collected, matching the Core local
/// credential boundary: package-store hardlinks stay usable unless they alias
/// a discovered credential identity.
pub fn workspace_credential_hardlink_aliases(
    workspace: &Path,
    sensitive: &[PathBuf],
) -> Result<Vec<PathBuf>> {
    let mut wanted = HashSet::new();
    for path in sensitive {
        let Some(metadata) = workspace_scan_result(std::fs::symlink_metadata(path), || {
            format!(
                "failed to inspect native sandbox credential path {}",
                path.display()
            )
        })?
        else {
            continue;
        };
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            continue;
        }
        if hard_link_count(path, &metadata) <= 1 {
            continue;
        }
        let Some(identity) = FileIdentity::from_path(path, &metadata) else {
            continue;
        };
        wanted.insert(identity);
    }
    if wanted.is_empty() {
        return Ok(Vec::new());
    }

    let mut pending = vec![(workspace.to_path_buf(), 0usize, false)];
    let mut scanned = 0usize;
    let mut aliases = Vec::new();
    let sensitive_set: HashSet<&Path> = sensitive.iter().map(PathBuf::as_path).collect();

    while let Some((directory, depth, in_package_store)) = pending.pop() {
        let Some(entries) = workspace_scan_result(std::fs::read_dir(&directory), || {
            format!(
                "failed to scan native sandbox package stores under {}",
                directory.display()
            )
        })?
        else {
            continue;
        };
        for entry in entries {
            let Some(entry) = workspace_scan_result(entry, || {
                format!(
                    "failed to enumerate native sandbox package stores under {}",
                    directory.display()
                )
            })?
            else {
                continue;
            };
            scanned = next_workspace_scan_entry(scanned, MAX_CREDENTIAL_ALIAS_SCAN_ENTRIES)?;
            let path = entry.path();
            let file_name = entry.file_name();
            let Some(file_type) = workspace_scan_result(entry.file_type(), || {
                format!(
                    "failed to inspect native sandbox package-store path {}",
                    path.display()
                )
            })?
            else {
                continue;
            };
            if file_type.is_symlink() {
                continue;
            }
            if file_type.is_dir() {
                if !in_package_store && is_git_directory(&file_name) {
                    continue;
                }
                let child_in_store =
                    in_package_store || is_package_or_build_store_directory(&file_name);
                if !child_in_store && is_protected_workspace_directory(&file_name) {
                    continue;
                }
                ensure_workspace_scan_depth(depth, &path)?;
                pending.push((path, depth + 1, child_in_store));
                continue;
            }
            if !in_package_store || !file_type.is_file() {
                continue;
            }
            if sensitive_set.contains(path.as_path()) {
                continue;
            }
            let Some(metadata) = workspace_scan_result(std::fs::symlink_metadata(&path), || {
                format!(
                    "failed to inspect native sandbox package-store path {}",
                    path.display()
                )
            })?
            else {
                continue;
            };
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                continue;
            }
            if hard_link_count(&path, &metadata) <= 1 {
                continue;
            }
            let Some(identity) = FileIdentity::from_path(&path, &metadata) else {
                continue;
            };
            if wanted.contains(&identity) {
                aliases.push(path);
            }
        }
    }
    deduplicate_paths(&mut aliases);
    Ok(aliases)
}

fn workspace_scan_result<T>(
    result: std::io::Result<T>,
    context: impl FnOnce() -> String,
) -> Result<Option<T>> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(context),
    }
}

fn next_workspace_scan_entry(scanned: usize, limit: usize) -> Result<usize> {
    let scanned = scanned
        .checked_add(1)
        .context("workspace scan entry count overflowed")?;
    if scanned > limit {
        bail!("workspace exceeds the {limit} entry scan limit");
    }
    Ok(scanned)
}

fn ensure_workspace_scan_depth(depth: usize, path: &Path) -> Result<()> {
    if depth >= MAX_WORKSPACE_SCAN_DEPTH {
        bail!(
            "workspace exceeds the {MAX_WORKSPACE_SCAN_DEPTH}-level scan depth at {}",
            path.display()
        );
    }
    Ok(())
}

/// Return whether recursive security scans should treat a directory as a
/// package/build store or VCS object store rather than source content.
pub fn should_skip_workspace_scan_directory(name: &OsStr) -> bool {
    is_git_directory(name) || is_package_or_build_store_directory(name)
}

fn is_package_or_build_store_directory(name: &OsStr) -> bool {
    name.to_str().is_some_and(|name| {
        ["node_modules", "target"]
            .iter()
            .any(|skipped| name.eq_ignore_ascii_case(skipped))
    })
}

fn is_git_directory(name: &OsStr) -> bool {
    name.to_str()
        .is_some_and(|name| name.eq_ignore_ascii_case(".git"))
}

fn is_protected_workspace_directory(name: &OsStr) -> bool {
    name.to_str().is_some_and(|name| {
        PROTECTED_WORKSPACE_DIRECTORIES
            .iter()
            .any(|protected| name.eq_ignore_ascii_case(protected))
    })
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
struct FileIdentity {
    device: u64,
    inode: u64,
}

#[cfg(unix)]
impl FileIdentity {
    fn from_path(_path: &Path, metadata: &std::fs::Metadata) -> Option<Self> {
        use std::os::unix::fs::MetadataExt;
        Some(Self {
            device: metadata.dev(),
            inode: metadata.ino(),
        })
    }
}

#[cfg(windows)]
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
struct FileIdentity {
    volume: u32,
    index: u64,
}

#[cfg(windows)]
impl FileIdentity {
    fn from_path(path: &Path, _metadata: &std::fs::Metadata) -> Option<Self> {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Storage::FileSystem::{
            GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
        };

        let file = std::fs::File::open(path).ok()?;
        let mut information = unsafe { std::mem::zeroed::<BY_HANDLE_FILE_INFORMATION>() };
        // SAFETY: `file` owns a valid handle and `information` is writable.
        if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut information) } == 0 {
            return None;
        }
        Some(Self {
            volume: information.dwVolumeSerialNumber,
            index: (u64::from(information.nFileIndexHigh) << 32)
                | u64::from(information.nFileIndexLow),
        })
    }
}

#[cfg(not(any(unix, windows)))]
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
struct FileIdentity;

#[cfg(not(any(unix, windows)))]
impl FileIdentity {
    fn from_path(_path: &Path, _metadata: &std::fs::Metadata) -> Option<Self> {
        None
    }
}

#[cfg(unix)]
/// Return a file's hard-link count, failing conservatively on platforms where
/// querying it requires reopening the path.
pub fn hard_link_count(_path: &Path, metadata: &std::fs::Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;
    metadata.nlink()
}

#[cfg(windows)]
/// Return a file's hard-link count, failing conservatively on platforms where
/// querying it requires reopening the path.
pub fn hard_link_count(path: &Path, metadata: &std::fs::Metadata) -> u64 {
    let Ok(file) = std::fs::File::open(path) else {
        return u64::MAX;
    };
    hard_link_count_for_open_file(&file, metadata)
}

#[cfg(windows)]
/// Return the hard-link count for an already-open file handle.
pub fn hard_link_count_for_open_file<T>(file: &T, _metadata: &std::fs::Metadata) -> u64
where
    T: std::os::windows::io::AsRawHandle,
{
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
    };

    let mut information = unsafe { std::mem::zeroed::<BY_HANDLE_FILE_INFORMATION>() };
    // SAFETY: `file` owns a valid handle and `information` is writable.
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut information) } == 0 {
        return u64::MAX;
    }
    u64::from(information.nNumberOfLinks.max(1))
}

/// Return the hard-link count for an already-open file handle.
#[cfg(unix)]
pub fn hard_link_count_for_open_file<T>(_file: &T, metadata: &std::fs::Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;
    metadata.nlink()
}

#[cfg(not(any(unix, windows)))]
/// Return a conservative hard-link count on unsupported filesystems.
pub fn hard_link_count(_path: &Path, _metadata: &std::fs::Metadata) -> u64 {
    1
}

#[cfg(not(any(unix, windows)))]
/// Return a conservative hard-link count on unsupported filesystems.
pub fn hard_link_count_for_open_file<T>(_file: &T, _metadata: &std::fs::Metadata) -> u64 {
    1
}

fn extend_configured_secret(paths: &mut Vec<PathBuf>, variable: &str, suffix: Option<&str>) {
    let Some(root) = std::env::var_os(variable).filter(|value| !value.is_empty()) else {
        return;
    };
    let root = PathBuf::from(root);
    if !root.is_absolute() {
        return;
    }
    paths.push(match suffix {
        Some(suffix) => root.join(suffix),
        None => root,
    });
}

fn protected_workspace_paths(workspace: &Path) -> Result<Vec<PathBuf>> {
    let mut paths = PROTECTED_WORKSPACE_DIRECTORIES
        .iter()
        .chain(PROTECTED_WORKSPACE_FILES)
        .copied()
        .map(|path| workspace.join(path))
        .collect::<Vec<_>>();

    // Linux permits names that differ only by case even when the host's
    // default filesystem does not. Discover those aliases explicitly so the
    // policy remains consistent across platforms instead of protecting only
    // the lowercase spelling of control metadata.
    let entries = std::fs::read_dir(workspace).with_context(|| {
        format!(
            "failed to scan protected workspace roots {}",
            workspace.display()
        )
    })?;
    for entry in entries {
        let entry = entry.with_context(|| {
            format!(
                "failed to enumerate protected workspace roots {}",
                workspace.display()
            )
        })?;
        let name = entry.file_name();
        if PROTECTED_WORKSPACE_DIRECTORIES
            .iter()
            .chain(PROTECTED_WORKSPACE_FILES)
            .any(|protected| {
                name.to_str()
                    .is_some_and(|name| name.eq_ignore_ascii_case(protected))
            })
        {
            paths.push(entry.path());
        }
    }
    Ok(paths)
}

fn resolved_git_dir(workspace: &Path) -> Option<PathBuf> {
    let dot_git = workspace.join(".git");
    let dot_git = if dot_git.exists() {
        dot_git
    } else {
        std::fs::read_dir(workspace)
            .ok()?
            .filter_map(Result::ok)
            .find(|entry| {
                entry
                    .file_name()
                    .to_str()
                    .is_some_and(|name| name.eq_ignore_ascii_case(".git"))
            })
            .map(|entry| entry.path())?
    };
    if dot_git.is_dir() {
        return dot_git.canonicalize().ok();
    }
    let source = std::fs::read_to_string(dot_git).ok()?;
    let relative = source.trim().strip_prefix("gitdir:")?.trim();
    let path = Path::new(relative);
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        workspace.join(path)
    };
    path.canonicalize().ok()
}

fn expand_existing_canonical_paths(paths: &mut Vec<PathBuf>) {
    let resolved = paths
        .iter()
        .filter_map(|path| path.canonicalize().ok())
        .collect::<Vec<_>>();
    paths.extend(resolved);
    deduplicate_paths(paths);
}

pub(crate) fn deduplicate_paths(paths: &mut Vec<PathBuf>) {
    paths.sort();
    paths.dedup();
}

fn remove_redundant_descendants(paths: &mut Vec<PathBuf>) {
    deduplicate_paths(paths);
    let candidates = paths.clone();
    paths.retain(|path| {
        !candidates
            .iter()
            .any(|ancestor| ancestor != path && path.starts_with(ancestor))
    });
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) fn path_ancestors(path: &Path) -> Vec<PathBuf> {
    let mut ancestors = path
        .parent()
        .into_iter()
        .flat_map(Path::ancestors)
        .take_while(|ancestor| ancestor.parent().is_some())
        .map(Path::to_path_buf)
        .collect::<Vec<_>>();
    ancestors.reverse();
    ancestors
}
