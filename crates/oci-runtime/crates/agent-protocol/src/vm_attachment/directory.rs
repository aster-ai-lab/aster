use aster_oci_sdk::{Result, StorageAccessMode, StorageAttachment, StorageCleanup, StorageOwnership};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::storage::parse_mount_index;
use super::{
    manifest_error, validate_configuration_attachment, validate_digest,
    AGENT_VM_DIRECTORY_GUEST_ROOT, AGENT_VM_DIRECTORY_RELATIVE_ROOT,
};

const TAG_DOMAIN: &[u8] = b"aster.oci.agent-vm-directory-tag.v1\0";

/// Exact Host directory identity retained across the VMM handoff.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentVmDirectorySourceIdentity {
    device: u64,
    inode: u64,
}

impl AgentVmDirectorySourceIdentity {
    /// Bind an authorized directory to its filesystem device and nonzero inode.
    pub fn new(device: u64, inode: u64) -> Result<Self> {
        if inode == 0 {
            return Err(manifest_error(
                "directory attachment requires a nonzero inode",
            ));
        }
        Ok(Self { device, inode })
    }

    /// Host filesystem device.
    #[must_use]
    pub const fn device(self) -> u64 {
        self.device
    }

    /// Exact Host directory inode.
    #[must_use]
    pub const fn inode(self) -> u64 {
        self.inode
    }
}

/// One caller-owned directory exposed through an independent virtio-fs device.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentVmDirectoryAttachment {
    attachment: StorageAttachment,
    host_source: String,
    source_identity: AgentVmDirectorySourceIdentity,
    tag: String,
}

impl AgentVmDirectoryAttachment {
    /// Bind the unchanged public storage grant to a pinned Host directory.
    pub fn new(
        attachment: StorageAttachment,
        host_source: impl Into<String>,
        source_identity: AgentVmDirectorySourceIdentity,
        attachment_digest: &str,
    ) -> Result<Self> {
        let entry = Self {
            tag: derive_tag(attachment_digest, attachment.identity().as_str())?,
            attachment,
            host_source: host_source.into(),
            source_identity,
        };
        entry.validate_shape(attachment_digest)?;
        Ok(entry)
    }

    /// Original identity, access, mount binding, and caller cleanup authority.
    #[must_use]
    pub const fn attachment(&self) -> &StorageAttachment {
        &self.attachment
    }

    /// Canonical Host path reopened and independently pinned by the Shim.
    #[must_use]
    pub fn host_source(&self) -> &str {
        &self.host_source
    }

    /// Device and inode that the Shim must observe on its retained descriptor.
    #[must_use]
    pub const fn source_identity(&self) -> AgentVmDirectorySourceIdentity {
        self.source_identity
    }

    /// Digest-derived virtio-fs tag, independent of device enumeration order.
    #[must_use]
    pub fn tag(&self) -> &str {
        &self.tag
    }

    /// Protected Guest mount point used before the workload namespace is created.
    #[must_use]
    pub fn guest_mount_path(&self) -> String {
        format!("{AGENT_VM_DIRECTORY_GUEST_ROOT}/{}", self.tag)
    }

    /// Relative source consumed by the unchanged descriptor-confined bundle resolver.
    #[must_use]
    pub fn guest_mount_relative_path(&self) -> String {
        format!("{AGENT_VM_DIRECTORY_RELATIVE_ROOT}/{}", self.tag)
    }

    /// Exact zero-based OCI mount index selected by this storage grant.
    pub fn mount_index(&self) -> Result<usize> {
        parse_mount_index(self.attachment.mount().json_pointer())
    }

    pub(super) fn validate_shape(&self, attachment_digest: &str) -> Result<()> {
        self.mount_index()?;
        validate_digest(
            self.attachment.mount().value_digest(),
            "directory mount value digest",
        )?;
        AgentVmDirectorySourceIdentity::new(
            self.source_identity.device,
            self.source_identity.inode,
        )?;
        if self.attachment.ownership() != StorageOwnership::Caller
            || self.attachment.cleanup() != StorageCleanup::DetachOnly
        {
            return Err(manifest_error(
                "directory transport requires caller ownership and detach-only cleanup",
            ));
        }
        let source = &self.host_source;
        if source.len() < 2
            || source.len() > 4096
            || !source.starts_with('/')
            || source.ends_with('/')
            || source.contains('\0')
            || source
                .split('/')
                .skip(1)
                .any(|part| part.is_empty() || matches!(part, "." | ".."))
        {
            return Err(manifest_error(
                "directory transport requires a normalized absolute Host path",
            ));
        }
        if self.tag != derive_tag(attachment_digest, self.attachment.identity().as_str())? {
            return Err(manifest_error(
                "directory virtio-fs tag is not bound to its immutable storage grant",
            ));
        }
        Ok(())
    }

    pub(super) fn validate_configuration(&self, configuration: &Value) -> Result<()> {
        validate_configuration_attachment(
            configuration,
            self.attachment.mount(),
            "directory mount",
        )?;
        let mount = configuration
            .pointer(self.attachment.mount().json_pointer())
            .and_then(Value::as_object)
            .ok_or_else(|| manifest_error("directory mount attachment is not an object"))?;
        if mount.get("source").and_then(Value::as_str) != Some(self.host_source.as_str()) {
            return Err(manifest_error(
                "directory grant does not bind its configured Host source",
            ));
        }
        let options = match mount.get("options") {
            Some(Value::Array(options)) => options.as_slice(),
            None => &[],
            Some(_) => return Err(manifest_error("directory mount options are not an array")),
        };
        let mut bind = false;
        let mut read_only = false;
        let mut read_write = false;
        for option in options {
            match option
                .as_str()
                .ok_or_else(|| manifest_error("directory mount option is not a string"))?
            {
                "bind" | "rbind" => bind = true,
                "ro" => read_only = true,
                "rw" => read_write = true,
                "loop" => return Err(manifest_error("directory transport cannot use loop mounts")),
                _ => {}
            }
        }
        let access = if read_only {
            StorageAccessMode::ReadOnly
        } else {
            StorageAccessMode::ReadWrite
        };
        if !bind || (read_only && read_write) || access != self.attachment.access_mode() {
            return Err(manifest_error(
                "directory transport requires a bind mount with its exact granted access",
            ));
        }
        Ok(())
    }
}

fn derive_tag(attachment_digest: &str, identity: &str) -> Result<String> {
    validate_digest(attachment_digest, "attachment digest")?;
    let mut hasher = Sha256::new();
    hasher.update(TAG_DOMAIN);
    for value in [attachment_digest, identity] {
        hasher.update((value.len() as u64).to_le_bytes());
        hasher.update(value.as_bytes());
    }
    Ok(format!(
        "aster-dir-{}",
        &format!("{:x}", hasher.finalize())[..24]
    ))
}
