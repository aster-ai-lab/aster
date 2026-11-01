use std::path::Path;

use aster_use_core::{UseError, UseResult};
use serde::{Deserialize, Serialize};
use tokio::fs;

use super::digest::package_fingerprint;
use super::package::{io_error, read_manifest, validate_surface_files};

pub const RELEASE_BUNDLE_SCHEMA_VERSION: u32 = 1;

/// One optional extension package shipped inside a verified Aster Use release.
///
/// The release bundle remains uninstallable and disabled until the user asks
/// to install it. Its expanded-package digest is included in the outer Aster
/// review plan and rechecked by Aster Use immediately before activation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseBundlePackage {
    pub schema_version: u32,
    pub package_id: String,
    pub component_id: String,
    pub version: String,
    pub route: String,
    pub package_sha256: String,
    pub file_count: u64,
    pub byte_count: u64,
    pub surfaces: Vec<String>,
    pub activity_count: u64,
}

impl ReleaseBundlePackage {
    pub fn validate(&self) -> UseResult<()> {
        if self.schema_version != RELEASE_BUNDLE_SCHEMA_VERSION {
            return Err(UseError::new(
                "use.extension.release_bundle_incompatible",
                format!(
                    "Release bundle schema {} is not supported.",
                    self.schema_version
                ),
            ));
        }
        if self.component_id != format!("use/{}", self.package_id)
            || self.package_sha256.len() != 64
            || !self
                .package_sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            return Err(UseError::new(
                "use.extension.release_bundle_invalid",
                format!(
                    "Release bundle metadata for '{}' is inconsistent.",
                    self.package_id
                ),
            ));
        }
        Ok(())
    }
}

pub async fn inspect_release_bundle(package_root: &Path) -> UseResult<ReleaseBundlePackage> {
    let metadata = fs::symlink_metadata(package_root)
        .await
        .map_err(|error| io_error("inspect release bundle", package_root, error))?;
    if aster_use_core::metadata_is_link_or_reparse_point(&metadata) || !metadata.is_dir() {
        return Err(UseError::new(
            "use.extension.release_bundle_invalid",
            format!(
                "Release bundle '{}' must be a real package directory.",
                package_root.display()
            ),
        ));
    }
    let package_root = fs::canonicalize(package_root)
        .await
        .map_err(|error| io_error("resolve release bundle", package_root, error))?;
    let (manifest, _) = read_manifest(&package_root).await?;
    validate_surface_files(&manifest, &package_root).await?;
    let fingerprint = package_fingerprint(&package_root).await?;
    let surfaces = manifest
        .surface_kinds()
        .into_iter()
        .map(str::to_string)
        .collect();
    let activity_count = manifest.ui_count() as u64;
    let package = ReleaseBundlePackage {
        schema_version: RELEASE_BUNDLE_SCHEMA_VERSION,
        component_id: format!("use/{}", manifest.package_id),
        package_id: manifest.package_id,
        version: manifest.version,
        route: manifest.route,
        package_sha256: fingerprint.sha256,
        file_count: fingerprint.file_count,
        byte_count: fingerprint.byte_count,
        surfaces,
        activity_count,
    };
    package.validate()?;
    Ok(package)
}

/// Validate an AIX archive with the installer's native reader and package contracts.
/// This performs bounded extraction/hashing in temporary storage and never installs,
/// executes, grants permissions to, or marks the package as trusted.
pub async fn inspect_aix_package(path: &Path) -> UseResult<ReleaseBundlePackage> {
    if path.extension() != Some(std::ffi::OsStr::new("aix")) {
        return Err(UseError::new(
            "use.extension.package_unsupported",
            "Published AIX packages require the exact .aix extension.",
        ));
    }
    let metadata = fs::symlink_metadata(path)
        .await
        .map_err(|error| io_error("inspect AIX archive", path, error))?;
    if !metadata.is_file() {
        return Err(UseError::new(
            "use.extension.package_unsupported",
            "Published AIX packages must be regular archive files.",
        ));
    }
    let source = super::source::prepare_package_source(path).await?;
    inspect_release_bundle(source.root()).await
}
