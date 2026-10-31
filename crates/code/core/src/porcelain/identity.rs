use super::*;

pub(super) fn stamp_lines(
    root: &Path,
    lines: &[String],
    byte_budget: &mut u64,
    path_budget: &mut usize,
) -> (Vec<ContentStamp>, bool, bool) {
    let mut paths = Vec::new();
    let mut incomplete = false;
    let mut read_failed = false;
    for line in lines {
        match paths_of(line) {
            Some(found) if paths.len() + found.len() <= FILE_WALK_CAP => paths.extend(found),
            Some(_) => {
                incomplete = true;
                break;
            }
            None => {
                incomplete = true;
                read_failed = true;
            }
        }
    }
    let (stamps, stamp_incomplete, stamp_failed) =
        stamp_paths(root, &paths, byte_budget, path_budget);
    (
        stamps,
        incomplete || stamp_incomplete,
        read_failed || stamp_failed,
    )
}

pub(super) fn stamp_paths(
    root: &Path,
    paths: &[String],
    byte_budget: &mut u64,
    path_budget: &mut usize,
) -> (Vec<ContentStamp>, bool, bool) {
    let mut stamps = Vec::new();
    let mut incomplete = false;
    let mut read_failed = false;
    let mut seen = HashSet::new();
    for path in paths {
        if path.len() > OBSERVATION_PATH_BYTE_CAP {
            incomplete = true;
            read_failed = true;
            continue;
        }
        if !seen.insert(path.clone()) || is_harness_path(path) {
            continue;
        }
        if *path_budget == 0 {
            incomplete = true;
            break;
        }
        *path_budget -= 1;
        #[cfg(unix)]
        let stamp = observe_identity_unix(root, path, byte_budget, true);
        #[cfg(windows)]
        let stamp = observe_identity_windows(root, path, byte_budget, true);
        #[cfg(not(any(unix, windows)))]
        let stamp = observe_file_identity(root, path, byte_budget);
        match stamp {
            Some(stamp) => stamps.push(stamp),
            None => {
                incomplete = true;
                read_failed = true;
            }
        }
    }
    stamps.sort_by(|left, right| left.path.cmp(&right.path));
    (stamps, incomplete, read_failed)
}

/// Paths `git status` hides. `S` is skip-worktree, `h`/`s` is assume-unchanged.
pub(super) enum HiddenIndex {
    Listed(Vec<String>, bool),
    OverCap,
    Failed,
}

pub(super) async fn hidden_index_paths(root: &Path) -> HiddenIndex {
    let Some(prefix) = workspace_git_prefix(root).await else {
        return HiddenIndex::Failed;
    };
    let output = match git_capture(root, &["ls-files", "--full-name", "-v", "-z", "--", "."]).await
    {
        GitCapture::Bytes(output) => output,
        GitCapture::OverCap => return HiddenIndex::OverCap,
        GitCapture::Failed => return HiddenIndex::Failed,
    };
    let (paths, mut incomplete) = parse_hidden_index_paths(&output);
    let mut scoped = Vec::new();
    for path in paths {
        match bind_git_path(&path, &prefix) {
            Ok(Some(relative)) => scoped.push(relative.to_string()),
            Ok(None) => {}
            Err(()) => incomplete = true,
        }
    }
    HiddenIndex::Listed(scoped, incomplete)
}

pub(super) fn parse_hidden_index_paths(output: &[u8]) -> (Vec<String>, bool) {
    let mut paths = Vec::new();
    let mut incomplete = false;
    for part in output
        .split(|byte| *byte == 0)
        .filter(|part| !part.is_empty())
    {
        if part.len() < 3 || part[1] != b' ' {
            incomplete = true;
            continue;
        }
        if !matches!(part[0], b'S' | b's' | b'h') {
            continue;
        }
        let Ok(path) = std::str::from_utf8(&part[2..]) else {
            incomplete = true;
            continue;
        };
        if path.is_empty() || path.contains('\n') || path.len() > OBSERVATION_PATH_BYTE_CAP {
            incomplete = true;
            continue;
        }
        if paths.len() == FILE_WALK_CAP {
            incomplete = true;
            break;
        }
        paths.push(path.to_string());
    }
    (paths, incomplete)
}

/// Read one identity within the workspace. `budget` is remaining readable bytes
/// shared across the observation, not a per-path allowance. A missing path has
/// a complete absent identity; any unsafe or unprovable read returns `None`.
pub(crate) fn observe_file_identity(
    root: &Path,
    relative: &str,
    budget: &mut u64,
) -> Option<ContentStamp> {
    if relative.len() > OBSERVATION_PATH_BYTE_CAP {
        return None;
    }
    #[cfg(unix)]
    {
        observe_identity_unix(root, relative, budget, false)
    }
    #[cfg(windows)]
    {
        observe_identity_windows(root, relative, budget, false)
    }
    #[cfg(not(any(unix, windows)))]
    {
        // Never substitute a following open on platforms where this observer
        // has no descriptor-anchored implementation. Receipts stay incomplete.
        let _ = (root, relative, budget);
        None
    }
}

pub(super) fn tool_file_identity(stamp: &ContentStamp) -> Option<aster_effect::ToolFileIdentityV1> {
    use aster_effect::{ToolFileIdentityV1, ToolFileKind};
    let kind = if !stamp.present {
        ToolFileKind::Missing
    } else {
        match stamp.kind {
            0 => ToolFileKind::File,
            1 => ToolFileKind::Directory,
            2 => ToolFileKind::Symlink,
            _ => return None,
        }
    };
    if matches!(kind, ToolFileKind::File | ToolFileKind::Symlink) && stamp.content_digest.is_none()
    {
        return None;
    }
    Some(ToolFileIdentityV1 {
        kind,
        content_digest: stamp
            .content_digest
            .as_ref()
            .map(|digest| format!("sha256:{digest}")),
        mode: stamp.present.then_some(stamp.mode),
    })
}

#[cfg(any(unix, windows))]
pub(super) fn missing_identity(relative: &str) -> ContentStamp {
    ContentStamp {
        path: relative.to_string(),
        present: false,
        len: 0,
        modified: 0,
        changed: 0,
        node: (0, 0),
        mode: 0,
        kind: 0,
        content_digest: None,
    }
}

#[cfg(any(unix, windows, test))]
pub(super) fn read_content_digest(
    reader: &mut impl Read,
    len: u64,
    budget: &mut u64,
) -> Option<String> {
    if len > *budget {
        return None;
    }
    let mut reader = reader.take(len);
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut read = 0_u64;
    loop {
        let count = match reader.read(&mut buffer) {
            Ok(count) => count,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return None,
        };
        if count == 0 {
            break;
        }
        *budget -= count as u64;
        read += count as u64;
        digest.update(&buffer[..count]);
    }
    (read == len).then(|| format!("{:x}", digest.finalize()))
}

#[cfg(any(unix, windows))]
pub(super) fn relative_components(relative: &Path) -> Option<Vec<&std::ffi::OsStr>> {
    let mut parts = Vec::new();
    for part in relative.components() {
        match part {
            std::path::Component::Normal(name) => parts.push(name),
            std::path::Component::CurDir => {}
            _ => return None,
        }
    }
    Some(parts)
}

#[cfg(unix)]
pub(super) fn open_root_directory(root: &Path) -> std::io::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    let metadata = std::fs::symlink_metadata(root)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(std::io::Error::other(
            "observation root is not a real directory",
        ));
    }
    std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(root)
}

#[cfg(unix)]
pub(super) fn open_at(
    parent: &std::fs::File,
    name: &std::ffi::CStr,
    flags: i32,
) -> std::io::Result<std::fs::File> {
    use std::os::fd::{AsRawFd, FromRawFd};
    // SAFETY: the parent descriptor and NUL-terminated name live throughout
    // openat; no mode argument is required because O_CREAT is never used.
    let fd = unsafe { libc::openat(parent.as_raw_fd(), name.as_ptr(), flags | libc::O_CLOEXEC) };
    if fd < 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: openat returned a new descriptor owned by this File alone.
    Ok(unsafe { std::fs::File::from_raw_fd(fd) })
}

#[cfg(unix)]
pub(super) fn open_parent_directory(
    root: std::fs::File,
    parts: &[&std::ffi::OsStr],
) -> std::io::Result<std::fs::File> {
    use std::os::unix::ffi::OsStrExt;
    let mut directory = root;
    for part in parts {
        let name = std::ffi::CString::new(part.as_bytes())
            .map_err(|_| std::io::Error::other("NUL in observation path"))?;
        directory = open_at(
            &directory,
            &name,
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_NONBLOCK,
        )?;
    }
    Ok(directory)
}

#[cfg(unix)]
pub(super) fn stat_at(
    parent: &std::fs::File,
    name: &std::ffi::CStr,
) -> std::io::Result<libc::stat> {
    use std::os::fd::AsRawFd;
    let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
    // SAFETY: stat points to writable storage; both inputs remain valid. The
    // no-follow flag observes the link itself and cannot read its target.
    if unsafe {
        libc::fstatat(
            parent.as_raw_fd(),
            name.as_ptr(),
            stat.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    } != 0
    {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: successful fstatat initialized the entire stat value.
    Ok(unsafe { stat.assume_init() })
}

#[cfg(unix)]
pub(super) fn stat_file(file: &std::fs::File) -> Option<libc::stat> {
    use std::os::fd::AsRawFd;
    let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
    // SAFETY: file owns a live descriptor and stat is writable storage.
    if unsafe { libc::fstat(file.as_raw_fd(), stat.as_mut_ptr()) } != 0 {
        return None;
    }
    // SAFETY: successful fstat initialized the entire stat value.
    Some(unsafe { stat.assume_init() })
}

#[cfg(unix)]
pub(super) fn same_node(before: &libc::stat, after: &libc::stat) -> bool {
    before.st_dev == after.st_dev
        && before.st_ino == after.st_ino
        && before.st_mode == after.st_mode
        && before.st_size == after.st_size
        && before.st_mtime == after.st_mtime
        && before.st_ctime == after.st_ctime
        && stat_subseconds(before) == stat_subseconds(after)
}

#[cfg(unix)]
pub(super) fn stat_subseconds(stat: &libc::stat) -> (i128, i128) {
    #[cfg(target_os = "netbsd")]
    {
        (stat.st_mtimensec as i128, stat.st_ctimensec as i128)
    }
    #[cfg(not(target_os = "netbsd"))]
    {
        (stat.st_mtime_nsec as i128, stat.st_ctime_nsec as i128)
    }
}

#[cfg(unix)]
#[allow(clippy::unnecessary_cast)] // libc::mode_t is u16 on macOS and u32 on Linux.
pub(super) fn identity_from_stat(
    relative: &str,
    stat: &libc::stat,
    kind: u8,
    digest: Option<String>,
) -> Option<ContentStamp> {
    let modified = u128::try_from(stat.st_mtime)
        .ok()?
        .checked_mul(1_000_000_000)?
        .checked_add(u128::try_from(stat_subseconds(stat).0).ok()?)?;
    let changed = u128::try_from(stat.st_ctime)
        .ok()?
        .checked_mul(1_000_000_000)?
        .checked_add(u128::try_from(stat_subseconds(stat).1).ok()?)?;
    Some(ContentStamp {
        path: relative.to_string(),
        present: true,
        len: u64::try_from(stat.st_size).ok()?,
        modified,
        changed,
        node: (stat.st_dev as u64, stat.st_ino as u64),
        mode: stat.st_mode as u32,
        kind,
        content_digest: digest,
    })
}

#[cfg(unix)]
pub(super) fn observe_identity_unix(
    root: &Path,
    relative: &str,
    budget: &mut u64,
    metadata_fallback: bool,
) -> Option<ContentStamp> {
    use std::os::fd::AsRawFd;
    use std::os::unix::ffi::OsStrExt;
    let parts = relative_components(Path::new(relative))?;
    let (leaf, ancestors) = parts.split_last()?;
    let root = open_root_directory(root).ok()?;
    let parent = match open_parent_directory(root, ancestors) {
        Ok(parent) => parent,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Some(missing_identity(relative));
        }
        Err(_) => return None,
    };
    let name = std::ffi::CString::new(leaf.as_bytes()).ok()?;
    let before = match stat_at(&parent, &name) {
        Ok(stat) => stat,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Some(missing_identity(relative));
        }
        Err(_) => return None,
    };
    let len = u64::try_from(before.st_size).ok()?;
    match before.st_mode & libc::S_IFMT {
        libc::S_IFREG => {
            // Gate kind and declared size BEFORE open: a FIFO may block open
            // itself. O_NONBLOCK also closes a regular-file-to-FIFO race.
            if len > *budget {
                // Keep no-follow metadata for candidate detection without
                // inventing a content hash or spending the baseline budget.
                return metadata_fallback
                    .then(|| identity_from_stat(relative, &before, 0, None))
                    .flatten();
            }
            let mut file = open_at(
                &parent,
                &name,
                libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK,
            )
            .ok()?;
            if !same_node(&before, &stat_file(&file)?) {
                return None;
            }
            let digest = read_content_digest(&mut file, len, budget)?;
            if !same_node(&before, &stat_file(&file)?)
                || !same_node(&before, &stat_at(&parent, &name).ok()?)
            {
                return None;
            }
            identity_from_stat(relative, &before, 0, Some(digest))
        }
        libc::S_IFDIR => identity_from_stat(relative, &before, 1, None),
        libc::S_IFLNK => {
            // Hash link spelling only. Never follow it, even to an internal
            // path. A retargeted link is a different observed identity.
            if len == 0 || len > 16 * 1024 || len > *budget {
                return None;
            }
            let mut target = vec![0_u8; len as usize];
            // SAFETY: the parent/name are live and target is writable for len
            // bytes. readlinkat never opens or reads the linked destination.
            let count = unsafe {
                libc::readlinkat(
                    parent.as_raw_fd(),
                    name.as_ptr(),
                    target.as_mut_ptr().cast(),
                    target.len(),
                )
            };
            let count = usize::try_from(count).ok()?;
            *budget = budget.saturating_sub(count as u64);
            if count != target.len() || !same_node(&before, &stat_at(&parent, &name).ok()?) {
                return None;
            }
            identity_from_stat(
                relative,
                &before,
                2,
                Some(format!("{:x}", Sha256::digest(&target))),
            )
        }
        _ => None,
    }
}

/// Names and their no-follow stats, read through the one anchored directory
/// descriptor so a deep tree costs one `fstatat` per entry.
#[cfg(unix)]
pub(super) fn directory_entries(
    root: &Path,
    relative: &Path,
    limit: usize,
) -> Option<(Vec<(std::ffi::OsString, libc::stat)>, bool, bool)> {
    use std::os::fd::IntoRawFd;
    use std::os::unix::ffi::{OsStrExt, OsStringExt};
    let errno = directory_errno()?;
    if errno.is_null() {
        return None;
    }
    let parts = relative_components(relative)?;
    let file = open_parent_directory(open_root_directory(root).ok()?, &parts).ok()?;
    let before = stat_file(&file)?;
    let anchor = file.try_clone().ok()?;
    let fd = file.into_raw_fd();
    // SAFETY: ownership of the directory descriptor transfers to fdopendir
    // on success. On failure it is still ours and is closed below.
    let directory = unsafe { libc::fdopendir(fd) };
    if directory.is_null() {
        // SAFETY: fdopendir failed and fd is still an owned live descriptor.
        unsafe {
            libc::close(fd);
        }
        return None;
    }
    let mut names = Vec::new();
    let mut incomplete = false;
    let mut capped = false;
    loop {
        // SAFETY: errno points to this thread's live errno cell; clearing it
        // distinguishes readdir EOF from failure. This stream is not shared.
        let found = unsafe {
            *errno = 0;
            libc::readdir(directory)
        };
        if found.is_null() {
            // SAFETY: the thread-local errno pointer remains valid here.
            incomplete |= unsafe { *errno } != 0;
            break;
        }
        // Some filesystems return a d_name longer than libc's fixed array.
        // Read only the record supplied by readdir, never copy a whole dirent
        // into a fixed readdir_r buffer. The entry lives until the next call.
        let offset = std::mem::offset_of!(libc::dirent, d_name);
        // SAFETY: successful readdir returns a valid initialized record.
        let record_len = unsafe { (*found).d_reclen as usize };
        let Some(name_capacity) = record_len.checked_sub(offset) else {
            incomplete = true;
            break;
        };
        if name_capacity == 0 || name_capacity > OBSERVATION_PATH_BYTE_CAP + 1 {
            incomplete = true;
            break;
        }
        let mut name_bytes = Vec::new();
        let mut terminated = false;
        for index in 0..name_capacity {
            // SAFETY: d_reclen bounds the record and readdir initializes the
            // name through its terminating NUL. Stop before any tail padding.
            let byte = unsafe { *found.cast::<u8>().add(offset + index) };
            if byte == 0 {
                terminated = true;
                break;
            }
            name_bytes.push(byte);
        }
        if !terminated {
            incomplete = true;
            break;
        }
        let name = std::ffi::OsString::from_vec(name_bytes);
        if name == "." || name == ".." {
            continue;
        }
        if names.len() == limit {
            capped = true;
            break;
        }
        let Ok(c_name) = std::ffi::CString::new(name.as_bytes()) else {
            incomplete = true;
            continue;
        };
        match stat_at(&anchor, &c_name) {
            Ok(stat) => names.push((name, stat)),
            // Vanished or unreadable mid-walk: never a complete baseline.
            Err(_) => incomplete = true,
        }
    }
    // SAFETY: fdopendir succeeded and this is the only close of its stream.
    if unsafe { libc::closedir(directory) } != 0 {
        incomplete = true;
    }
    let current = open_root_directory(root)
        .and_then(|root| open_parent_directory(root, &parts))
        .ok()
        .and_then(|file| stat_file(&file));
    if !current.is_some_and(|current| same_node(&before, &current)) {
        // A directory changed/replaced during enumeration. A transient prefix
        // cannot serve as a complete before/after authority baseline.
        incomplete = true;
    }
    Some((names, incomplete, capped))
}

#[cfg(all(unix, any(target_vendor = "apple", target_os = "freebsd")))]
pub(super) fn directory_errno() -> Option<*mut libc::c_int> {
    // SAFETY: this libc accessor returns the current thread's errno cell.
    Some(unsafe { libc::__error() })
}

#[cfg(all(unix, any(target_os = "linux", target_os = "dragonfly")))]
pub(super) fn directory_errno() -> Option<*mut libc::c_int> {
    // SAFETY: this libc accessor returns the current thread's errno cell.
    Some(unsafe { libc::__errno_location() })
}

#[cfg(all(
    unix,
    any(target_os = "android", target_os = "netbsd", target_os = "openbsd")
))]
pub(super) fn directory_errno() -> Option<*mut libc::c_int> {
    // SAFETY: this libc accessor returns the current thread's errno cell.
    Some(unsafe { libc::__errno() })
}

#[cfg(all(unix, any(target_os = "solaris", target_os = "illumos")))]
pub(super) fn directory_errno() -> Option<*mut libc::c_int> {
    // SAFETY: this libc accessor returns the current thread's errno cell.
    Some(unsafe { libc::___errno() })
}

#[cfg(all(unix, target_os = "aix"))]
pub(super) fn directory_errno() -> Option<*mut libc::c_int> {
    // SAFETY: this libc accessor returns the current thread's errno cell.
    Some(unsafe { libc::_Errno() })
}

#[cfg(all(
    unix,
    not(any(
        target_vendor = "apple",
        target_os = "freebsd",
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "android",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "solaris",
        target_os = "illumos",
        target_os = "aix"
    ))
))]
pub(super) fn directory_errno() -> Option<*mut libc::c_int> {
    None
}

#[cfg(not(any(unix, windows)))]
pub(super) fn directory_entries(
    _root: &Path,
    _relative: &Path,
    _limit: usize,
) -> Option<(Vec<(std::ffi::OsString, ())>, bool, bool)> {
    None
}
