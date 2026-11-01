//! Prepare native AIX publication from package-identity sources and domain policy.

use std::path::{Path, PathBuf};

use aster_use_core::{UseError, UseResult};
use serde::Serialize;
use sha2::{Digest, Sha256};
use tokio::fs;
use tokio::io::AsyncReadExt;

use crate::package::{io_error, validate_text_asset};
use crate::remote::oci::encode_aix_manifest;
use crate::source::MAX_ARCHIVE_BYTES;
use crate::{inspect_aix_package, inspect_release_bundle, ReleaseBundlePackage};

mod domains;
#[cfg(test)]
mod tests;

const ERROR: &str = "use.extension.publication_invalid";

/// Admitted source identity and immutable OCI bytes, without signing or upload.
/// Only the validating preparation function can construct this value.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedAixPublication {
    domain: String,
    owners: Vec<String>,
    source_path: PathBuf,
    package: ReleaseBundlePackage,
    oci_repository: String,
    archive_sha256: String,
    archive_length: u64,
    manifest_sha256: String,
    #[serde(skip)]
    manifest_bytes: Vec<u8>,
}

impl PreparedAixPublication {
    /// Return the validated package identity and complete expanded-file evidence.
    pub fn package(&self) -> &ReleaseBundlePackage {
        &self.package
    }

    /// Return the native source directory whose content matches the AIX archive.
    pub fn source_path(&self) -> &Path {
        &self.source_path
    }

    /// Return the exact OCI manifest bytes identified by the serialized digest.
    pub fn manifest_bytes(&self) -> &[u8] {
        &self.manifest_bytes
    }
}

/// Check domain/publisher admission, native source layout, license and AIX bytes.
///
/// Reads bounded local inputs and extracts the archive into temporary storage.
/// It does not modify the repository, install, execute, sign, upload or establish
/// trust. Invalid policy, source/archive drift and invalid native packages return
/// stable SDK errors. I/O and extraction failures preserve existing SDK codes.
/// Cancellation drops asynchronous work; the existing blocking extraction and
/// package fingerprint tasks are not abortable once running.
pub async fn prepare_aix_publication(
    repository_root: &Path,
    domain: &str,
    archive: &Path,
) -> UseResult<PreparedAixPublication> {
    let package = inspect_aix_package(archive).await?;
    let (publisher, name) = package.package_id.split_once('/').ok_or_else(|| {
        invalid("The validated package does not contain a publisher/name identity.")
    })?;
    let repository_root = fs::canonicalize(repository_root)
        .await
        .map_err(|error| io_error("resolve plugin repository", repository_root, error))?;
    let owners = domains::admit(&repository_root, domain, publisher).await?;
    let expected_source = repository_root.join("plugins").join(publisher).join(name);
    let source_path = fs::canonicalize(&expected_source)
        .await
        .map_err(|error| io_error("resolve plugin source", &expected_source, error))?;
    if source_path != expected_source {
        return Err(invalid(
            "Plugin source directories must not be redirected through links.",
        ));
    }
    validate_text_asset(
        ERROR,
        "plugin license",
        "text",
        &source_path,
        &source_path.join("LICENSE"),
        256 * 1024,
    )
    .await?;
    if inspect_release_bundle(&source_path).await? != package {
        return Err(invalid(
            "The AIX archive must contain exactly the current plugin source files.",
        ));
    }
    let mut input = fs::File::open(archive)
        .await
        .map_err(|error| io_error("open publication archive", archive, error))?
        .take(MAX_ARCHIVE_BYTES + 1);
    let mut digest = Sha256::new();
    let mut archive_length = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = input
            .read(&mut buffer)
            .await
            .map_err(|error| io_error("hash publication archive", archive, error))?;
        if count == 0 {
            break;
        }
        archive_length += count as u64;
        if archive_length > MAX_ARCHIVE_BYTES {
            return Err(invalid(
                "The publication archive exceeds the native AIX byte limit.",
            ));
        }
        digest.update(&buffer[..count]);
    }
    let archive_sha256 = format!("sha256:{:x}", digest.finalize());
    let manifest_bytes = encode_aix_manifest(
        format!("{name}.aix"),
        archive_sha256.clone(),
        archive_length,
    )?;
    Ok(PreparedAixPublication {
        domain: domain.into(),
        owners,
        source_path,
        oci_repository: format!("ghcr.io/aster-ai-lab/plugins/{}", package.package_id),
        package,
        archive_sha256,
        archive_length,
        manifest_sha256: format!("sha256:{:x}", Sha256::digest(&manifest_bytes)),
        manifest_bytes,
    })
}

fn invalid(message: impl Into<String>) -> UseError {
    UseError::new(ERROR, message)
}
