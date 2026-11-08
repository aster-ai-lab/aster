use std::collections::BTreeMap;
use std::sync::Arc;

use crate::error::ToolAcquireError;
use crate::spec::ToolSpec;
use crate::version::{semver_prerelease, VersionListing};
use crate::ArchivePayload;

/// One file that must sit beside the executable in the version directory.
///
/// `relative_path` uses `/` and stays inside that directory. The store refuses
/// `..`, absolute paths, and a path that replaces the executable.
#[derive(Clone, PartialEq, Eq)]
pub struct CompanionFile {
    pub relative_path: String,
    pub bytes: Vec<u8>,
    pub executable: bool,
}

impl std::fmt::Debug for CompanionFile {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CompanionFile")
            .field("relative_path", &self.relative_path)
            .field("bytes", &self.bytes.len())
            .field("executable", &self.executable)
            .finish()
    }
}

/// One verified executable the store can publish.
#[derive(Clone, PartialEq, Eq)]
pub struct ToolPayload {
    pub version: String,
    pub sha256: String,
    pub bytes: Vec<u8>,
    pub companions: Vec<CompanionFile>,
}

impl std::fmt::Debug for ToolPayload {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ToolPayload")
            .field("version", &self.version)
            .field("sha256", &self.sha256)
            .field("bytes", &self.bytes.len())
            .field("companions", &self.companions)
            .finish()
    }
}

impl ToolPayload {
    /// An executable with no sibling files. Single-file tools use this.
    pub fn new(version: impl Into<String>, sha256: impl Into<String>, bytes: Vec<u8>) -> Self {
        Self {
            version: version.into(),
            sha256: sha256.into(),
            bytes,
            companions: Vec::new(),
        }
    }
}

/// A resolved spec, the executable name inside the payload, and an optional pin.
#[derive(Debug, Clone)]
pub struct ToolQuery {
    pub spec: ToolSpec,
    pub version: Option<String>,
    pub executable: String,
}

/// A legacy standalone executable or a complete verified runtime archive.
#[derive(Debug, Clone)]
pub enum InstallPayload {
    Executable(ToolPayload),
    Archive(ArchivePayload),
}

impl InstallPayload {
    pub fn version(&self) -> &str {
        match self {
            Self::Executable(payload) => &payload.version,
            Self::Archive(payload) => payload.version(),
        }
    }
}

/// Supplies payload bytes. The store verifies `sha256` before it publishes.
pub trait ToolSource: Send + Sync {
    fn fetch(&self, query: &ToolQuery) -> Result<ToolPayload, ToolAcquireError>;
    fn latest(&self, spec: &ToolSpec) -> Result<String, ToolAcquireError>;
    /// Fetch a complete installation. Existing sources retain standalone executable behavior.
    fn fetch_installation(&self, query: &ToolQuery) -> Result<InstallPayload, ToolAcquireError> {
        self.fetch(query).map(InstallPayload::Executable)
    }
    /// List provider versions and aliases. Older sources may support exact fetches only.
    fn versions(&self, _spec: &ToolSpec) -> Result<VersionListing, ToolAcquireError> {
        Err(ToolAcquireError::VersionListingUnsupported)
    }
}

/// In-memory source for fixtures and hosts that already hold the bytes.
#[derive(Debug, Default, Clone)]
pub struct StaticSource {
    payloads: BTreeMap<String, Arc<ToolPayload>>,
    version_payloads: BTreeMap<(String, String), Arc<ToolPayload>>,
    latest: BTreeMap<String, String>,
    latest_failures: BTreeMap<String, ()>,
    listings: BTreeMap<String, VersionListing>,
}

impl StaticSource {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, spec: impl Into<String>, payload: ToolPayload) -> &mut Self {
        let spec = spec.into();
        let payload = Arc::new(payload);
        self.version_payloads
            .insert((spec.clone(), payload.version.clone()), payload.clone());
        self.payloads.insert(spec, payload);
        self
    }

    pub fn insert_latest(
        &mut self,
        spec: impl Into<String>,
        version: impl Into<String>,
    ) -> &mut Self {
        let spec = spec.into();
        self.latest_failures.remove(&spec);
        self.latest.insert(spec, version.into());
        self
    }

    pub fn fail_latest(&mut self, spec: impl Into<String>) -> &mut Self {
        let spec = spec.into();
        self.latest.remove(&spec);
        self.latest_failures.insert(spec, ());
        self
    }

    /// Supply provider metadata independently of payload availability.
    pub fn insert_versions(
        &mut self,
        spec: impl Into<String>,
        listing: VersionListing,
    ) -> &mut Self {
        self.listings.insert(spec.into(), listing);
        self
    }
}

impl ToolSource for StaticSource {
    fn fetch(&self, query: &ToolQuery) -> Result<ToolPayload, ToolAcquireError> {
        if let Some(payload) = query.version.as_ref().and_then(|version| {
            self.version_payloads
                .get(&(query.spec.raw().to_string(), version.clone()))
        }) {
            return Ok(payload.as_ref().clone());
        }
        self.payloads
            .get(query.spec.raw())
            .map(|payload| payload.as_ref().clone())
            .ok_or(ToolAcquireError::AssetMissing)
    }

    fn latest(&self, spec: &ToolSpec) -> Result<String, ToolAcquireError> {
        if self.latest_failures.contains_key(spec.raw()) {
            return Err(ToolAcquireError::LatestUnavailable);
        }
        self.latest
            .get(spec.raw())
            .cloned()
            .ok_or(ToolAcquireError::LatestUnavailable)
    }

    fn versions(&self, spec: &ToolSpec) -> Result<VersionListing, ToolAcquireError> {
        if let Some(listing) = self.listings.get(spec.raw()) {
            return Ok(listing.clone());
        }
        let versions = self
            .version_payloads
            .keys()
            .filter(|(candidate, _)| candidate == spec.raw())
            .map(|(_, version)| (version.clone(), semver_prerelease(version)))
            .collect();
        VersionListing::new(versions, BTreeMap::new())
    }
}

#[cfg(test)]
mod tests;
