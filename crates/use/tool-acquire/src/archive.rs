//! Verified runtime archives whose native directory layout must be retained.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::{Cursor, Read, Write};
use std::path::{Component, Path, PathBuf};

use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::installation::{validate_relative_path, MAX_TREE_BYTES};
use crate::spec::{parse_tool_name, parse_tool_version};
use crate::store::{hex_encode, set_mode};
use crate::ToolAcquireError;

mod store;

pub use store::InstallationPaths;
pub(crate) use store::{installation_paths, ArchiveLock, ArchiveRecord};

pub(crate) const MAX_ARCHIVE_BYTES: usize = 512 * 1024 * 1024;
const MAX_ARCHIVE_ENTRIES: usize = 32_768;
pub(crate) const MAX_ARCHIVE_RECORD_BYTES: u64 = 4 * 1024 * 1024;

/// An archive encoding supported by the native runtime installer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveFormat {
    TarGz,
    Zip,
}

/// A checksum-verified archive and its source-declared installation layout.
/// Extraction is bounded to 32,768 entries and 4 GiB and performs blocking I/O.
#[derive(Clone)]
pub struct ArchivePayload {
    version: String,
    sha256: String,
    bytes: Vec<u8>,
    format: ArchiveFormat,
    prefix: PathBuf,
    entrypoints: BTreeMap<String, PathBuf>,
    optional_entrypoints: BTreeMap<String, PathBuf>,
    bin_dirs: Vec<PathBuf>,
    executable_entrypoints: bool,
    lock: Option<ArchiveLock>,
}

impl std::fmt::Debug for ArchivePayload {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ArchivePayload")
            .field("version", &self.version)
            .field("sha256", &self.sha256)
            .field("bytes", &self.bytes.len())
            .field("format", &self.format)
            .field("prefix", &self.prefix)
            .field("entrypoints", &self.entrypoints)
            .field("optional_entrypoints", &self.optional_entrypoints)
            .field("bin_dirs", &self.bin_dirs)
            .field("executable_entrypoints", &self.executable_entrypoints)
            .finish()
    }
}

impl ArchivePayload {
    /// Verify publisher bytes and admit an exact prefix, command map and PATH directories.
    /// Layout paths use portable UTF-8 archive names; native store paths remain `PathBuf`.
    pub fn new(
        version: String,
        sha256: &str,
        bytes: Vec<u8>,
        format: ArchiveFormat,
        prefix: PathBuf,
        entrypoints: BTreeMap<String, PathBuf>,
        bin_dirs: Vec<PathBuf>,
    ) -> Result<Self, ToolAcquireError> {
        parse_tool_version(&version)?;
        if bytes.len() > MAX_ARCHIVE_BYTES {
            return Err(invalid("archive exceeds 512 MiB"));
        }
        let sha256 = crate::store::normalize_sha256(sha256)?;
        if hex_encode(&Sha256::digest(&bytes)) != sha256 {
            return Err(ToolAcquireError::ChecksumMismatch);
        }
        if !prefix.as_os_str().is_empty() {
            portable_path(&prefix)?;
        }
        validate_layout(&entrypoints, &bin_dirs, cfg!(windows))?;
        Ok(Self {
            version,
            sha256,
            bytes,
            format,
            prefix,
            entrypoints,
            optional_entrypoints: BTreeMap::new(),
            bin_dirs,
            executable_entrypoints: false,
            lock: None,
        })
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    /// Apply a publisher-declared 0755 mode to present command entrypoints.
    /// Other archive members retain their original modes. Windows retains its
    /// native executable semantics. This changes the recorded installed layout.
    pub fn with_executable_entrypoints(mut self) -> Self {
        self.executable_entrypoints = true;
        self
    }

    pub(crate) fn locked_to(mut self, lock: &ArchiveLock) -> Result<Self, ToolAcquireError> {
        if self.sha256 != lock.archive_sha256() {
            return Err(ToolAcquireError::ChecksumMismatch);
        }
        self.lock = Some(lock.clone());
        Ok(self)
    }

    /// Admit commands that the source declares optional across release versions.
    pub fn with_optional_entrypoints(
        mut self,
        optional: BTreeMap<String, PathBuf>,
    ) -> Result<Self, ToolAcquireError> {
        let mut all = self.entrypoints.clone();
        for (name, path) in &optional {
            if all.insert(name.clone(), path.clone()).is_some() {
                return Err(invalid("duplicate optional entrypoint"));
            }
        }
        validate_layout(&all, &self.bin_dirs, cfg!(windows))?;
        self.optional_entrypoints = optional;
        Ok(self)
    }

    fn extract(
        &self,
        destination: &Path,
    ) -> Result<BTreeMap<String, ArchiveEntry>, ToolAcquireError> {
        let mut extraction = Extraction {
            destination,
            prefix: &self.prefix,
            entries: BTreeMap::new(),
            declared: BTreeSet::new(),
            links: BTreeMap::new(),
            remaining: MAX_TREE_BYTES,
        };
        match self.format {
            ArchiveFormat::TarGz => {
                validate_tar(&self.bytes)?;
                let mut archive = tar::Archive::new(GzDecoder::new(Cursor::new(&self.bytes)));
                let entries = archive
                    .entries()
                    .map_err(|error| archive_io("read archive", error))?;
                for (count, entry) in entries.enumerate() {
                    check_count(count)?;
                    let mut entry =
                        entry.map_err(|error| archive_io("read archive entry", error))?;
                    let path = entry
                        .path()
                        .map_err(|error| archive_io("read archive path", error))?
                        .into_owned();
                    let kind = entry.header().entry_type();
                    let mode = entry
                        .header()
                        .mode()
                        .map_err(|error| archive_io("read archive mode", error))?;
                    if kind.is_dir() {
                        extraction.directory(&path)?;
                    } else if kind.is_file() {
                        extraction.file(&path, mode, &mut entry)?;
                    } else if kind.is_symlink() || kind.is_hard_link() {
                        let target = entry
                            .link_name()
                            .map_err(|error| archive_io("read archive link", error))?
                            .ok_or_else(|| invalid("archive link has no target"))?
                            .into_owned();
                        extraction.link(&path, &target, kind.is_hard_link())?;
                    } else {
                        return Err(invalid("unsupported archive entry type"));
                    }
                }
            }
            ArchiveFormat::Zip => {
                let mut archive = zip::ZipArchive::new(Cursor::new(&self.bytes))
                    .map_err(|error| invalid(&format!("read ZIP archive: {error}")))?;
                for index in 0..archive.len() {
                    check_count(index)?;
                    let mut entry = archive
                        .by_index(index)
                        .map_err(|error| invalid(&format!("read ZIP entry: {error}")))?;
                    let path = entry
                        .enclosed_name()
                        .ok_or_else(|| invalid("invalid ZIP path"))?;
                    let mode = entry.unix_mode().unwrap_or(0o644);
                    if entry.is_dir() {
                        extraction.directory(&path)?;
                    } else if mode & 0o170000 == 0o120000 {
                        let mut target = String::new();
                        entry
                            .take(4097)
                            .read_to_string(&mut target)
                            .map_err(|error| archive_io("read ZIP link", error))?;
                        if target.len() > 4096 {
                            return Err(invalid("archive link target exceeds 4096 bytes"));
                        }
                        extraction.link(&path, Path::new(&target), false)?;
                    } else if entry.is_file() && matches!(mode & 0o170000, 0 | 0o100000) {
                        extraction.file(&path, mode, &mut entry)?;
                    } else {
                        return Err(invalid("unsupported ZIP entry type"));
                    }
                }
            }
        }
        extraction.finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum ArchiveEntry {
    Directory,
    File { sha256: String, mode: Option<u32> },
    Symlink { target: PathBuf },
}

struct PendingLink {
    path: PathBuf,
    target: PathBuf,
    hard: bool,
}

struct Extraction<'a> {
    destination: &'a Path,
    prefix: &'a Path,
    entries: BTreeMap<String, ArchiveEntry>,
    declared: BTreeSet<PathBuf>,
    links: BTreeMap<PathBuf, PendingLink>,
    remaining: u64,
}

impl Extraction<'_> {
    fn relative(&self, path: &Path) -> Result<PathBuf, ToolAcquireError> {
        // tar paths can contain `.`; normalize it without admitting parent traversal.
        let mut clean = PathBuf::new();
        for part in path.components() {
            match part {
                Component::CurDir => {}
                Component::Normal(part) => clean.push(part),
                _ => return Err(invalid("archive path escapes its root")),
            }
        }
        clean
            .strip_prefix(self.prefix)
            .map(Path::to_path_buf)
            .map_err(|_| invalid("archive member is outside its declared prefix"))
    }

    fn declare(&mut self, path: &Path) -> Result<PathBuf, ToolAcquireError> {
        let relative = self.relative(path)?;
        portable_path(&relative)?;
        if !self.declared.insert(relative.clone()) {
            return Err(invalid("duplicate archive member"));
        }
        Ok(relative)
    }

    fn directory(&mut self, path: &Path) -> Result<(), ToolAcquireError> {
        let relative = self.relative(path)?;
        if relative.as_os_str().is_empty() {
            return Ok(());
        }
        let relative = self.declare(path)?;
        self.parents(&relative)?;
        if self
            .entries
            .get(&portable_path(&relative)?)
            .is_some_and(|entry| !matches!(entry, ArchiveEntry::Directory))
        {
            return Err(invalid("archive directory replaces a file"));
        }
        self.entries
            .insert(portable_path(&relative)?, ArchiveEntry::Directory);
        fs::create_dir_all(self.destination.join(&relative))
            .map_err(|error| ToolAcquireError::io("create archive directory", &relative, error))
    }

    fn parents(&mut self, path: &Path) -> Result<(), ToolAcquireError> {
        for parent in path
            .ancestors()
            .skip(1)
            .take_while(|path| !path.as_os_str().is_empty())
        {
            let key = portable_path(parent)?;
            if self
                .entries
                .get(&key)
                .is_some_and(|entry| !matches!(entry, ArchiveEntry::Directory))
                || self.links.contains_key(parent)
            {
                return Err(invalid("archive member traverses a non-directory"));
            }
            self.entries.insert(key, ArchiveEntry::Directory);
            if self.entries.len() > MAX_ARCHIVE_ENTRIES {
                return Err(invalid("archive exceeds 32768 installed entries"));
            }
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(self.destination.join(parent))
                .map_err(|error| ToolAcquireError::io("create archive parent", parent, error))?;
        }
        Ok(())
    }

    fn file(
        &mut self,
        path: &Path,
        mode: u32,
        input: &mut impl Read,
    ) -> Result<(), ToolAcquireError> {
        let relative = self.declare(path)?;
        self.parents(&relative)?;
        let output = self.destination.join(&relative);
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&output)
            .map_err(|error| ToolAcquireError::io("create archive file", &output, error))?;
        let mut hasher = Sha256::new();
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let count = input
                .read(&mut buffer)
                .map_err(|error| archive_io("read archive file", error))?;
            if count == 0 {
                break;
            }
            self.remaining = self
                .remaining
                .checked_sub(count as u64)
                .ok_or_else(|| invalid("archive expands beyond 4 GiB"))?;
            file.write_all(&buffer[..count])
                .map_err(|error| ToolAcquireError::io("write archive file", &output, error))?;
            hasher.update(&buffer[..count]);
        }
        file.sync_all()
            .map_err(|error| ToolAcquireError::io("sync archive file", &output, error))?;
        set_mode(&output, mode & 0o777)?;
        self.entries.insert(
            portable_path(&relative)?,
            ArchiveEntry::File {
                sha256: hex_encode(&hasher.finalize()),
                mode: native_mode(mode & 0o777),
            },
        );
        Ok(())
    }

    fn link(&mut self, path: &Path, target: &Path, hard: bool) -> Result<(), ToolAcquireError> {
        let relative = self.declare(path)?;
        self.parents(&relative)?;
        let resolved = if hard {
            self.relative(target)?
        } else {
            normalize_link(&relative, target)?
        };
        portable_path(&resolved)?;
        if self.entries.contains_key(&portable_path(&relative)?) {
            return Err(invalid("archive link replaces a directory"));
        }
        self.links.insert(
            relative.clone(),
            PendingLink {
                path: relative,
                target: if hard { resolved } else { target.to_path_buf() },
                hard,
            },
        );
        Ok(())
    }

    fn finish(mut self) -> Result<BTreeMap<String, ArchiveEntry>, ToolAcquireError> {
        for link in self.links.values().filter(|link| !link.hard) {
            create_symlink(
                &link.target,
                &self.destination.join(&link.path),
                self.destination,
            )?;
            self.entries.insert(
                portable_path(&link.path)?,
                ArchiveEntry::Symlink {
                    target: link.target.clone(),
                },
            );
        }
        let mut hard: Vec<_> = self.links.values().filter(|link| link.hard).collect();
        while !hard.is_empty() {
            let before = hard.len();
            let mut pending = Vec::new();
            for link in hard {
                let key = portable_path(&link.target)?;
                let Some(ArchiveEntry::File { .. }) = self.entries.get(&key) else {
                    pending.push(link);
                    continue;
                };
                let target = self.destination.join(&link.target);
                fs::hard_link(&target, self.destination.join(&link.path)).map_err(|error| {
                    ToolAcquireError::io("create archive hard link", &target, error)
                })?;
                let entry = self
                    .entries
                    .get(&key)
                    .cloned()
                    .ok_or_else(|| invalid("missing hard link target"))?;
                self.entries.insert(portable_path(&link.path)?, entry);
            }
            if pending.len() == before {
                return Err(invalid("archive has cyclic or missing hard link targets"));
            }
            hard = pending;
        }
        if self.entries.len() > MAX_ARCHIVE_ENTRIES {
            return Err(invalid("archive exceeds 32768 installed entries"));
        }
        for link in self.links.values().filter(|link| !link.hard) {
            resolve_inside(self.destination, &link.path)?;
        }
        Ok(self.entries)
    }
}

pub(crate) fn validate_layout(
    entrypoints: &BTreeMap<String, PathBuf>,
    bin_dirs: &[PathBuf],
    windows_commands: bool,
) -> Result<(), ToolAcquireError> {
    if entrypoints.is_empty()
        || entrypoints.len() > MAX_ARCHIVE_ENTRIES
        || bin_dirs.is_empty()
        || bin_dirs.len() > MAX_ARCHIVE_ENTRIES
    {
        return Err(invalid(
            "archive requires bounded entrypoints and PATH directories",
        ));
    }
    for (name, path) in entrypoints {
        parse_tool_name(name)?;
        portable_path(path)?;
        let filename = path
            .file_name()
            .ok_or_else(|| invalid("entrypoint has no file name"))?;
        if filename != std::ffi::OsStr::new(name)
            && !(windows_commands
                && ["exe", "cmd", "bat"].iter().any(|extension| {
                    filename == std::ffi::OsStr::new(&format!("{name}.{extension}"))
                }))
        {
            return Err(invalid(
                "entrypoint file name must provide its declared command",
            ));
        }
        if !bin_dirs
            .iter()
            .any(|directory| path.parent() == Some(directory.as_path()))
        {
            return Err(invalid(
                "archive entrypoint is outside its declared PATH directories",
            ));
        }
    }
    for path in bin_dirs {
        if !path.as_os_str().is_empty() {
            portable_path(path)?;
        }
    }
    Ok(())
}

pub(crate) fn portable_path(path: &Path) -> Result<String, ToolAcquireError> {
    if path.as_os_str().len() > 4096 {
        return Err(invalid("archive path exceeds 4096 bytes"));
    }
    let parts: Result<Vec<_>, _> = path
        .components()
        .map(|part| match part {
            Component::Normal(part) => part
                .to_str()
                .ok_or_else(|| invalid("archive names must be UTF-8")),
            _ => Err(invalid("invalid archive path component")),
        })
        .collect();
    let text = parts?.join("/");
    validate_relative_path(&text)?;
    Ok(text)
}

fn validate_tar(bytes: &[u8]) -> Result<(), ToolAcquireError> {
    // tar buffers extension headers before yielding members; bound them before normal parsing.
    let mut archive = tar::Archive::new(GzDecoder::new(Cursor::new(bytes)));
    let mut remaining = MAX_TREE_BYTES;
    for (count, entry) in archive
        .entries()
        .map_err(|error| archive_io("read raw archive", error))?
        .raw(true)
        .enumerate()
    {
        if count >= MAX_ARCHIVE_ENTRIES * 4 {
            return Err(invalid("archive exceeds its header budget"));
        }
        let entry = entry.map_err(|error| archive_io("read raw archive member", error))?;
        let kind = entry.header().entry_type();
        let extension = kind.is_gnu_longname()
            || kind.is_gnu_longlink()
            || kind.is_pax_local_extensions()
            || kind.is_pax_global_extensions();
        if extension && entry.size() > 64 * 1024 {
            return Err(invalid("archive extension header exceeds 64 KiB"));
        }
        if !extension
            && !(kind.is_file() || kind.is_dir() || kind.is_symlink() || kind.is_hard_link())
        {
            return Err(invalid("unsupported archive entry type"));
        }
        remaining = remaining
            .checked_sub(entry.size())
            .ok_or_else(|| invalid("archive expands beyond 4 GiB"))?;
    }
    Ok(())
}

fn normalize_link(path: &Path, target: &Path) -> Result<PathBuf, ToolAcquireError> {
    let mut result = path.parent().unwrap_or(Path::new("")).to_path_buf();
    for part in target.components() {
        match part {
            Component::Normal(part) => result.push(part),
            Component::CurDir => {}
            Component::ParentDir if result.pop() => {}
            _ => return Err(invalid("archive link escapes its root")),
        }
    }
    Ok(result)
}

fn resolve_inside(root: &Path, path: &Path) -> Result<PathBuf, ToolAcquireError> {
    let target = root
        .join(path)
        .canonicalize()
        .map_err(|error| ToolAcquireError::io("resolve archive entry", path, error))?;
    let root = root
        .canonicalize()
        .map_err(|error| ToolAcquireError::io("resolve archive root", root, error))?;
    if !target.starts_with(root) {
        return Err(invalid("archive entry resolves outside its root"));
    }
    Ok(target)
}

fn create_symlink(target: &Path, path: &Path, root: &Path) -> Result<(), ToolAcquireError> {
    #[cfg(unix)]
    let result = {
        let _ = root;
        std::os::unix::fs::symlink(target, path)
    };
    #[cfg(windows)]
    let result = {
        let resolved = root.join(normalize_link(
            path.strip_prefix(root)
                .map_err(|_| invalid("invalid link path"))?,
            target,
        )?);
        if resolved.is_dir() {
            std::os::windows::fs::symlink_dir(target, path)
        } else {
            std::os::windows::fs::symlink_file(target, path)
        }
    };
    #[cfg(not(any(unix, windows)))]
    let result = {
        let _ = (target, root);
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "native symlinks are unsupported",
        ))
    };
    result.map_err(|error| ToolAcquireError::io("create archive symlink", path, error))
}

fn native_mode(mode: u32) -> Option<u32> {
    if cfg!(unix) {
        Some(mode)
    } else {
        None
    }
}

fn check_count(count: usize) -> Result<(), ToolAcquireError> {
    if count >= MAX_ARCHIVE_ENTRIES {
        Err(invalid("archive exceeds 32768 entries"))
    } else {
        Ok(())
    }
}

fn archive_io(action: &'static str, source: std::io::Error) -> ToolAcquireError {
    ToolAcquireError::io(action, Path::new("<archive>"), source)
}

fn invalid(message: &str) -> ToolAcquireError {
    ToolAcquireError::InvalidInstallation(message.into())
}

#[cfg(test)]
mod tests;
