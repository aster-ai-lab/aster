//! Source-declared installation membership and bounded content verification.

use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Component, Path};

use aster_use_core::metadata_is_link_or_reparse_point;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::store::{companion_destination, ensure_real_file, hex_encode};
use crate::{CompanionFile, ToolAcquireError};

pub(crate) const MANIFEST_SCHEMA: &str = "aster.use.tool-acquire.files.v1";
pub(crate) const MAX_TREE_ENTRIES: usize = 8192;
pub(crate) const MAX_TREE_BYTES: u64 = 4 * 1024 * 1024 * 1024;
pub(crate) const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct InstallManifest {
    pub(crate) schema: String,
    pub(crate) files: BTreeMap<String, String>,
}

impl InstallManifest {
    pub(crate) fn from_payload(
        executable: &str,
        checksum: &str,
        companions: &[CompanionFile],
    ) -> Result<Self, ToolAcquireError> {
        if companions.len() >= MAX_TREE_ENTRIES {
            return Err(invalid("installation exceeds 8192 files"));
        }
        let mut files = BTreeMap::from([(executable.to_string(), checksum.to_string())]);
        for companion in companions {
            let path = companion_destination(Path::new(""), executable, &companion.relative_path)?;
            let parts: Result<Vec<_>, _> = path
                .components()
                .map(|part| {
                    part.as_os_str()
                        .to_str()
                        .ok_or_else(|| invalid("file name is not UTF-8"))
                })
                .collect();
            let portable = parts?.join("/");
            validate_relative_path(&portable)?;
            if files
                .insert(portable, hex_encode(&Sha256::digest(&companion.bytes)))
                .is_some()
            {
                return Err(invalid("duplicate installation file"));
            }
        }
        Ok(Self {
            schema: MANIFEST_SCHEMA.into(),
            files,
        })
    }
}

pub(crate) fn fingerprint(
    root: &Path,
    remaining: &mut u64,
) -> Result<BTreeMap<String, String>, ToolAcquireError> {
    let mut files = BTreeMap::new();
    let mut directories = vec![root.to_path_buf()];
    let mut entries = 0;
    while let Some(directory) = directories.pop() {
        let children = fs::read_dir(&directory)
            .map_err(|error| ToolAcquireError::io("read installation", &directory, error))?;
        for entry in children {
            let entry = entry.map_err(|error| {
                ToolAcquireError::io("read installation entry", &directory, error)
            })?;
            entries += 1;
            if entries > MAX_TREE_ENTRIES {
                return Err(invalid("installation exceeds 8192 entries"));
            }
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)
                .map_err(|error| ToolAcquireError::io("inspect installation file", &path, error))?;
            if metadata_is_link_or_reparse_point(&metadata) {
                return Err(ToolAcquireError::LinkRefused { path });
            }
            if metadata.is_dir() {
                directories.push(path);
                continue;
            }
            ensure_real_file(&path)?;
            let relative = path
                .strip_prefix(root)
                .map_err(|_| invalid("file outside installation"))?;
            let parts: Result<Vec<_>, _> = relative
                .components()
                .map(|part| {
                    part.as_os_str()
                        .to_str()
                        .ok_or_else(|| invalid("file name is not UTF-8"))
                })
                .collect();
            let portable = parts?.join("/");
            validate_relative_path(&portable)?;
            let mut file = fs::File::open(&path)
                .map_err(|error| ToolAcquireError::io("open installation file", &path, error))?;
            let mut hasher = Sha256::new();
            let mut buffer = [0_u8; 64 * 1024];
            loop {
                let read = file.read(&mut buffer).map_err(|error| {
                    ToolAcquireError::io("hash installation file", &path, error)
                })?;
                if read == 0 {
                    break;
                }
                *remaining = remaining
                    .checked_sub(read as u64)
                    .ok_or_else(|| invalid("installations exceed 4 GiB"))?;
                hasher.update(&buffer[..read]);
            }
            files.insert(portable, hex_encode(&hasher.finalize()));
        }
    }
    Ok(files)
}

pub(crate) fn validate_relative_path(path: &str) -> Result<(), ToolAcquireError> {
    if path.is_empty()
        || path.contains(['\\', ':', '\0'])
        || path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
        || !Path::new(path)
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
    {
        return Err(invalid("invalid relative installation path"));
    }
    Ok(())
}

fn invalid(reason: &str) -> ToolAcquireError {
    ToolAcquireError::InvalidInstallation(reason.into())
}
