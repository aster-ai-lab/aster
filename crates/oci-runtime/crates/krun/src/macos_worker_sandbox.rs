use std::collections::BTreeSet;
use std::ffi::{c_char, CStr};
use std::path::{Component, Path, PathBuf};
use std::ptr;

use aster_oci_sdk::{Error, ErrorCode, Result};

use crate::ffi::{value_to_cstring, FfiStringArray};
use crate::unix_process::PreparedConsoleOutput;

const OPERATION: &str = "confine-macos-vm-worker";
const BASE_PROFILE: &str = r#"(version 1)
(deny default)
(deny file-link)
(allow sysctl-read)
(allow file-read-metadata (literal "/var") (literal "/tmp") (literal "/etc"))
(allow file-read* (subpath "/System/Library") (subpath "/usr/lib"))
(allow file-read* file-write-data file-ioctl
  (literal "/dev/null") (literal "/dev/zero")
  (literal "/dev/random") (literal "/dev/urandom"))
(allow mach-lookup (global-name "com.apple.logd"))
"#;

unsafe extern "C" {
    fn sandbox_init_with_parameters(
        profile: *const c_char,
        flags: u64,
        parameters: *const *const c_char,
        error: *mut *mut c_char,
    ) -> i32;
    fn sandbox_free_error(error: *mut c_char);
}

pub(crate) fn confine(
    read_only_files: &[&Path],
    runtime_share: &Path,
    console: &PreparedConsoleOutput,
    agent_socket: Option<&Path>,
    read_only_directories: &[&Path],
    read_write_directories: &[&Path],
    path_restrictions: &[(PathBuf, aster_oci_sdk::StoragePathRestriction)],
) -> Result<()> {
    let directories: Vec<_> = read_only_directories
        .iter()
        .chain(read_write_directories)
        .copied()
        .collect();
    for (index, directory) in directories.iter().enumerate() {
        if [runtime_share, console.path()]
            .into_iter()
            .chain(agent_socket)
            .chain(read_only_files.iter().copied())
            .any(|path| overlaps(directory, path))
            || directories[..index]
                .iter()
                .any(|path| overlaps(directory, path))
        {
            return Err(sandbox_error(
                "caller directories overlap VM assets, control paths, or each other",
            ));
        }
    }
    for directory in read_write_directories {
        if [Path::new("/System/Library"), Path::new("/usr/lib")]
            .into_iter()
            .any(|path| overlaps(directory, path))
        {
            return Err(sandbox_error(
                "caller writable directory overlaps system libraries",
            ));
        }
    }
    let mut profile = BASE_PROFILE.to_string();
    let mut parameters = Vec::new();
    let mut ancestors = BTreeSet::new();
    for path in read_only_files {
        if path.starts_with(runtime_share) || *path == console.path() {
            return Err(sandbox_error(
                "read-only VM assets overlap a writable grant",
            ));
        }
        add_path_rule(&mut profile, &mut parameters, "file-read*", "literal", path)?;
        ancestors.extend(path.ancestors().skip(1));
    }
    add_path_rule(
        &mut profile,
        &mut parameters,
        "file-read* file-write*",
        "subpath",
        runtime_share,
    )?;
    for (paths, operations) in [
        (read_only_directories, "file-read*"),
        (read_write_directories, "file-read* file-write*"),
    ] {
        for path in paths {
            add_path_rule(&mut profile, &mut parameters, operations, "subpath", path)?;
            ancestors.extend(path.ancestors().skip(1));
        }
    }
    add_path_rule(
        &mut profile,
        &mut parameters,
        "file-write-data",
        "literal",
        console.path(),
    )?;
    add_path_rule(
        &mut profile,
        &mut parameters,
        "file-write-data",
        "literal",
        &console.pinned_path(),
    )?;
    ancestors.extend(runtime_share.ancestors().skip(1));
    ancestors.extend(console.path().ancestors().skip(1));
    if let Some(socket) = agent_socket {
        let parameter = add_parameter(&mut parameters, socket)?;
        profile.push_str(&format!(
            "(allow network-outbound (remote unix-socket (path-literal (param \"{parameter}\"))))\n"
        ));
        add_path_rule(
            &mut profile,
            &mut parameters,
            "file-read*",
            "literal",
            socket,
        )?;
        ancestors.extend(socket.ancestors().skip(1));
    }
    for path in ancestors {
        add_path_rule(
            &mut profile,
            &mut parameters,
            "file-read-metadata",
            "literal",
            path,
        )?;
    }
    for (source, restrictions) in path_restrictions {
        if !directories.contains(&source.as_path())
            || (!restrictions.write_exceptions().is_empty()
                && !read_write_directories.contains(&source.as_path()))
        {
            return Err(sandbox_error(
                "path restrictions do not match a writable directory grant",
            ));
        }
        apply_path_restrictions(&mut profile, &mut parameters, source, restrictions)?;
    }
    let profile = value_to_cstring(OPERATION, "Seatbelt profile", &profile)?;
    let parameters = FfiStringArray::new(OPERATION, "Seatbelt parameters", &parameters)?;
    let mut error = ptr::null_mut();
    // SAFETY: all strings and the terminated pointer table remain alive for this call.
    let status = unsafe {
        sandbox_init_with_parameters(profile.as_ptr(), 0, parameters.as_ptr(), &mut error)
    };
    let reason = if error.is_null() {
        None
    } else {
        // SAFETY: Seatbelt owns this terminated error buffer until sandbox_free_error.
        let reason = unsafe { CStr::from_ptr(error) }
            .to_string_lossy()
            .into_owned();
        // SAFETY: this is the buffer returned by Seatbelt, released exactly once.
        unsafe { sandbox_free_error(error) };
        Some(reason)
    };
    if status != 0 {
        return Err(sandbox_error(format!(
            "Seatbelt rejected VM worker confinement: {}",
            reason.unwrap_or_else(|| format!("status {status}"))
        )));
    }
    Ok(())
}

fn apply_path_restrictions(
    profile: &mut String,
    parameters: &mut Vec<String>,
    source: &Path,
    restrictions: &aster_oci_sdk::StoragePathRestriction,
) -> Result<()> {
    for (paths, operations) in [
        (restrictions.deny_read(), "file-read*"),
        (restrictions.deny_write(), "file-write*"),
    ] {
        for path in paths {
            add_denied_rule(
                profile,
                parameters,
                operations,
                "subpath",
                &source.join(path).components().collect::<PathBuf>(),
            )?;
        }
    }
    for path in restrictions.write_exceptions() {
        add_path_rule(
            profile,
            parameters,
            "file-write*",
            "subpath",
            &source.join(path).components().collect::<PathBuf>(),
        )?;
    }
    let mut pinned = BTreeSet::new();
    for path in restrictions.deny_write() {
        let path: PathBuf = source.join(path).components().collect();
        pinned.extend(
            path.ancestors()
                .take_while(|path| path.starts_with(source))
                .map(Path::to_path_buf),
        );
    }
    for exception in restrictions.write_exceptions() {
        let exception = source.join(exception);
        pinned.retain(|path| !path.starts_with(&exception));
    }
    for path in pinned {
        add_denied_rule(
            profile,
            parameters,
            "file-write-create file-write-unlink",
            "literal",
            &path,
        )?;
    }
    let root = add_parameter(parameters, source)?;
    profile.push_str(&format!("(allow file-read-metadata (require-all (subpath (param \"{root}\")) (vnode-type DIRECTORY)))\n"));
    Ok(())
}

fn add_denied_rule(
    profile: &mut String,
    parameters: &mut Vec<String>,
    operations: &str,
    filter: &str,
    path: &Path,
) -> Result<()> {
    let parameter = add_parameter(parameters, path)?;
    profile.push_str(&format!(
        "(deny {operations} ({filter} (param \"{parameter}\")))\n"
    ));
    Ok(())
}

fn add_path_rule(
    profile: &mut String,
    parameters: &mut Vec<String>,
    operations: &str,
    filter: &str,
    path: &Path,
) -> Result<()> {
    let parameter = add_parameter(parameters, path)?;
    profile.push_str(&format!(
        "(allow {operations} ({filter} (param \"{parameter}\")))\n"
    ));
    Ok(())
}

fn add_parameter(parameters: &mut Vec<String>, path: &Path) -> Result<String> {
    if !path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        return Err(sandbox_error(
            "VM sandbox paths must be absolute and normalized",
        ));
    }
    let value = path
        .to_str()
        .ok_or_else(|| sandbox_error("VM sandbox path is not UTF-8"))?;
    let key = format!("PATH_{}", parameters.len() / 2);
    parameters.push(key.clone());
    parameters.push(value.to_string());
    Ok(key)
}

fn sandbox_error(reason: impl Into<String>) -> Error {
    Error::new(ErrorCode::FailedPrecondition, reason).for_operation(OPERATION)
}

#[cfg(test)]
#[path = "macos_worker_sandbox_tests.rs"]
mod tests;

fn overlaps(left: &Path, right: &Path) -> bool {
    left.starts_with(right) || right.starts_with(left)
}
