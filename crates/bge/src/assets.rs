//! Family directories, install receipts, and readiness status.

use std::path::{Path, PathBuf};

use aster_use_core::{UseError, UseResult};
use serde::{Deserialize, Serialize};

pub(crate) const RECEIPT_FILE: &str = ".aster-bge.json";
const RECEIPT_SCHEMA_VERSION: u32 = 1;
const MAX_MODEL_FILE_BYTES: u64 = 3 * 1024 * 1024 * 1024;
const MAX_TOKENIZER_FILE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_RECEIPT_BYTES: u64 = 1024 * 1024;

/// The engine families this crate serves. A closed set: parsing names the
/// two accepted values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Family {
    BgeM3,
    BgeRerankerV2M3,
}

impl Family {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BgeM3 => "bge-m3",
            Self::BgeRerankerV2M3 => "bge-reranker-v2-m3",
        }
    }

    pub fn parse(value: &str) -> Result<Self, UseError> {
        match value.trim() {
            "bge-m3" => Ok(Self::BgeM3),
            "bge-reranker-v2-m3" => Ok(Self::BgeRerankerV2M3),
            other => Err(crate::bge_error(
                "use.bge.family_invalid",
                format!("family must be bge-m3 or bge-reranker-v2-m3, got {other:?}"),
            )),
        }
    }

    fn env_override(self) -> &'static str {
        match self {
            Self::BgeM3 => "ASTER_BGE_M3_MODEL_DIR",
            Self::BgeRerankerV2M3 => "ASTER_BGE_RERANKER_MODEL_DIR",
        }
    }

    fn directory_name(self) -> &'static str {
        self.as_str()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BgeInstallSource {
    Environment,
    Managed,
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BgeRuntimeStatus {
    pub available: bool,
    pub source: BgeInstallSource,
    pub family: String,
    pub model_dir: Option<PathBuf>,
    pub managed_root: Option<PathBuf>,
    pub detail: String,
}

/// The receipt written by the installer and trusted by asset resolution.
/// Every installed file is bound to its sha256 so `owned_install` can tell
/// a managed directory from an unrelated one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InstallReceipt {
    pub schema_version: u32,
    pub family: String,
    pub model_id: String,
    pub source_api: String,
    pub model_file: String,
    pub tokenizer_file: String,
    pub files: Vec<ReceiptFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReceiptFile {
    pub path: String,
    pub sha256: String,
    pub bytes: u64,
}

#[derive(Debug, Clone)]
pub(crate) struct FamilyAssets {
    pub root: PathBuf,
    pub model_file: PathBuf,
    pub tokenizer_file: PathBuf,
    pub source: BgeInstallSource,
    /// Digest of the install receipt. A forced reinstall onto the same path
    /// changes this, which is the engines' reload signal.
    pub install_stamp: String,
}

pub fn bge_status(family: Family) -> BgeRuntimeStatus {
    let managed_root = managed_root().ok();
    match resolve_family_assets(family) {
        Ok(assets) => BgeRuntimeStatus {
            available: true,
            source: assets.source,
            family: family.as_str().to_string(),
            model_dir: Some(assets.root),
            managed_root,
            detail: "ready".to_string(),
        },
        Err(error) => BgeRuntimeStatus {
            available: false,
            source: match error.details.get("source").and_then(|value| value.as_str()) {
                Some("environment") => BgeInstallSource::Environment,
                Some("managed") => BgeInstallSource::Managed,
                _ => BgeInstallSource::Missing,
            },
            family: family.as_str().to_string(),
            model_dir: error
                .details
                .get("modelDir")
                .and_then(|value| value.as_str())
                .map(PathBuf::from),
            managed_root,
            detail: error.message,
        },
    }
}

pub(crate) fn resolve_family_assets(family: Family) -> UseResult<FamilyAssets> {
    if let Some(path) = std::env::var_os(family.env_override())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
    {
        let path = absolute(path)?;
        return validate_assets(&path, family, BgeInstallSource::Environment);
    }

    let managed = managed_family_dir(family)?;
    if path_exists(&managed)? {
        return validate_assets(&managed, family, BgeInstallSource::Managed);
    }

    Err(model_missing(family, &managed))
}

pub(crate) fn validate_assets(
    root: &Path,
    family: Family,
    source: BgeInstallSource,
) -> UseResult<FamilyAssets> {
    let root = std::fs::canonicalize(root).map_err(|error| {
        model_error(
            family,
            source,
            root,
            format!(
                "Failed to resolve the {} model directory '{}': {error}",
                family.as_str(),
                root.display()
            ),
        )
    })?;
    let receipt = read_receipt(&root, family, source)?;
    let model_file = checked_file(
        &root,
        &receipt.model_file,
        MAX_MODEL_FILE_BYTES,
        family,
        source,
    )?;
    let tokenizer_file = checked_file(
        &root,
        &receipt.tokenizer_file,
        MAX_TOKENIZER_FILE_BYTES,
        family,
        source,
    )?;
    let install_stamp = install_stamp(&root);
    Ok(FamilyAssets {
        root,
        model_file,
        tokenizer_file,
        source,
        install_stamp,
    })
}

pub(crate) fn read_receipt(
    root: &Path,
    family: Family,
    source: BgeInstallSource,
) -> UseResult<InstallReceipt> {
    let path = root.join(RECEIPT_FILE);
    let bytes = std::fs::read(&path).map_err(|error| {
        model_error(
            family,
            source,
            root,
            format!(
                "The {} install receipt '{}' is unreadable: {error}",
                family.as_str(),
                path.display()
            ),
        )
    })?;
    if bytes.len() as u64 > MAX_RECEIPT_BYTES {
        return Err(model_error(
            family,
            source,
            root,
            format!(
                "The {} install receipt exceeds the {MAX_RECEIPT_BYTES}-byte limit.",
                family.as_str()
            ),
        ));
    }
    let receipt: InstallReceipt = serde_json::from_slice(&bytes).map_err(|error| {
        model_error(
            family,
            source,
            root,
            format!(
                "The {} install receipt is not valid JSON: {error}",
                family.as_str()
            ),
        )
    })?;
    if receipt.schema_version != RECEIPT_SCHEMA_VERSION
        || receipt.family != family.as_str()
        || receipt.model_file.is_empty()
        || receipt.tokenizer_file.is_empty()
        || receipt.files.is_empty()
    {
        return Err(model_error(
            family,
            source,
            root,
            format!(
                "The {} install receipt does not describe this engine family.",
                family.as_str()
            ),
        ));
    }
    Ok(receipt)
}

/// A directory is owned by the installer when it carries a parseable
/// receipt for the same family. Activation refuses to replace anything else.
pub(crate) fn owned_install(path: &Path, family: Family) -> bool {
    let Ok(bytes) = std::fs::read(path.join(RECEIPT_FILE)) else {
        return false;
    };
    serde_json::from_slice::<InstallReceipt>(&bytes).is_ok_and(|receipt| {
        receipt.schema_version == RECEIPT_SCHEMA_VERSION && receipt.family == family.as_str()
    })
}

pub(crate) fn managed_root() -> UseResult<PathBuf> {
    // Set-but-empty values are unset: without the filter an empty string
    // resolves to the process working directory and installs land there.
    if let Some(value) = std::env::var_os("ASTER_USE_BGE_HOME").filter(|v| !v.is_empty()) {
        return absolute(PathBuf::from(value));
    }
    if let Some(value) = std::env::var_os("ASTER_DATA_HOME").filter(|v| !v.is_empty()) {
        return Ok(absolute(PathBuf::from(value))?.join("use/bge"));
    }
    if let Some(value) = std::env::var_os("XDG_DATA_HOME").filter(|v| !v.is_empty()) {
        return Ok(absolute(PathBuf::from(value))?.join("aster/use/bge"));
    }
    if let Some(home) = std::env::var_os("HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
    {
        return Ok(absolute(home)?.join(".local/share/aster/use/bge"));
    }
    #[cfg(windows)]
    if let Some(value) = std::env::var_os("LOCALAPPDATA") {
        return Ok(absolute(PathBuf::from(value))?.join("aster/use/bge"));
    }
    Err(crate::bge_error(
        "use.bge.data_home_missing",
        "Cannot determine the Aster Use BGE data directory.",
    ))
}

pub(crate) fn managed_family_dir(family: Family) -> UseResult<PathBuf> {
    Ok(managed_root()?.join(family.directory_name()))
}

fn checked_file(
    root: &Path,
    relative: &str,
    max_bytes: u64,
    family: Family,
    source: BgeInstallSource,
) -> UseResult<PathBuf> {
    let path = root.join(relative);
    let canonical = std::fs::canonicalize(&path).map_err(|error| {
        model_error(
            family,
            source,
            root,
            format!(
                "Required {} asset '{}' is unreadable: {error}",
                family.as_str(),
                path.display()
            ),
        )
    })?;
    if !canonical.starts_with(root) {
        return Err(model_error(
            family,
            source,
            root,
            format!(
                "Required {} asset '{}' escapes its model directory.",
                family.as_str(),
                path.display()
            ),
        ));
    }
    let metadata = std::fs::metadata(&canonical).map_err(|error| {
        model_error(
            family,
            source,
            root,
            format!(
                "Failed to inspect {} asset '{}': {error}",
                family.as_str(),
                canonical.display()
            ),
        )
    })?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > max_bytes {
        return Err(model_error(
            family,
            source,
            root,
            format!(
                "{} asset '{}' must be a non-empty regular file no larger than {max_bytes} bytes.",
                family.as_str(),
                canonical.display()
            ),
        ));
    }
    Ok(canonical)
}

/// Content digest of the receipt; cheap to recompute on every resolve and
/// stable across canonicalization, unlike the directory path.
fn install_stamp(root: &Path) -> String {
    use sha2::Digest;
    match std::fs::read(root.join(RECEIPT_FILE)) {
        Ok(bytes) => format!("{:x}", sha2::Sha256::digest(bytes)),
        Err(_) => String::new(),
    }
}

fn model_missing(family: Family, managed: &Path) -> UseError {
    UseError::new(
        "use.bge.model_missing",
        format!(
            "The local {} model is not installed in '{}'.",
            family.as_str(),
            managed.display()
        ),
    )
    .with_suggestion(
        "Import the model through the platform model supply, then run 'aster-use-bge install --help' for the exact install command.",
    )
    .with_detail("source", "missing")
    .with_detail("modelDir", managed.display().to_string())
}

fn model_error(
    family: Family,
    source: BgeInstallSource,
    root: &Path,
    message: impl Into<String>,
) -> UseError {
    UseError::new("use.bge.model_invalid", message)
        .with_suggestion(format!(
            "Run 'aster-use-bge install {} --force …' (see 'aster-use-bge install --help') to restore the model.",
            family.as_str()
        ))
        .with_detail("source", source_name(source))
        .with_detail("modelDir", root.display().to_string())
}

fn source_name(source: BgeInstallSource) -> &'static str {
    match source {
        BgeInstallSource::Environment => "environment",
        BgeInstallSource::Managed => "managed",
        BgeInstallSource::Missing => "missing",
    }
}

fn path_exists(path: &Path) -> UseResult<bool> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(crate::bge_error(
            "use.bge.model_unreadable",
            format!(
                "Failed to inspect the BGE model path '{}': {error}",
                path.display()
            ),
        )),
    }
}

fn absolute(path: PathBuf) -> UseResult<PathBuf> {
    if path.is_absolute() {
        Ok(path)
    } else {
        std::env::current_dir()
            .map(|directory| directory.join(path))
            .map_err(|error| {
                crate::bge_error(
                    "use.bge.path_resolution_failed",
                    format!("Failed to resolve the BGE data path: {error}"),
                )
            })
    }
}
