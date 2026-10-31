use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use aster_oci_agent_protocol::{
    AgentVmAttachmentManifest, AgentVmDirectoryAttachment, AgentVmDirectorySourceIdentity,
    AGENT_VM_ATTACHMENT_MANIFEST_FILE_NAME,
};
use aster_oci_sdk::{Error, ErrorCode, Result};

use super::layout::canonical_plain_directory;
use super::vm_attachment_transport::{
    ensure_absent, ensure_dedicated_bundle_path, persist_manifest, PENDING_MANIFEST_FILE_NAME,
};
use super::UtilityVmLaunchRequest;

pub(crate) async fn prepare(request: &UtilityVmLaunchRequest<'_>) -> Result<Option<String>> {
    request.attachment_contract.validate(request.bundle)?;
    if !request.attachment_contract.network_attachments().is_empty()
        || request.attachment_contract.guest_session().is_some()
    {
        return Err(directory_error(
            ErrorCode::Unsupported,
            "HVF directory transport requires a dedicated VM without network attachments",
        ));
    }
    let restrictions = request
        .attachment_contract
        .storage_path_restrictions(request.bundle)?;
    if restrictions.is_none()
        && aster_oci_sdk::StoragePathRestrictions::from_bundle(request.bundle)?.is_some()
    {
        return Err(directory_error(
            ErrorCode::InvalidArgument,
            "directory restrictions require a declared required extension",
        ));
    }
    if request.attachment_contract.storage().is_empty() {
        ensure_absent(
            &request
                .runtime_share
                .join(AGENT_VM_ATTACHMENT_MANIFEST_FILE_NAME),
            &request.runtime_share.join(PENDING_MANIFEST_FILE_NAME),
        )
        .await?;
        return Ok(None);
    }
    let digest = request.attachment_contract.digest()?;
    let mut directories = Vec::with_capacity(request.attachment_contract.storage().len());
    let mut paths: Vec<PathBuf> = Vec::new();
    let configuration: serde_json::Value = serde_json::from_str(request.bundle.config_json())
        .map_err(|error| directory_error(ErrorCode::Internal, error.to_string()))?;
    for attachment in request.attachment_contract.storage() {
        let source = configuration
            .pointer(attachment.mount().json_pointer())
            .and_then(|mount| mount.get("source"))
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                directory_error(ErrorCode::InvalidArgument, "directory grant has no source")
            })?;
        let source = canonical_plain_directory(Path::new(source), "HVF caller directory").await?;
        if overlaps(&source, request.runtime_share)
            || paths.iter().any(|path| overlaps(&source, path))
        {
            return Err(directory_error(
                ErrorCode::FailedPrecondition,
                "HVF directory grants must be disjoint from the runtime share and each other",
            ));
        }
        let metadata = tokio::fs::symlink_metadata(&source)
            .await
            .map_err(|error| {
                directory_error(
                    ErrorCode::FailedPrecondition,
                    format!(
                        "failed to pin directory identity {}: {error}",
                        source.display()
                    ),
                )
            })?;
        let encoded = source.to_str().ok_or_else(|| {
            directory_error(
                ErrorCode::InvalidArgument,
                "HVF directory path is not UTF-8",
            )
        })?;
        directories.push(AgentVmDirectoryAttachment::new(
            attachment.clone(),
            encoded,
            AgentVmDirectorySourceIdentity::new(metadata.dev(), metadata.ino())?,
            &digest,
        )?);
        paths.push(source);
    }
    directories.sort();
    let mut manifest = AgentVmAttachmentManifest::new_with_directories(
        request.target.clone(),
        request.guest_bundle.clone(),
        request.bundle.config_digest(),
        digest,
        Vec::new(),
        Vec::new(),
        directories,
    )?;
    if let Some(restrictions) = restrictions {
        manifest = manifest.with_storage_path_restrictions(request.bundle, &restrictions)?;
    }
    manifest.validate_bundle(request.bundle)?;
    ensure_dedicated_bundle_path(&manifest)?;
    persist_manifest(request.runtime_share, &manifest).await?;
    manifest.digest().map(Some)
}

fn overlaps(left: &Path, right: &Path) -> bool {
    left.starts_with(right) || right.starts_with(left)
}

fn directory_error(code: ErrorCode, message: impl Into<String>) -> Error {
    Error::new(code, message).for_operation("prepare-hvf-directories")
}

#[cfg(test)]
#[path = "hvf_directory_tests.rs"]
mod tests;
