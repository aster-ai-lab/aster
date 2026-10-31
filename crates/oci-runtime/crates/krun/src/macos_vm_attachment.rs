use std::fs::{self, File, OpenOptions};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use aster_oci_agent_protocol::{
    AgentVmAttachmentManifest, AgentVmDirectoryAttachment, AGENT_RUNTIME_SHARE_GUEST_ROOT,
    AGENT_VM_ATTACHMENT_MANIFEST_FILE_NAME, AGENT_VM_ATTACHMENT_MANIFEST_MAX_BYTES,
};
use aster_oci_sdk::{
    Error, ErrorCode, OciBundle, Result, StoragePathRestriction, StoragePathRestrictions,
    MAX_CONFIG_BYTES,
};

use crate::macos_assets::PinnedFile;
use crate::macos_runtime_share::MacosRuntimeShare;

pub(crate) struct MacosVmAttachments {
    manifest: PinnedFile,
    configuration: PinnedFile,
    directories: Vec<MacosDirectoryAttachment>,
    path_restrictions: Vec<(PathBuf, StoragePathRestriction)>,
    restrictions_digest: Option<String>,
}

impl MacosVmAttachments {
    pub(crate) fn open(share: &MacosRuntimeShare, expected: Option<&str>) -> Result<Option<Self>> {
        share.reverify()?;
        let path = share.path().join(AGENT_VM_ATTACHMENT_MANIFEST_FILE_NAME);
        let Some(expected) = expected else {
            return match fs::symlink_metadata(&path) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
                _ => Err(attachment_error(
                    "unexpected VM attachment manifest without a digest",
                )),
            };
        };
        let manifest = PinnedFile::open_bounded(
            &path,
            "macOS VM attachment manifest",
            AGENT_VM_ATTACHMENT_MANIFEST_MAX_BYTES as u64,
        )?;
        if manifest.path().parent() != Some(share.path())
            || format!("sha256:{}", manifest.sha256()) != expected
        {
            return Err(attachment_error(
                "VM attachment manifest path or digest differs from its Host binding",
            ));
        }
        require_private_control(&manifest)?;
        let decoded = AgentVmAttachmentManifest::from_bytes(
            &manifest.read_bounded(AGENT_VM_ATTACHMENT_MANIFEST_MAX_BYTES as u64)?,
        )?;
        if decoded.directories().is_empty()
            || !decoded.storage().is_empty()
            || !decoded.network().is_empty()
            || decoded.guest_bundle().as_str() != format!("{AGENT_RUNTIME_SHARE_GUEST_ROOT}/bundle")
        {
            return Err(attachment_error(
                "macOS VM attachments require directories and the exact dedicated Guest bundle",
            ));
        }
        let bundle = share.path().join("bundle");
        let configuration = PinnedFile::open_bounded(
            &bundle.join("config.json"),
            "macOS VM attachment configuration",
            MAX_CONFIG_BYTES,
        )?;
        if configuration.path().parent() != Some(bundle.as_path()) {
            return Err(attachment_error(
                "VM attachment configuration escaped its exact bundle",
            ));
        }
        require_private_control(&configuration)?;
        let encoded = String::from_utf8(configuration.read_bounded(MAX_CONFIG_BYTES)?)
            .map_err(|error| attachment_error(error.to_string()))?;
        let bundle = OciBundle::from_json(&bundle, encoded)?;
        decoded.validate_bundle(&bundle)?;
        let restrictions = StoragePathRestrictions::from_bundle(&bundle)?;
        let restrictions_digest = restrictions
            .as_ref()
            .map(StoragePathRestrictions::digest)
            .transpose()?;
        let mut path_restrictions = Vec::new();
        if let Some(restrictions) = restrictions {
            for restriction in restrictions.entries() {
                let directory = decoded
                    .directories()
                    .iter()
                    .find(|directory| directory.attachment().identity() == restriction.identity())
                    .ok_or_else(|| attachment_error("path restrictions have no bound directory"))?;
                path_restrictions
                    .push((PathBuf::from(directory.host_source()), restriction.clone()));
            }
        }
        let mut directories = Vec::with_capacity(decoded.directories().len());
        for grant in decoded.directories() {
            directories.push(MacosDirectoryAttachment::open(grant.clone())?);
        }
        for (index, directory) in directories.iter().enumerate() {
            if overlaps(directory.path(), share.path())
                || directories[..index]
                    .iter()
                    .any(|other| overlaps(directory.path(), other.path()))
            {
                return Err(attachment_error(
                    "directory grants overlap the private runtime share or each other",
                ));
            }
        }
        share.reverify()?;
        Ok(Some(Self {
            manifest,
            configuration,
            directories,
            path_restrictions,
            restrictions_digest,
        }))
    }

    pub(crate) fn directories(&self) -> &[MacosDirectoryAttachment] {
        &self.directories
    }

    pub(crate) fn path_restrictions(&self) -> &[(PathBuf, StoragePathRestriction)] {
        &self.path_restrictions
    }

    pub(crate) fn restrictions_digest(&self) -> Option<&str> {
        self.restrictions_digest.as_deref()
    }

    pub(crate) fn reverify(&self) -> Result<()> {
        self.manifest.reverify("VM entry")?;
        self.configuration.reverify("VM entry")?;
        for directory in &self.directories {
            directory.reverify()?;
        }
        Ok(())
    }
}

pub(crate) struct MacosDirectoryAttachment {
    grant: AgentVmDirectoryAttachment,
    directory: File,
}

impl MacosDirectoryAttachment {
    fn open(grant: AgentVmDirectoryAttachment) -> Result<Self> {
        let path = Path::new(grant.host_source());
        let canonical = path
            .canonicalize()
            .map_err(|error| attachment_error(error.to_string()))?;
        if canonical != path {
            return Err(attachment_error(
                "directory grant resolves through a Host path alias",
            ));
        }
        let directory = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_CLOEXEC | libc::O_DIRECTORY | libc::O_NOFOLLOW_ANY)
            .open(path)
            .map_err(|error| {
                attachment_error(format!("failed to pin caller directory: {error}"))
            })?;
        let pinned = Self { grant, directory };
        pinned.reverify()?;
        Ok(pinned)
    }

    pub(crate) fn grant(&self) -> &AgentVmDirectoryAttachment {
        &self.grant
    }
    pub(crate) fn path(&self) -> &Path {
        Path::new(self.grant.host_source())
    }
    pub(crate) fn pinned_path(&self) -> PathBuf {
        PathBuf::from(format!("/dev/fd/{}", self.directory.as_raw_fd()))
    }

    fn reverify(&self) -> Result<()> {
        let path = fs::symlink_metadata(self.path())
            .map_err(|error| attachment_error(error.to_string()))?;
        let opened = self
            .directory
            .metadata()
            .map_err(|error| attachment_error(error.to_string()))?;
        if self
            .path()
            .canonicalize()
            .map_err(|error| attachment_error(error.to_string()))?
            != self.path()
        {
            return Err(attachment_error(
                "caller directory gained a Host path alias before VM entry",
            ));
        }
        let expected = self.grant.source_identity();
        // SAFETY: geteuid has no preconditions or failure return.
        let uid = unsafe { libc::geteuid() };
        for metadata in [path, opened] {
            if !metadata.is_dir()
                || metadata.file_type().is_symlink()
                || metadata.uid() != uid
                || metadata.dev() != expected.device()
                || metadata.ino() != expected.inode()
            {
                return Err(attachment_error(
                    "caller directory identity changed before VM entry",
                ));
            }
        }
        Ok(())
    }
}

fn require_private_control(file: &PinnedFile) -> Result<()> {
    let metadata =
        fs::symlink_metadata(file.path()).map_err(|error| attachment_error(error.to_string()))?;
    // SAFETY: geteuid has no preconditions or failure return.
    let uid = unsafe { libc::geteuid() };
    if metadata.uid() != uid || metadata.mode() & 0o777 != 0o600 || metadata.nlink() != 1 {
        return Err(attachment_error(
            "VM attachment control files must be private and unaliased",
        ));
    }
    Ok(())
}

fn overlaps(left: &Path, right: &Path) -> bool {
    left.starts_with(right) || right.starts_with(left)
}
fn attachment_error(message: impl Into<String>) -> Error {
    Error::new(ErrorCode::FailedPrecondition, message).for_operation("pin-macos-vm-attachments")
}

#[cfg(test)]
#[path = "macos_vm_attachment_tests.rs"]
mod tests;
