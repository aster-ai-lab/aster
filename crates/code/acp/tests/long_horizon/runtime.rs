#![cfg(all(test, unix))]

use sha2::{Digest, Sha256};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Identity {
    sha256: String,
    entries: usize,
    bytes: u64,
}

/// Freeze actual executable, library, image and compiler bytes, including link targets.
pub fn identity(directory: &Path) -> std::io::Result<Identity> {
    let directory = directory.canonicalize()?;
    let mut pending = vec![directory.clone()];
    let mut entries = std::collections::BTreeMap::new();
    let mut bytes = 0_u64;
    while let Some(path) = pending.pop() {
        let metadata = path.symlink_metadata()?;
        let relative = path
            .strip_prefix(&directory)
            .map_err(std::io::Error::other)?;
        let relative = relative
            .to_str()
            .ok_or_else(|| std::io::Error::other("runtime path encoding"))?;
        let value = if metadata.file_type().is_symlink() {
            format!(
                "link:{}",
                path.read_link()?
                    .to_str()
                    .ok_or_else(|| std::io::Error::other("runtime link encoding"))?
            )
        } else if metadata.is_dir() {
            if relative.split('/').count() > 64 {
                return Err(std::io::Error::other("runtime depth bound"));
            }
            for entry in path.read_dir()? {
                pending.push(entry?.path());
            }
            "directory".into()
        } else if metadata.is_file() {
            bytes = bytes
                .checked_add(metadata.len())
                .ok_or_else(|| std::io::Error::other("runtime byte overflow"))?;
            if bytes > 4 * 1024 * 1024 * 1024 {
                return Err(std::io::Error::other("runtime byte bound"));
            }
            super::fixture::digest(&path)?
        } else {
            return Err(std::io::Error::other("runtime special file refused"));
        };
        entries.insert(
            relative.to_owned(),
            format!("{}:{value}", metadata.permissions().mode()),
        );
        if entries.len() + pending.len() > 100_000 {
            return Err(std::io::Error::other("runtime entry bound"));
        }
    }
    let mut digest = Sha256::new();
    digest.update(b"acp-qualified-runtime-v1\0");
    for (path, value) in &entries {
        for field in [path.as_bytes(), value.as_bytes()] {
            digest.update((field.len() as u64).to_le_bytes());
            digest.update(field);
        }
    }
    Ok(Identity {
        sha256: format!("{:x}", digest.finalize()),
        entries: entries.len(),
        bytes,
    })
}

#[test]
fn runtime_identity_binds_modes_links_and_current_bytes() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("binary"), b"original").unwrap();
    std::os::unix::fs::symlink("binary", root.path().join("link")).unwrap();
    let before = identity(root.path()).unwrap();
    assert_eq!(before, identity(root.path()).unwrap());
    std::fs::write(root.path().join("binary"), b"changed").unwrap();
    assert_ne!(before, identity(root.path()).unwrap());
    let changed = identity(root.path()).unwrap();
    std::fs::set_permissions(
        root.path().join("binary"),
        std::fs::Permissions::from_mode(0o700),
    )
    .unwrap();
    assert_ne!(changed, identity(root.path()).unwrap());
    let changed = identity(root.path()).unwrap();
    std::os::unix::fs::symlink("other", root.path().join("new-link")).unwrap();
    assert_ne!(changed, identity(root.path()).unwrap());
}
