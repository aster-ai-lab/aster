//! Handle-anchored Windows file observations for invocation receipts.

use super::*;
use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt, OpenOptionsMaybeDirExt};
use cap_std::fs::{Dir, OpenOptions};
use std::ffi::OsString;
use std::io;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::AsRawHandle;
use windows_sys::Win32::Storage::FileSystem::{
    FileBasicInfo, GetFileInformationByHandle, GetFileInformationByHandleEx,
    BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT,
    FILE_BASIC_INFO, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ,
    FILE_SHARE_WRITE,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct NodeInfo {
    volume: u64,
    index: u64,
    attributes: u32,
    len: u64,
    modified: i64,
    changed: i64,
}

pub(super) fn observe_identity_windows(
    root: &Path,
    relative: &str,
    budget: &mut u64,
    metadata_fallback: bool,
) -> Option<ContentStamp> {
    let parts = relative_components(Path::new(relative))?;
    let (leaf, ancestors) = parts.split_last()?;
    let directory = open_directory(root, &[]).ok()?;
    let parent = match open_ancestors(directory, ancestors) {
        Ok(parent) => parent,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Some(missing_identity(relative))
        }
        Err(_) => return None,
    };
    let mut file = match open_node(&parent, leaf) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Some(missing_identity(relative))
        }
        Err(_) => return None,
    };
    let before = node_info(&file)?;
    let digest = if before.attributes & FILE_ATTRIBUTE_DIRECTORY != 0 {
        None
    } else if before.len > *budget {
        if !metadata_fallback {
            return None;
        }
        None
    } else {
        Some(read_content_digest(&mut file, before.len, budget)?)
    };
    if node_info(&file)? != before || node_info(&open_node(&parent, leaf).ok()?)? != before {
        return None;
    }
    Some(ContentStamp {
        path: relative.into(),
        present: true,
        len: before.len,
        modified: u128::try_from(before.modified).ok()? * 100,
        changed: u128::try_from(before.changed).ok()? * 100,
        node: (before.volume, before.index),
        mode: before.attributes,
        kind: u8::from(before.attributes & FILE_ATTRIBUTE_DIRECTORY != 0),
        content_digest: digest,
    })
}

pub(super) fn directory_entries(
    root: &Path,
    relative: &Path,
    limit: usize,
) -> Option<(Vec<(OsString, NodeInfo)>, bool, bool)> {
    let directory = open_directory(root, &relative_components(relative)?).ok()?;
    let before = node_info(&directory)?;
    let mut found = Vec::new();
    let mut incomplete = false;
    let mut capped = false;
    for entry in directory.entries().ok()? {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => {
                incomplete = true;
                continue;
            }
        };
        if found.len() == limit {
            capped = true;
            break;
        }
        let name = entry.file_name();
        // Runtime metadata directories are outside the observed source tree.
        if name == ".git" || name == ".a3s" || name == ".a3s-code" {
            continue;
        }
        match open_node(&directory, &name)
            .ok()
            .and_then(|file| node_info(&file))
        {
            Some(info) => found.push((name, info)),
            None => incomplete = true,
        }
    }
    incomplete |= node_info(&directory).as_ref() != Some(&before);
    Some((found, incomplete, capped))
}

pub(super) fn node_stamp(relative: &str, info: &NodeInfo) -> Option<NodeStamp> {
    if relative.is_empty() || relative.len() > OBSERVATION_PATH_BYTE_CAP {
        return None;
    }
    Some(NodeStamp {
        path: relative.replace('\\', "/"),
        kind: u8::from(info.attributes & FILE_ATTRIBUTE_DIRECTORY != 0),
        dev: info.volume,
        ino: info.index,
        mode: info.attributes,
        len: info.len,
        modified: (info.modified, 0),
        changed: (info.changed, 0),
    })
}

fn open_directory(root: &Path, parts: &[&std::ffi::OsStr]) -> io::Result<Dir> {
    let file = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(root)?;
    if node_info(&file).is_none_or(|info| info.attributes & FILE_ATTRIBUTE_DIRECTORY == 0) {
        return Err(io::Error::other("observation root is not a real directory"));
    }
    open_ancestors(Dir::from_std_file(file), parts)
}

fn open_ancestors(mut directory: Dir, parts: &[&std::ffi::OsStr]) -> io::Result<Dir> {
    for part in parts {
        if part.to_string_lossy().contains(':') {
            return Err(io::Error::other("alternate data stream"));
        }
        directory = directory.open_dir_nofollow(part)?;
        if node_info(&directory).is_none() {
            return Err(io::Error::other("unobservable directory"));
        }
    }
    Ok(directory)
}

fn open_node(directory: &Dir, name: &std::ffi::OsStr) -> io::Result<cap_std::fs::File> {
    if name.to_string_lossy().contains(':') {
        return Err(io::Error::other("alternate data stream"));
    }
    let mut options = OpenOptions::new();
    options
        .read(true)
        .follow(FollowSymlinks::No)
        .maybe_dir(true);
    directory.open_with(name, &options)
}

fn node_info(file: &impl AsRawHandle) -> Option<NodeInfo> {
    let mut info = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::uninit();
    let mut basic = std::mem::MaybeUninit::<FILE_BASIC_INFO>::uninit();
    // SAFETY: both Win32 calls receive a live owned handle and correctly sized writable storage.
    let success = unsafe {
        GetFileInformationByHandle(file.as_raw_handle(), info.as_mut_ptr()) != 0
            && GetFileInformationByHandleEx(
                file.as_raw_handle(),
                FileBasicInfo,
                basic.as_mut_ptr().cast(),
                std::mem::size_of::<FILE_BASIC_INFO>() as u32,
            ) != 0
    };
    if !success {
        return None;
    }
    // SAFETY: successful calls initialized both information structures.
    let (info, basic) = unsafe { (info.assume_init(), basic.assume_init()) };
    if info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return None;
    }
    Some(NodeInfo {
        volume: u64::from(info.dwVolumeSerialNumber),
        index: (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow),
        attributes: info.dwFileAttributes,
        len: (u64::from(info.nFileSizeHigh) << 32) | u64::from(info.nFileSizeLow),
        modified: basic.LastWriteTime,
        changed: basic.ChangeTime,
    })
}

#[cfg(test)]
#[path = "windows_tests.rs"]
mod tests;
