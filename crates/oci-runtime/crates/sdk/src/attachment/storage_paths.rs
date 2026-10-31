use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{invalid_attachment, CreateAttachments, StorageAccessMode};
use crate::{OciBundle, Result, StorageAttachmentId};

/// Required Host-enforced restrictions beneath caller-owned directory grants.
pub const STORAGE_PATH_RESTRICTIONS_EXTENSION: &str = "dev.aster.storage-path-restrictions";
/// First exact-path restriction contract.
pub const STORAGE_PATH_RESTRICTIONS_VERSION: u16 = 1;
const SCHEMA: &str = "aster.oci.storage-path-restrictions.v1";
const MAX_BYTES: usize = 2 * 1024 * 1024;
const MAX_PATHS: usize = 16_384;

/// Source-relative denied subtrees and narrower writable exceptions for one grant.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoragePathRestriction {
    identity: StorageAttachmentId,
    deny_read: Vec<String>,
    deny_write: Vec<String>,
    write_exceptions: Vec<String>,
}

impl StoragePathRestriction {
    /// Construct canonical exact paths; `.` denotes the complete source subtree.
    pub fn new(
        identity: StorageAttachmentId,
        mut deny_read: Vec<String>,
        mut deny_write: Vec<String>,
        mut write_exceptions: Vec<String>,
    ) -> Result<Self> {
        for paths in [&mut deny_read, &mut deny_write, &mut write_exceptions] {
            paths.sort();
            paths.dedup();
        }
        let restriction = Self {
            identity,
            deny_read,
            deny_write,
            write_exceptions,
        };
        restriction.validate()?;
        Ok(restriction)
    }

    /// Immutable caller-issued storage identity.
    pub const fn identity(&self) -> &StorageAttachmentId {
        &self.identity
    }
    /// Source-relative subtrees whose data cannot be read.
    pub fn deny_read(&self) -> &[String] {
        &self.deny_read
    }
    /// Source-relative subtrees that cannot be mutated.
    pub fn deny_write(&self) -> &[String] {
        &self.deny_write
    }
    /// Writable subtrees strictly beneath a write-denied ancestor.
    pub fn write_exceptions(&self) -> &[String] {
        &self.write_exceptions
    }

    fn validate(&self) -> Result<()> {
        for paths in [&self.deny_read, &self.deny_write, &self.write_exceptions] {
            if paths.len() > MAX_PATHS || paths.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(invalid_attachment(
                    "storage restriction paths exceed their bound or canonical order",
                ));
            }
            for path in paths {
                if path.len() > 4096
                    || (path != "." && crate::admit_workspace_path(path, false).is_err())
                {
                    return Err(invalid_attachment(
                        "storage restrictions require normalized source-relative paths",
                    ));
                }
            }
        }
        for exception in &self.write_exceptions {
            if !self.deny_write.iter().any(|denied| {
                denied != exception && (denied == "." || Path::new(exception).starts_with(denied))
            }) {
                return Err(invalid_attachment(
                    "writable exception is not beneath a write-denied subtree",
                ));
            }
        }
        Ok(())
    }
}

/// Bounded, immutable restrictions configured through the required OCI annotation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoragePathRestrictions {
    schema_version: String,
    entries: Vec<StoragePathRestriction>,
}

impl StoragePathRestrictions {
    /// Construct restrictions in canonical storage-identity order.
    pub fn new(mut entries: Vec<StoragePathRestriction>) -> Result<Self> {
        entries.sort();
        let restrictions = Self {
            schema_version: SCHEMA.into(),
            entries,
        };
        restrictions.validate()?;
        Ok(restrictions)
    }

    /// Exact restrictions selected by immutable storage identity.
    pub fn entries(&self) -> &[StoragePathRestriction] {
        &self.entries
    }

    /// Encode the exact OCI annotation value.
    pub fn to_annotation_value(&self) -> Result<String> {
        self.validate()?;
        let encoded =
            serde_json::to_string(self).map_err(|error| invalid_attachment(error.to_string()))?;
        if encoded.len() > MAX_BYTES {
            return Err(invalid_attachment(
                "storage restrictions exceed their encoded bound",
            ));
        }
        Ok(encoded)
    }

    /// Decode a bounded annotation without granting any path access.
    pub fn from_annotation_value(encoded: &str) -> Result<Self> {
        if encoded.is_empty() || encoded.len() > MAX_BYTES {
            return Err(invalid_attachment(
                "storage restrictions exceed their encoded bound",
            ));
        }
        let restrictions: Self =
            serde_json::from_str(encoded).map_err(|error| invalid_attachment(error.to_string()))?;
        restrictions.validate()?;
        Ok(restrictions)
    }

    /// Decode the optional restriction annotation in the immutable configuration.
    pub fn from_bundle(bundle: &OciBundle) -> Result<Option<Self>> {
        bundle
            .spec()
            .annotations()
            .as_ref()
            .and_then(|annotations| annotations.get(STORAGE_PATH_RESTRICTIONS_EXTENSION))
            .map(|value| Self::from_annotation_value(value))
            .transpose()
    }

    /// Digest bound into the Host-to-Shim-to-Guest enforcement handshake.
    pub fn digest(&self) -> Result<String> {
        Ok(format!(
            "sha256:{:x}",
            Sha256::digest(self.to_annotation_value()?.as_bytes())
        ))
    }

    pub(crate) fn validate_storage(&self, storage: &[super::StorageAttachment]) -> Result<()> {
        for entry in &self.entries {
            let grant = storage
                .iter()
                .find(|grant| grant.identity() == entry.identity())
                .ok_or_else(|| {
                    invalid_attachment("storage restrictions reference an ungranted identity")
                })?;
            if grant.access_mode() == StorageAccessMode::ReadOnly
                && !entry.write_exceptions.is_empty()
            {
                return Err(invalid_attachment(
                    "a writable exception cannot widen a read-only grant",
                ));
            }
        }
        Ok(())
    }

    fn validate(&self) -> Result<()> {
        if self.schema_version != SCHEMA
            || self.entries.is_empty()
            || self.entries.len() > 128
            || self
                .entries
                .windows(2)
                .any(|pair| pair[0].identity >= pair[1].identity)
        {
            return Err(invalid_attachment(
                "storage restrictions require a supported schema and unique canonical identities",
            ));
        }
        let count: usize = self
            .entries
            .iter()
            .map(|entry| {
                entry.deny_read.len() + entry.deny_write.len() + entry.write_exceptions.len()
            })
            .sum();
        if count > MAX_PATHS {
            return Err(invalid_attachment(
                "storage restrictions exceed the total path bound",
            ));
        }
        for entry in &self.entries {
            entry.validate()?;
        }
        Ok(())
    }
}

impl CreateAttachments {
    /// Require exact Host enforcement of the configured directory restrictions.
    pub fn with_storage_path_restrictions(self, bundle: &OciBundle) -> Result<Self> {
        self.add_extension_from_annotation(
            bundle,
            STORAGE_PATH_RESTRICTIONS_EXTENSION,
            STORAGE_PATH_RESTRICTIONS_VERSION,
            true,
        )
    }

    /// Resolve a required restriction extension against its exact granted storage.
    pub fn storage_path_restrictions(
        &self,
        bundle: &OciBundle,
    ) -> Result<Option<StoragePathRestrictions>> {
        let Some(extension) = self.extensions().get(STORAGE_PATH_RESTRICTIONS_EXTENSION) else {
            return Ok(None);
        };
        if extension.version() != STORAGE_PATH_RESTRICTIONS_VERSION || !extension.required() {
            return Err(invalid_attachment(
                "storage path restrictions must use required extension version 1",
            ));
        }
        let restrictions = StoragePathRestrictions::from_bundle(bundle)?
            .ok_or_else(|| invalid_attachment("storage path restriction annotation is missing"))?;
        restrictions.validate_storage(self.storage())?;
        Ok(Some(restrictions))
    }
}

#[cfg(test)]
#[path = "storage_paths_tests.rs"]
mod tests;
