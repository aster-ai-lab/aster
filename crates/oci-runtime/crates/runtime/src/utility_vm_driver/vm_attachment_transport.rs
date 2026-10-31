use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use aster_oci_agent_protocol::{
    AgentVmAttachmentManifest, AGENT_RUNTIME_SHARE_GUEST_ROOT,
    AGENT_VM_ATTACHMENT_MANIFEST_FILE_NAME, AGENT_VM_ATTACHMENT_MANIFEST_MAX_BYTES,
};
use aster_oci_sdk::{Error, ErrorCode, Result, RUNTIME_BUNDLE_HANDOFF_BUNDLE_DIRECTORY};
use tokio::io::AsyncReadExt;

use super::atomic_publication;
use super::layout::{is_private_file, path_metadata};

pub(super) const PENDING_MANIFEST_FILE_NAME: &str = ".aster-oci-agent-vm-attachments.pending";
const STAGING_MANIFEST_PREFIX: &str = ".aster-oci-agent-vm-attachments.pending.";
const PUBLISH_ATTEMPTS: usize = atomic_publication::PUBLISH_ATTEMPTS;

pub(super) fn ensure_dedicated_bundle_path(manifest: &AgentVmAttachmentManifest) -> Result<()> {
    let expected =
        format!("{AGENT_RUNTIME_SHARE_GUEST_ROOT}/{RUNTIME_BUNDLE_HANDOFF_BUNDLE_DIRECTORY}");
    if manifest.guest_bundle().as_str() != expected {
        return Err(attachment_error(
            ErrorCode::FailedPrecondition,
            format!(
                "VM attachment transport requires the dedicated Guest bundle path {expected}, received {}",
                manifest.guest_bundle().as_str()
            ),
        ));
    }
    Ok(())
}

pub(super) async fn persist_manifest(
    runtime_share: &Path,
    manifest: &AgentVmAttachmentManifest,
) -> Result<()> {
    let final_path = runtime_share.join(AGENT_VM_ATTACHMENT_MANIFEST_FILE_NAME);
    let pending_path = runtime_share.join(PENDING_MANIFEST_FILE_NAME);
    let encoded = manifest.to_bytes()?;
    // Adoption stays inside the attempt loop so a winner that publishes and
    // deletes pending is adopted instead of racing that cleanup.
    for attempt in 0..PUBLISH_ATTEMPTS {
        if adopt_published_manifest(runtime_share, &final_path, &pending_path, manifest).await? {
            return Ok(());
        }
        if let Err(error) =
            create_or_reuse_pending(runtime_share, &pending_path, &encoded, manifest).await
        {
            match classify_concurrent_loss(
                error,
                attempt,
                adopt_published_manifest(runtime_share, &final_path, &pending_path, manifest)
                    .await?,
            )? {
                ConcurrentPublication::Retry => continue,
                ConcurrentPublication::Adopted => return Ok(()),
            }
        }
        if let Err(error) =
            publish_manifest(runtime_share, &pending_path, &final_path, manifest).await
        {
            match classify_concurrent_loss(
                error,
                attempt,
                adopt_published_manifest(runtime_share, &final_path, &pending_path, manifest)
                    .await?,
            )? {
                ConcurrentPublication::Retry => continue,
                ConcurrentPublication::Adopted => return Ok(()),
            }
        }
        return Ok(());
    }
    Err(attachment_error(
        ErrorCode::Unavailable,
        "VM attachment transport manifest publication kept losing its concurrent owner",
    )
    .retryable(true))
}

async fn adopt_published_manifest(
    runtime_share: &Path,
    final_path: &Path,
    pending_path: &Path,
    manifest: &AgentVmAttachmentManifest,
) -> Result<bool> {
    let Some(retained) = read_manifest_if_present(final_path).await? else {
        return Ok(false);
    };
    if retained != *manifest {
        return Err(manifest_conflict());
    }
    remove_matching_pending(pending_path, manifest).await?;
    sync_directory(runtime_share).await?;
    Ok(true)
}

enum ConcurrentPublication {
    Retry,
    Adopted,
}

fn classify_concurrent_loss(
    error: Error,
    attempt: usize,
    adopted: bool,
) -> Result<ConcurrentPublication> {
    if adopted {
        return Ok(ConcurrentPublication::Adopted);
    }
    if error.retryable && attempt + 1 < PUBLISH_ATTEMPTS {
        return Ok(ConcurrentPublication::Retry);
    }
    Err(error)
}

async fn create_or_reuse_pending(
    runtime_share: &Path,
    pending: &Path,
    encoded: &[u8],
    expected: &AgentVmAttachmentManifest,
) -> Result<()> {
    if let Some(initial_metadata) = path_metadata(pending).await? {
        match read_manifest(pending).await {
            Ok(retained) if retained == *expected => return Ok(()),
            Ok(_) => return Err(manifest_conflict()),
            Err(error) if error.code == ErrorCode::FailedPrecondition => {
                // The new writer never exposes a partial pending inode: it
                // links a fully synced staging inode into this name. A
                // malformed private pending file therefore belongs to an
                // interrupted writer from the legacy path and may be
                // discarded. Non-private or disappearing files remain
                // fail-closed below.
                let Some(metadata) = path_metadata(pending).await? else {
                    return Err(error.retryable(true));
                };
                if metadata.dev() != initial_metadata.dev()
                    || metadata.ino() != initial_metadata.ino()
                {
                    return Err(error.retryable(true));
                }
                if !is_private_file(&metadata) {
                    return Err(error);
                }
                remove_private_file_if_present(pending).await?;
            }
            Err(error) => return Err(error),
        }
    }

    let staging = atomic_publication::create_complete_staging(
        runtime_share,
        pending,
        encoded,
        STAGING_MANIFEST_PREFIX,
    )
    .await
    .map_err(|error| {
        attachment_error(
            ErrorCode::Internal,
            format!(
                "failed to create VM attachment transport staging manifest near {}: {error}",
                pending.display()
            ),
        )
    })?;
    match tokio::fs::hard_link(&staging, pending).await {
        Ok(()) => {
            atomic_publication::remove_file_if_present(&staging)
                .await
                .map_err(|error| {
                    attachment_error(
                        ErrorCode::Internal,
                        format!(
                            "failed to remove VM attachment transport staging manifest {}: {error}",
                            staging.display()
                        ),
                    )
                })?;
            sync_directory(runtime_share).await
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            let _ = atomic_publication::remove_file_if_present(&staging).await;
            ensure_pending_matches(pending, expected).await
        }
        Err(error) => {
            let _ = atomic_publication::remove_file_if_present(&staging).await;
            Err(attachment_error(
                ErrorCode::Internal,
                format!(
                    "failed to publish VM attachment transport pending manifest {}: {error}",
                    pending.display()
                ),
            ))
        }
    }
}

async fn ensure_pending_matches(
    pending: &Path,
    expected: &AgentVmAttachmentManifest,
) -> Result<()> {
    match read_manifest_if_present(pending).await? {
        Some(retained) if retained == *expected => Ok(()),
        Some(_) => Err(manifest_conflict()),
        None => Err(attachment_error(
            ErrorCode::Unavailable,
            format!(
                "VM attachment transport pending manifest disappeared before adoption: {}",
                pending.display()
            ),
        )
        .retryable(true)),
    }
}

async fn publish_manifest(
    runtime_share: &Path,
    pending: &Path,
    final_path: &Path,
    expected: &AgentVmAttachmentManifest,
) -> Result<()> {
    match tokio::fs::hard_link(pending, final_path).await {
        Ok(()) => {
            let retained = read_manifest(final_path).await?;
            if retained != *expected {
                return Err(manifest_conflict());
            }
            remove_matching_pending(pending, expected).await?;
            sync_directory(runtime_share).await
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            let retained = read_manifest(final_path).await?;
            if retained != *expected {
                return Err(manifest_conflict());
            }
            remove_matching_pending(pending, expected).await?;
            sync_directory(runtime_share).await
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            match read_manifest_if_present(final_path).await? {
                Some(retained) if retained == *expected => sync_directory(runtime_share).await,
                Some(_) => Err(manifest_conflict()),
                None => Err(attachment_error(
                    ErrorCode::Unavailable,
                    format!(
                        "VM attachment transport pending manifest disappeared before publication: {}",
                        pending.display()
                    ),
                )
                .retryable(true)),
            }
        }
        Err(error) => Err(attachment_error(
            ErrorCode::Internal,
            format!(
                "failed to commit VM attachment transport manifest {}: {error}",
                final_path.display()
            ),
        )),
    }
}

async fn read_manifest_if_present(path: &Path) -> Result<Option<AgentVmAttachmentManifest>> {
    let Some(initial_metadata) = path_metadata(path).await? else {
        return Ok(None);
    };
    match read_manifest_bound(path, &initial_metadata).await {
        Ok(manifest) => Ok(Some(manifest)),
        Err(error) => match path_metadata(path).await? {
            None => Ok(None),
            Some(current_metadata)
                if !atomic_publication::same_file_identity(
                    &initial_metadata,
                    &current_metadata,
                ) =>
            {
                Err(attachment_error(
                    ErrorCode::Unavailable,
                    format!(
                        "VM attachment transport manifest changed while it was being read: {}",
                        path.display()
                    ),
                )
                .retryable(true))
            }
            Some(_) => Err(error),
        },
    }
}

async fn remove_matching_pending(
    pending: &Path,
    expected: &AgentVmAttachmentManifest,
) -> Result<()> {
    if let Some(retained) = read_manifest_if_present(pending).await? {
        if retained != *expected {
            return Err(manifest_conflict());
        }
        remove_private_file_if_present(pending).await?;
    }
    Ok(())
}

fn manifest_conflict() -> Error {
    attachment_error(
        ErrorCode::Conflict,
        "VM attachment transport manifest differs from the requested Create",
    )
}

async fn read_manifest(path: &Path) -> Result<AgentVmAttachmentManifest> {
    let metadata = tokio::fs::symlink_metadata(path).await.map_err(|error| {
        attachment_error(
            ErrorCode::FailedPrecondition,
            format!(
                "failed to inspect VM attachment transport manifest {}: {error}",
                path.display()
            ),
        )
    })?;
    read_manifest_bound(path, &metadata).await
}

async fn read_manifest_bound(
    path: &Path,
    metadata: &std::fs::Metadata,
) -> Result<AgentVmAttachmentManifest> {
    if !is_private_file(metadata)
        || metadata.len() == 0
        || metadata.len() > AGENT_VM_ATTACHMENT_MANIFEST_MAX_BYTES as u64
    {
        return Err(attachment_error(
            ErrorCode::FailedPrecondition,
            format!(
                "VM attachment transport manifest is not a bounded private file: {}",
                path.display()
            ),
        ));
    }
    let mut verified = crate::file_security::open_verified_regular_file(path)
        .await
        .map_err(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                attachment_error(
                    ErrorCode::Unavailable,
                    format!(
                        "VM attachment transport manifest disappeared while it was being opened: {}",
                        path.display()
                    ),
                )
                .retryable(true)
            } else if error.kind() == io::ErrorKind::InvalidData {
                attachment_error(
                    ErrorCode::Unavailable,
                    format!(
                        "VM attachment transport manifest changed while it was being opened: {} ({error})",
                        path.display()
                    ),
                )
                .retryable(true)
            } else {
                attachment_error(
                    ErrorCode::FailedPrecondition,
                    format!(
                        "failed to open VM attachment transport manifest {}: {error}",
                        path.display()
                    ),
                )
            }
        })?;
    let opened_metadata = verified.file.metadata().await.map_err(|error| {
        attachment_error(
            ErrorCode::FailedPrecondition,
            format!(
                "failed to inspect opened VM attachment transport manifest {}: {error}",
                path.display()
            ),
        )
    })?;
    if opened_metadata.len() != metadata.len()
        || !atomic_publication::same_file_identity(metadata, &opened_metadata)
    {
        return Err(attachment_error(
            ErrorCode::Unavailable,
            format!(
                "VM attachment transport manifest changed while it was being opened: {}",
                path.display()
            ),
        )
        .retryable(true));
    }
    if !is_private_file(&opened_metadata)
        || opened_metadata.len() == 0
        || opened_metadata.len() > AGENT_VM_ATTACHMENT_MANIFEST_MAX_BYTES as u64
    {
        return Err(attachment_error(
            ErrorCode::FailedPrecondition,
            format!(
                "VM attachment transport manifest is not a bounded private file: {}",
                path.display()
            ),
        ));
    }
    let mut encoded = Vec::with_capacity(opened_metadata.len() as usize);
    (&mut verified.file)
        .take((AGENT_VM_ATTACHMENT_MANIFEST_MAX_BYTES + 1) as u64)
        .read_to_end(&mut encoded)
        .await
        .map_err(|error| {
            attachment_error(
                ErrorCode::FailedPrecondition,
                format!(
                    "failed to open VM attachment transport manifest {}: {error}",
                    path.display()
                ),
            )
        })?;
    let final_metadata = verified.file.metadata().await.map_err(|error| {
        attachment_error(
            ErrorCode::FailedPrecondition,
            format!(
                "failed to inspect VM attachment transport manifest after reading {}: {error}",
                path.display()
            ),
        )
    })?;
    if final_metadata.len() != opened_metadata.len()
        || encoded.len() != opened_metadata.len() as usize
        || !atomic_publication::same_file_identity(&opened_metadata, &final_metadata)
    {
        return Err(attachment_error(
            ErrorCode::Unavailable,
            format!(
                "VM attachment transport manifest changed while it was being read: {}",
                path.display()
            ),
        )
        .retryable(true));
    }
    if !is_private_file(&final_metadata)
        || final_metadata.len() == 0
        || final_metadata.len() > AGENT_VM_ATTACHMENT_MANIFEST_MAX_BYTES as u64
    {
        return Err(attachment_error(
            ErrorCode::FailedPrecondition,
            format!(
                "VM attachment transport manifest is not a bounded private file: {}",
                path.display()
            ),
        ));
    }
    verified
        .verify_unchanged(encoded.len() as u64)
        .await
        .map_err(|error| {
            attachment_error(
                ErrorCode::Unavailable,
                format!(
                    "VM attachment transport manifest changed while it was being read: {} ({error})",
                    path.display()
                ),
            )
            .retryable(true)
        })?;
    verified
        .verify_path_unchanged(path)
        .await
        .map_err(|error| {
            attachment_error(
                ErrorCode::Unavailable,
                format!(
                    "VM attachment transport manifest path changed while it was being read: {} ({error})",
                    path.display()
                ),
            )
            .retryable(true)
        })?;
    AgentVmAttachmentManifest::from_bytes(&encoded).map_err(|error| {
        attachment_error(
            ErrorCode::FailedPrecondition,
            format!(
                "invalid VM attachment transport manifest {}: {error}",
                path.display()
            ),
        )
    })
}

pub(super) async fn ensure_absent(final_path: &Path, pending_path: &Path) -> Result<()> {
    for path in [final_path, pending_path] {
        if path_metadata(path).await?.is_some() {
            return Err(attachment_error(
                ErrorCode::Conflict,
                format!(
                    "a Create without VM attachments found stale transport evidence: {}",
                    path.display()
                ),
            ));
        }
    }
    Ok(())
}

async fn remove_private_file_if_present(path: &Path) -> Result<()> {
    let Some(metadata) = path_metadata(path).await? else {
        return Ok(());
    };
    remove_private_file_bound(path, &metadata).await
}

async fn remove_private_file_bound(
    path: &Path,
    initial_metadata: &std::fs::Metadata,
) -> Result<()> {
    if !is_private_file(initial_metadata) {
        return Err(attachment_error(
            ErrorCode::FailedPrecondition,
            format!(
                "refusing to remove non-private KVM evidence: {}",
                path.display()
            ),
        ));
    }
    let verified = match crate::file_security::open_verified_regular_file(path).await {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) if error.kind() == io::ErrorKind::InvalidData => {
            return Err(attachment_error(
                ErrorCode::Unavailable,
                format!(
                    "refusing to remove changed KVM evidence {} ({error})",
                    path.display()
                ),
            )
            .retryable(true));
        }
        Err(error) => {
            return Err(attachment_error(
                ErrorCode::Internal,
                format!(
                    "failed to open KVM evidence {} for identity binding: {error}",
                    path.display()
                ),
            ));
        }
    };
    let opened_metadata = verified.file.metadata().await.map_err(|error| {
        attachment_error(
            ErrorCode::Internal,
            format!(
                "failed to inspect opened KVM evidence {}: {error}",
                path.display()
            ),
        )
    })?;
    if !is_private_file(&opened_metadata)
        || opened_metadata.len() != initial_metadata.len()
        || !atomic_publication::same_file_identity(initial_metadata, &opened_metadata)
    {
        return Err(attachment_error(
            ErrorCode::Unavailable,
            format!(
                "refusing to remove replaced KVM evidence {}",
                path.display()
            ),
        )
        .retryable(true));
    }
    match verified.verify_path_unchanged(path).await {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(attachment_error(
                ErrorCode::Unavailable,
                format!(
                    "refusing to remove changed KVM evidence {} ({error})",
                    path.display()
                ),
            )
            .retryable(true));
        }
    }
    match tokio::fs::remove_file(path).await {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(attachment_error(
                ErrorCode::Internal,
                format!("failed to remove {}: {error}", path.display()),
            ));
        }
    }
    let parent = path.parent().ok_or_else(|| {
        attachment_error(
            ErrorCode::Internal,
            format!("KVM evidence path has no parent: {}", path.display()),
        )
    })?;
    sync_directory(parent).await
}

async fn sync_directory(path: &Path) -> Result<()> {
    tokio::fs::File::open(path)
        .await
        .map_err(|error| {
            attachment_error(
                ErrorCode::Internal,
                format!(
                    "failed to open directory {} for sync: {error}",
                    path.display()
                ),
            )
        })?
        .sync_all()
        .await
        .map_err(|error| {
            attachment_error(
                ErrorCode::Internal,
                format!("failed to sync directory {}: {error}", path.display()),
            )
        })
}

fn attachment_error(code: ErrorCode, message: impl Into<String>) -> Error {
    Error::new(code, message).for_operation("prepare-vm-attachments")
}

#[cfg(test)]
#[path = "vm_attachment_transport_tests.rs"]
mod tests;
