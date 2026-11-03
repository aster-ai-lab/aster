//! Declarative native tool acquisition entries carried by an AIX plugin.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use aster_acl::{parse_acl, Block, Value};

use super::{debug_origin, official_asset_url, HttpSource, MAX_BYTES};
use crate::archive::{portable_path, validate_layout, MAX_ARCHIVE_BYTES};
use crate::spec::{parse_tool_name, parse_tool_version};
use crate::{
    parse_tool_spec, AcquireRequest, ArchiveFormat, ArchivePayload, InstallationPaths,
    ToolAcquireError, ToolReceipt, ToolSpec, ToolStore,
};

const MAX_DECLARATION_BYTES: usize = 64 * 1024;

/// An admitted artifact that can be installed without another network request.
#[derive(Debug)]
pub struct PreparedToolchain {
    plugin: ToolchainPlugin,
    payload: ArchivePayload,
}

impl PreparedToolchain {
    /// The admitted source declaration for this exact archive.
    pub fn plugin(&self) -> &ToolchainPlugin {
        &self.plugin
    }

    /// Extract and publish into the caller-owned store. This is blocking filesystem work.
    pub fn install(self, store: &ToolStore) -> Result<ToolReceipt, ToolAcquireError> {
        store.acquire_archive(
            AcquireRequest {
                name: self.plugin.name.clone(),
                spec: self.plugin.spec.raw().into(),
                version: Some(self.plugin.version.clone()),
                executable_name: self.plugin.executable.clone(),
            },
            self.payload,
        )
    }
}

/// Complete file-tree evidence for the exact source-declared installation.
#[derive(Debug)]
pub struct InstalledToolchain {
    receipt: ToolReceipt,
    digest: String,
    paths: InstallationPaths,
}

impl InstalledToolchain {
    pub fn receipt(&self) -> &ToolReceipt {
        &self.receipt
    }

    pub fn digest(&self) -> &str {
        &self.digest
    }
    pub fn paths(&self) -> &InstallationPaths {
        &self.paths
    }
}

/// One bounded, immutable archive declaration from a trusted plugin.
/// Parsing does not download or install. Publisher authentication and package
/// authorization remain the caller's responsibility. URLs use the existing
/// GitHub release or npm registry transport policy; credentials and URL queries are refused.
/// Embedded archive paths are relative to the owning package and capped at 512 MiB.
#[derive(Clone)]
pub struct ToolchainPlugin {
    name: String,
    spec: ToolSpec,
    version: String,
    executable: String,
    platform: Platform,
    source: ArchiveSource,
    length: usize,
    sha256: String,
    format: ArchiveFormat,
    prefix: PathBuf,
    entrypoints: BTreeMap<String, PathBuf>,
    optional_entrypoints: BTreeMap<String, PathBuf>,
    bin_dirs: Vec<PathBuf>,
    executable_entrypoints: bool,
}

#[derive(Clone)]
enum ArchiveSource {
    Http(String),
    Embedded(PathBuf),
}

impl std::fmt::Debug for ToolchainPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut value = f.debug_struct("ToolchainPlugin");
        value
            .field("name", &self.name)
            .field("spec", &self.spec)
            .field("version", &self.version)
            .field("executable", &self.executable)
            .field("platform", &self.platform);
        match &self.source {
            ArchiveSource::Http(url) => value.field("origin", &debug_origin(url)),
            ArchiveSource::Embedded(path) => value.field("archive", path),
        };
        value
            .field("length", &self.length)
            .field("sha256", &self.sha256)
            .finish()
    }
}

impl ToolchainPlugin {
    /// Parse schema-v1 or schema-v2 `toolchain` ACL without I/O. V2 validates
    /// every declared platform archive and selects the exact current host.
    /// Unknown fields, duplicate/unsupported platforms, invalid paths, missing
    /// host archives and declarations over 64 KiB refuse before acquisition.
    pub fn parse_acl(text: &str) -> Result<Self, ToolAcquireError> {
        if text.len() > MAX_DECLARATION_BYTES {
            return Err(invalid("declaration exceeds 64 KiB"));
        }
        let document = parse_acl(text).map_err(|_| invalid("invalid ACL syntax"))?;
        let [block] = document.blocks.as_slice() else {
            return Err(invalid("one toolchain block is required"));
        };
        if block.name != "toolchain" || !block.labels.is_empty() {
            return Err(invalid("one unlabelled toolchain block is required"));
        }
        let schema = match block.attributes.get("schema_version") {
            Some(Value::Number(value)) if *value == 1.0 => 1,
            Some(Value::Number(value)) if *value == 2.0 => 2,
            _ => return Err(invalid("schema_version must be 1 or 2")),
        };
        let name = string(block, "name")?;
        let spec = parse_tool_spec(string(block, "spec")?)?;
        let version = string(block, "version")?;
        let executable = string(block, "executable")?;
        parse_tool_name(name)?;
        parse_tool_name(executable)?;
        parse_tool_version(version)?;
        if schema == 1 {
            known_attributes(
                block,
                &[
                    "schema_version",
                    "name",
                    "spec",
                    "version",
                    "executable",
                    "platform",
                    "url",
                    "archive",
                    "length",
                    "sha256",
                    "format",
                    "prefix",
                    "bin_dirs",
                    "executable_mode",
                ],
            )?;
            let platform = Platform::parse(string(block, "platform")?)?;
            return Self::parse_archive(block, name, &spec, version, executable, platform);
        }
        known_attributes(
            block,
            &["schema_version", "name", "spec", "version", "executable"],
        )?;
        let host = crate::env_lock::platform();
        let mut seen = BTreeSet::new();
        let mut selected = None;
        for artifact in &block.blocks {
            if artifact.name != "platform" || artifact.labels.len() != 1 {
                return Err(invalid(
                    "schema-v2 children must be labelled platform blocks",
                ));
            }
            let platform = Platform::parse(&artifact.labels[0])?;
            if !seen.insert(platform.as_str()) {
                return Err(invalid("duplicate platform"));
            }
            known_attributes(
                artifact,
                &[
                    "url",
                    "archive",
                    "length",
                    "sha256",
                    "format",
                    "prefix",
                    "bin_dirs",
                    "executable_mode",
                ],
            )?;
            let candidate =
                Self::parse_archive(artifact, name, &spec, version, executable, platform)?;
            if platform.as_str() == host {
                selected = Some(candidate);
            }
        }
        selected.ok_or_else(|| invalid("plugin has no archive for this host platform"))
    }

    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn version(&self) -> &str {
        &self.version
    }

    /// An admitted package-relative payload path, absent for an HTTP source.
    pub fn embedded_archive(&self) -> Option<&Path> {
        match &self.source {
            ArchiveSource::Embedded(path) => Some(path),
            ArchiveSource::Http(_) => None,
        }
    }

    /// Exact bounded payload length admitted from the declaration.
    pub fn archive_length(&self) -> usize {
        self.length
    }

    /// Install through the existing bounded HTTP/archive/store transaction.
    /// This performs blocking network and filesystem I/O and must run off async
    /// executor threads. No upstream installer script or tool process is run.
    /// Platform, length, checksum and layout failures preserve prior installs.
    pub fn install(
        &self,
        store: &ToolStore,
        http: &HttpSource,
    ) -> Result<ToolReceipt, ToolAcquireError> {
        self.prepare(http)?.install(store)
    }

    /// Download and verify without installing. Run blocking HTTP off executor threads.
    pub fn prepare(&self, http: &HttpSource) -> Result<PreparedToolchain, ToolAcquireError> {
        self.require_current_platform()?;
        let ArchiveSource::Http(url) = &self.source else {
            return Err(invalid(
                "embedded archive bytes must come from the owning package",
            ));
        };
        let url = http.prepare_toolchain_download(url)?;
        Ok(PreparedToolchain {
            payload: self.payload(http.download_bounded(&url, self.length)?)?,
            plugin: self.clone(),
        })
    }

    /// Admit already downloaded bytes for a later filesystem transaction.
    pub fn prepare_archive(&self, bytes: Vec<u8>) -> Result<PreparedToolchain, ToolAcquireError> {
        self.require_current_platform()?;
        Ok(PreparedToolchain {
            payload: self.payload(bytes)?,
            plugin: self.clone(),
        })
    }

    /// Verify source identity, every installed file/link/mode and the immutable receipt binding.
    /// This performs bounded blocking I/O and returns `None` for another or absent installation.
    pub fn installed(
        &self,
        store: &ToolStore,
    ) -> Result<Option<InstalledToolchain>, ToolAcquireError> {
        self.require_current_platform()?;
        let Some(receipt) = store.installed_receipt(&self.name, None)? else {
            return Ok(None);
        };
        if receipt.spec != self.spec.raw()
            || receipt.version != self.version
            || receipt.executable != self.executable
        {
            return Ok(None);
        }
        let Some(record) = crate::archive::ArchiveRecord::read(store.root_path(), &receipt)? else {
            return Ok(None);
        };
        let lock = record.lock()?;
        if lock.archive_sha256() != self.sha256 {
            return Ok(None);
        }
        if record.bin_dirs != self.bin_dirs
            || self
                .entrypoints
                .iter()
                .any(|(name, path)| record.entrypoints.get(name) != Some(path))
            || record.entrypoints.iter().any(|(name, path)| {
                self.entrypoints
                    .get(name)
                    .or_else(|| self.optional_entrypoints.get(name))
                    != Some(path)
            })
        {
            return Ok(None);
        }
        let directory = store
            .root_path()
            .join("installs")
            .join(&self.name)
            .join(&self.version);
        let mut remaining = crate::installation::MAX_TREE_BYTES;
        record.verify_tree(&directory, &mut remaining)?;
        Ok(Some(InstalledToolchain {
            digest: format!("sha256:{}", lock.record_sha256()),
            paths: store.installation_paths(&receipt)?,
            receipt,
        }))
    }

    /// Install already acquired bytes without network access. Length, checksum,
    /// platform and archive validation are identical to online installation.
    /// This performs bounded blocking filesystem and decompression work.
    pub fn install_archive(
        &self,
        store: &ToolStore,
        bytes: Vec<u8>,
    ) -> Result<ToolReceipt, ToolAcquireError> {
        self.prepare_archive(bytes)?.install(store)
    }

    fn parse_archive(
        block: &Block,
        name: &str,
        spec: &ToolSpec,
        version: &str,
        executable: &str,
        platform: Platform,
    ) -> Result<Self, ToolAcquireError> {
        let source = archive_source(block)?;
        let max_bytes = match &source {
            ArchiveSource::Http(_) => MAX_BYTES,
            ArchiveSource::Embedded(_) => MAX_ARCHIVE_BYTES,
        };
        let Some(Value::Number(length)) = block.attributes.get("length") else {
            return Err(invalid("length must be an integer"));
        };
        if !length.is_finite()
            || *length < 1.0
            || *length > max_bytes as f64
            || length.fract() != 0.0
        {
            return Err(invalid(&format!(
                "length must be between 1 and {max_bytes} bytes"
            )));
        }
        let sha256 = crate::store::normalize_sha256(string(block, "sha256")?)?;
        let format = match string(block, "format")? {
            "tar.gz" => ArchiveFormat::TarGz,
            "zip" => ArchiveFormat::Zip,
            _ => return Err(invalid("format must be tar.gz or zip")),
        };
        let prefix = PathBuf::from(string(block, "prefix")?);
        if !prefix.as_os_str().is_empty() {
            portable_path(&prefix)?;
        }
        let Some(Value::List(dirs)) = block.attributes.get("bin_dirs") else {
            return Err(invalid("bin_dirs must be a string list"));
        };
        let bin_dirs = dirs
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .map(PathBuf::from)
                    .ok_or_else(|| invalid("bin_dirs must be a string list"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let executable_entrypoints = match block.attributes.get("executable_mode") {
            None => false,
            Some(Value::String(mode)) if mode == "archive" => false,
            Some(Value::String(mode)) if mode == "0755" => true,
            _ => return Err(invalid("executable_mode must be archive or 0755")),
        };
        let mut entrypoints = BTreeMap::new();
        let mut optional_entrypoints = BTreeMap::new();
        for child in &block.blocks {
            if child.name != "command" || child.labels.len() != 1 || !child.blocks.is_empty() {
                return Err(invalid(
                    "toolchain children must be labelled command blocks",
                ));
            }
            known_attributes(child, &["path", "optional"])?;
            let optional = match child.attributes.get("optional") {
                None => false,
                Some(Value::Bool(value)) => *value,
                _ => return Err(invalid("command optional must be a boolean")),
            };
            let name = &child.labels[0];
            if entrypoints.contains_key(name) || optional_entrypoints.contains_key(name) {
                return Err(invalid("duplicate command"));
            }
            let commands = if optional {
                &mut optional_entrypoints
            } else {
                &mut entrypoints
            };
            commands.insert(name.clone(), PathBuf::from(string(child, "path")?));
        }
        if !entrypoints.contains_key(executable) {
            return Err(invalid("primary executable must be a required command"));
        }
        let mut all = entrypoints.clone();
        all.extend(optional_entrypoints.clone());
        validate_layout(
            &all,
            &bin_dirs,
            matches!(platform, Platform::WindowsArm64 | Platform::WindowsX64),
        )?;
        Ok(Self {
            name: name.into(),
            spec: spec.clone(),
            version: version.into(),
            executable: executable.into(),
            platform,
            source,
            length: *length as usize,
            sha256,
            format,
            prefix,
            entrypoints,
            optional_entrypoints,
            bin_dirs,
            executable_entrypoints,
        })
    }

    fn require_current_platform(&self) -> Result<(), ToolAcquireError> {
        if self.platform.as_str() == crate::env_lock::platform() {
            Ok(())
        } else {
            Err(invalid("plugin platform does not match this host"))
        }
    }

    fn payload(&self, bytes: Vec<u8>) -> Result<ArchivePayload, ToolAcquireError> {
        if bytes.len() != self.length {
            return Err(invalid("artifact length does not match the declaration"));
        }
        let payload = ArchivePayload::new(
            self.version().into(),
            &self.sha256,
            bytes,
            self.format,
            self.prefix.clone(),
            self.entrypoints.clone(),
            self.bin_dirs.clone(),
        )?
        .with_optional_entrypoints(self.optional_entrypoints.clone())?;
        Ok(if self.executable_entrypoints {
            payload.with_executable_entrypoints()
        } else {
            payload
        })
    }
}

fn archive_source(block: &Block) -> Result<ArchiveSource, ToolAcquireError> {
    match (block.attributes.get("url"), block.attributes.get("archive")) {
        (Some(Value::String(url)), None) => {
            let parsed = reqwest::Url::parse(url).map_err(|_| invalid("invalid artifact URL"))?;
            let npm_artifact = parsed.origin().ascii_serialization() == super::NPM_REGISTRY
                && parsed.username().is_empty()
                && parsed.password().is_none();
            if !(official_asset_url(url) || npm_artifact)
                || parsed.query().is_some()
                || parsed.fragment().is_some()
            {
                return Err(invalid(
                    "artifact requires a credential-free HTTPS GitHub release or npm registry URL",
                ));
            }
            Ok(ArchiveSource::Http(parsed.into()))
        }
        (None, Some(Value::String(path))) => {
            if path.len() > 4096 {
                return Err(invalid("embedded archive path exceeds 4096 bytes"));
            }
            crate::installation::validate_relative_path(path)?;
            Ok(ArchiveSource::Embedded(PathBuf::from(path)))
        }
        _ => Err(invalid("declare exactly one string url or archive")),
    }
}

fn known_attributes(block: &Block, allowed: &[&str]) -> Result<(), ToolAcquireError> {
    if block
        .attributes
        .keys()
        .any(|key| !allowed.contains(&key.as_str()))
    {
        return Err(invalid("unknown toolchain attribute"));
    }
    Ok(())
}

fn string<'a>(block: &'a Block, name: &str) -> Result<&'a str, ToolAcquireError> {
    block
        .attributes
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(&format!("missing string attribute: {name}")))
}

fn invalid(reason: &str) -> ToolAcquireError {
    ToolAcquireError::InvalidToolchainPlugin(reason.into())
}

#[derive(Debug, Clone, Copy)]
enum Platform {
    MacArm64,
    MacX64,
    LinuxGnuArm64,
    LinuxGnuX64,
    LinuxMuslArm64,
    LinuxMuslX64,
    WindowsArm64,
    WindowsX64,
}

impl Platform {
    fn parse(value: &str) -> Result<Self, ToolAcquireError> {
        match value {
            "aarch64-macos-" => Ok(Self::MacArm64),
            "x86_64-macos-" => Ok(Self::MacX64),
            "aarch64-linux-gnu" => Ok(Self::LinuxGnuArm64),
            "x86_64-linux-gnu" => Ok(Self::LinuxGnuX64),
            "aarch64-linux-musl" => Ok(Self::LinuxMuslArm64),
            "x86_64-linux-musl" => Ok(Self::LinuxMuslX64),
            "aarch64-windows-msvc" => Ok(Self::WindowsArm64),
            "x86_64-windows-msvc" => Ok(Self::WindowsX64),
            _ => Err(invalid("unsupported native toolchain platform")),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::MacArm64 => "aarch64-macos-",
            Self::MacX64 => "x86_64-macos-",
            Self::LinuxGnuArm64 => "aarch64-linux-gnu",
            Self::LinuxGnuX64 => "x86_64-linux-gnu",
            Self::LinuxMuslArm64 => "aarch64-linux-musl",
            Self::LinuxMuslX64 => "x86_64-linux-musl",
            Self::WindowsArm64 => "aarch64-windows-msvc",
            Self::WindowsX64 => "x86_64-windows-msvc",
        }
    }
}

#[cfg(test)]
mod tests;
