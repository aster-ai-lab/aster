//! PID-namespace init for one macOS guest command.
//!
//! The system-image builder installs this static binary at
//! `/usr/bin/aster-oci-agent`. The guest init execs
//! `aster-oci-agent --guest-command ./.aster-guest-command`. This process becomes
//! the VM's PID 1, then creates a new PID namespace whose init is the command
//! script. When that init exits, the kernel kills every remaining process in
//! the namespace before this parent observes the exit.

use std::ffi::CString;
use std::io::Error;
use std::os::unix::ffi::OsStrExt;
use std::process::exit;

const CLONE_NEWPID: i32 = 0x2000_0000;
const EINTR: i32 = 4;

unsafe extern "C" {
    fn unshare(flags: i32) -> i32;
    fn fork() -> i32;
    fn waitpid(pid: i32, status: *mut i32, options: i32) -> i32;
    fn execve(
        path: *const std::ffi::c_char,
        argv: *const *const std::ffi::c_char,
        envp: *const *const std::ffi::c_char,
    ) -> i32;
    static environ: *const *const std::ffi::c_char;
}

fn main() {
    let mut args = std::env::args_os();
    let _argv0 = args.next();
    let flag = args.next();
    let script = args.next();
    let Some(flag) = flag else {
        exit(2);
    };
    let Some(script) = script else {
        exit(2);
    };
    if flag != "--guest-command" || args.next().is_some() {
        exit(2);
    }

    let program = c_string("/bin/sh");
    let arg0 = c_string("sh");
    let script = match CString::new(script.as_bytes()) {
        Ok(script) => script,
        Err(_) => exit(127),
    };

    let rc = unsafe { unshare(CLONE_NEWPID) };
    if rc != 0 {
        exit(127);
    }
    let pid = unsafe { fork() };
    if pid < 0 {
        exit(127);
    }
    if pid == 0 {
        let argv = [arg0.as_ptr(), script.as_ptr(), std::ptr::null()];
        unsafe {
            execve(program.as_ptr(), argv.as_ptr(), environ);
        }
        exit(127);
    }

    let mut status = 0;
    loop {
        let waited = unsafe { waitpid(pid, &mut status, 0) };
        if waited == pid {
            break;
        }
        if waited < 0 {
            let err = Error::last_os_error().raw_os_error();
            if err == Some(EINTR) {
                continue;
            }
            exit(127);
        }
    }
    if status & 0x7f == 0 {
        exit((status >> 8) & 0xff);
    }
    exit(128 + (status & 0x7f));
}

fn c_string(value: &str) -> CString {
    match CString::new(value) {
        Ok(value) => value,
        Err(_) => exit(127),
    }
}
