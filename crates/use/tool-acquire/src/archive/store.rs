use super::*;
use crate::lock::{join_under, InstallLock};
use crate::store::{
    ensure_parent_real, ensure_real_file, json_bytes, publish_current, read_json_bounded,
    remove_owned_tree, write_atomic,
};
use crate::{AcquireRequest, ToolReceipt, ToolStore};

const SCHEMA: &str = "aster.use.tool-acquire.archive.v1";

/// Native executable and PATH directories for an identified installation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallationPaths {
    executable: PathBuf,
    bin_dirs: Vec<PathBuf>,
}

impl InstallationPaths {
    pub fn executable(&self) -> &Path {
        &self.executable
    }

    pub fn bin_dirs(&self) -> &[PathBuf] {
        &self.bin_dirs
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ArchiveRecord {
    schema: String,
    name: String,
    spec: String,
    version: String,
    archive_sha256: String,
    pub(crate) entrypoints: BTreeMap<String, PathBuf>,
    pub(crate) bin_dirs: Vec<PathBuf>,
    entries: BTreeMap<String, ArchiveEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ArchiveLock {
    archive_sha256: String,
    record_sha256: String,
}

impl ArchiveLock {
    pub(crate) fn new(archive_sha256: &str, record_sha256: &str) -> Result<Self, ToolAcquireError> {
        Ok(Self {
            archive_sha256: crate::store::normalize_sha256(archive_sha256)?,
            record_sha256: crate::store::normalize_sha256(record_sha256)?,
        })
    }

    pub(crate) fn archive_sha256(&self) -> &str {
        &self.archive_sha256
    }

    pub(crate) fn record_sha256(&self) -> &str {
        &self.record_sha256
    }

    pub(crate) fn matches(&self, record: &ArchiveRecord) -> Result<bool, ToolAcquireError> {
        Ok(self == &record.lock()?)
    }
}

impl ToolStore {
    /// Resolve native paths from an admitted receipt and its source-declared layout.
    /// Missing, malformed, escaping or non-executable entries return an error.
    pub fn installation_paths(
        &self,
        receipt: &ToolReceipt,
    ) -> Result<InstallationPaths, ToolAcquireError> {
        receipt.validate(&receipt.name, None)?;
        installation_paths(self.root_path(), receipt)
    }

    pub(crate) fn acquire_archive(
        &self,
        request: AcquireRequest,
        mut payload: ArchivePayload,
    ) -> Result<ToolReceipt, ToolAcquireError> {
        let root = self.root_path();
        let _lock = InstallLock::acquire(root)?;
        if !payload.entrypoints.contains_key(&request.executable_name) {
            return Err(invalid("archive does not declare the requested executable"));
        }
        ensure_parent_real(root, &["installs", &request.name])?;
        let staging = join_under(
            root,
            &[
                "installs",
                &request.name,
                &format!(".staging-{}", payload.version),
            ],
        )?;
        remove_owned_tree(&staging)?;
        fs::create_dir(&staging)
            .map_err(|error| ToolAcquireError::io("create archive staging", &staging, error))?;
        let result = (|| {
            let mut entries = payload.extract(&staging)?;
            for (name, path) in &payload.optional_entrypoints {
                if entries.contains_key(&portable_path(path)?) {
                    payload.entrypoints.insert(name.clone(), path.clone());
                }
            }
            if payload.executable_entrypoints {
                for path in payload.entrypoints.values() {
                    let key = portable_path(path)?;
                    let Some(ArchiveEntry::File { mode, .. }) = entries.get_mut(&key) else {
                        return Err(invalid("executable mode requires a regular archive member"));
                    };
                    set_mode(&staging.join(path), 0o755)?;
                    #[cfg(unix)]
                    {
                        *mode = Some(0o755);
                    }
                    #[cfg(not(unix))]
                    let _ = mode;
                }
            }
            let primary = payload
                .entrypoints
                .get(&request.executable_name)
                .ok_or_else(|| invalid("archive has no primary entrypoint"))?;
            let resolved = resolve_inside(&staging, primary)?;
            ensure_executable(&resolved)?;
            let primary_key = portable_path(
                resolved
                    .strip_prefix(&staging.canonicalize().map_err(|error| {
                        ToolAcquireError::io("resolve archive staging", &staging, error)
                    })?)
                    .map_err(|_| invalid("entrypoint escaped staging"))?,
            )?;
            let Some(ArchiveEntry::File { sha256, .. }) = entries.get(&primary_key) else {
                return Err(invalid("archive entrypoint is not an admitted file"));
            };
            let receipt = ToolReceipt::new(
                &request.name,
                &request.spec,
                &payload.version,
                &request.executable_name,
                sha256,
            )?;
            let record = ArchiveRecord {
                schema: SCHEMA.into(),
                name: request.name.clone(),
                spec: request.spec.clone(),
                version: payload.version.clone(),
                archive_sha256: payload.sha256,
                entrypoints: payload.entrypoints,
                bin_dirs: payload.bin_dirs,
                entries,
            };
            record.paths(&staging, &receipt)?;
            let mut remaining = MAX_TREE_BYTES;
            if inspect(&staging, &mut remaining)? != record.entries {
                return Err(invalid(
                    "extracted archive differs from its declared members",
                ));
            }
            let record_bytes = json_bytes(&record)?;
            if record_bytes.len() as u64 > MAX_ARCHIVE_RECORD_BYTES {
                return Err(invalid("archive record exceeds 4 MiB"));
            }
            if payload.lock.as_ref().is_some_and(|lock| {
                lock.archive_sha256 != record.archive_sha256
                    || lock.record_sha256 != hex_encode(&Sha256::digest(&record_bytes))
            }) {
                return Err(ToolAcquireError::LockContentMismatch { name: request.name });
            }
            let version = root
                .join("installs")
                .join(&receipt.name)
                .join(&receipt.version);
            if version.try_exists().map_err(|error| {
                ToolAcquireError::io("inspect archive installation", &version, error)
            })? {
                let existing = self.installed_receipt(&receipt.name, Some(&receipt.version))?;
                let old = ArchiveRecord::read(root, &receipt)?;
                let mut remaining = MAX_TREE_BYTES;
                if old.as_ref() != Some(&record)
                    || inspect(&version, &mut remaining)? != record.entries
                {
                    return Err(ToolAcquireError::VersionConflict {
                        version: receipt.version,
                    });
                }
                let existing = existing
                    .ok_or_else(|| invalid("archive installation has no identity receipt"))?;
                if existing.spec != receipt.spec
                    || existing.executable != receipt.executable
                    || existing.sha256 != receipt.sha256
                {
                    return Err(ToolAcquireError::InstallationIdentityMismatch {
                        name: receipt.name,
                        version: receipt.version,
                    });
                }
                publish_current(root, &existing)?;
                return Ok(existing);
            }
            fs::rename(&staging, &version).map_err(|error| {
                ToolAcquireError::io("publish archive installation", &version, error)
            })?;
            ensure_parent_real(root, &["receipts", &receipt.name])?;
            let path = root
                .join("receipts")
                .join(&receipt.name)
                .join(format!("{}.json", receipt.version));
            write_atomic(&path, &json_bytes(&receipt)?)?;
            write_atomic(&record_path(root, &receipt), &record_bytes)?;
            publish_current(root, &receipt)?;
            Ok(receipt)
        })();
        remove_owned_tree(&staging)?;
        result
    }
}

pub(crate) fn installation_paths(
    root: &Path,
    receipt: &ToolReceipt,
) -> Result<InstallationPaths, ToolAcquireError> {
    let directory = root
        .join("installs")
        .join(&receipt.name)
        .join(&receipt.version);
    if let Some(record) = ArchiveRecord::read(root, receipt)? {
        record.paths(&directory, receipt)
    } else {
        let executable = directory.join(&receipt.executable);
        ensure_real_file(&executable)?;
        Ok(InstallationPaths {
            executable,
            bin_dirs: vec![directory],
        })
    }
}

impl ArchiveRecord {
    pub(crate) fn lock(&self) -> Result<ArchiveLock, ToolAcquireError> {
        Ok(ArchiveLock {
            archive_sha256: self.archive_sha256.clone(),
            record_sha256: hex_encode(&Sha256::digest(json_bytes(self)?)),
        })
    }

    pub(crate) fn verify_tree(
        &self,
        directory: &Path,
        remaining: &mut u64,
    ) -> Result<(), ToolAcquireError> {
        if inspect(directory, remaining)? != self.entries {
            return Err(ToolAcquireError::LockContentMismatch {
                name: self.name.clone(),
            });
        }
        Ok(())
    }

    pub(crate) fn read(
        root: &Path,
        receipt: &ToolReceipt,
    ) -> Result<Option<Self>, ToolAcquireError> {
        let path = record_path(root, receipt);
        if !path
            .try_exists()
            .map_err(|error| ToolAcquireError::io("inspect archive record", &path, error))?
        {
            return Ok(None);
        }
        let record: Self = read_json_bounded(&path, MAX_ARCHIVE_RECORD_BYTES)?;
        if record.schema != SCHEMA
            || record.name != receipt.name
            || record.spec != receipt.spec
            || record.version != receipt.version
        {
            return Err(invalid("archive record identity mismatch"));
        }
        crate::store::normalize_sha256(&record.archive_sha256)?;
        validate_layout(&record.entrypoints, &record.bin_dirs, cfg!(windows))?;
        if record.entries.len() > MAX_ARCHIVE_ENTRIES {
            return Err(invalid("archive record exceeds 32768 entries"));
        }
        for (path, entry) in &record.entries {
            validate_relative_path(path)?;
            match entry {
                ArchiveEntry::File { sha256, mode } => {
                    crate::store::normalize_sha256(sha256)?;
                    if mode.is_some_and(|mode| mode > 0o777) {
                        return Err(invalid("invalid archive file mode"));
                    }
                }
                ArchiveEntry::Symlink { target } => {
                    portable_path(&normalize_link(Path::new(path), target)?)?;
                }
                ArchiveEntry::Directory => {}
            }
        }
        Ok(Some(record))
    }

    fn paths(
        &self,
        directory: &Path,
        receipt: &ToolReceipt,
    ) -> Result<InstallationPaths, ToolAcquireError> {
        let mut executable = None;
        for (name, relative) in &self.entrypoints {
            let target = resolve_inside(directory, relative)?;
            ensure_executable(&target)?;
            let root = directory
                .canonicalize()
                .map_err(|error| ToolAcquireError::io("resolve installation", directory, error))?;
            let key = portable_path(
                target
                    .strip_prefix(root)
                    .map_err(|_| invalid("invalid entrypoint target"))?,
            )?;
            let Some(ArchiveEntry::File { sha256, .. }) = self.entries.get(&key) else {
                return Err(invalid("entrypoint is absent from archive membership"));
            };
            if name == &receipt.executable {
                if sha256 != &receipt.sha256 {
                    return Err(invalid("archive primary checksum differs from receipt"));
                }
                executable = Some(directory.join(relative));
            }
        }
        let executable = executable.ok_or_else(|| invalid("archive has no primary entrypoint"))?;
        let mut bin_dirs = Vec::new();
        for relative in &self.bin_dirs {
            let path = resolve_inside(directory, relative)?;
            if !path.is_dir() {
                return Err(invalid("archive PATH entry is not a directory"));
            }
            bin_dirs.push(directory.join(relative));
        }
        Ok(InstallationPaths {
            executable,
            bin_dirs,
        })
    }
}

pub(crate) fn inspect(
    root: &Path,
    remaining: &mut u64,
) -> Result<BTreeMap<String, ArchiveEntry>, ToolAcquireError> {
    let mut entries = BTreeMap::new();
    let mut directories = vec![root.to_path_buf()];
    while let Some(directory) = directories.pop() {
        for entry in fs::read_dir(&directory)
            .map_err(|error| ToolAcquireError::io("read runtime tree", &directory, error))?
        {
            let entry = entry
                .map_err(|error| ToolAcquireError::io("read runtime member", &directory, error))?;
            check_count(entries.len())?;
            let path = entry.path();
            let relative = path
                .strip_prefix(root)
                .map_err(|_| invalid("runtime member escaped root"))?;
            let key = portable_path(relative)?;
            let metadata = fs::symlink_metadata(&path)
                .map_err(|error| ToolAcquireError::io("inspect runtime member", &path, error))?;
            let value = if metadata.file_type().is_symlink() {
                let target = fs::read_link(&path)
                    .map_err(|error| ToolAcquireError::io("read runtime link", &path, error))?;
                portable_path(&normalize_link(relative, &target)?)?;
                resolve_inside(root, relative)?;
                ArchiveEntry::Symlink { target }
            } else if aster_use_core::metadata_is_link_or_reparse_point(&metadata) {
                return Err(ToolAcquireError::LinkRefused { path });
            } else if metadata.is_dir() {
                directories.push(path);
                ArchiveEntry::Directory
            } else if metadata.is_file() {
                let mut file = fs::File::open(&path)
                    .map_err(|error| ToolAcquireError::io("open runtime member", &path, error))?;
                let mut hasher = Sha256::new();
                let mut buffer = [0_u8; 64 * 1024];
                loop {
                    let count = file.read(&mut buffer).map_err(|error| {
                        ToolAcquireError::io("hash runtime member", &path, error)
                    })?;
                    if count == 0 {
                        break;
                    }
                    *remaining = remaining
                        .checked_sub(count as u64)
                        .ok_or_else(|| invalid("runtime tree exceeds 4 GiB"))?;
                    hasher.update(&buffer[..count]);
                }
                #[cfg(unix)]
                let mode = {
                    use std::os::unix::fs::MetadataExt;
                    Some(metadata.mode() & 0o777)
                };
                #[cfg(not(unix))]
                let mode = None;
                ArchiveEntry::File {
                    sha256: hex_encode(&hasher.finalize()),
                    mode,
                }
            } else {
                return Err(invalid("runtime contains an unsupported file type"));
            };
            entries.insert(key, value);
        }
    }
    Ok(entries)
}

fn ensure_executable(path: &Path) -> Result<(), ToolAcquireError> {
    ensure_real_file(path)?;
    let metadata = fs::metadata(path)
        .map_err(|error| ToolAcquireError::io("inspect runtime executable", path, error))?;
    if metadata.len() == 0 {
        return Err(ToolAcquireError::NotExecutable);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 == 0 {
            return Err(ToolAcquireError::NotExecutable);
        }
    }
    Ok(())
}

fn record_path(root: &Path, receipt: &ToolReceipt) -> PathBuf {
    root.join("receipts")
        .join(&receipt.name)
        .join(format!("{}.archive.json", receipt.version))
}
