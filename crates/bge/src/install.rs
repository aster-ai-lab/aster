//! Manifest-driven installation for the BGE engine families.
//!
//! Every file arrives with an explicit URL, byte length, and sha256 —
//! values the platform model supply computed when the model was imported.
//! The installer never invents a source, never auto-downloads from the
//! engine path, and refuses to replace a directory it does not own.

use std::fs::OpenOptions;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use aster_use_core::{UseError, UseResult};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;

use crate::assets::{
    bge_status, managed_family_dir, managed_root, owned_install, validate_assets, BgeInstallSource,
    BgeRuntimeStatus, Family, RECEIPT_FILE,
};

mod download;

const INSTALL_LOCK: &str = ".install.lock";
const STAGE_PREFIX: &str = ".stage-";
const BACKUP_PREFIX: &str = ".backup-";
const MAX_MANIFEST_FILES: usize = 64;
const MAX_FILE_BYTES: u64 = 3 * 1024 * 1024 * 1024;

/// One file to install: where it comes from and what it must hash to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestFile {
    /// Path relative to the family directory, e.g. `onnx/model_quantized.onnx`.
    pub path: String,
    /// Presigned or plain http(s) URL serving the exact bytes.
    pub url: String,
    /// Expected length in bytes.
    pub bytes: u64,
    /// Expected lowercase hex sha256 of the file contents.
    pub sha256: String,
}

/// A complete install plan for one family.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallManifest {
    pub family: String,
    /// The model supply model id the files were imported under.
    pub model_id: String,
    /// Base URL of the aster-api that produced the presigned URLs.
    pub source_api: String,
    /// Manifest path serving as the ONNX weight (must be listed in `files`).
    pub model_file: String,
    /// Manifest path serving as the tokenizer (must be listed in `files`).
    pub tokenizer_file: String,
    pub files: Vec<ManifestFile>,
}

impl InstallManifest {
    pub fn new(
        family: Family,
        model_id: impl Into<String>,
        source_api: impl Into<String>,
        model_file: impl Into<String>,
        tokenizer_file: impl Into<String>,
        files: Vec<ManifestFile>,
    ) -> UseResult<Self> {
        let manifest = Self {
            family: family.as_str().to_string(),
            model_id: model_id.into(),
            source_api: source_api.into(),
            model_file: model_file.into(),
            tokenizer_file: tokenizer_file.into(),
            files,
        };
        manifest.validate()?;
        Ok(manifest)
    }

    pub fn family(&self) -> UseResult<Family> {
        Family::parse(&self.family)
    }

    pub(crate) fn validate(&self) -> UseResult<()> {
        Family::parse(&self.family)?;
        if self.model_id.trim().is_empty() || self.model_id.len() > 128 {
            return Err(manifest_error(
                "the model id must be 1-128 non-space characters.",
            ));
        }
        let origin = reqwest::Url::parse(&self.source_api).map_err(|error| {
            manifest_error(format!("the source api is not a valid URL: {error}"))
        })?;
        if !matches!(origin.scheme(), "http" | "https")
            || !origin.has_authority()
            || !origin.username().is_empty()
            || origin.password().is_some()
        {
            return Err(manifest_error(
                "the source api must be a credential-free http(s) origin.",
            ));
        }
        if self.files.is_empty() || self.files.len() > MAX_MANIFEST_FILES {
            return Err(manifest_error(format!(
                "the manifest must hold 1-{MAX_MANIFEST_FILES} files, got {}.",
                self.files.len()
            )));
        }
        let mut seen = std::collections::HashSet::new();
        for file in &self.files {
            validate_relative_path(&file.path)?;
            if !seen.insert(file.path.clone()) {
                return Err(manifest_error(format!(
                    "the manifest repeats path {:?}.",
                    file.path
                )));
            }
            let url = reqwest::Url::parse(&file.url).map_err(|error| {
                manifest_error(format!("the URL for {:?} is not valid: {error}", file.path))
            })?;
            if !matches!(url.scheme(), "http" | "https")
                || url.host_str().is_none()
                || !url.username().is_empty()
                || url.password().is_some()
            {
                return Err(manifest_error(format!(
                    "the URL for {:?} must be credential-free http(s).",
                    file.path
                )));
            }
            if file.bytes == 0 || file.bytes > MAX_FILE_BYTES {
                return Err(manifest_error(format!(
                    "the byte length for {:?} must be 1-{MAX_FILE_BYTES}.",
                    file.path
                )));
            }
            if !is_lowercase_hex64(&file.sha256) {
                return Err(manifest_error(format!(
                    "the sha256 for {:?} must be 64 lowercase hex characters.",
                    file.path
                )));
            }
        }
        const MAX_TOKENIZER_ROLE_BYTES: u64 = 64 * 1024 * 1024;
        for role in [&self.model_file, &self.tokenizer_file] {
            validate_relative_path(role)?;
            let file = self
                .files
                .iter()
                .find(|candidate| &candidate.path == role)
                .ok_or_else(|| {
                    manifest_error(format!(
                        "the {role} role is not present in the manifest files."
                    ))
                })?;
            if role == &self.tokenizer_file && file.bytes > MAX_TOKENIZER_ROLE_BYTES {
                return Err(manifest_error(format!(
                    "the tokenizer role must stay within {MAX_TOKENIZER_ROLE_BYTES} bytes"
                )));
            }
        }
        Ok(())
    }
}

pub async fn install_family(manifest: InstallManifest, force: bool) -> UseResult<BgeRuntimeStatus> {
    let family = manifest.family()?;
    let current = bge_status(family);
    if !force && current.available {
        return Ok(current);
    }

    let root = managed_root()?;
    let _lock = acquire_lock(&root).await?;
    cleanup_stale(&root, family).await?;

    let current = bge_status(family);
    if !force && current.available {
        return Ok(current);
    }

    let stage = create_stage(&root).await?;
    let install_result = install_into(&stage, &manifest).await;
    if install_result.is_err() {
        let _ = tokio::fs::remove_dir_all(&stage).await;
    }
    install_result?;
    // Validate the staged install before activation: a bad role size must
    // fail while the previous install is still intact.
    validate_assets(&stage, family, BgeInstallSource::Managed)?;

    let target = managed_family_dir(family)?;
    let backup = activate(&stage, &target, family).await?;
    if let Err(error) = validate_assets(&target, family, BgeInstallSource::Managed) {
        // Validation failed on an activated install: restore the backup
        // before refusing, so the previous working engine survives.
        if let Some(backup) = backup {
            let _ = tokio::fs::remove_dir_all(&target).await;
            let _ = tokio::fs::rename(&backup, &target).await;
        }
        return Err(error);
    }
    if let Some(backup) = &backup {
        let _ = tokio::fs::remove_dir_all(backup).await;
    }

    let status = bge_status(family);
    if status.available {
        Ok(status)
    } else {
        Err(crate::bge_error(
            "use.bge.install_failed",
            format!(
                "The {} installation completed without a usable model.",
                family.as_str()
            ),
        ))
    }
}

pub async fn uninstall_family(family: Family) -> UseResult<bool> {
    let root = managed_root()?;
    let _lock = acquire_lock(&root).await?;
    let target = managed_family_dir(family)?;
    if !owned_install(&target, family) {
        return Ok(false);
    }
    tokio::fs::remove_dir_all(&target).await.map_err(|error| {
        crate::bge_error(
            "use.bge.uninstall_failed",
            format!(
                "Failed to remove the managed {} model '{}': {error}",
                family.as_str(),
                target.display()
            ),
        )
    })?;
    Ok(true)
}

async fn install_into(stage: &Path, manifest: &InstallManifest) -> UseResult<()> {
    let client = download::client()?;
    for file in &manifest.files {
        let destination = stage.join(&file.path);
        if let Some(parent) = destination.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|error| {
                crate::bge_error(
                    "use.bge.install_failed",
                    format!(
                        "Failed to create the staged directory '{}': {error}",
                        parent.display()
                    ),
                )
            })?;
        }
        let downloaded =
            download::verified(&client, &file.url, &destination, file.bytes, &file.sha256).await?;
        if downloaded.bytes != file.bytes || downloaded.sha256 != file.sha256 {
            return Err(crate::bge_error(
                "use.bge.integrity_mismatch",
                format!(
                    "File {:?} integrity mismatch: expected {} bytes and {}, got {} bytes and {}.",
                    file.path, file.bytes, file.sha256, downloaded.bytes, downloaded.sha256
                ),
            ));
        }
    }
    write_receipt(stage, manifest).await?;
    Ok(())
}

async fn write_receipt(stage: &Path, manifest: &InstallManifest) -> UseResult<()> {
    use crate::assets::{InstallReceipt, ReceiptFile};
    let receipt = InstallReceipt {
        schema_version: 1,
        family: manifest.family.clone(),
        model_id: manifest.model_id.clone(),
        source_api: manifest.source_api.clone(),
        model_file: manifest.model_file.clone(),
        tokenizer_file: manifest.tokenizer_file.clone(),
        files: manifest
            .files
            .iter()
            .map(|file| ReceiptFile {
                path: file.path.clone(),
                sha256: file.sha256.clone(),
                bytes: file.bytes,
            })
            .collect(),
    };
    let bytes = serde_json::to_vec_pretty(&receipt).map_err(|error| {
        crate::bge_error(
            "use.bge.install_failed",
            format!("Failed to encode the install receipt: {error}"),
        )
    })?;
    let path = stage.join(RECEIPT_FILE);
    let mut file = tokio::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&path)
        .await
        .map_err(|error| {
            crate::bge_error(
                "use.bge.install_failed",
                format!(
                    "Failed to create the install receipt '{}': {error}",
                    path.display()
                ),
            )
        })?;
    file.write_all(&bytes).await.map_err(|error| {
        crate::bge_error(
            "use.bge.install_failed",
            format!(
                "Failed to write the install receipt '{}': {error}",
                path.display()
            ),
        )
    })?;
    file.sync_all().await.map_err(|error| {
        crate::bge_error(
            "use.bge.install_failed",
            format!(
                "Failed to sync the install receipt '{}': {error}",
                path.display()
            ),
        )
    })
}

struct InstallLock {
    _file: std::fs::File,
}

async fn acquire_lock(root: &Path) -> UseResult<InstallLock> {
    tokio::fs::create_dir_all(root).await.map_err(|error| {
        crate::bge_error(
            "use.bge.install_failed",
            format!(
                "Failed to create the BGE data root '{}': {error}",
                root.display()
            ),
        )
    })?;
    let path = root.join(INSTALL_LOCK);
    tokio::task::spawn_blocking(move || {
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(&path)
            .map_err(|error| {
                crate::bge_error(
                    "use.bge.install_failed",
                    format!(
                        "Failed to open the BGE install lock '{}': {error}",
                        path.display()
                    ),
                )
            })?;
        file.lock_exclusive().map_err(|error| {
            crate::bge_error(
                "use.bge.install_failed",
                format!(
                    "Failed to acquire the BGE install lock '{}': {error}",
                    path.display()
                ),
            )
        })?;
        Ok(InstallLock { _file: file })
    })
    .await
    .map_err(|error| {
        crate::bge_error(
            "use.bge.install_failed",
            format!("The BGE install lock task failed: {error}"),
        )
    })?
}

async fn create_stage(root: &Path) -> UseResult<PathBuf> {
    static NEXT_STAGE: AtomicU64 = AtomicU64::new(1);
    for _ in 0..32 {
        let path = root.join(format!(
            "{STAGE_PREFIX}{}-{}",
            std::process::id(),
            NEXT_STAGE.fetch_add(1, Ordering::Relaxed)
        ));
        match tokio::fs::create_dir(&path).await {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(crate::bge_error(
                    "use.bge.install_failed",
                    format!(
                        "Failed to create the BGE staging directory '{}': {error}",
                        path.display()
                    ),
                ))
            }
        }
    }
    Err(crate::bge_error(
        "use.bge.install_failed",
        "Failed to allocate a unique BGE staging directory.",
    ))
}

async fn cleanup_stale(root: &Path, family: Family) -> UseResult<()> {
    let mut entries = tokio::fs::read_dir(root).await.map_err(|error| {
        crate::bge_error(
            "use.bge.install_failed",
            format!(
                "Failed to inspect the BGE data root '{}': {error}",
                root.display()
            ),
        )
    })?;
    while let Some(entry) = entries.next_entry().await.map_err(|error| {
        crate::bge_error(
            "use.bge.install_failed",
            format!(
                "Failed to inspect the BGE data root '{}': {error}",
                root.display()
            ),
        )
    })? {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let is_owned_backup =
            name.starts_with(BACKUP_PREFIX) && owned_install(&entry.path(), family);
        if name.starts_with(STAGE_PREFIX) || is_owned_backup {
            tokio::fs::remove_dir_all(entry.path())
                .await
                .map_err(|error| {
                    crate::bge_error(
                        "use.bge.install_failed",
                        format!("Failed to remove a stale BGE staging directory: {error}"),
                    )
                })?;
        }
    }
    Ok(())
}

async fn activate(stage: &Path, target: &Path, family: Family) -> UseResult<Option<PathBuf>> {
    static NEXT_BACKUP: AtomicU64 = AtomicU64::new(1);
    let parent = target.parent().ok_or_else(|| {
        crate::bge_error(
            "use.bge.install_failed",
            "The BGE install target has no parent directory.",
        )
    })?;
    let backup = parent.join(format!(
        "{BACKUP_PREFIX}{}-{}",
        std::process::id(),
        NEXT_BACKUP.fetch_add(1, Ordering::Relaxed)
    ));
    let had_target = tokio::fs::try_exists(target).await.map_err(|error| {
        crate::bge_error(
            "use.bge.install_failed",
            format!(
                "Failed to inspect the BGE install target '{}': {error}",
                target.display()
            ),
        )
    })?;
    if had_target {
        if !owned_install(target, family) {
            return Err(crate::bge_error(
                "use.bge.install_target_unowned",
                format!(
                    "Refusing to replace the unowned model directory '{}'.",
                    target.display()
                ),
            ));
        }
        tokio::fs::rename(target, &backup).await.map_err(|error| {
            crate::bge_error(
                "use.bge.install_failed",
                format!(
                    "Failed to stage the existing {} install '{}': {error}",
                    family.as_str(),
                    target.display()
                ),
            )
        })?;
    }
    if let Err(error) = tokio::fs::rename(stage, target).await {
        if had_target {
            let _ = tokio::fs::rename(&backup, target).await;
        }
        return Err(crate::bge_error(
            "use.bge.install_failed",
            format!(
                "Failed to activate the {} install '{}': {error}",
                family.as_str(),
                target.display()
            ),
        ));
    }
    // The caller removes the backup once the activated install validates;
    // a staged backup path means an old install awaits that confirmation.
    Ok(had_target.then_some(backup))
}

fn validate_relative_path(path: &str) -> UseResult<()> {
    let valid = !path.is_empty()
        && path.len() <= 512
        && !path.contains('\\')
        && Path::new(path)
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
        && Path::new(path).components().count() >= 1;
    if valid {
        Ok(())
    } else {
        Err(manifest_error(format!(
            "the manifest path {path:?} must be a relative path without '..', '.', or absolute components."
        )))
    }
}

fn is_lowercase_hex64(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn manifest_error(message: impl Into<String>) -> UseError {
    crate::bge_error("use.bge.manifest_invalid", message)
}

#[cfg(test)]
#[path = "install/tests.rs"]
mod tests;
