//! Compatibility API backed by the canonical, complete native OCR plugin.

use std::io::Write;
use std::time::Duration;

use aster_use_core::{UseError, UseResult};
use aster_use_extension::{
    prepare_remote_package, ExtensionLifecycleIdentity, ExtensionLifecyclePackage, ExtensionPaths,
    ExtensionRegistry, GitHubRegistryRepository, TrustedRegistry,
};

use crate::assets::{
    managed_root, ocr_status, validate_assets, OcrInstallSource, OcrRuntimeStatus, RECEIPT_FILE,
};
use crate::config::MODEL_FAMILY;

const PACKAGE: &str = "aster/aster-ocr";
const OFFICIAL_ROOT: &str = "e1cf8be656c072939721b765eef12a597c668fe9c33cc0de2379370d3879ae9b";

/// Install the complete official OCR AIX plugin, including its engine and models.
/// Unchanged verified releases reuse the canonical generation without downloading.
/// This explicit API authorizes local native execution; inference never installs.
pub async fn install_ppocr_v6(force: bool) -> UseResult<OcrRuntimeStatus> {
    let paths = ExtensionPaths::from_env()?;
    let registry = ExtensionRegistry::new(paths);
    let source = TrustedRegistry::new(
        "aster-official",
        GitHubRegistryRepository::parse("aster-ai-lab/plugins")?.registry_url()?,
        OFFICIAL_ROOT,
        None,
        managed_root()?.join("registry"),
    )?;
    let prepared = prepare_remote_package(&source, PACKAGE, None, "stable", None).await?;
    let record = &prepared.verified_catalog().record;
    let previous = registry.get(PACKAGE).await?;
    if !force {
        if let Some(current) = previous.as_ref().filter(|current| {
            current.receipt.version == record.version
                && record
                    .package
                    .sha256
                    .as_deref()
                    .map(|s| s.trim_start_matches("sha256:"))
                    == current.receipt.package_sha256.as_deref()
                && record
                    .package
                    .manifest_sha256
                    .as_deref()
                    .map(|s| s.trim_start_matches("sha256:"))
                    == Some(current.receipt.manifest_sha256.as_str())
        }) {
            if let Some(lease) = registry.acquire_published_route("aster-ocr").await? {
                if lease.extension().receipt == current.receipt {
                    return register_models(
                        current
                            .receipt
                            .package_root
                            .join("payload/ocr-models")
                            .join(MODEL_FAMILY),
                    )
                    .await;
                }
            }
        }
    }
    let package =
        ExtensionLifecyclePackage::prepare_remote(PACKAGE, prepared.download().await?).await?;
    let generation = previous
        .as_ref()
        .and_then(|p| p.receipt.lifecycle_generation)
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| UseError::new("use.ocr.install_failed", "OCR generation overflow."))?;
    let identity = ExtensionLifecycleIdentity::new(
        PACKAGE,
        package.package_digest(),
        package.manifest_digest(),
        generation,
    )?;
    registry
        .commit_lifecycle_package(&identity, &package)
        .await?;
    let models = registry
        .lifecycle_package_root(&identity)
        .join("payload/ocr-models")
        .join(MODEL_FAMILY);
    let validated = models.clone();
    tokio::task::spawn_blocking(move || validate_assets(&validated, OcrInstallSource::Managed))
        .await
        .map_err(install_error)??;
    registry.publish_lifecycle_package(&identity).await?;
    register_models(models).await
}

/// Repair by replacing the complete plugin through the same native lifecycle.
pub async fn repair_ppocr_v6() -> UseResult<OcrRuntimeStatus> {
    install_ppocr_v6(true).await
}

/// Remove the managed native plugin and its local model-location reference.
pub async fn uninstall_managed_ppocr_v6() -> UseResult<bool> {
    let registry = ExtensionRegistry::new(ExtensionPaths::from_env()?);
    let Some(current) = registry.get(PACKAGE).await? else {
        return Ok(false);
    };
    let identity = ExtensionLifecycleIdentity::new(
        PACKAGE,
        format!(
            "sha256:{}",
            current
                .receipt
                .package_sha256
                .ok_or_else(|| install_error("Missing package digest"))?
        ),
        format!("sha256:{}", current.receipt.manifest_sha256),
        current
            .receipt
            .lifecycle_generation
            .ok_or_else(|| install_error("Missing lifecycle generation"))?,
    )?;
    registry.hide_lifecycle_package(&identity).await?;
    registry
        .remove_lifecycle_package(&identity, Duration::from_secs(5))
        .await?;
    match tokio::fs::remove_file(managed_root()?.join(RECEIPT_FILE)).await {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(install_error(error)),
    }
    Ok(true)
}

async fn register_models(models: std::path::PathBuf) -> UseResult<OcrRuntimeStatus> {
    tokio::task::spawn_blocking(move || {
        validate_assets(&models, OcrInstallSource::Managed)?;
        let root = managed_root()?;
        std::fs::create_dir_all(&root).map_err(install_error)?;
        let mut temporary = tempfile::NamedTempFile::new_in(&root).map_err(install_error)?;
        temporary
            .write_all(&serde_json::to_vec(&models).map_err(install_error)?)
            .map_err(install_error)?;
        temporary.as_file().sync_all().map_err(install_error)?;
        aster_use_extension::persist_temporary_replace_blocking(
            temporary.into_temp_path().keep().map_err(install_error)?,
            &root.join(RECEIPT_FILE),
        )
        .map_err(install_error)?;
        Ok(ocr_status())
    })
    .await
    .map_err(install_error)?
}

fn install_error(error: impl std::fmt::Display) -> UseError {
    UseError::new("use.ocr.install_failed", error.to_string())
}
