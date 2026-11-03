use std::path::PathBuf;

/// Closed failure set for tool acquisition.
#[derive(Debug, thiserror::Error)]
pub enum ToolAcquireError {
    #[error("no version matches request: {0}")]
    VersionNotFound(String),
    #[error("this backend does not support version request: {0}")]
    UnsupportedVersionRequest(String),
    #[error("this source does not provide a version listing")]
    VersionListingUnsupported,
    #[error("version listing exceeds 10000 entries")]
    VersionListLimit,
    #[error("version resolution exceeds 32 steps")]
    VersionResolutionLimit,
    #[error("version alias cycle at {name}")]
    VersionAliasCycle { name: String },
    #[error("cannot subtract from non-numeric version: {version}")]
    InvalidSubtraction { version: String },
    #[error("cached version metadata missing for {spec}; refresh with `aster tool ls-remote` while online")]
    VersionMetadataMissing { spec: String },
    #[error("version cache exceeds 8 MiB")]
    VersionMetadataLimit,
    #[error("invalid tool name: {0}")]
    InvalidName(String),
    #[error("invalid tool spec: {0}")]
    InvalidSpec(String),
    #[error("invalid tool version: {0}")]
    InvalidVersion(String),
    #[error("unknown tool backend: {0}")]
    UnknownBackend(String),
    #[error("tool spec is missing a checksum")]
    ChecksumRequired,
    #[error("tool checksum does not match the payload")]
    ChecksumMismatch,
    #[error("installed version {version} already has a different checksum")]
    VersionConflict { version: String },
    #[error("installed identity for {name}@{version} does not match its declaration")]
    InstallationIdentityMismatch { name: String, version: String },
    #[error("refusing to follow a link at {}", path.display())]
    LinkRefused { path: PathBuf },
    #[error("payload is not a regular file: {}", path.display())]
    PayloadInvalid { path: PathBuf },
    #[error("staged tool is not a regular executable file")]
    NotExecutable,
    #[error("receipt for {name} does not match the current version")]
    ReceiptDisagrees { name: String },
    #[error("{action} failed at {path}: {source}")]
    Io {
        action: &'static str,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("tool install task failed: {0}")]
    Task(String),
    #[error("tool asset is missing")]
    AssetMissing,
    #[error("tool asset is ambiguous")]
    AssetAmbiguous,
    #[error("tool download was rejected")]
    DownloadRejected,
    #[error("tool HTTP request failed: {source}")]
    Http {
        #[source]
        source: reqwest::Error,
    },
    #[error("read tool HTTP response: {0}")]
    HttpRead(#[source] std::io::Error),
    #[error("invalid tool metadata: {0}")]
    MetadataJson(#[source] serde_json::Error),
    #[error(transparent)]
    NodeSignature(#[from] crate::NodeSignatureError),
    #[error("{runtime} requires the complete-installation source API")]
    RuntimeLayoutRequired { runtime: &'static str },
    #[error("{runtime} has no admitted prebuilt target for {os}/{arch}/{abi}")]
    UnsupportedRuntimeTarget {
        runtime: &'static str,
        os: &'static str,
        arch: &'static str,
        abi: &'static str,
    },
    #[error("tool latest version is unavailable")]
    LatestUnavailable,
    #[error("tool runtime is missing: {0}")]
    RuntimeMissing(String),
    #[error("invalid env acl: {0}")]
    InvalidEnvAcl(String),
    #[error("invalid toolchain plugin: {0}")]
    InvalidToolchainPlugin(String),
    #[error("unknown env tool: {0}")]
    UnknownEnvTool(String),
    #[error("invalid env lock: {0}")]
    InvalidEnvLock(String),
    #[error("env lock platform is {expected}; this binary targets {actual}")]
    LockPlatformMismatch { expected: String, actual: String },
    #[error("env lock does not match tool declarations; run `aster tool lock` to update it")]
    LockDeclarationMismatch,
    #[error("installed or downloaded files do not match env lock for {name}")]
    LockContentMismatch { name: String },
    #[error("tool not installed: {name}@{version} (downloads disabled)")]
    ToolNotInstalled { name: String, version: String },
    #[error("invalid installation: {0}")]
    InvalidInstallation(String),
    #[error("installation manifest missing for {name}@{version}; reinstall this version before creating a lock")]
    InstallationManifestMissing { name: String, version: String },
    #[error("invalid environment tracking: {0}")]
    InvalidEnvTracking(String),
    #[error("cannot inspect native process {pid}")]
    ProcessInspection {
        pid: u32,
        #[source]
        source: std::io::Error,
    },
}

impl ToolAcquireError {
    /// Stable code for logs and host IPC.
    pub fn code(&self) -> &'static str {
        match self {
            Self::VersionNotFound(_) => "use.tool_acquire.version_not_found",
            Self::UnsupportedVersionRequest(_) => "use.tool_acquire.unsupported_version_request",
            Self::VersionListingUnsupported => "use.tool_acquire.version_listing_unsupported",
            Self::VersionListLimit => "use.tool_acquire.version_list_limit",
            Self::VersionResolutionLimit => "use.tool_acquire.version_resolution_limit",
            Self::VersionAliasCycle { .. } => "use.tool_acquire.version_alias_cycle",
            Self::InvalidSubtraction { .. } => "use.tool_acquire.invalid_subtraction",
            Self::VersionMetadataMissing { .. } => "use.tool_acquire.version_metadata_missing",
            Self::VersionMetadataLimit => "use.tool_acquire.version_metadata_limit",
            Self::InvalidName(_) => "use.tool_acquire.invalid_name",
            Self::InvalidSpec(_) => "use.tool_acquire.invalid_spec",
            Self::InvalidVersion(_) => "use.tool_acquire.invalid_version",
            Self::UnknownBackend(_) => "use.tool_acquire.unknown_backend",
            Self::ChecksumRequired => "use.tool_acquire.checksum_required",
            Self::ChecksumMismatch => "use.tool_acquire.checksum_mismatch",
            Self::VersionConflict { .. } => "use.tool_acquire.version_conflict",
            Self::InstallationIdentityMismatch { .. } => {
                "use.tool_acquire.installation_identity_mismatch"
            }
            Self::LinkRefused { .. } => "use.tool_acquire.link_refused",
            Self::PayloadInvalid { .. } => "use.tool_acquire.payload_invalid",
            Self::NotExecutable => "use.tool_acquire.not_executable",
            Self::ReceiptDisagrees { .. } => "use.tool_acquire.receipt_disagrees",
            Self::Io { .. } => "use.tool_acquire.io",
            Self::Task(_) => "use.tool_acquire.task",
            Self::AssetMissing => "use.tool_acquire.asset_missing",
            Self::AssetAmbiguous => "use.tool_acquire.asset_ambiguous",
            Self::DownloadRejected => "use.tool_acquire.download_rejected",
            Self::Http { .. } => "use.tool_acquire.http",
            Self::HttpRead(_) => "use.tool_acquire.http_read",
            Self::MetadataJson(_) => "use.tool_acquire.metadata_json",
            Self::NodeSignature(_) => "use.tool_acquire.node_signature",
            Self::RuntimeLayoutRequired { .. } => "use.tool_acquire.runtime_layout_required",
            Self::UnsupportedRuntimeTarget { .. } => "use.tool_acquire.unsupported_runtime_target",
            Self::LatestUnavailable => "use.tool_acquire.latest_unavailable",
            Self::RuntimeMissing(_) => "use.tool_acquire.runtime_missing",
            Self::InvalidEnvAcl(_) => "use.tool_acquire.invalid_env_acl",
            Self::InvalidToolchainPlugin(_) => "use.tool_acquire.invalid_toolchain_plugin",
            Self::UnknownEnvTool(_) => "use.tool_acquire.unknown_env_tool",
            Self::InvalidEnvLock(_) => "use.tool_acquire.invalid_env_lock",
            Self::LockPlatformMismatch { .. } => "use.tool_acquire.lock_platform_mismatch",
            Self::LockDeclarationMismatch => "use.tool_acquire.lock_declaration_mismatch",
            Self::LockContentMismatch { .. } => "use.tool_acquire.lock_content_mismatch",
            Self::ToolNotInstalled { .. } => "use.tool_acquire.tool_not_installed",
            Self::InvalidInstallation(_) => "use.tool_acquire.invalid_installation",
            Self::InstallationManifestMissing { .. } => {
                "use.tool_acquire.installation_manifest_missing"
            }
            Self::InvalidEnvTracking(_) => "use.tool_acquire.invalid_env_tracking",
            Self::ProcessInspection { .. } => "use.tool_acquire.process_inspection",
        }
    }

    pub fn io(action: &'static str, path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            action,
            path: path.into(),
            source,
        }
    }

    pub(crate) fn http(source: reqwest::Error) -> Self {
        Self::Http {
            source: source.without_url(),
        }
    }
}
