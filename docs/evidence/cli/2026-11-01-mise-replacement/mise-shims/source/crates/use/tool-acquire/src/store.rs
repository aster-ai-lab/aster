use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use aster_use_core::metadata_is_link_or_reparse_point;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::ToolAcquireError;
use crate::installation::{
    fingerprint, validate_relative_path, InstallManifest, MANIFEST_SCHEMA, MAX_MANIFEST_BYTES,
    MAX_TREE_BYTES, MAX_TREE_ENTRIES,
};
use crate::lock::{join_under, InstallLock};
use crate::receipt::{ToolFailureRecord, ToolReceipt};
use crate::source::CompanionFile;
use crate::source::ToolSource;
use crate::spec::{parse_tool_name, parse_tool_spec, parse_tool_version, ToolSpec};
use crate::version::{
    compare_versions, semver_prerelease, VersionListing, VersionListingData, VersionRequest,
    VersionRequestKind, MAX_VERSIONS,
};

const VERSION_CACHE_SCHEMA: &str = "aster.use.tool-acquire.versions.v1";
const MAX_VERSION_CACHE_BYTES: u64 = 8 * 1024 * 1024;
const DISPATCH_SHIM_HEADER: &[u8] = b"#!/bin/sh\n# Aster project dispatch shim v1\n";

#[derive(Serialize)]
struct VersionCache<'a> {
    schema: &'static str,
    spec: &'a str,
    listing: &'a VersionListing,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VersionCacheData {
    schema: String,
    spec: String,
    listing: VersionListingData,
}

/// Copy one regular file into the store as a versioned executable.
#[derive(Debug, Clone)]
pub struct LocalApply {
    pub name: String,
    pub spec: String,
    pub version: String,
    pub executable_name: String,
    pub payload: PathBuf,
    pub sha256: Option<String>,
    pub require_checksum: bool,
    /// Sibling files written into the version directory next to the executable.
    pub companions: Vec<CompanionFile>,
}

/// Host-supplied root for installs, shims, and receipts.
#[derive(Debug, Clone)]
pub struct ToolStore {
    root: PathBuf,
}

#[derive(Clone, Copy)]
enum VersionRecordCheck {
    Complete,
    Identity,
}

pub(crate) fn root_of(store: &ToolStore) -> &Path {
    &store.root
}

impl ToolStore {
    /// Create the root when it is missing. A symlink root is refused.
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, ToolAcquireError> {
        let root = root.into();
        fs::create_dir_all(&root)
            .map_err(|error| ToolAcquireError::io("create tool root", &root, error))?;
        ensure_real_directory(&root)?;
        let root = root
            .canonicalize()
            .map_err(|error| ToolAcquireError::io("canonicalize tool root", &root, error))?;
        Ok(Self { root })
    }

    pub async fn apply_local(&self, request: LocalApply) -> Result<ToolReceipt, ToolAcquireError> {
        let root = self.root.clone();
        spawn_blocking(move || apply_local(&root, request)).await
    }

    pub async fn list(&self) -> Result<Vec<ToolReceipt>, ToolAcquireError> {
        let root = self.root.clone();
        spawn_blocking(move || list(&root)).await
    }

    pub fn list_sync(&self) -> Result<Vec<ToolReceipt>, ToolAcquireError> {
        list(&self.root)
    }

    /// Host-visible store root (installs, shims, receipts, current pointers).
    pub fn root_path(&self) -> &Path {
        &self.root
    }

    /// Versioned install directory for a tool, when a current pointer exists.
    pub fn install_dir(&self, name: &str) -> Result<Option<PathBuf>, ToolAcquireError> {
        parse_tool_name(name)?;
        let Some(version) = read_pointer(&self.root, name)? else {
            return Ok(None);
        };
        let directory = join_under(&self.root, &["installs", name, &version])?;
        if !directory.exists() {
            return Ok(None);
        }
        ensure_real_directory(&directory)?;
        Ok(Some(directory))
    }

    /// Read a verified installation's identity without changing the current pointer.
    /// `None` selects the current version. A missing installation returns `None`;
    /// malformed records, links, and missing executable files return an error.
    pub fn installed_receipt(
        &self,
        name: &str,
        version: Option<&str>,
    ) -> Result<Option<ToolReceipt>, ToolAcquireError> {
        parse_tool_name(name)?;
        let version = match version {
            Some(version) => version.to_string(),
            None => match read_pointer(&self.root, name)? {
                Some(version) => version,
                None => return Ok(None),
            },
        };
        parse_tool_version(&version)?;
        let directory = join_under(&self.root, &["installs", name, &version])?;
        if !directory
            .try_exists()
            .map_err(|error| ToolAcquireError::io("inspect installed version", &directory, error))?
        {
            return Ok(None);
        }
        ensure_real_directory(&directory)?;
        let Some(receipt) = read_version_receipt(&self.root, name, &version)? else {
            return Ok(None);
        };
        crate::archive::installation_paths(&self.root, &receipt)?;
        Ok(Some(receipt))
    }

    /// List identified installed versions without consulting or changing the current pointer.
    pub fn installed_versions(&self, name: &str) -> Result<Vec<ToolReceipt>, ToolAcquireError> {
        parse_tool_name(name)?;
        let mut remaining = MAX_VERSIONS;
        self.version_records(name, VersionRecordCheck::Complete, &mut remaining)
    }

    /// Select all recorded installations for removal, without requiring current
    /// pointers or complete payloads. No network requests or deletions occur.
    /// Malformed identities, links, filesystem failures and more than 10000
    /// installation-tree entries return an error. This is blocking I/O.
    pub fn all_uninstall_candidates(&self) -> Result<Vec<ToolReceipt>, ToolAcquireError> {
        let _lock = InstallLock::acquire(&self.root)?;
        self.recorded_versions()
    }

    fn recorded_versions(&self) -> Result<Vec<ToolReceipt>, ToolAcquireError> {
        let directory = join_under(&self.root, &["installs"])?;
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => {
                return Err(ToolAcquireError::io(
                    "list installed tools",
                    &directory,
                    error,
                ))
            }
        };
        ensure_real_directory(&directory)?;
        let mut remaining = MAX_VERSIONS;
        let mut selected = Vec::new();
        for entry in entries {
            remaining = remaining
                .checked_sub(1)
                .ok_or(ToolAcquireError::VersionListLimit)?;
            let entry = entry.map_err(|error| {
                ToolAcquireError::io("read installed tool entry", &directory, error)
            })?;
            let name = entry.file_name();
            let name = name
                .to_str()
                .ok_or_else(|| ToolAcquireError::PayloadInvalid { path: entry.path() })?;
            parse_tool_name(name)?;
            selected.extend(self.version_records(
                name,
                VersionRecordCheck::Identity,
                &mut remaining,
            )?);
        }
        selected.sort_by(|a, b| {
            a.name
                .cmp(&b.name)
                .then_with(|| compare_versions(&a.version, &b.version))
        });
        for receipt in &selected {
            crate::archive::ArchiveRecord::read(&self.root, receipt)?;
        }
        Ok(selected)
    }

    /// Select exact or prefix-matching recorded installations without changing them.
    /// An exact stored label wins over prefix matches, including legacy operator-like
    /// labels. Missing payload files do not prevent selection for removal. Malformed
    /// identity records, unsupported requests and filesystem failures are returned.
    /// Bare fragments retain all matches. Operators select only an installed
    /// provider-resolved version, without falling back to an older installation.
    /// This is blocking I/O. Operators and provider aliases use source-identified
    /// cached metadata; this method never performs network requests.
    pub fn uninstall_candidates(
        &self,
        name: &str,
        request: Option<&str>,
    ) -> Result<Vec<ToolReceipt>, ToolAcquireError> {
        parse_tool_name(name)?;
        let request = match request {
            Some(raw) => {
                let exact = if parse_tool_version(raw).is_ok() {
                    read_version_receipt(&self.root, name, raw)?
                } else {
                    None
                };
                if let Some(receipt) = exact {
                    crate::archive::ArchiveRecord::read(&self.root, &receipt)?;
                    return Ok(vec![receipt]);
                }
                let request = VersionRequest::parse(raw)?;
                if matches!(
                    request.kind(),
                    VersionRequestKind::Path(_)
                        | VersionRequestKind::System
                        | VersionRequestKind::Reference { .. }
                ) {
                    return Err(ToolAcquireError::UnsupportedVersionRequest(
                        request.canonical(),
                    ));
                }
                Some(request)
            }
            None => None,
        };
        let mut remaining = MAX_VERSIONS;
        let installed = self.version_records(name, VersionRecordCheck::Identity, &mut remaining)?;
        let mut selected: Vec<_> = installed
            .iter()
            .filter(|receipt| {
                request.as_ref().is_none_or(|request| {
                    matches!(request.kind(), VersionRequestKind::Version(_))
                        && request.matches_version(&receipt.version) == Some(true)
                })
            })
            .cloned()
            .collect();
        if selected.is_empty() {
            if let (Some(request), Some(first)) = (&request, installed.first()) {
                if let Some(other) = installed.iter().find(|receipt| {
                    receipt.spec != first.spec || receipt.executable != first.executable
                }) {
                    return Err(ToolAcquireError::InstallationIdentityMismatch {
                        name: other.name.clone(),
                        version: other.version.clone(),
                    });
                }
                let spec = parse_tool_spec(&first.spec)?;
                let version = match self.remote_versions_with(&spec, None)?.resolve(request) {
                    Ok(version) => version,
                    Err(ToolAcquireError::VersionNotFound(_)) => return Ok(Vec::new()),
                    Err(error) => return Err(error),
                };
                selected = installed
                    .into_iter()
                    .filter(|receipt| receipt.version == version)
                    .collect();
            }
        }
        for receipt in &selected {
            crate::archive::ArchiveRecord::read(&self.root, receipt)?;
        }
        Ok(selected)
    }

    fn version_records(
        &self,
        name: &str,
        check: VersionRecordCheck,
        remaining: &mut usize,
    ) -> Result<Vec<ToolReceipt>, ToolAcquireError> {
        let directory = join_under(&self.root, &["installs", name])?;
        if !directory.try_exists().map_err(|error| {
            ToolAcquireError::io("inspect installed versions", &directory, error)
        })? {
            return Ok(Vec::new());
        }
        ensure_real_directory(&directory)?;
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => {
                return Err(ToolAcquireError::io(
                    "list installed versions",
                    &directory,
                    error,
                ))
            }
        };
        let mut receipts = Vec::new();
        for entry in entries {
            *remaining = remaining
                .checked_sub(1)
                .ok_or(ToolAcquireError::VersionListLimit)?;
            let entry = entry.map_err(|error| {
                ToolAcquireError::io("read installed version", &directory, error)
            })?;
            let file_name = entry.file_name();
            let name_on_disk = file_name
                .to_str()
                .ok_or_else(|| ToolAcquireError::PayloadInvalid { path: entry.path() })?;
            if name_on_disk.starts_with('.') {
                continue;
            }
            let receipt = match check {
                VersionRecordCheck::Complete => self.installed_receipt(name, Some(name_on_disk))?,
                VersionRecordCheck::Identity => {
                    let path = entry.path();
                    let metadata = fs::symlink_metadata(&path).map_err(|error| {
                        ToolAcquireError::io("inspect installed version", &path, error)
                    })?;
                    if metadata_is_link_or_reparse_point(&metadata) || !metadata.is_dir() {
                        return Err(ToolAcquireError::LinkRefused { path });
                    }
                    read_version_receipt(&self.root, name, name_on_disk)?
                }
            };
            if let Some(receipt) = receipt {
                receipts.push(receipt);
            }
        }
        receipts.sort_by(|a, b| compare_versions(&a.version, &b.version));
        Ok(receipts)
    }

    /// Resolve a managed request against identified local installations and cached aliases.
    /// Missing matches return `None`; malformed identity or cache data returns an error.
    pub fn installed_for(
        &self,
        tool: &crate::ResolvedTool,
    ) -> Result<Option<ToolReceipt>, ToolAcquireError> {
        self.installed_selection(tool)
            .map(|selection| selection.map(|(receipt, _)| receipt))
    }

    pub(crate) fn installed_selection(
        &self,
        tool: &crate::ResolvedTool,
    ) -> Result<Option<(ToolReceipt, VersionRequest)>, ToolAcquireError> {
        self.select_installed(tool, false)
    }

    /// Select the newest complete installation for this tool identity, ignoring
    /// its requested version and provider aliases. Provider prerelease metadata
    /// is retained, and stable installs are preferred. No network requests or
    /// pointer updates occur. Identity, cache and filesystem errors are returned.
    /// This is blocking I/O.
    pub fn latest_installed_for(
        &self,
        tool: &crate::ResolvedTool,
    ) -> Result<Option<ToolReceipt>, ToolAcquireError> {
        self.select_installed(tool, true)
            .map(|selection| selection.map(|(receipt, _)| receipt))
    }

    fn select_installed(
        &self,
        tool: &crate::ResolvedTool,
        latest_installed: bool,
    ) -> Result<Option<(ToolReceipt, VersionRequest)>, ToolAcquireError> {
        let request = VersionRequest::parse(if latest_installed {
            "latest"
        } else {
            tool.version.as_deref().unwrap_or("latest")
        })?;
        let effective;
        let receipt = match request.kind() {
            VersionRequestKind::Path(_)
            | VersionRequestKind::System
            | VersionRequestKind::Reference { .. } => {
                return Err(ToolAcquireError::UnsupportedVersionRequest(
                    request.canonical(),
                ))
            }
            _ => {
                let spec = parse_tool_spec(&tool.spec)?;
                let installed: Vec<_> = self
                    .installed_versions(&tool.name)?
                    .into_iter()
                    .filter(|receipt| {
                        receipt.spec == tool.spec && receipt.executable == tool.executable
                    })
                    .collect();
                let cached = self.cached_versions(&spec)?;
                if matches!(request.kind(), VersionRequestKind::Subtract { base, .. } if matches!(base.kind(), VersionRequestKind::Latest))
                    && cached.is_none()
                {
                    return Err(ToolAcquireError::VersionMetadataMissing {
                        spec: tool.spec.clone(),
                    });
                }
                let flags: BTreeMap<_, _> = cached
                    .as_ref()
                    .map(|listing| {
                        listing
                            .versions()
                            .iter()
                            .map(|entry| (entry.version(), entry.is_prerelease()))
                            .collect()
                    })
                    .unwrap_or_default();
                let listing = VersionListing::new(
                    installed
                        .iter()
                        .map(|receipt| {
                            (
                                receipt.version.clone(),
                                flags
                                    .get(receipt.version.as_str())
                                    .copied()
                                    .unwrap_or_else(|| semver_prerelease(&receipt.version)),
                            )
                        })
                        .collect(),
                    cached
                        .as_ref()
                        .map(|listing| listing.aliases().clone())
                        .unwrap_or_default(),
                )?;
                let resolver = cached.as_ref().unwrap_or(&listing);
                effective = if latest_installed {
                    request.clone()
                } else {
                    resolver.effective_request(&request)?
                };
                let version = match listing.select_installed(&effective) {
                    Ok(version) => version,
                    Err(ToolAcquireError::VersionNotFound(_)) => return Ok(None),
                    Err(error) => return Err(error),
                };
                installed
                    .into_iter()
                    .find(|receipt| receipt.version == version)
            }
        };
        if let Some(receipt) = &receipt {
            if receipt.spec != tool.spec || receipt.executable != tool.executable {
                return Err(ToolAcquireError::InstallationIdentityMismatch {
                    name: receipt.name.clone(),
                    version: receipt.version.clone(),
                });
            }
        }
        Ok(receipt.map(|receipt| (receipt, effective)))
    }

    /// Fetch and cache a provider listing, or read only cached metadata when `source` is absent.
    pub fn remote_versions_with(
        &self,
        spec: &ToolSpec,
        source: Option<&dyn ToolSource>,
    ) -> Result<VersionListing, ToolAcquireError> {
        let Some(source) = source else {
            return self.cached_versions(spec)?.ok_or_else(|| {
                ToolAcquireError::VersionMetadataMissing {
                    spec: spec.raw().into(),
                }
            });
        };
        let listing = source.versions(spec)?;
        let bytes = json_bytes(&VersionCache {
            schema: VERSION_CACHE_SCHEMA,
            spec: spec.raw(),
            listing: &listing,
        })?;
        if bytes.len() as u64 > MAX_VERSION_CACHE_BYTES {
            return Err(ToolAcquireError::VersionMetadataLimit);
        }
        ensure_parent_real(&self.root, &["cache", "versions"])?;
        write_atomic(&self.version_cache_path(spec)?, &bytes)?;
        Ok(listing)
    }

    pub(crate) fn cached_versions(
        &self,
        spec: &ToolSpec,
    ) -> Result<Option<VersionListing>, ToolAcquireError> {
        let path = self.version_cache_path(spec)?;
        if !path
            .try_exists()
            .map_err(|error| ToolAcquireError::io("inspect version cache", &path, error))?
        {
            return Ok(None);
        }
        let record: VersionCacheData = read_json_bounded(&path, MAX_VERSION_CACHE_BYTES)?;
        if record.schema != VERSION_CACHE_SCHEMA || record.spec != spec.raw() {
            return Err(ToolAcquireError::InvalidSpec(
                "version cache identity mismatch".into(),
            ));
        }
        VersionListing::from_data(record.listing).map(Some)
    }

    fn version_cache_path(&self, spec: &ToolSpec) -> Result<PathBuf, ToolAcquireError> {
        join_under(
            &self.root,
            &[
                "cache",
                "versions",
                &format!(
                    "{}.json",
                    hex_encode(&Sha256::digest(spec.raw().as_bytes()))
                ),
            ],
        )
    }

    /// Shim directory that should be prepended to PATH.
    pub fn shim_dir(&self) -> PathBuf {
        self.root.join("shims")
    }

    /// Rebuild Unix dispatch shims for every command in the installed receipts.
    /// The host renders a shell body for each `(tool, command)`; it owns selected
    /// version verification and execution. No tool is installed or selected here.
    /// Rendering holds the store lock and must not call store mutations.
    /// Only obsolete shims carrying this method's header are removed. I/O and
    /// malformed receipt errors are returned; non-Unix targets refuse explicitly.
    pub fn reshim_with(
        &self,
        render: impl Fn(&str, &str) -> Result<Vec<u8>, ToolAcquireError>,
    ) -> Result<usize, ToolAcquireError> {
        if !cfg!(unix) {
            return Err(ToolAcquireError::InvalidInstallation(
                "project dispatch shims require a Unix host".into(),
            ));
        }
        let _lock = InstallLock::acquire(&self.root)?;
        let receipts = self.recorded_versions()?;
        let mut commands = BTreeMap::new();
        for receipt in receipts {
            let names = match crate::archive::ArchiveRecord::read(&self.root, &receipt)? {
                Some(record) => record.entrypoints.into_keys().collect::<Vec<_>>(),
                None => {
                    #[cfg(not(unix))]
                    let names = vec![receipt.executable.clone()];
                    #[cfg(unix)]
                    let mut names = vec![receipt.executable.clone()];
                    #[cfg(unix)]
                    if let Some(files) = self.installation_files(&receipt.name, &receipt.version)? {
                        use std::os::unix::fs::PermissionsExt;
                        for name in files.into_keys().filter(|name| name != &receipt.executable) {
                            if Path::new(&name).components().count() != 1 {
                                continue;
                            }
                            let path = self
                                .root
                                .join("installs")
                                .join(&receipt.name)
                                .join(&receipt.version)
                                .join(&name);
                            let metadata = match fs::symlink_metadata(&path) {
                                Ok(metadata) => metadata,
                                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                                    continue
                                }
                                Err(error) => {
                                    return Err(ToolAcquireError::io(
                                        "inspect shim command",
                                        &path,
                                        error,
                                    ))
                                }
                            };
                            if metadata.is_file()
                                && !metadata_is_link_or_reparse_point(&metadata)
                                && metadata.permissions().mode() & 0o111 != 0
                            {
                                names.push(name);
                            }
                        }
                    }
                    names
                }
            };
            for name in names {
                commands.insert(name, receipt.name.clone());
                if commands.len() > MAX_TREE_ENTRIES {
                    return Err(ToolAcquireError::InvalidInstallation(
                        "dispatch shim count exceeds 8192".into(),
                    ));
                }
            }
        }
        let scripts = commands
            .iter()
            .map(|(command, tool)| {
                let mut script = DISPATCH_SHIM_HEADER.to_vec();
                script.extend(render(tool, command)?);
                Ok((command, script))
            })
            .collect::<Result<Vec<_>, ToolAcquireError>>()?;
        fs::create_dir_all(self.shim_dir()).map_err(|error| {
            ToolAcquireError::io("create command shims", self.shim_dir(), error)
        })?;
        ensure_real_directory(&self.shim_dir())?;
        let names: BTreeSet<_> = commands.keys().map(std::ffi::OsString::from).collect();
        let mut obsolete = Vec::new();
        let mut remaining = MAX_TREE_ENTRIES;
        for entry in fs::read_dir(self.shim_dir())
            .map_err(|error| ToolAcquireError::io("list command shims", self.shim_dir(), error))?
        {
            remaining = remaining.checked_sub(1).ok_or_else(|| {
                ToolAcquireError::InvalidInstallation("shim directory exceeds 8192 entries".into())
            })?;
            let entry = entry.map_err(|error| {
                ToolAcquireError::io("read command shim", self.shim_dir(), error)
            })?;
            if names.contains(entry.file_name().as_os_str()) {
                continue;
            }
            let metadata = fs::symlink_metadata(entry.path()).map_err(|error| {
                ToolAcquireError::io("inspect command shim", entry.path(), error)
            })?;
            if !metadata.is_file() || metadata_is_link_or_reparse_point(&metadata) {
                continue;
            }
            if metadata.len() < DISPATCH_SHIM_HEADER.len() as u64 {
                continue;
            }
            let mut header = vec![0; DISPATCH_SHIM_HEADER.len()];
            let mut file = File::open(entry.path()).map_err(|error| {
                ToolAcquireError::io("read command shim header", entry.path(), error)
            })?;
            file.read_exact(&mut header).map_err(|error| {
                ToolAcquireError::io("read command shim header", entry.path(), error)
            })?;
            if header == DISPATCH_SHIM_HEADER {
                obsolete.push(entry.path());
            }
        }
        for (command, script) in scripts {
            let path = self.shim_dir().join(command);
            write_atomic(&path, &script)?;
            set_mode(&path, 0o755)?;
        }
        for path in obsolete {
            fs::remove_file(&path).map_err(|error| {
                ToolAcquireError::io("remove obsolete command shim", &path, error)
            })?;
        }
        Ok(commands.len())
    }

    /// Read source-declared installation membership. Legacy installs may have no manifest.
    /// This reads bounded metadata without hashing file contents or changing current pointers.
    pub fn installation_files(
        &self,
        name: &str,
        version: &str,
    ) -> Result<Option<BTreeMap<String, String>>, ToolAcquireError> {
        parse_tool_name(name)?;
        parse_tool_version(version)?;
        let path = join_under(
            &self.root,
            &["receipts", name, &format!("{version}.files.json")],
        )?;
        if !path
            .try_exists()
            .map_err(|error| ToolAcquireError::io("inspect installation manifest", &path, error))?
        {
            return Ok(None);
        }
        let manifest: InstallManifest = read_json_bounded(&path, MAX_MANIFEST_BYTES)?;
        if manifest.schema != MANIFEST_SCHEMA
            || manifest.files.is_empty()
            || manifest.files.len() > MAX_TREE_ENTRIES
        {
            return Err(ToolAcquireError::InvalidInstallation(
                "invalid manifest schema or file count".into(),
            ));
        }
        for (path, hash) in &manifest.files {
            validate_relative_path(path)?;
            normalize_sha256(hash)?;
        }
        Ok(Some(manifest.files))
    }

    pub async fn remove(&self, name: &str) -> Result<(), ToolAcquireError> {
        let root = self.root.clone();
        let name = name.to_string();
        spawn_blocking(move || remove(&root, &name)).await
    }

    pub fn remove_sync(&self, name: &str) -> Result<(), ToolAcquireError> {
        remove(&self.root, name)
    }

    /// Remove one exact installed version and its records, leaving other versions intact.
    /// Removing the current version clears its pointer and command shims; it does not
    /// select another version or edit declarations. An identified version can be
    /// removed with missing payload files. Versions without records are a no-op.
    /// Filesystem validation and removal failures are returned.
    /// Canceling the future does not stop a blocking removal that has begun.
    pub async fn remove_version(&self, name: &str, version: &str) -> Result<(), ToolAcquireError> {
        let store = self.clone();
        let name = name.to_owned();
        let version = version.to_owned();
        spawn_blocking(move || store.remove_version_sync(&name, &version)).await
    }

    /// Blocking equivalent of [`Self::remove_version`], with the same errors and effects.
    pub fn remove_version_sync(&self, name: &str, version: &str) -> Result<(), ToolAcquireError> {
        parse_tool_name(name)?;
        parse_tool_version(version)?;
        let _lock = InstallLock::acquire(&self.root)?;
        let Some(receipt) = read_version_receipt(&self.root, name, version)? else {
            return Ok(());
        };
        if read_pointer(&self.root, name)?.as_deref() == Some(version) {
            if read_receipt(&self.root, name)? != receipt {
                return Err(ToolAcquireError::ReceiptDisagrees { name: name.into() });
            }
            remove_shims(&self.root, &receipt)?;
            remove_owned_file(&join_under(&self.root, &["current", name])?)?;
            remove_owned_file(&join_under(
                &self.root,
                &["receipts", &format!("{name}.json")],
            )?)?;
        } else {
            crate::archive::ArchiveRecord::read(&self.root, &receipt)?;
        }
        remove_owned_tree(&join_under(&self.root, &["installs", name, version])?)?;
        for suffix in [".files.json", ".archive.json", ".json"] {
            remove_owned_file(&join_under(
                &self.root,
                &["receipts", name, &format!("{version}{suffix}")],
            )?)?;
        }
        Ok(())
    }
}

pub(crate) fn apply_local(
    root: &Path,
    request: LocalApply,
) -> Result<ToolReceipt, ToolAcquireError> {
    parse_tool_name(&request.name)?;
    parse_tool_name(&request.executable_name)?;
    let spec = parse_tool_spec(&request.spec)?;
    parse_tool_version(&request.version)?;
    let expected = match &request.sha256 {
        Some(value) => Some(normalize_sha256(value)?),
        None if request.require_checksum => return Err(ToolAcquireError::ChecksumRequired),
        None => None,
    };

    let _lock = InstallLock::acquire(root)?;
    if let Some(current) = read_pointer(root, &request.name)? {
        if current == request.version {
            return same_version(root, &request, &spec, expected.as_deref());
        }
    }

    let staging_name = format!(".staging-{}", request.version);
    let staging = join_under(root, &["installs", &request.name, &staging_name])?;
    ensure_parent_real(root, &["installs", &request.name])?;
    remove_owned_tree(&staging)?;
    fs::create_dir_all(&staging)
        .map_err(|error| ToolAcquireError::io("create tool staging directory", &staging, error))?;

    let staged_executable = join_under(
        root,
        &[
            "installs",
            &request.name,
            &staging_name,
            &request.executable_name,
        ],
    )?;
    let hashed = match stage_payload(&request.payload, &staged_executable) {
        Ok(hashed) => hashed,
        Err(error) => {
            remove_owned_tree(&staging)?;
            return Err(error);
        }
    };
    if let Some(expected) = expected.as_deref() {
        if hashed != expected {
            remove_owned_tree(&staging)?;
            record_failure(root, &request, &ToolAcquireError::ChecksumMismatch)?;
            return Err(ToolAcquireError::ChecksumMismatch);
        }
    }
    if let Err(error) = mark_executable(&staged_executable) {
        remove_owned_tree(&staging)?;
        record_failure(root, &request, &error)?;
        return Err(error);
    }
    if let Err(error) = stage_companions(&staging, &request.executable_name, &request.companions) {
        remove_owned_tree(&staging)?;
        record_failure(root, &request, &error)?;
        return Err(error);
    }
    let manifest =
        match InstallManifest::from_payload(&request.executable_name, &hashed, &request.companions)
        {
            Ok(manifest) => manifest,
            Err(error) => {
                remove_owned_tree(&staging)?;
                return Err(error);
            }
        };
    if json_bytes(&manifest)?.len() as u64 > MAX_MANIFEST_BYTES {
        remove_owned_tree(&staging)?;
        return Err(ToolAcquireError::InvalidInstallation(
            "manifest exceeds 1 MiB".into(),
        ));
    }

    let version_dir = join_under(root, &["installs", &request.name, &request.version])?;
    if version_dir.exists() {
        let pointer = read_pointer(root, &request.name)?;
        if pointer.as_deref() == Some(request.version.as_str()) {
            remove_owned_tree(&staging)?;
            return Err(ToolAcquireError::VersionConflict {
                version: request.version,
            });
        }
        remove_owned_tree(&version_dir)?;
    }
    fs::rename(&staging, &version_dir).map_err(|error| {
        ToolAcquireError::io("publish tool version directory", &version_dir, error)
    })?;

    let receipt = ToolReceipt::new(
        &request.name,
        spec.raw(),
        &request.version,
        &request.executable_name,
        expected.as_deref().unwrap_or(&hashed),
    )?;
    ensure_parent_real(root, &["receipts", &receipt.name])?;
    let version_receipt = join_under(
        root,
        &[
            "receipts",
            &receipt.name,
            &format!("{}.json", receipt.version),
        ],
    )?;
    write_atomic(&version_receipt, &json_bytes(&receipt)?)?;
    write_manifest(root, &receipt, &manifest)?;
    publish_current(root, &receipt)?;
    clear_failure(root, &request.name)?;
    Ok(receipt)
}

fn same_version(
    root: &Path,
    request: &LocalApply,
    spec: &ToolSpec,
    expected: Option<&str>,
) -> Result<ToolReceipt, ToolAcquireError> {
    let receipt = read_receipt(root, &request.name)?;
    let matches = receipt.spec == spec.raw()
        && receipt.version == request.version
        && receipt.executable == request.executable_name
        && expected.is_none_or(|expected| expected == receipt.sha256);
    if !matches {
        return Err(ToolAcquireError::VersionConflict {
            version: request.version.clone(),
        });
    }
    let executable = join_under(
        root,
        &[
            "installs",
            &request.name,
            &request.version,
            &request.executable_name,
        ],
    )?;
    ensure_real_file(&executable)?;
    let manifest =
        InstallManifest::from_payload(&receipt.executable, &receipt.sha256, &request.companions)?;
    let directory = executable
        .parent()
        .ok_or_else(|| ToolAcquireError::PayloadInvalid {
            path: executable.clone(),
        })?;
    let mut remaining = MAX_TREE_BYTES;
    if fingerprint(directory, &mut remaining)? != manifest.files {
        return Err(ToolAcquireError::InstallationIdentityMismatch {
            name: receipt.name.clone(),
            version: receipt.version.clone(),
        });
    }
    write_manifest(root, &receipt, &manifest)?;
    Ok(receipt)
}

fn write_manifest(
    root: &Path,
    receipt: &ToolReceipt,
    manifest: &InstallManifest,
) -> Result<(), ToolAcquireError> {
    let path = join_under(
        root,
        &[
            "receipts",
            &receipt.name,
            &format!("{}.files.json", receipt.version),
        ],
    )?;
    let bytes = json_bytes(manifest)?;
    if bytes.len() as u64 > MAX_MANIFEST_BYTES {
        return Err(ToolAcquireError::InvalidInstallation(
            "manifest exceeds 1 MiB".into(),
        ));
    }
    ensure_parent_real(root, &["receipts", &receipt.name])?;
    write_atomic(&path, &bytes)
}

pub(crate) fn publish_current(root: &Path, receipt: &ToolReceipt) -> Result<(), ToolAcquireError> {
    ensure_real_directory(&join_under(root, &["shims"])?)?;
    ensure_real_directory(&join_under(root, &["current"])?)?;
    ensure_real_directory(&join_under(root, &["receipts"])?)?;
    let archive = crate::archive::ArchiveRecord::read(root, receipt)?;
    let entrypoints = match &archive {
        Some(record) => record.entrypoints.clone(),
        None => BTreeMap::from([(
            receipt.executable.clone(),
            PathBuf::from(&receipt.executable),
        )]),
    };
    for (name, target) in entrypoints {
        let shim = join_under(root, &["shims", &shim_name(&name, archive.is_some())])?;
        let target = crate::archive::portable_path(&target)?;
        write_atomic(&shim, &shim_bytes(&receipt.name, &target))?;
        set_mode(&shim, 0o755)?;
    }
    let pointer = join_under(root, &["current", &receipt.name])?;
    let previous =
        if pointer.exists() {
            ensure_real_file(&pointer)?;
            Some(fs::read(&pointer).map_err(|error| {
                ToolAcquireError::io("read current tool version", &pointer, error)
            })?)
        } else {
            None
        };
    write_atomic(&pointer, format!("{}\n", receipt.version).as_bytes())?;
    let receipt_path = join_under(root, &["receipts", &format!("{}.json", receipt.name)])?;
    if let Err(error) = write_atomic(&receipt_path, &json_bytes(receipt)?) {
        restore_pointer(&pointer, previous)?;
        return Err(error);
    }
    Ok(())
}

fn restore_pointer(pointer: &Path, previous: Option<Vec<u8>>) -> Result<(), ToolAcquireError> {
    match previous {
        Some(bytes) => write_atomic(pointer, &bytes),
        None => remove_owned_file(pointer),
    }
}

fn list(root: &Path) -> Result<Vec<ToolReceipt>, ToolAcquireError> {
    let _lock = InstallLock::acquire(root)?;
    let receipts = join_under(root, &["receipts"])?;
    if !receipts.exists() {
        return Ok(Vec::new());
    }
    ensure_real_directory(&receipts)?;
    let mut names = Vec::new();
    for entry in fs::read_dir(&receipts)
        .map_err(|error| ToolAcquireError::io("read tool receipts", &receipts, error))?
    {
        let entry = entry
            .map_err(|error| ToolAcquireError::io("read tool receipt entry", &receipts, error))?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if name.ends_with(".failure.json") || !name.ends_with(".json") || name.starts_with('.') {
            continue;
        }
        names.push(name.trim_end_matches(".json").to_string());
    }
    names.sort();
    let mut listed = Vec::with_capacity(names.len());
    for name in names {
        let receipt = read_receipt(root, &name)?;
        let pointer = read_pointer(root, &name)?;
        if pointer.as_deref() != Some(receipt.version.as_str()) {
            return Err(ToolAcquireError::ReceiptDisagrees { name });
        }
        listed.push(receipt);
    }
    Ok(listed)
}

fn remove(root: &Path, name: &str) -> Result<(), ToolAcquireError> {
    parse_tool_name(name)?;
    let _lock = InstallLock::acquire(root)?;
    let receipt = join_under(root, &["receipts", &format!("{name}.json")])?;
    if receipt.exists() {
        let parsed = read_receipt(root, name)?;
        remove_shims(root, &parsed)?;
    }
    remove_owned_file(&receipt)?;
    remove_owned_file(&join_under(
        root,
        &["receipts", &format!("{name}.failure.json")],
    )?)?;
    remove_owned_file(&join_under(root, &["current", name])?)?;
    remove_owned_tree(&join_under(root, &["receipts", name])?)?;
    remove_owned_tree(&join_under(root, &["installs", name])?)?;
    Ok(())
}

fn remove_shims(root: &Path, receipt: &ToolReceipt) -> Result<(), ToolAcquireError> {
    let archive = crate::archive::ArchiveRecord::read(root, receipt)?;
    let names = match &archive {
        Some(record) => record.entrypoints.keys().cloned().collect(),
        None => vec![receipt.executable.clone()],
    };
    for name in names {
        remove_owned_file(&join_under(
            root,
            &["shims", &shim_name(&name, archive.is_some())],
        )?)?;
    }
    Ok(())
}

fn stage_payload(source: &Path, destination: &Path) -> Result<String, ToolAcquireError> {
    let metadata = fs::symlink_metadata(source)
        .map_err(|error| ToolAcquireError::io("inspect tool payload", source, error))?;
    if metadata_is_link_or_reparse_point(&metadata) || !metadata.is_file() {
        return Err(ToolAcquireError::PayloadInvalid {
            path: source.to_path_buf(),
        });
    }
    if metadata.len() == 0 {
        return Err(ToolAcquireError::NotExecutable);
    }
    let mut input = File::open(source)
        .map_err(|error| ToolAcquireError::io("open tool payload", source, error))?;
    let mut output = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(destination)
        .map_err(|error| ToolAcquireError::io("create staged tool", destination, error))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = input
            .read(&mut buffer)
            .map_err(|error| ToolAcquireError::io("read tool payload", source, error))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        output
            .write_all(&buffer[..read])
            .map_err(|error| ToolAcquireError::io("write staged tool", destination, error))?;
    }
    output
        .sync_all()
        .map_err(|error| ToolAcquireError::io("sync staged tool", destination, error))?;
    Ok(hex_encode(&hasher.finalize()))
}

fn stage_companions(
    staging: &Path,
    executable_name: &str,
    companions: &[CompanionFile],
) -> Result<(), ToolAcquireError> {
    for companion in companions {
        let destination =
            companion_destination(staging, executable_name, &companion.relative_path)?;
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                ToolAcquireError::io("create companion directory", parent, error)
            })?;
        }
        let mut output = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&destination)
            .map_err(|error| ToolAcquireError::io("create companion file", &destination, error))?;
        output
            .write_all(&companion.bytes)
            .map_err(|error| ToolAcquireError::io("write companion file", &destination, error))?;
        output
            .sync_all()
            .map_err(|error| ToolAcquireError::io("sync companion file", &destination, error))?;
        let mode = if companion.executable { 0o755 } else { 0o644 };
        set_mode(&destination, mode)?;
    }
    Ok(())
}

pub(crate) fn companion_destination(
    staging: &Path,
    executable_name: &str,
    relative_path: &str,
) -> Result<PathBuf, ToolAcquireError> {
    if relative_path.starts_with('/') || relative_path.contains('\0') {
        return Err(ToolAcquireError::PayloadInvalid {
            path: PathBuf::from(relative_path),
        });
    }
    let parts: Vec<&str> = relative_path
        .split(['/', '\\'])
        .filter(|part| !part.is_empty())
        .collect();
    let invalid = parts.is_empty()
        || parts.iter().any(|part| {
            *part == "."
                || *part == ".."
                || part.contains('/')
                || part.contains('\\')
                || part.contains('\0')
        });
    if invalid {
        return Err(ToolAcquireError::PayloadInvalid {
            path: PathBuf::from(relative_path),
        });
    }
    let replaces_executable = parts.len() == 1
        && (parts[0] == executable_name
            || parts[0].eq_ignore_ascii_case(&format!("{executable_name}.exe")));
    if replaces_executable {
        return Err(ToolAcquireError::PayloadInvalid {
            path: PathBuf::from(relative_path),
        });
    }
    let mut destination = staging.to_path_buf();
    for part in parts {
        destination.push(part);
    }
    Ok(destination)
}

fn mark_executable(path: &Path) -> Result<(), ToolAcquireError> {
    ensure_real_file(path)?;
    set_mode(path, 0o755)?;
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| ToolAcquireError::io("inspect staged tool", path, error))?;
    if metadata_is_link_or_reparse_point(&metadata) || !metadata.is_file() || metadata.len() == 0 {
        return Err(ToolAcquireError::NotExecutable);
    }
    Ok(())
}

fn record_failure(
    root: &Path,
    request: &LocalApply,
    error: &ToolAcquireError,
) -> Result<(), ToolAcquireError> {
    let record = ToolFailureRecord::new(&request.name, &request.spec, &request.version, error)?;
    ensure_real_directory(&join_under(root, &["receipts"])?)?;
    let path = join_under(
        root,
        &["receipts", &format!("{}.failure.json", request.name)],
    )?;
    write_atomic(&path, &json_bytes(&record)?)
}

fn clear_failure(root: &Path, name: &str) -> Result<(), ToolAcquireError> {
    remove_owned_file(&join_under(
        root,
        &["receipts", &format!("{name}.failure.json")],
    )?)
}

pub(crate) fn read_receipt(root: &Path, name: &str) -> Result<ToolReceipt, ToolAcquireError> {
    let path = join_under(root, &["receipts", &format!("{name}.json")])?;
    let receipt: ToolReceipt = read_json(&path)?;
    receipt.validate(name, None)?;
    Ok(receipt)
}

fn read_version_receipt(
    root: &Path,
    name: &str,
    version: &str,
) -> Result<Option<ToolReceipt>, ToolAcquireError> {
    let record = join_under(root, &["receipts", name, &format!("{version}.json")])?;
    let receipt: ToolReceipt = if record
        .try_exists()
        .map_err(|error| ToolAcquireError::io("inspect version receipt", &record, error))?
    {
        read_json(&record)?
    } else if read_pointer(root, name)?.as_deref() == Some(version) {
        // Older installs have only the current receipt; its exact identity remains authoritative.
        read_receipt(root, name)?
    } else {
        return Ok(None);
    };
    receipt.validate(name, Some(version))?;
    Ok(Some(receipt))
}

pub(crate) fn read_pointer(root: &Path, name: &str) -> Result<Option<String>, ToolAcquireError> {
    let path = join_under(root, &["current", name])?;
    if !path.exists() {
        return Ok(None);
    }
    ensure_real_file(&path)?;
    let text = fs::read_to_string(&path)
        .map_err(|error| ToolAcquireError::io("read current tool version", &path, error))?;
    let version = text.trim();
    parse_tool_version(version)?;
    Ok(Some(version.to_string()))
}

pub(crate) fn ensure_parent_real(root: &Path, parts: &[&str]) -> Result<(), ToolAcquireError> {
    let mut current = root.to_path_buf();
    for part in parts {
        current = join_under(&current, &[part])?;
        if current.exists() {
            ensure_real_directory(&current)?;
        } else {
            fs::create_dir(&current)
                .map_err(|error| ToolAcquireError::io("create tool directory", &current, error))?;
        }
    }
    Ok(())
}

pub(crate) fn ensure_real_directory(path: &Path) -> Result<(), ToolAcquireError> {
    if !path.exists() {
        fs::create_dir_all(path)
            .map_err(|error| ToolAcquireError::io("create tool directory", path, error))?;
    }
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| ToolAcquireError::io("inspect tool directory", path, error))?;
    if metadata_is_link_or_reparse_point(&metadata) || !metadata.is_dir() {
        return Err(ToolAcquireError::LinkRefused {
            path: path.to_path_buf(),
        });
    }
    Ok(())
}

pub(crate) fn ensure_real_file(path: &Path) -> Result<(), ToolAcquireError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| ToolAcquireError::io("inspect tool file", path, error))?;
    if metadata_is_link_or_reparse_point(&metadata) || !metadata.is_file() {
        return Err(ToolAcquireError::LinkRefused {
            path: path.to_path_buf(),
        });
    }
    Ok(())
}

fn remove_owned_file(path: &Path) -> Result<(), ToolAcquireError> {
    if !path.exists() && fs::symlink_metadata(path).is_err() {
        return Ok(());
    }
    if path.exists() || fs::symlink_metadata(path).is_ok() {
        let metadata = fs::symlink_metadata(path)
            .map_err(|error| ToolAcquireError::io("inspect tool file", path, error))?;
        if metadata_is_link_or_reparse_point(&metadata) {
            return Err(ToolAcquireError::LinkRefused {
                path: path.to_path_buf(),
            });
        }
        if metadata.is_file() {
            fs::remove_file(path)
                .map_err(|error| ToolAcquireError::io("remove tool file", path, error))?;
        }
    }
    Ok(())
}

pub(crate) fn remove_owned_tree(path: &Path) -> Result<(), ToolAcquireError> {
    if fs::symlink_metadata(path).is_err() {
        return Ok(());
    }
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| ToolAcquireError::io("inspect tool directory", path, error))?;
    if metadata_is_link_or_reparse_point(&metadata) {
        return Err(ToolAcquireError::LinkRefused {
            path: path.to_path_buf(),
        });
    }
    if metadata.is_dir() {
        fs::remove_dir_all(path)
            .map_err(|error| ToolAcquireError::io("remove tool directory", path, error))?;
    }
    Ok(())
}

pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), ToolAcquireError> {
    let parent = path.parent().ok_or_else(|| ToolAcquireError::Io {
        action: "locate tool file parent",
        path: path.to_path_buf(),
        source: std::io::Error::other("missing parent"),
    })?;
    let temporary = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("file"),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0)
    ));
    {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .map_err(|error| {
                ToolAcquireError::io("create temporary tool file", &temporary, error)
            })?;
        file.write_all(bytes).map_err(|error| {
            ToolAcquireError::io("write temporary tool file", &temporary, error)
        })?;
        file.sync_all()
            .map_err(|error| ToolAcquireError::io("sync temporary tool file", &temporary, error))?;
    }
    fs::rename(&temporary, path)
        .map_err(|error| ToolAcquireError::io("publish tool file", path, error))?;
    Ok(())
}

fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T, ToolAcquireError> {
    const MAX_RECORD_BYTES: u64 = 64 * 1024;
    read_json_bounded(path, MAX_RECORD_BYTES)
}

pub(crate) fn read_json_bounded<T: DeserializeOwned>(
    path: &Path,
    max_bytes: u64,
) -> Result<T, ToolAcquireError> {
    ensure_real_file(path)?;
    let mut bytes = Vec::new();
    File::open(path)
        .and_then(|file| file.take(max_bytes + 1).read_to_end(&mut bytes))
        .map_err(|error| ToolAcquireError::io("read tool record", path, error))?;
    if bytes.len() as u64 > max_bytes {
        return Err(ToolAcquireError::io(
            "read tool record",
            path,
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("tool record exceeds {max_bytes} bytes"),
            ),
        ));
    }
    serde_json::from_slice(&bytes).map_err(|error| {
        ToolAcquireError::io(
            "parse tool record",
            path,
            std::io::Error::new(std::io::ErrorKind::InvalidData, error),
        )
    })
}

pub(crate) fn json_bytes(value: &impl Serialize) -> Result<Vec<u8>, ToolAcquireError> {
    serde_json::to_vec_pretty(value).map_err(|error| {
        ToolAcquireError::io(
            "encode tool record",
            PathBuf::from("."),
            std::io::Error::other(error),
        )
    })
}

pub(crate) fn normalize_sha256(value: &str) -> Result<String, ToolAcquireError> {
    let value = value.trim().to_ascii_lowercase();
    if value.len() == 64 && value.chars().all(|character| character.is_ascii_hexdigit()) {
        Ok(value)
    } else {
        Err(ToolAcquireError::ChecksumMismatch)
    }
}

pub(crate) fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

fn shim_bytes(name: &str, executable: &str) -> Vec<u8> {
    #[cfg(windows)]
    {
        let executable = executable.replace('%', "%%").replace('/', "\\");
        format!(
            "@echo off\r\nsetlocal DisableDelayedExpansion\r\nset \"root=%~dp0..\"\r\nset /p version=<\"%root%\\current\\{name}\"\r\n\"%root%\\installs\\{name}\\%version%\\{executable}\" %*\r\n"
        )
        .into_bytes()
    }
    #[cfg(not(windows))]
    {
        let executable = format!("'{}'", executable.replace('\'', "'\"'\"'"));
        format!(
            "#!/bin/sh\nset -eu\nhere=$(CDPATH= cd -- \"$(dirname -- \"$0\")\" && pwd)\nroot=$(CDPATH= cd -- \"$here/..\" && pwd)\nversion=$(cat -- \"$root/current/{name}\")\nexec \"$root/installs/{name}/$version\"/{executable} \"$@\"\n"
        )
        .into_bytes()
    }
}

pub(crate) fn set_mode(path: &Path, mode: u32) -> Result<(), ToolAcquireError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(mode))
            .map_err(|error| ToolAcquireError::io("set tool file mode", path, error))?;
    }
    #[cfg(not(unix))]
    {
        let _ = (path, mode);
    }
    Ok(())
}

pub(crate) fn shim_name(name: &str, archived: bool) -> String {
    if cfg!(windows) && archived {
        format!("{name}.cmd")
    } else {
        name.to_string()
    }
}

async fn spawn_blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, ToolAcquireError> + Send + 'static,
) -> Result<T, ToolAcquireError> {
    match tokio::task::spawn_blocking(work).await {
        Ok(result) => result,
        Err(error) => Err(ToolAcquireError::Task(error.to_string())),
    }
}

#[cfg(test)]
mod tests;
