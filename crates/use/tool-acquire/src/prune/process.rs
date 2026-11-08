//! Native executable and argument paths, retaining OS path units.
//! Platform queries follow mise v2026.10.25 (MIT); see licenses/mise-MIT.txt.

use std::io;
use std::path::PathBuf;

use sysinfo::{ProcessRefreshKind, RefreshKind, System, UpdateKind};

use crate::ToolAcquireError;

#[cfg(any(target_os = "linux", target_os = "macos"))]
const MAX_ARGUMENT_BYTES: usize = 2 * 1024 * 1024;

pub(super) fn running_paths() -> Result<Vec<PathBuf>, ToolAcquireError> {
    if !sysinfo::IS_SUPPORTED_SYSTEM {
        return Err(ToolAcquireError::InvalidEnvTracking(
            "native process inspection is unsupported on this platform".into(),
        ));
    }
    let refresh = ProcessRefreshKind::new().with_exe(if cfg!(target_os = "macos") {
        UpdateKind::Never
    } else {
        UpdateKind::Always
    });
    let system = System::new_with_specifics(RefreshKind::new().with_processes(refresh));
    let mut paths = Vec::new();
    for (pid, process) in system.processes() {
        #[cfg(not(target_os = "macos"))]
        if let Some(exe) = process.exe() {
            paths.push(exe.to_path_buf());
        }
        #[cfg(target_os = "macos")]
        {
            let _ = process;
            if let Some(exe) = macos_executable(pid.as_u32()) {
                paths.push(exe);
            }
        }
        match argument_paths(pid.as_u32()) {
            Ok(args) => paths.extend(args),
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::NotFound
                        | io::ErrorKind::PermissionDenied
                        | io::ErrorKind::InvalidInput
                ) => {}
            Err(source) => {
                return Err(ToolAcquireError::ProcessInspection {
                    pid: pid.as_u32(),
                    source,
                })
            }
        }
    }
    Ok(paths
        .into_iter()
        .map(|path| path.canonicalize().unwrap_or(path))
        .collect())
}

#[cfg(target_os = "macos")]
fn macos_executable(pid: u32) -> Option<PathBuf> {
    use std::os::unix::ffi::OsStrExt;
    let mut bytes = vec![0u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];
    // SAFETY: the live writable buffer has exactly the byte capacity passed to libproc.
    let length = unsafe {
        libc::proc_pidpath(
            pid as libc::c_int,
            bytes.as_mut_ptr().cast(),
            bytes.len() as u32,
        )
    };
    let length = usize::try_from(length)
        .ok()
        .filter(|length| *length > 0 && *length <= bytes.len())?;
    let bytes = bytes[..length]
        .strip_suffix(&[0])
        .unwrap_or(&bytes[..length]);
    Some(PathBuf::from(std::ffi::OsStr::from_bytes(bytes)))
}

#[cfg(target_os = "linux")]
fn argument_paths(pid: u32) -> io::Result<Vec<PathBuf>> {
    use std::io::Read;
    let path = std::path::Path::new("/proc")
        .join(pid.to_string())
        .join("cmdline");
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(MAX_ARGUMENT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_ARGUMENT_BYTES {
        return Err(invalid_arguments());
    }
    Ok(unix_paths(bytes.split(|byte| *byte == 0)))
}

#[cfg(target_os = "macos")]
fn argument_paths(pid: u32) -> io::Result<Vec<PathBuf>> {
    let mut mib = [libc::CTL_KERN, libc::KERN_PROCARGS2, pid as libc::c_int];
    let mut size = 0usize;
    // SAFETY: the fixed MIB and writable size are live; a null old buffer queries its size.
    let result = unsafe {
        libc::sysctl(
            mib.as_mut_ptr(),
            mib.len() as u32,
            std::ptr::null_mut(),
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    };
    if result != 0 {
        return Err(process_error());
    }
    if size > MAX_ARGUMENT_BYTES {
        return Err(invalid_arguments());
    }
    if size == 0 {
        return Ok(Vec::new());
    }
    let mut bytes = vec![0u8; size];
    // SAFETY: the initialized buffer is size bytes, and sysctl receives that exact capacity.
    let result = unsafe {
        libc::sysctl(
            mib.as_mut_ptr(),
            mib.len() as u32,
            bytes.as_mut_ptr().cast(),
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    };
    if result != 0 {
        return Err(process_error());
    }
    if size > bytes.len() {
        return Err(invalid_arguments());
    }
    bytes.truncate(size);
    macos_paths(&bytes)
}

#[cfg(target_os = "macos")]
fn macos_paths(bytes: &[u8]) -> io::Result<Vec<PathBuf>> {
    let (argc, rest) = bytes.split_at_checked(4).ok_or_else(invalid_arguments)?;
    let argc = i32::from_ne_bytes(argc.try_into().map_err(|_| invalid_arguments())?);
    let argc = usize::try_from(argc).map_err(|_| invalid_arguments())?;
    let mut fields = rest.split(|byte| *byte == 0);
    fields.next();
    Ok(unix_paths(
        fields.skip_while(|field| field.is_empty()).take(argc),
    ))
}

#[cfg(target_os = "macos")]
fn process_error() -> io::Error {
    let error = io::Error::last_os_error();
    if error.raw_os_error() == Some(libc::ESRCH) {
        io::Error::new(io::ErrorKind::NotFound, error)
    } else {
        error
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn unix_paths<'a>(fields: impl Iterator<Item = &'a [u8]>) -> Vec<PathBuf> {
    use std::os::unix::ffi::OsStrExt;
    fields
        .map(|field| PathBuf::from(std::ffi::OsStr::from_bytes(field)))
        .filter(|path| path.is_absolute())
        .collect()
}

#[cfg(windows)]
fn argument_paths(pid: u32) -> io::Result<Vec<PathBuf>> {
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Wdk::System::Threading::NtQueryInformationProcess;
    use windows_sys::Win32::Foundation::{CloseHandle, LocalFree, RtlNtStatusToDosError, HANDLE};
    use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
    use windows_sys::Win32::UI::Shell::CommandLineToArgvW;

    struct Handle(HANDLE);
    impl Drop for Handle {
        fn drop(&mut self) {
            // SAFETY: this handle is owned by the successful OpenProcess below.
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
    struct Arguments(*mut *mut u16);
    impl Drop for Arguments {
        fn drop(&mut self) {
            // SAFETY: CommandLineToArgvW returned this single LocalAlloc allocation.
            unsafe {
                LocalFree(self.0.cast());
            }
        }
    }
    // SAFETY: OpenProcess accepts a numeric PID and returns an owned handle or null.
    let raw = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if raw.is_null() {
        return Err(io::Error::last_os_error());
    }
    let handle = Handle(raw);
    // UNICODE_STRING uses a pointer-aligned header; its u16 byte length bounds
    // the UTF-16 payload below 64 KiB. Keep the whole response pointer-aligned.
    let mut words = vec![0usize; 128 * 1024 / std::mem::size_of::<usize>()];
    let capacity = std::mem::size_of_val(words.as_slice());
    let mut returned = 0u32;
    // SAFETY: the live aligned buffer has capacity bytes and returned is writable.
    let status = unsafe {
        NtQueryInformationProcess(
            handle.0,
            60,
            words.as_mut_ptr().cast(),
            capacity as u32,
            &mut returned,
        )
    };
    if status < 0 {
        // SAFETY: this converts the returned NTSTATUS without dereferencing memory.
        return Err(io::Error::from_raw_os_error(
            unsafe { RtlNtStatusToDosError(status) } as i32,
        ));
    }
    let header = words[0].to_ne_bytes();
    let length = u16::from_ne_bytes([header[0], header[1]]) as usize;
    if length == 0 {
        return Ok(Vec::new());
    }
    if length % 2 != 0 || returned as usize > capacity {
        return Err(invalid_arguments());
    }
    let offset = words[1]
        .checked_sub(words.as_ptr() as usize)
        .ok_or_else(invalid_arguments)?;
    let end = offset.checked_add(length).ok_or_else(invalid_arguments)?;
    // SAFETY: initialized usize storage can be viewed as bytes for its exact allocation size.
    let bytes = unsafe { std::slice::from_raw_parts(words.as_ptr().cast::<u8>(), capacity) };
    let payload = bytes.get(offset..end).ok_or_else(invalid_arguments)?;
    let mut line = payload
        .chunks_exact(2)
        .map(|unit| u16::from_ne_bytes([unit[0], unit[1]]))
        .collect::<Vec<_>>();
    line.push(0);
    let mut count = 0i32;
    // SAFETY: line is live and NUL-terminated, and count is writable.
    let raw = unsafe { CommandLineToArgvW(line.as_ptr(), &mut count) };
    if raw.is_null() {
        return Err(io::Error::last_os_error());
    }
    let arguments = Arguments(raw);
    let count = usize::try_from(count).map_err(|_| invalid_arguments())?;
    if count > line.len() {
        return Err(invalid_arguments());
    }
    // SAFETY: the API returns count argument pointers in this still-owned allocation.
    let pointers = unsafe { std::slice::from_raw_parts(arguments.0, count) };
    let mut paths = Vec::new();
    for &pointer in pointers {
        if pointer.is_null() {
            return Err(invalid_arguments());
        }
        let mut units = Vec::new();
        for index in 0..line.len() {
            // SAFETY: each API-owned argument is NUL-terminated and no longer than its input.
            let unit = unsafe { *pointer.add(index) };
            if unit == 0 {
                break;
            }
            units.push(unit);
        }
        let path = PathBuf::from(std::ffi::OsString::from_wide(&units));
        if path.is_absolute() {
            paths.push(path);
        }
    }
    Ok(paths)
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
fn argument_paths(_: u32) -> io::Result<Vec<PathBuf>> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "native process arguments are unavailable",
    ))
}

fn invalid_arguments() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "invalid or oversized native process arguments",
    )
}

#[cfg(test)]
mod tests;
