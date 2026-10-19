//! Portable disk preconditions. Directory-relative containment is qualified separately on Unix.

use std::fs::{self, OpenOptions};
use std::hash::{Hash, Hasher};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Component, PathBuf};

use same_file::Handle;
use sha2::{Digest, Sha256};

use super::Snapshot;

struct IdentityDigest(Sha256);
impl Hasher for IdentityDigest {
    fn finish(&self) -> u64 {
        let digest = self.0.clone().finalize();
        let mut prefix = [0u8; 8];
        prefix.copy_from_slice(&digest[..8]);
        u64::from_le_bytes(prefix)
    }
    fn write(&mut self, bytes: &[u8]) {
        self.0.update(bytes);
    }
}

struct BoundFile {
    root_path: PathBuf,
    target_path: PathBuf,
    root: Handle,
    parent: Handle,
    file: Handle,
}

impl BoundFile {
    fn open(raw_root: &str, raw_path: &str) -> Result<Self, u16> {
        let root_path = crate::local_documents::canonical_workspace(raw_root)?;
        let path = PathBuf::from(raw_path);
        if raw_path.len() > 4096
            || raw_path.contains('\0')
            || !path.is_absolute()
            || path.components().any(|part| part == Component::ParentDir)
        {
            return Err(400);
        }
        let target_path = fs::canonicalize(path).map_err(|_| 404u16)?;
        if !target_path.starts_with(&root_path) {
            return Err(404);
        }
        let root = Handle::from_path(&root_path).map_err(|_| 404u16)?;
        let parent = Handle::from_path(target_path.parent().ok_or(404u16)?).map_err(|_| 404u16)?;
        let file = Handle::from_path(&target_path).map_err(|_| 404u16)?;
        if !file.as_file().metadata().map_err(|_| 500u16)?.is_file() {
            return Err(404);
        }
        Ok(Self {
            root_path,
            target_path,
            root,
            parent,
            file,
        })
    }

    fn revalidate(&self, root: &str, path: &str) -> Result<Self, u16> {
        let current = Self::open(root, path)?;
        if current.root_path != self.root_path
            || current.target_path != self.target_path
            || current.root != self.root
            || current.parent != self.parent
        {
            return Err(409);
        }
        Ok(current)
    }

    fn snapshot(&self) -> Result<Snapshot, u16> {
        let mut file = self.file.as_file().try_clone().map_err(|_| 500u16)?;
        let before = file.metadata().map_err(|_| 500u16)?;
        if before.len() > crate::workspace_file::RAW_LIMIT {
            return Err(413);
        }
        file.seek(SeekFrom::Start(0)).map_err(|_| 500u16)?;
        let mut bytes = Vec::new();
        Read::by_ref(&mut file)
            .take(crate::workspace_file::RAW_LIMIT + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| 500u16)?;
        if bytes.len() as u64 > crate::workspace_file::RAW_LIMIT {
            return Err(413);
        }
        let after = file.metadata().map_err(|_| 500u16)?;
        let stamp = |meta: &fs::Metadata| {
            (
                meta.len(),
                meta.modified().ok(),
                meta.created().ok(),
                meta.permissions().readonly(),
            )
        };
        if stamp(&before) != stamp(&after) {
            return Err(409);
        }
        let mut digest = IdentityDigest(Sha256::new());
        for path in [&self.root_path, &self.target_path] {
            let path = path.as_os_str().as_encoded_bytes();
            digest.0.update((path.len() as u64).to_le_bytes());
            digest.0.update(path);
        }
        for handle in [&self.root, &self.parent, &self.file] {
            handle.hash(&mut digest);
        }
        digest.0.update(format!("{:?}", stamp(&after)).as_bytes());
        digest.0.update(&bytes);
        Ok(Snapshot {
            bytes,
            revision: format!("{:x}", digest.0.finalize()),
        })
    }
}

pub(super) fn read(root: &str, path: &str) -> Result<Snapshot, u16> {
    let bound = BoundFile::open(root, path)?;
    let snapshot = bound.snapshot()?;
    if bound.revalidate(root, path)?.file != bound.file {
        return Err(409);
    }
    Ok(snapshot)
}

pub(super) fn write(root: &str, path: &str, expected: &str, bytes: &[u8]) -> Result<Snapshot, u16> {
    let bound = BoundFile::open(root, path)?;
    if bound.snapshot()?.revision != expected {
        return Err(409);
    }
    let temporary = bound
        .target_path
        .parent()
        .ok_or(404u16)?
        .join(format!(".a3s-save-{}", uuid::Uuid::now_v7()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|_| 500u16)?;
    let result = (|| {
        file.write_all(bytes).map_err(|_| 500u16)?;
        file.set_permissions(
            bound
                .file
                .as_file()
                .metadata()
                .map_err(|_| 500u16)?
                .permissions(),
        )
        .map_err(|_| 500u16)?;
        file.sync_all().map_err(|_| 500u16)?;
        let current = bound.revalidate(root, path)?;
        if current.file != bound.file || current.snapshot()?.revision != expected {
            return Err(409);
        }
        fs::rename(&temporary, &bound.target_path).map_err(|_| 500u16)?;
        let written =
            Handle::from_file(file.try_clone().map_err(|_| 500u16)?).map_err(|_| 500u16)?;
        let current = bound.revalidate(root, path)?;
        if current.file != written {
            return Err(409);
        }
        let snapshot = current.snapshot()?;
        if snapshot.bytes != bytes {
            return Err(409);
        }
        Ok(snapshot)
    })();
    let _ = fs::remove_file(&temporary);
    result
}
