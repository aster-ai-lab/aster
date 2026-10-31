use std::ffi::CString;
use std::fs::{self, DirBuilder, OpenOptions};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::Path;

use aster_oci_agent_protocol::{AgentVmAttachmentManifest, AGENT_VM_DIRECTORY_GUEST_ROOT};
use aster_oci_sdk::{Error, ErrorCode, Result, StorageAccessMode};

pub(super) fn configure_guest_directories(manifest: &AgentVmAttachmentManifest) -> Result<()> {
    if manifest.directories().is_empty() {
        return Ok(());
    }
    let parent = Path::new(AGENT_VM_DIRECTORY_GUEST_ROOT);
    ensure_mountpoint(parent)?;
    for directory in manifest.directories() {
        let destination = directory.guest_mount_path();
        ensure_mountpoint(Path::new(&destination))?;
        let target = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_DIRECTORY)
            .open(&destination)
            .map_err(|error| directory_error(error.to_string()))?;
        let path = CString::new(format!("/proc/self/fd/{}", target.as_raw_fd()))
            .map_err(|error| directory_error(error.to_string()))?;
        let tag =
            CString::new(directory.tag()).map_err(|error| directory_error(error.to_string()))?;
        let flags = libc::MS_NOSUID
            | libc::MS_NODEV
            | if directory.attachment().access_mode() == StorageAccessMode::ReadOnly {
                libc::MS_RDONLY
            } else {
                0
            };
        // SAFETY: the fixed filesystem, tag, and descriptor-pinned destination remain live for mount.
        if unsafe {
            libc::mount(
                tag.as_ptr(),
                path.as_ptr(),
                c"virtiofs".as_ptr(),
                flags,
                std::ptr::null(),
            )
        } != 0
        {
            return Err(directory_error(format!(
                "failed to mount authorized directory {}: {}",
                directory.attachment().identity(),
                std::io::Error::last_os_error()
            )));
        }
    }
    Ok(())
}

fn ensure_mountpoint(path: &Path) -> Result<()> {
    // Searchable by the mapped macOS caller. umask can clear the bit mkdir
    // requests, so the mode is set again after the directory exists.
    let mode = aster_oci_sdk::WORKSPACE_STAGING_DIRECTORY_MODE;
    match DirBuilder::new().mode(mode).create(path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(directory_error(error.to_string())),
    }
    let metadata =
        fs::symlink_metadata(path).map_err(|error| directory_error(error.to_string()))?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || path
            .canonicalize()
            .map_err(|error| directory_error(error.to_string()))?
            != path
    {
        return Err(directory_error(
            "directory mount point is not a plain canonical directory",
        ));
    }
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
        .map_err(|error| directory_error(error.to_string()))?;
    if path != Path::new(AGENT_VM_DIRECTORY_GUEST_ROOT)
        && fs::read_dir(path)
            .map_err(|error| directory_error(error.to_string()))?
            .next()
            .is_some()
    {
        return Err(directory_error("directory mount point is not empty"));
    }
    Ok(())
}

fn directory_error(message: impl Into<String>) -> Error {
    Error::new(ErrorCode::FailedPrecondition, message).for_operation("mount-guest-directories")
}
