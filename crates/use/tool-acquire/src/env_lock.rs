//! Project tool locks, independent of mutable machine-local current pointers.

use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use aster_acl::builder::BlockBuilder;
use aster_acl::{generate, parse_acl, Block, Document, Value};
use sha2::{Digest, Sha256};

use crate::archive::{ArchiveLock, ArchiveRecord};
use crate::env_acl::resolve_tool_identity;
use crate::installation::{
    fingerprint, validate_relative_path, InstallManifest, MAX_TREE_BYTES, MAX_TREE_ENTRIES,
};
use crate::store::hex_encode;
use crate::{
    resolve_declarations, AcquireRequest, EnvConfig, EnvDeclarations, InstallPayload, ResolvedTool,
    ToolAcquireError, ToolPayload, ToolQuery, ToolReceipt, ToolSource, ToolSpec, ToolStore,
    VersionRequest, VersionRequestKind,
};

const SCHEMA: &str = "aster.env-lock.v3";
const MULTI_SCHEMA: &str = "aster.env-lock.v4";
const SELECTOR_SCHEMA: &str = "aster.env-lock.v2";
const LEGACY_SCHEMA: &str = "aster.env-lock.v1";
const MAX_LOCK_BYTES: u64 = 1024 * 1024;

/// An admitted lock for this platform. Environment variable values are never stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvLock {
    // shortcut: one platform per lock, expand when cross-platform distribution resolution lands.
    platform: String,
    tools: BTreeMap<String, Vec<LockedTool>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LockedTool {
    request: String,
    selector: LockedSelector,
    spec: String,
    version: String,
    executable: String,
    installation: LockedInstallation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum LockedInstallation {
    Files(BTreeMap<String, String>),
    Archive(ArchiveLock),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum LockedSelector {
    Legacy,
    Resolved(VersionRequest),
}

/// The lock beside a project's environment declaration.
pub fn project_env_lock_path(project: impl AsRef<Path>) -> PathBuf {
    project.as_ref().join(".aster").join("env.lock.acl")
}

impl EnvLock {
    /// Fingerprint all files of selected installations without changing pointers.
    /// This performs bounded blocking I/O and must run off async executor threads.
    pub fn capture(config: &EnvConfig, store: &ToolStore) -> Result<Self, ToolAcquireError> {
        Self::capture_declarations(&EnvDeclarations::from(config), store)
    }

    /// Fingerprint all ordered versions of managed declarations using a shared work budget.
    /// This is blocking I/O. Missing or changed installations return before any lock is written.
    pub fn capture_declarations(
        config: &EnvDeclarations,
        store: &ToolStore,
    ) -> Result<Self, ToolAcquireError> {
        let mut tools = BTreeMap::<String, Vec<LockedTool>>::new();
        let mut remaining = MAX_TREE_BYTES;
        for (tool, request) in resolve_declarations(config)?
            .into_iter()
            .zip(config.tools.values().flatten())
        {
            let (receipt, selector) = store
                .installed_selection(&tool)?
                .ok_or_else(|| missing(&tool.name, tool.version.as_deref().unwrap_or("latest")))?;
            if receipt.spec != tool.spec || receipt.executable != tool.executable {
                return Err(content_mismatch(&tool.name));
            }
            let installation = match ArchiveRecord::read(store.root_path(), &receipt)? {
                Some(record) => {
                    record.verify_tree(&install_path(store, &receipt), &mut remaining)?;
                    LockedInstallation::Archive(record.lock()?)
                }
                None => {
                    let files = store
                        .installation_files(&tool.name, &receipt.version)?
                        .ok_or_else(|| ToolAcquireError::InstallationManifestMissing {
                            name: tool.name.clone(),
                            version: receipt.version.clone(),
                        })?;
                    if files.get(&receipt.executable) != Some(&receipt.sha256)
                        || fingerprint(&install_path(store, &receipt), &mut remaining)? != files
                    {
                        return Err(content_mismatch(&tool.name));
                    }
                    LockedInstallation::Files(files)
                }
            };
            tools
                .entry(tool.name.clone())
                .or_default()
                .push(LockedTool {
                    selector: LockedSelector::Resolved(selector),
                    request: request.clone(),
                    spec: receipt.spec,
                    version: receipt.version,
                    executable: receipt.executable,
                    installation,
                });
        }
        Ok(Self {
            platform: platform(),
            tools,
        })
    }

    /// Read bounded UTF-8 ACL. Only an absent file returns `None`.
    pub fn read(path: impl AsRef<Path>) -> Result<Option<Self>, ToolAcquireError> {
        let path = path.as_ref();
        let file = match fs::File::open(path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(ToolAcquireError::io("open env lock", path, error)),
        };
        let mut text = String::new();
        file.take(MAX_LOCK_BYTES + 1)
            .read_to_string(&mut text)
            .map_err(|error| ToolAcquireError::io("read env lock", path, error))?;
        Self::parse(&text).map(Some)
    }

    /// Find the nearest ancestor lock, returning malformed locks and I/O errors.
    pub fn discover(
        project: impl AsRef<Path>,
    ) -> Result<Option<(PathBuf, Self)>, ToolAcquireError> {
        let absolute = std::path::absolute(project.as_ref()).map_err(|error| {
            ToolAcquireError::io("resolve lock directory", project.as_ref(), error)
        })?;
        for ancestor in absolute.ancestors() {
            let path = project_env_lock_path(ancestor);
            if let Some(lock) = Self::read(&path)? {
                return Ok(Some((path, lock)));
            }
        }
        Ok(None)
    }

    /// Admit a closed lock schema; this does not inspect local installations.
    pub fn parse(text: &str) -> Result<Self, ToolAcquireError> {
        if text.len() as u64 > MAX_LOCK_BYTES {
            return Err(invalid("lock exceeds 1 MiB"));
        }
        let document = parse_acl(text).map_err(|error| invalid(&error.to_string()))?;
        let [root] = document.blocks.as_slice() else {
            return Err(invalid("expected one env_lock block"));
        };
        if root.name != "env_lock" || !root.labels.is_empty() || root.attributes.len() != 2 {
            return Err(invalid(
                "expected env_lock with schema and platform attributes",
            ));
        }
        let schema = string_attr(root, "schema")?;
        if ![SCHEMA, SELECTOR_SCHEMA, LEGACY_SCHEMA, MULTI_SCHEMA].contains(&schema.as_str()) {
            return Err(invalid("unsupported lock schema"));
        }
        let mut tools = BTreeMap::<String, Vec<LockedTool>>::new();
        for block in &root.blocks {
            let [name] = block.labels.as_slice() else {
                return Err(invalid("tool requires one name"));
            };
            if block.name != "tool"
                || block.attributes.len() != if schema == LEGACY_SCHEMA { 4 } else { 5 }
            {
                return Err(invalid("invalid locked tool fields"));
            }
            let declared = resolve_tool_identity(name)?;
            let request = string_attr(block, "request")?;
            let spec = string_attr(block, "spec")?;
            crate::spec::parse_tool_spec(&spec)?;
            let version = string_attr(block, "version")?;
            crate::spec::parse_tool_version(&version)?;
            let executable = string_attr(block, "executable")?;
            crate::spec::parse_tool_name(&executable)?;
            if declared.spec != spec || declared.executable != executable {
                return Err(invalid("locked tool identity differs from its declaration"));
            }
            let selector = if schema == LEGACY_SCHEMA {
                crate::spec::parse_tool_version(&request)?;
                if request != "latest" && request != version {
                    return Err(invalid(
                        "locked version differs from its literal v1 request",
                    ));
                }
                LockedSelector::Legacy
            } else {
                let parsed_request = VersionRequest::parse(&request)?;
                if matches!(
                    parsed_request.kind(),
                    VersionRequestKind::Path(_)
                        | VersionRequestKind::System
                        | VersionRequestKind::Reference { .. }
                ) {
                    return Err(invalid(
                        "context-dependent requests are not supported by this managed lock",
                    ));
                }
                let selector = VersionRequest::parse(&string_attr(block, "selector")?)?;
                if !matches!(
                    selector.kind(),
                    VersionRequestKind::Version(_)
                        | VersionRequestKind::Latest
                        | VersionRequestKind::Prefix(_)
                ) {
                    return Err(invalid(
                        "locked selector must be a resolved version or prefix",
                    ));
                }
                if selector.matches_version(&version) != Some(true) {
                    return Err(invalid("locked version differs from its resolved selector"));
                }
                if matches!(parsed_request.kind(), VersionRequestKind::Prefix(_))
                    && parsed_request.matches_version(&version) != Some(true)
                {
                    return Err(invalid("locked version differs from its declared prefix"));
                }
                LockedSelector::Resolved(selector)
            };
            let installation = parse_installation(block, &schema, &executable)?;
            let tool = LockedTool {
                request,
                selector,
                spec,
                version,
                executable,
                installation,
            };
            let versions = tools.entry(name.clone()).or_default();
            if schema != MULTI_SCHEMA && !versions.is_empty() {
                return Err(invalid("duplicate locked tool"));
            }
            versions.push(tool);
        }
        Ok(Self {
            platform: string_attr(root, "platform")?,
            tools,
        })
    }

    /// Write deterministically and atomically, preserving existing symlinks and permissions.
    pub fn write(&self, path: impl AsRef<Path>) -> Result<(), ToolAcquireError> {
        let text = self.render();
        if text.len() as u64 > MAX_LOCK_BYTES {
            return Err(invalid("lock exceeds 1 MiB"));
        }
        crate::env_acl::write_acl_file(path.as_ref(), &text)
    }

    /// Serialize the admitted lock as ACL without machine-specific absolute paths.
    pub fn render(&self) -> String {
        // Re-encoding literal requests as a newer schema would change their meaning.
        let legacy = self
            .tools
            .values()
            .flatten()
            .any(|tool| matches!(tool.selector, LockedSelector::Legacy));
        let multi = self.tools.values().any(|versions| versions.len() > 1);
        let mut root = BlockBuilder::new("env_lock")
            .attr(
                "schema",
                Value::String(
                    if legacy {
                        LEGACY_SCHEMA
                    } else if multi {
                        MULTI_SCHEMA
                    } else {
                        SCHEMA
                    }
                    .into(),
                ),
            )
            .attr("platform", Value::String(self.platform.clone()));
        for (name, tool) in self
            .tools
            .iter()
            .flat_map(|(name, versions)| versions.iter().map(move |tool| (name, tool)))
        {
            let mut block = BlockBuilder::new("tool")
                .label(name)
                .attr("request", Value::String(tool.request.clone()))
                .attr("spec", Value::String(tool.spec.clone()))
                .attr("version", Value::String(tool.version.clone()))
                .attr("executable", Value::String(tool.executable.clone()));
            if let LockedSelector::Resolved(selector) = &tool.selector {
                block = block.attr("selector", Value::String(selector.canonical()));
            }
            match &tool.installation {
                LockedInstallation::Files(files) => {
                    for (path, hash) in files {
                        block = block.nested_block(
                            BlockBuilder::new("file")
                                .label(path)
                                .attr("sha256", Value::String(hash.clone()))
                                .build(),
                        );
                    }
                }
                LockedInstallation::Archive(lock) => {
                    block = block.nested_block(
                        BlockBuilder::new("archive")
                            .attr("sha256", Value::String(lock.archive_sha256().into()))
                            .attr("record_sha256", Value::String(lock.record_sha256().into()))
                            .build(),
                    );
                }
            }
            root = root.nested_block(block.build());
        }
        generate(&Document {
            blocks: vec![root.build()],
        })
    }

    /// Bind declarations to exact locked versions, refusing platform or declaration drift.
    pub fn resolve(&self, config: &EnvConfig) -> Result<Vec<ResolvedTool>, ToolAcquireError> {
        self.resolve_declarations(&EnvDeclarations::from(config))
    }

    /// Bind every ordered request, refusing changed order, membership or platform.
    /// Variable values are neither compared nor stored in the lock.
    pub fn resolve_declarations(
        &self,
        config: &EnvDeclarations,
    ) -> Result<Vec<ResolvedTool>, ToolAcquireError> {
        if self.platform != platform() {
            return Err(ToolAcquireError::LockPlatformMismatch {
                expected: self.platform.clone(),
                actual: platform(),
            });
        }
        if self.tools.len() != config.tools.len() {
            return Err(ToolAcquireError::LockDeclarationMismatch);
        }
        let mut resolved = Vec::new();
        for (name, requests) in &config.tools {
            let Some(versions) = self.tools.get(name) else {
                return Err(ToolAcquireError::LockDeclarationMismatch);
            };
            if requests.is_empty() || versions.len() != requests.len() {
                return Err(ToolAcquireError::LockDeclarationMismatch);
            }
            for (request, locked) in requests.iter().zip(versions) {
                let mut tool = resolve_tool_identity(name)?;
                if request != &locked.request
                    || tool.spec != locked.spec
                    || tool.executable != locked.executable
                {
                    return Err(ToolAcquireError::LockDeclarationMismatch);
                }
                tool.version = Some(locked.version.clone());
                resolved.push(tool);
            }
        }
        Ok(resolved)
    }

    /// Verify complete installed file sets, fetching absent versions only when a source is supplied.
    /// Downloaded bytes are checked before publication. This performs bounded blocking I/O;
    /// dropping an async caller does not cancel an active source request.
    pub fn install_with(
        &self,
        store: &ToolStore,
        source: Option<&dyn ToolSource>,
    ) -> Result<Vec<ToolReceipt>, ToolAcquireError> {
        if self.platform != platform() {
            return Err(ToolAcquireError::LockPlatformMismatch {
                expected: self.platform.clone(),
                actual: platform(),
            });
        }
        let mut receipts = Vec::with_capacity(self.tools.len());
        let mut remaining = MAX_TREE_BYTES;
        for (name, tool) in self
            .tools
            .iter()
            .flat_map(|(name, versions)| versions.iter().map(move |tool| (name, tool)))
        {
            let receipt = match store.installed_receipt(name, Some(&tool.version))? {
                Some(receipt) => receipt,
                None => {
                    let source = source.ok_or_else(|| missing(name, &tool.version))?;
                    store.acquire_with(
                        AcquireRequest {
                            name: name.clone(),
                            spec: tool.spec.clone(),
                            version: Some(tool.version.clone()),
                            executable_name: tool.executable.clone(),
                        },
                        &LockedSource { source, name, tool },
                    )?
                }
            };
            if receipt.spec != tool.spec || receipt.executable != tool.executable {
                return Err(content_mismatch(name));
            }
            let record = ArchiveRecord::read(store.root_path(), &receipt)?;
            match (&tool.installation, record) {
                (LockedInstallation::Files(files), None) => {
                    if files.get(&tool.executable) != Some(&receipt.sha256)
                        || fingerprint(&install_path(store, &receipt), &mut remaining)? != *files
                    {
                        return Err(content_mismatch(name));
                    }
                }
                (LockedInstallation::Archive(lock), Some(record)) => {
                    if !lock.matches(&record)? {
                        return Err(content_mismatch(name));
                    }
                    record.verify_tree(&install_path(store, &receipt), &mut remaining)?;
                }
                _ => return Err(content_mismatch(name)),
            }
            receipts.push(receipt);
        }
        Ok(receipts)
    }
}

struct LockedSource<'a> {
    source: &'a dyn ToolSource,
    name: &'a str,
    tool: &'a LockedTool,
}

impl ToolSource for LockedSource<'_> {
    fn fetch(&self, query: &ToolQuery) -> Result<ToolPayload, ToolAcquireError> {
        let LockedInstallation::Files(files) = &self.tool.installation else {
            return Err(content_mismatch(self.name));
        };
        let payload = self.source.fetch(query)?;
        let total_bytes = payload
            .companions
            .iter()
            .try_fold(payload.bytes.len() as u64, |total, file| {
                total.checked_add(file.bytes.len() as u64)
            });
        if payload.companions.len() >= MAX_TREE_ENTRIES
            || total_bytes.is_none_or(|bytes| bytes > MAX_TREE_BYTES)
        {
            return Err(invalid("downloaded installation exceeds lock limits"));
        }
        let manifest = InstallManifest::from_payload(
            &self.tool.executable,
            &hex_encode(&Sha256::digest(&payload.bytes)),
            &payload.companions,
        )?;
        if payload.version != self.tool.version || manifest.files != *files {
            return Err(content_mismatch(self.name));
        }
        Ok(payload)
    }

    fn fetch_installation(&self, query: &ToolQuery) -> Result<InstallPayload, ToolAcquireError> {
        match &self.tool.installation {
            LockedInstallation::Files(_) => self.fetch(query).map(InstallPayload::Executable),
            LockedInstallation::Archive(lock) => {
                let InstallPayload::Archive(payload) = self.source.fetch_installation(query)?
                else {
                    return Err(content_mismatch(self.name));
                };
                if payload.version() != self.tool.version {
                    return Err(content_mismatch(self.name));
                }
                payload.locked_to(lock).map(InstallPayload::Archive)
            }
        }
    }

    fn latest(&self, spec: &ToolSpec) -> Result<String, ToolAcquireError> {
        self.source.latest(spec)
    }
}

fn parse_installation(
    block: &Block,
    schema: &str,
    executable: &str,
) -> Result<LockedInstallation, ToolAcquireError> {
    if let [archive] = block.blocks.as_slice() {
        if archive.name == "archive" {
            if ![SCHEMA, MULTI_SCHEMA].contains(&schema)
                || !archive.labels.is_empty()
                || archive.attributes.len() != 2
                || !archive.blocks.is_empty()
            {
                return Err(invalid("invalid locked archive fields"));
            }
            return Ok(LockedInstallation::Archive(ArchiveLock::new(
                &string_attr(archive, "sha256")?,
                &string_attr(archive, "record_sha256")?,
            )?));
        }
    }
    let mut files = BTreeMap::new();
    for file in &block.blocks {
        let [path] = file.labels.as_slice() else {
            return Err(invalid("file requires one relative path"));
        };
        if file.name != "file" || file.attributes.len() != 1 || !file.blocks.is_empty() {
            return Err(invalid("invalid locked file fields"));
        }
        validate_relative_path(path)?;
        let hash = string_attr(file, "sha256")?;
        if hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(invalid("invalid file checksum"));
        }
        if files
            .insert(path.clone(), hash.to_ascii_lowercase())
            .is_some()
        {
            return Err(invalid("duplicate locked file"));
        }
    }
    if !files.contains_key(executable) || files.len() > MAX_TREE_ENTRIES {
        return Err(invalid("locked executable missing or too many files"));
    }
    Ok(LockedInstallation::Files(files))
}

fn install_path(store: &ToolStore, receipt: &ToolReceipt) -> PathBuf {
    store
        .root_path()
        .join("installs")
        .join(&receipt.name)
        .join(&receipt.version)
}

fn string_attr(block: &Block, name: &str) -> Result<String, ToolAcquireError> {
    block
        .attributes
        .get(name)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| invalid(&format!("missing string attribute: {name}")))
}

pub(crate) fn platform() -> String {
    let abi = if cfg!(target_env = "musl") {
        "musl"
    } else if cfg!(target_env = "gnu") {
        "gnu"
    } else if cfg!(target_env = "msvc") {
        "msvc"
    } else {
        ""
    };
    format!("{}-{}-{abi}", std::env::consts::ARCH, std::env::consts::OS)
}

fn invalid(reason: &str) -> ToolAcquireError {
    ToolAcquireError::InvalidEnvLock(reason.to_owned())
}

fn content_mismatch(name: &str) -> ToolAcquireError {
    ToolAcquireError::LockContentMismatch { name: name.into() }
}

fn missing(name: &str, version: &str) -> ToolAcquireError {
    ToolAcquireError::ToolNotInstalled {
        name: name.into(),
        version: version.into(),
    }
}

#[cfg(test)]
mod tests;
