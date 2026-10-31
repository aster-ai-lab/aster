//! ShimHandler — concrete VmHandler for a libkrun shim subprocess.

pub use aster_box_core::vmm::{VmHandler, VmMetrics, DEFAULT_SHUTDOWN_TIMEOUT_MS};

use aster_box_core::error::{BoxError, Result};
use aster_box_core::vmm::{OwnedCloseBudget, VmExitConfirmation};
use std::process::{Child, ExitStatus};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use sysinfo::{Pid, System};

#[cfg(windows)]
fn wait_then_terminate_attached_process(
    pid: u32,
    expected_start_time: u64,
    box_id: &str,
    timeout_ms: u64,
) -> Result<()> {
    use windows_sys::Win32::Foundation::{
        CloseHandle, FILETIME, WAIT_FAILED, WAIT_OBJECT_0, WAIT_TIMEOUT,
    };
    use windows_sys::Win32::System::Threading::{
        GetProcessTimes, OpenProcess, TerminateProcess, WaitForSingleObject,
        PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE, PROCESS_TERMINATE,
    };

    const TERMINATION_WAIT_MS: u32 = 5_000;
    const MAX_FINITE_WAIT_MS: u64 = (u32::MAX - 1) as u64;

    // SAFETY: OpenProcess returns our own handle, closed below. Identity is
    // checked on that same process object before waiting or terminating it.
    let handle = unsafe {
        OpenProcess(
            PROCESS_TERMINATE | PROCESS_SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION,
            0,
            pid,
        )
    };
    if handle == 0 {
        let error = std::io::Error::last_os_error();
        return Err(BoxError::ExecError(format!(
            "Failed to open attached VM process {pid} for box {box_id}: {error}"
        )));
    }

    let result = (|| {
        let mut creation = FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        };
        let mut exit = creation;
        let mut kernel = creation;
        let mut user = creation;
        // SAFETY: The handle is valid until the CloseHandle below; all output
        // pointers refer to initialized local FILETIME values.
        if unsafe { GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user) } == 0
        {
            return Err(BoxError::ExecError(format!(
                "Failed to query attached VM process {pid} identity for box {box_id}: {}",
                std::io::Error::last_os_error()
            )));
        }
        let actual_start_time =
            ((creation.dwHighDateTime as u64) << 32) | creation.dwLowDateTime as u64;
        if actual_start_time == 0 || actual_start_time != expected_start_time {
            return Err(BoxError::StateError(format!(
                "Attached VM process {pid} identity mismatch for box {box_id}"
            )));
        }
        let graceful_wait_ms = timeout_ms.min(MAX_FINITE_WAIT_MS) as u32;
        // SAFETY: See the enclosing function's safety documentation.
        match unsafe { WaitForSingleObject(handle, graceful_wait_ms) } {
            WAIT_OBJECT_0 => return Ok(()),
            WAIT_TIMEOUT => tracing::warn!(
                pid,
                box_id,
                timeout_ms,
                "Attached VM process did not exit after the guest stop request; forcing termination"
            ),
            WAIT_FAILED => {
                return Err(BoxError::ExecError(format!(
                    "Failed while waiting for attached VM process {pid} for box {box_id}: {}",
                    std::io::Error::last_os_error()
                )));
            }
            status => {
                return Err(BoxError::ExecError(format!(
                    "Unexpected wait status {status} for attached VM process {pid} of box {box_id}"
                )));
            }
        }

        // SAFETY: See the enclosing function's safety documentation.
        if unsafe { TerminateProcess(handle, 1) } == 0 {
            let error = std::io::Error::last_os_error();
            return Err(BoxError::ExecError(format!(
                "Failed to terminate attached VM process {pid} for box {box_id}: {error}"
            )));
        }

        // SAFETY: See the enclosing function's safety documentation.
        match unsafe { WaitForSingleObject(handle, TERMINATION_WAIT_MS) } {
            WAIT_OBJECT_0 => Ok(()),
            WAIT_TIMEOUT => Err(BoxError::ExecError(format!(
                "Attached VM process {pid} for box {box_id} did not exit after termination"
            ))),
            WAIT_FAILED => Err(BoxError::ExecError(format!(
                "Failed to wait for attached VM process {pid} for box {box_id}: {}",
                std::io::Error::last_os_error()
            ))),
            status => Err(BoxError::ExecError(format!(
                "Unexpected wait status {status} for attached VM process {pid} of box {box_id}"
            ))),
        }
    })();
    // SAFETY: See the enclosing function's safety documentation.
    unsafe {
        CloseHandle(handle);
    }
    result
}

// Private seam for the original owned Child only. A force wait is a sequence
// of nonblocking try_wait calls: a blocking Child::wait would outlive the budget.
trait OwnedChildOps {
    fn now(&mut self) -> Instant;
    fn sleep(&mut self, duration: Duration);
    fn poll(&mut self, process: &mut Child) -> std::io::Result<Option<ExitStatus>>;
    fn signal(&mut self, process: &mut Child, signal: i32) -> std::io::Result<()>;
    fn force_kill(&mut self, process: &mut Child) -> std::io::Result<()>;
}

struct SystemOwnedChildOps;

impl OwnedChildOps for SystemOwnedChildOps {
    fn now(&mut self) -> Instant {
        Instant::now()
    }

    fn sleep(&mut self, duration: Duration) {
        std::thread::sleep(duration);
    }

    fn poll(&mut self, process: &mut Child) -> std::io::Result<Option<ExitStatus>> {
        process.try_wait()
    }

    fn signal(&mut self, process: &mut Child, signal: i32) -> std::io::Result<()> {
        #[cfg(unix)]
        {
            // SAFETY: this PID belongs to the retained, unreaped original Child.
            // The preceding try_wait did not reap it, so its PID is not reusable.
            if unsafe { libc::kill(process.id() as i32, signal) } == -1 {
                return Err(std::io::Error::last_os_error());
            }
        }
        #[cfg(windows)]
        {
            // Windows has no equivalent stop signal. Preserve the existing
            // owned path's grace for the guest stop request before Child::kill.
            let _ = (process, signal);
        }
        Ok(())
    }

    fn force_kill(&mut self, process: &mut Child) -> std::io::Result<()> {
        process.kill()
    }
}

fn owned_deadline_error(pid: u32) -> BoxError {
    BoxError::ExecError(format!(
        "Owned VM process {pid} did not complete its wait within the original physical deadline"
    ))
}

fn require_owned_time(pid: u32, budget: OwnedCloseBudget, now: Instant) -> Result<()> {
    if budget.remaining_physical_at(now).is_zero() {
        return Err(owned_deadline_error(pid));
    }
    Ok(())
}

fn stop_owned_child(
    process: &mut Child,
    signal: i32,
    budget: OwnedCloseBudget,
    ops: &mut impl OwnedChildOps,
) -> Result<ExitStatus> {
    const POLL_INTERVAL: Duration = Duration::from_millis(50);
    let pid = process.id();
    require_owned_time(pid, budget, ops.now())?;
    // Always consult the owned Child before signalling. Host liveness and an
    // optional numeric exit code are not wait/reap evidence, including races.
    if let Some(status) = ops.poll(process).map_err(|error| {
        BoxError::ExecError(format!("Failed to poll owned VM process {pid}: {error}"))
    })? {
        require_owned_time(pid, budget, ops.now())?;
        return Ok(status);
    }
    require_owned_time(pid, budget, ops.now())?;
    ops.signal(process, signal).map_err(|error| {
        BoxError::ExecError(format!("Failed to signal owned VM process {pid}: {error}"))
    })?;

    let mut forced = false;
    loop {
        require_owned_time(pid, budget, ops.now())?;
        let status = ops.poll(process).map_err(|error| {
            let action = if forced { "wait for" } else { "poll" };
            BoxError::ExecError(format!(
                "Failed to {action} owned VM process {pid}: {error}"
            ))
        })?;
        if let Some(status) = status {
            // A late OS result is still retained by Child, but it cannot turn
            // an expired original attempt into a successful bounded close.
            require_owned_time(pid, budget, ops.now())?;
            return Ok(status);
        }
        let now = ops.now();
        require_owned_time(pid, budget, now)?;
        if !forced && budget.remaining_grace_at(now).is_zero() {
            ops.force_kill(process).map_err(|error| {
                BoxError::ExecError(format!("Failed to kill owned VM process {pid}: {error}"))
            })?;
            forced = true;
            continue;
        }
        let remaining = if forced {
            budget.remaining_physical_at(now)
        } else {
            budget.remaining_grace_at(now)
        };
        ops.sleep(POLL_INTERVAL.min(remaining));
    }
}

/// Handler for a running VM subprocess (shim process).
///
/// Provides lifecycle operations (stop, metrics, status) for a VM identified by PID.
pub struct ShimHandler {
    pid: u32,
    /// Stable host-process identity captured when the handler is created.
    pid_start_time: Option<u64>,
    box_id: String,
    /// Child process handle for proper lifecycle management.
    /// When we spawn the process, we keep the Child to properly wait() on stop.
    /// When we attach to an existing process, this is None.
    process: Option<Child>,
    /// Shared System instance for CPU metrics calculation across calls.
    /// CPU usage requires comparing snapshots over time, so we must reuse the same System.
    metrics_sys: Mutex<System>,
    /// Exit code of the shim process, set when stop() collects the exit status.
    exit_code: Option<i32>,
    /// Actual owned wait status, including signal exits without a numeric code.
    exit_confirmation: Option<VmExitConfirmation>,
}

impl ShimHandler {
    /// Create a handler for a spawned VM with process ownership.
    ///
    /// This constructor takes ownership of the Child process handle for proper
    /// lifecycle management (clean shutdown with wait()).
    pub fn from_child(process: Child, box_id: String) -> Self {
        let pid = process.id();
        Self {
            pid,
            pid_start_time: crate::process::pid_start_time(pid),
            box_id,
            process: Some(process),
            metrics_sys: Mutex::new(System::new()),
            exit_code: None,
            exit_confirmation: None,
        }
    }

    /// Create a handler for an existing VM (attach mode).
    ///
    /// Used when reconnecting to a running box. We don't have a Child handle,
    /// so we manage the process by PID only.
    pub fn from_pid(pid: u32, box_id: String) -> Self {
        Self {
            pid,
            pid_start_time: crate::process::pid_start_time(pid),
            box_id,
            process: None,
            metrics_sys: Mutex::new(System::new()),
            exit_code: None,
            exit_confirmation: None,
        }
    }

    /// Bind an attached handler to the original record token, without turning
    /// a fresh process query into ownership or termination evidence.
    pub(crate) fn from_recorded_pid(pid: u32, pid_start_time: u64, box_id: String) -> Result<Self> {
        if pid == 0 {
            return Err(BoxError::StateError(
                "recorded attached process requires a nonzero PID".to_string(),
            ));
        }
        Ok(Self {
            pid,
            pid_start_time: Some(pid_start_time),
            box_id,
            process: None,
            metrics_sys: Mutex::new(System::new()),
            exit_code: None,
            exit_confirmation: None,
        })
    }

    fn remember_owned_exit(&mut self, status: ExitStatus) -> Result<()> {
        let confirmation = VmExitConfirmation::owned_reaped(self.pid, self.pid_start_time, status)?;
        self.exit_confirmation = Some(confirmation);
        self.exit_code = status.code();
        Ok(())
    }

    fn stop_owned_with(
        &mut self,
        signal: i32,
        budget: OwnedCloseBudget,
        ops: &mut impl OwnedChildOps,
    ) -> Result<()> {
        // Expiry is checked even when a later observation has established exit.
        // Reusing an expired attempt never manufactures a new force allowance.
        require_owned_time(self.pid, budget, ops.now())?;
        if self.exit_confirmation.is_some() {
            return Ok(());
        }
        let process = self.process.as_mut().ok_or_else(|| {
            BoxError::StateError("owned-close budget requires the original Child".to_string())
        })?;
        // Borrow in place. No error, timeout or caller unwind takes the only
        // Child out of the handler, and successful reaping remains idempotent.
        let status = stop_owned_child(process, signal, budget, ops)?;
        self.remember_owned_exit(status)
    }

    /// Get the box ID.
    pub fn box_id(&self) -> &str {
        &self.box_id
    }
}

impl VmHandler for ShimHandler {
    fn pid(&self) -> u32 {
        self.pid
    }

    #[cfg(unix)]
    fn stop(&mut self, signal: i32, timeout_ms: u64) -> Result<()> {
        if self.process.is_some() {
            let budget = OwnedCloseBudget::from_admission(
                Instant::now(),
                Duration::from_millis(timeout_ms),
            )?;
            return self.stop_with_budget(signal, budget);
        }
        if !self.is_running() {
            // Legacy compatibility only: absence/unknown is not a physical
            // confirmation and must never qualify an admitted close.
            return Ok(());
        }
        // Checking a start token then signalling a numeric PID is not atomic.
        // This handler has no trusted identity-atomic Unix signal capability.
        Err(BoxError::StateError(format!(
            "Cannot stop live attached VM process {} for box {} without identity-atomic signalling",
            self.pid, self.box_id
        )))
    }

    #[cfg(windows)]
    fn stop(&mut self, _signal: i32, timeout_ms: u64) -> Result<()> {
        if self.process.is_some() {
            let budget = OwnedCloseBudget::from_admission(
                Instant::now(),
                Duration::from_millis(timeout_ms),
            )?;
            return self.stop_with_budget(_signal, budget);
        }
        // Every CLI invocation reconstructs the managed runtime from its
        // durable PID, so normal `stop` reaches this attached path. The VM
        // manager has already sent the guest stop request. Hold a handle to
        // this exact process object while waiting, then force-terminate it
        // on timeout so PID reuse cannot target an unrelated process.
        tracing::debug!(pid = self.pid, box_id = %self.box_id, timeout_ms, "Waiting for attached VM process to stop");
        let expected_start_time = self.pid_start_time.ok_or_else(|| {
            BoxError::StateError(format!(
                "Attached VM process {} for box {} has no original start token",
                self.pid, self.box_id
            ))
        })?;
        wait_then_terminate_attached_process(
            self.pid,
            expected_start_time,
            &self.box_id,
            timeout_ms,
        )
    }

    fn stop_with_budget(&mut self, signal: i32, budget: OwnedCloseBudget) -> Result<()> {
        if self.process.is_none() {
            return Err(BoxError::StateError(
                "admitted attached close requires original generation and stable identity control"
                    .to_string(),
            ));
        }
        self.stop_owned_with(signal, budget, &mut SystemOwnedChildOps)
    }

    fn exit_confirmation(&self) -> Option<VmExitConfirmation> {
        self.exit_confirmation
    }

    fn metrics(&self) -> VmMetrics {
        if !self.is_running() {
            return VmMetrics::default();
        }

        let pid = Pid::from_u32(self.pid);

        // Use the shared System instance for stateful CPU tracking
        let mut sys = match self.metrics_sys.lock() {
            Ok(guard) => guard,
            Err(e) => {
                tracing::warn!(error = %e, "metrics_sys lock poisoned");
                return VmMetrics::default();
            }
        };

        // Refresh process info - this updates the internal state for delta calculation
        sys.refresh_process(pid);

        // Try to get process information
        if let Some(proc_info) = sys.process(pid) {
            return VmMetrics {
                cpu_percent: Some(proc_info.cpu_usage()),
                memory_bytes: Some(proc_info.memory()),
            };
        }

        // Process not found or not running - return empty metrics
        VmMetrics::default()
    }

    #[cfg(unix)]
    fn is_running(&self) -> bool {
        if self.exit_confirmation.is_some() {
            return false;
        }
        crate::process::is_process_alive_with_identity(self.pid, self.pid_start_time)
    }

    #[cfg(windows)]
    fn is_running(&self) -> bool {
        if self.exit_confirmation.is_some() {
            return false;
        }
        crate::process::is_process_alive_with_identity(self.pid, self.pid_start_time)
    }

    fn exit_code(&self) -> Option<i32> {
        self.exit_code
    }

    fn try_wait_exit(&mut self) -> Result<Option<i32>> {
        if self.exit_confirmation.is_some() {
            return Ok(self.exit_code);
        }

        let Some(process) = self.process.as_mut() else {
            return Ok(None);
        };

        match process.try_wait() {
            Ok(Some(status)) => {
                self.remember_owned_exit(status)?;
                Ok(self.exit_code)
            }
            Ok(None) => Ok(None),
            Err(e) => Err(aster_box_core::error::BoxError::ExecError(format!(
                "Failed to poll VM process {}: {}",
                self.pid, e
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vm_metrics_default() {
        let m = VmMetrics::default();
        assert!(m.cpu_percent.is_none());
        assert!(m.memory_bytes.is_none());
    }

    #[test]
    fn test_vm_metrics_clone() {
        let m = VmMetrics {
            cpu_percent: Some(50.0),
            memory_bytes: Some(1024 * 1024),
        };
        let cloned = m.clone();
        assert_eq!(cloned.cpu_percent, Some(50.0));
        assert_eq!(cloned.memory_bytes, Some(1024 * 1024));
    }

    #[test]
    fn test_shim_handler_from_pid() {
        let handler = ShimHandler::from_pid(12345, "box-abc".to_string());
        assert_eq!(handler.pid(), 12345);
        assert_eq!(handler.box_id(), "box-abc");
        assert_eq!(handler.exit_code(), None);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn attached_handler_rejects_a_reused_pid_identity() {
        let mut child = std::process::Command::new("sleep")
            .arg("30")
            .spawn()
            .unwrap();
        let mut handler = ShimHandler::from_pid(child.id(), "box-stale-pid".to_string());
        handler.pid_start_time = Some(u64::MAX);

        assert!(!handler.is_running());
        let metrics = handler.metrics();
        assert!(metrics.cpu_percent.is_none());
        assert!(metrics.memory_bytes.is_none());
        handler.stop(libc::SIGTERM, 0).unwrap();
        assert!(child.try_wait().unwrap().is_none());

        let _ = child.kill();
        let _ = child.wait();
    }

    #[test]
    fn test_shim_handler_try_wait_exit_captures_child_exit_code() {
        #[cfg(windows)]
        let child = std::process::Command::new("cmd")
            .args(["/C", "exit 7"])
            .spawn()
            .unwrap();
        #[cfg(not(windows))]
        let child = std::process::Command::new("sh")
            .arg("-c")
            .arg("exit 7")
            .spawn()
            .unwrap();
        let mut handler = ShimHandler::from_child(child, "box-child-exit".to_string());

        let exit_code = loop {
            if let Some(code) = handler.try_wait_exit().unwrap() {
                break code;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        };

        assert_eq!(exit_code, 7);
        assert_eq!(handler.exit_code(), Some(7));
        assert_eq!(handler.try_wait_exit().unwrap(), Some(7));
    }

    #[cfg(unix)]
    #[test]
    fn test_shim_handler_stop_terminates_owned_child() {
        let child = std::process::Command::new("sh")
            .arg("-c")
            .arg("trap 'exit 0' TERM; while :; do sleep 0.05; done")
            .spawn()
            .unwrap();
        let mut handler = ShimHandler::from_child(child, "box-child-stop".to_string());

        assert!(handler.is_running());
        handler.stop(libc::SIGTERM, 2_000).unwrap();

        assert!(!handler.is_running());
        let exit_code = handler.exit_code();
        assert!(matches!(exit_code, None | Some(0)));
        assert_eq!(handler.try_wait_exit().unwrap(), exit_code);
    }

    #[cfg(unix)]
    #[test]
    fn test_shim_handler_stop_attached_missing_pid_is_ok() {
        let mut handler = ShimHandler::from_pid(999_999_999, "missing".to_string());

        handler.stop(libc::SIGTERM, 10).unwrap();

        assert!(!handler.is_running());
        assert_eq!(handler.exit_code(), None);
    }

    #[cfg(windows)]
    #[test]
    fn test_shim_handler_stop_terminates_attached_child() {
        let mut child = std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-Command", "Start-Sleep -Seconds 30"])
            .spawn()
            .unwrap();
        let mut handler = ShimHandler::from_pid(child.id(), "box-attached-stop".to_string());

        assert!(handler.is_running());
        let stop_result = handler.stop(15, 0);
        let exited = child.try_wait().unwrap().is_some();
        if !exited {
            let _ = child.kill();
            let _ = child.wait();
        }

        stop_result.unwrap();
        assert!(exited, "attached Windows process survived handler.stop()");
        assert!(!handler.is_running());
    }

    #[cfg(windows)]
    #[test]
    fn test_shim_handler_stop_allows_attached_child_to_exit_gracefully() {
        let mut child = std::process::Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-Command",
                "Start-Sleep -Milliseconds 150; exit 0",
            ])
            .spawn()
            .unwrap();
        let mut handler = ShimHandler::from_pid(child.id(), "box-attached-graceful".to_string());

        handler.stop(15, 2_000).unwrap();
        let status = child.wait().unwrap();

        assert!(status.success(), "graceful child was force-terminated");
        assert!(!handler.is_running());
    }

    #[test]
    fn test_shim_handler_is_running_nonexistent_pid() {
        // PID 999999999 should not exist
        let handler = ShimHandler::from_pid(999_999_999, "test".to_string());
        assert!(!handler.is_running());
    }

    #[test]
    fn test_shim_handler_metrics_nonexistent_pid() {
        let handler = ShimHandler::from_pid(999_999_999, "test".to_string());
        let m = handler.metrics();
        // Non-existent process should return default metrics
        assert!(m.cpu_percent.is_none() || m.cpu_percent == Some(0.0));
    }

    #[test]
    fn test_shim_handler_is_running_current_process() {
        // Current process PID should be running
        let pid = std::process::id();
        let handler = ShimHandler::from_pid(pid, "self".to_string());
        assert!(handler.is_running());
    }

    #[test]
    fn test_default_shutdown_timeout() {
        assert_eq!(DEFAULT_SHUTDOWN_TIMEOUT_MS, 10_000);
    }

    #[test]
    fn test_vm_metrics_debug() {
        let m = VmMetrics {
            cpu_percent: Some(25.5),
            memory_bytes: Some(512),
        };
        let debug = format!("{:?}", m);
        assert!(debug.contains("25.5"));
        assert!(debug.contains("512"));
    }
}

#[cfg(test)]
#[path = "handler_owned_exit_tests.rs"]
mod owned_exit_tests;

#[cfg(test)]
#[path = "handler_attached_exit_tests.rs"]
mod attached_exit_tests;
