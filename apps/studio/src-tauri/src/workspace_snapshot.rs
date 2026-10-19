//! Disk observations for conditional saves, separate from plugin grants and unsaved models.

use std::sync::Mutex;

pub(crate) struct Snapshot {
    pub bytes: Vec<u8>,
    pub revision: String,
}

#[derive(Default)]
pub(crate) struct WorkspaceSnapshots {
    serial: Mutex<()>,
}

impl WorkspaceSnapshots {
    pub fn replace_legacy(&self, root: &str, path: &str, bytes: &[u8]) -> Result<(), u16> {
        if bytes.len() as u64 > crate::workspace_file::RAW_LIMIT {
            return Err(413);
        }
        let _guard = self.serial.lock().map_err(|_| 503u16)?;
        #[cfg(unix)]
        {
            let current = platform::read(root, path)?;
            platform::write(root, path, &current.revision, bytes).map(|_| ())
        }
        #[cfg(not(unix))]
        crate::workspace_file::write_bytes(root, path, bytes)
    }

    pub fn import(
        &self,
        root: &str,
        directory: &str,
        name: &str,
        bytes: &[u8],
    ) -> Result<String, u16> {
        let _guard = self.serial.lock().map_err(|_| 503u16)?;
        crate::workspace_file::import_bytes(root, directory, name, bytes)
    }

    pub fn import_new(
        &self,
        root: &str,
        directory: &str,
        name: &str,
        bytes: &[u8],
    ) -> Result<String, u16> {
        let _guard = self.serial.lock().map_err(|_| 503u16)?;
        crate::workspace_file::import_new_bytes(root, directory, name, bytes)
    }

    pub fn read(&self, root: &str, path: &str) -> Result<Snapshot, u16> {
        let _guard = self.serial.lock().map_err(|_| 503u16)?;
        platform::read(root, path)
    }

    pub fn write(
        &self,
        root: &str,
        path: &str,
        expected: &str,
        bytes: &[u8],
    ) -> Result<Snapshot, u16> {
        if expected.len() != 64
            || !expected
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            return Err(400);
        }
        if bytes.len() as u64 > crate::workspace_file::RAW_LIMIT {
            return Err(413);
        }
        let _guard = self.serial.lock().map_err(|_| 503u16)?;
        platform::write(root, path, expected, bytes)
    }
}

#[cfg(unix)]
mod platform {
    use std::fs::{self, File, Metadata};
    use std::io::{Read, Write};
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use std::path::{Component, PathBuf};

    use rustix::fs::{AtFlags, Mode, OFlags, open, openat, renameat, unlinkat};
    use sha2::{Digest, Sha256};

    use super::Snapshot;

    struct BoundFile {
        root_path: PathBuf,
        target_path: PathBuf,
        root: File,
        parent: File,
        name: std::ffi::OsString,
    }

    fn identity(metadata: &Metadata) -> (u64, u64) {
        (metadata.dev(), metadata.ino())
    }

    fn stamp(metadata: &Metadata) -> [u64; 10] {
        [
            metadata.dev(),
            metadata.ino(),
            metadata.len(),
            metadata.mode() as u64,
            metadata.uid() as u64,
            metadata.gid() as u64,
            metadata.mtime() as u64,
            metadata.mtime_nsec() as u64,
            metadata.ctime() as u64,
            metadata.ctime_nsec() as u64,
        ]
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
            let relative = target_path.strip_prefix(&root_path).map_err(|_| 404u16)?;
            let name = relative.file_name().ok_or(404u16)?.to_os_string();
            let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
            let root = File::from(open(&root_path, flags, Mode::empty()).map_err(|_| 404u16)?);
            let mut parent = root.try_clone().map_err(|_| 500u16)?;
            for part in relative.parent().ok_or(404u16)?.components() {
                let Component::Normal(name) = part else {
                    return Err(404);
                };
                parent =
                    File::from(openat(&parent, name, flags, Mode::empty()).map_err(|_| 404u16)?);
            }
            Ok(Self {
                root_path,
                target_path,
                root,
                parent,
                name,
            })
        }

        fn snapshot(&self) -> Result<(Snapshot, Metadata), u16> {
            let file = File::from(
                openat(
                    &self.parent,
                    &self.name,
                    OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
                    Mode::empty(),
                )
                .map_err(|_| 404u16)?,
            );
            let before = file.metadata().map_err(|_| 500u16)?;
            if !before.is_file() {
                return Err(404);
            }
            if before.len() > crate::workspace_file::RAW_LIMIT {
                return Err(413);
            }
            let mut bytes = Vec::new();
            (&file)
                .take(crate::workspace_file::RAW_LIMIT + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| 500u16)?;
            if bytes.len() as u64 > crate::workspace_file::RAW_LIMIT {
                return Err(413);
            }
            let after = file.metadata().map_err(|_| 500u16)?;
            if stamp(&before) != stamp(&after) {
                return Err(409);
            }
            let mut digest = Sha256::new();
            for path in [&self.root_path, &self.target_path] {
                let path = path.as_os_str().as_encoded_bytes();
                digest.update((path.len() as u64).to_le_bytes());
                digest.update(path);
            }
            for metadata in [
                self.root.metadata().map_err(|_| 500u16)?,
                self.parent.metadata().map_err(|_| 500u16)?,
            ] {
                let (dev, ino) = identity(&metadata);
                digest.update(dev.to_le_bytes());
                digest.update(ino.to_le_bytes());
            }
            for value in stamp(&after) {
                digest.update(value.to_le_bytes());
            }
            digest.update(&bytes);
            Ok((
                Snapshot {
                    bytes,
                    revision: format!("{:x}", digest.finalize()),
                },
                after,
            ))
        }

        fn revalidate(&self, root: &str, path: &str) -> Result<(), u16> {
            let current = Self::open(root, path)?;
            if current.root_path != self.root_path
                || current.target_path != self.target_path
                || identity(&current.root.metadata().map_err(|_| 500u16)?)
                    != identity(&self.root.metadata().map_err(|_| 500u16)?)
                || identity(&current.parent.metadata().map_err(|_| 500u16)?)
                    != identity(&self.parent.metadata().map_err(|_| 500u16)?)
            {
                return Err(409);
            }
            Ok(())
        }
    }

    pub fn read(root: &str, path: &str) -> Result<Snapshot, u16> {
        let bound = BoundFile::open(root, path)?;
        let (snapshot, _) = bound.snapshot()?;
        bound.revalidate(root, path)?;
        Ok(snapshot)
    }

    pub fn write(root: &str, path: &str, expected: &str, bytes: &[u8]) -> Result<Snapshot, u16> {
        let bound = BoundFile::open(root, path)?;
        let (current, metadata) = bound.snapshot()?;
        if current.revision != expected {
            return Err(409);
        }
        let temporary = format!(".a3s-save-{}", uuid::Uuid::now_v7());
        let mut file = File::from(
            openat(
                &bound.parent,
                temporary.as_str(),
                OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::RUSR | Mode::WUSR,
            )
            .map_err(|_| 500u16)?,
        );
        let result = (|| {
            file.write_all(bytes).map_err(|_| 500u16)?;
            file.set_permissions(fs::Permissions::from_mode(metadata.mode() & 0o777))
                .map_err(|_| 500u16)?;
            file.sync_all().map_err(|_| 500u16)?;
            bound.revalidate(root, path)?;
            if bound.snapshot()?.0.revision != expected {
                return Err(409);
            }
            renameat(
                &bound.parent,
                temporary.as_str(),
                &bound.parent,
                &bound.name,
            )
            .map_err(|_| 500u16)?;
            // Observe the exact descriptor written, never a later external replacement.
            let after = file.metadata().map_err(|_| 500u16)?;
            let (snapshot, observed) = bound.snapshot()?;
            if identity(&after) != identity(&observed) || snapshot.bytes != bytes {
                return Err(409);
            }
            Ok(snapshot)
        })();
        let _ = unlinkat(&bound.parent, temporary.as_str(), AtFlags::empty());
        result
    }
}

#[cfg(not(unix))]
use portable as platform;

#[cfg(any(not(unix), test))]
#[path = "workspace_snapshot_portable.rs"]
mod portable;

#[cfg(test)]
#[path = "workspace_snapshot_tests.rs"]
mod tests;
