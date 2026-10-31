use super::{mount_rootfs_maintenance_device, unmount_rootfs_maintenance_device, BootstrapMode};
use aster_box_guest_init::exec_server;
use std::sync::atomic::{AtomicI32, Ordering};
use tracing::{error, info, warn};

/// Signal forwarded to the container process group during graceful shutdown.
/// Zero means no shutdown has been requested.
static SHUTDOWN_SIGNAL: AtomicI32 = AtomicI32::new(0);

pub(super) fn has_isolated_shutdown_scope(mode: BootstrapMode) -> bool {
    isolated_shutdown_scope(
        mode,
        nix::unistd::getpid().as_raw(),
        nix::unistd::getppid().as_raw(),
        std::fs::read_link("/proc/1/exe").ok().as_deref(),
    )
}

pub(super) fn isolated_shutdown_scope(
    mode: BootstrapMode,
    pid: i32,
    parent: i32,
    init: Option<&std::path::Path>,
) -> bool {
    pid == 1
        || (mode == BootstrapMode::Microvm
            && parent == 1
            && init == Some(std::path::Path::new("/init.krun")))
}

pub(super) fn register_child_subreaper(
    mode: BootstrapMode,
) -> Result<(), Box<dyn std::error::Error>> {
    if !has_isolated_shutdown_scope(mode) {
        return Err("guest supervision requires an isolated process namespace".into());
    }
    if unsafe { libc::prctl(libc::PR_SET_CHILD_SUBREAPER, 1, 0, 0, 0) } != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(())
}

pub(super) fn request_shutdown(signal: i32) {
    let signal = if (1..=64).contains(&signal) {
        signal
    } else {
        libc::SIGTERM
    };
    let _ = SHUTDOWN_SIGNAL.compare_exchange(0, signal, Ordering::SeqCst, Ordering::SeqCst);
}

pub(super) fn shutdown_signal() -> i32 {
    SHUTDOWN_SIGNAL.load(Ordering::SeqCst)
}

/// Register a SIGTERM handler that sets the shutdown flag.
///
/// As PID 1 inside the VM, we must explicitly handle SIGTERM — the kernel
/// does not deliver unhandled signals to init. When the host kills the shim
/// process, libkrun triggers a guest shutdown and the kernel sends SIGTERM
/// to PID 1.
pub(super) fn register_sigterm_handler() -> Result<(), Box<dyn std::error::Error>> {
    use nix::sys::signal::{sigaction, SaFlags, SigAction, SigHandler, SigSet, Signal};

    let handler = SigHandler::Handler(sigterm_handler);
    let action = SigAction::new(handler, SaFlags::empty(), SigSet::empty());
    unsafe { sigaction(Signal::SIGTERM, &action)? };
    info!("Registered SIGTERM handler");
    Ok(())
}

extern "C" fn sigterm_handler(signal: libc::c_int) {
    request_shutdown(signal);
}

pub(super) fn run_rootfs_maintenance() -> Result<(), Box<dyn std::error::Error>> {
    register_sigterm_handler()
        .map_err(|error| format!("failed to register maintenance signal handler: {error}"))?;
    mount_rootfs_maintenance_device()
        .map_err(|error| format!("failed to mount maintenance rootfs: {error}"))?;
    let listener = exec_server::bind_exec_server()
        .map_err(|error| format!("failed to bind maintenance control server: {error}"))?;
    std::thread::spawn(move || {
        if let Err(error) = exec_server::serve_rootfs_maintenance_server(listener, request_shutdown)
        {
            error!(%error, "Rootfs maintenance server failed");
            request_shutdown(libc::SIGTERM);
        }
    });
    info!("Trusted read-only rootfs maintenance guest is ready");

    let idle_deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while shutdown_signal() == 0 {
        if std::time::Instant::now() >= idle_deadline
            && exec_server::begin_rootfs_maintenance_idle_shutdown()
        {
            warn!("Rootfs maintenance guest reached its idle lifetime; shutting down");
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    unmount_rootfs_maintenance_device()
        .map_err(|error| format!("failed to unmount maintenance rootfs: {error}"))?;
    info!("Rootfs maintenance guest shut down cleanly");
    Ok(())
}
