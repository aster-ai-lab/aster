//! Runtime handler for a live Aster OCI Sandbox container.

use std::path::{Path, PathBuf};
use std::process::Child;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use aster_box_core::error::{BoxError, Result};
use aster_box_core::vmm::{VmHandler, VmMetrics};
use aster_oci_sdk::{
    ContainerId, ContainerOperationRequest, ContainerRecord, ContainerTarget, DeleteMode,
    DeleteRequest, DriverKind, ExitStatus, Generation, KillRequest, LinuxResources,
    OciContainerState, OperationContext, OperationId, Signal, StateRequest, StatsRequest,
    UpdateRequest, WaitRequest,
};
use sysinfo::{Pid, System};

use super::aster_oci_client::AsterOciClient;

const SIGKILL_NUMBER: i32 = 9;
const LIFECYCLE_POLL_INTERVAL: Duration = Duration::from_millis(25);
const CLEANUP_RETRY_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug)]
pub(crate) struct AsterOciState {
    pub(crate) status: String,
    pub(crate) pid: u32,
}

pub(crate) struct AsterOciHandlerSpec {
    pub(crate) runtime_socket: PathBuf,
    pub(crate) runtime_root: PathBuf,
    pub(crate) container_id: ContainerId,
    pub(crate) generation: Generation,
    pub(crate) init_pid: u32,
    pub(crate) owner_pid: u32,
    pub(crate) owner_pid_start_time: u64,
    pub(crate) bundle_dir: PathBuf,
    pub(crate) runtime_record: PathBuf,
}

/// Owns one exact Aster OCI generation and its detached runtime owner.
pub struct AsterOciHandler {
    client: AsterOciClient,
    target: ContainerTarget,
    init_pid: u32,
    owner: Option<Child>,
    owner_pid: u32,
    owner_pid_start_time: u64,
    log_worker: Option<Child>,
    log_worker_pid: Option<u32>,
    log_worker_pid_start_time: Option<u64>,
    metrics_sys: Mutex<System>,
    exit_code: Option<i32>,
    runtime_root: PathBuf,
    bundle_dir: PathBuf,
    runtime_record: PathBuf,
    cleaned: bool,
}

impl AsterOciHandler {
    pub(crate) fn from_child(
        spec: AsterOciHandlerSpec,
        client: AsterOciClient,
        owner: Child,
        log_worker: Child,
        log_worker_pid_start_time: u64,
    ) -> Self {
        let log_worker_pid = log_worker.id();
        Self {
            client,
            target: ContainerTarget::exact(spec.container_id, spec.generation),
            init_pid: spec.init_pid,
            owner: Some(owner),
            owner_pid: spec.owner_pid,
            owner_pid_start_time: spec.owner_pid_start_time,
            log_worker: Some(log_worker),
            log_worker_pid: Some(log_worker_pid),
            log_worker_pid_start_time: Some(log_worker_pid_start_time),
            metrics_sys: Mutex::new(System::new()),
            exit_code: None,
            runtime_root: spec.runtime_root,
            bundle_dir: spec.bundle_dir,
            runtime_record: spec.runtime_record,
            cleaned: false,
        }
    }

    pub(crate) async fn from_recorded_runtime(
        spec: AsterOciHandlerSpec,
        log_worker_pid: Option<u32>,
        log_worker_pid_start_time: Option<u64>,
    ) -> Result<Self> {
        let client = AsterOciClient::connect(spec.runtime_socket).await?;
        let target = ContainerTarget::exact(spec.container_id, spec.generation);
        let record = client
            .state_optional(StateRequest {
                target: target.clone(),
            })?
            .ok_or_else(|| {
                BoxError::StateError("Recorded Aster OCI generation is absent".to_string())
            })?;
        validate_record(&record, &target, Some(spec.init_pid))?;
        Ok(Self {
            client,
            target,
            init_pid: spec.init_pid,
            owner: None,
            owner_pid: spec.owner_pid,
            owner_pid_start_time: spec.owner_pid_start_time,
            log_worker: None,
            log_worker_pid,
            log_worker_pid_start_time,
            metrics_sys: Mutex::new(System::new()),
            exit_code: None,
            runtime_root: spec.runtime_root,
            bundle_dir: spec.bundle_dir,
            runtime_record: spec.runtime_record,
            cleaned: false,
        })
    }

    pub(crate) fn query_state_at(
        runtime_socket: &Path,
        container_id: &str,
        generation: u64,
    ) -> Result<Option<AsterOciState>> {
        let client = AsterOciClient::connect_blocking(runtime_socket.to_path_buf())?;
        let id = ContainerId::new(container_id).map_err(sdk_argument_error)?;
        let target = ContainerTarget::exact(id, Generation(generation));
        let state = client.state_optional(StateRequest {
            target: target.clone(),
        })?;
        client.close();
        state
            .map(|record| state_summary(&record, &target, None))
            .transpose()
    }

    /// Poll the exact detached Aster OCI generation for its terminal status.
    /// The runtime retains this status until Box explicitly deletes it.
    pub(crate) fn try_wait_at(
        runtime_socket: &Path,
        container_id: &str,
        generation: u64,
    ) -> Result<Option<i32>> {
        let client = AsterOciClient::connect_blocking(runtime_socket.to_path_buf())?;
        let id = ContainerId::new(container_id).map_err(sdk_argument_error)?;
        let result = client
            .try_wait(WaitRequest {
                target: ContainerTarget::exact(id, Generation(generation)),
                timeout_ms: Some(0),
            })
            .map(|status| status.map(|status| exit_code(&status)));
        client.close();
        result
    }

    pub(crate) fn pause_at(
        runtime_socket: &Path,
        container_id: &str,
        generation: u64,
    ) -> Result<()> {
        Self::transition_at(runtime_socket, container_id, generation, true)
    }

    pub(crate) fn resume_at(
        runtime_socket: &Path,
        container_id: &str,
        generation: u64,
    ) -> Result<()> {
        Self::transition_at(runtime_socket, container_id, generation, false)
    }

    /// Apply one complete resource contract to an exact live Sandbox generation.
    pub(crate) fn update_at(
        runtime_socket: &Path,
        container_id: &str,
        generation: u64,
        resources: LinuxResources,
    ) -> Result<()> {
        let client = AsterOciClient::connect_blocking(runtime_socket.to_path_buf())?;
        let result = (|| {
            let id = ContainerId::new(container_id).map_err(sdk_argument_error)?;
            let target = ContainerTarget::exact(id, Generation(generation));
            let record = client.update(UpdateRequest {
                context: operation_context(container_id, "update")?,
                target: target.clone(),
                resources,
            })?;
            validate_record(&record, &target, None)
        })();
        client.close();
        result
    }

    fn transition_at(
        runtime_socket: &Path,
        container_id: &str,
        generation: u64,
        pause: bool,
    ) -> Result<()> {
        let client = AsterOciClient::connect_blocking(runtime_socket.to_path_buf())?;
        let id = ContainerId::new(container_id).map_err(sdk_argument_error)?;
        let target = ContainerTarget::exact(id, Generation(generation));
        let context = operation_context(container_id, if pause { "pause" } else { "resume" })?;
        let request = ContainerOperationRequest {
            context,
            target: target.clone(),
        };
        let record = if pause {
            client.pause(request)?
        } else {
            client.resume(request)?
        };
        validate_record(&record, &target, None)?;
        if record.is_paused() != pause {
            return Err(BoxError::StateError(format!(
                "Aster OCI runtime did not {} {container_id}",
                if pause { "pause" } else { "resume" }
            )));
        }
        client.close();
        Ok(())
    }

    fn query_state(&self) -> Result<Option<ContainerRecord>> {
        self.client.state_optional(StateRequest {
            target: self.target.clone(),
        })
    }

    fn signal_container(&self, signal: i32, suffix: &str) -> Result<()> {
        let signal = Signal::new(signal).map_err(sdk_argument_error)?;
        let record = self.client.kill(KillRequest {
            context: operation_context(self.target.id.as_str(), suffix)?,
            target: self.target.clone(),
            signal,
            all: true,
        })?;
        validate_record(&record, &self.target, None)
    }

    fn wait_for_exit(&mut self, timeout_ms: u64) -> Result<bool> {
        let deadline = Instant::now() + Duration::from_millis(timeout_ms);
        loop {
            match self.query_state()? {
                None => return Ok(true),
                Some(record) if *record.state.status() == OciContainerState::Stopped => {
                    self.capture_exit_status()?;
                    return Ok(true);
                }
                Some(_) if Instant::now() < deadline => {
                    std::thread::sleep(LIFECYCLE_POLL_INTERVAL);
                }
                Some(_) => return Ok(false),
            }
        }
    }

    fn capture_exit_status(&mut self) -> Result<()> {
        if self.exit_code.is_some() {
            return Ok(());
        }
        let status = self.client.wait(WaitRequest {
            target: self.target.clone(),
            timeout_ms: Some(0),
        })?;
        self.exit_code = Some(exit_code(&status));
        Ok(())
    }

    fn delete_runtime_state(&mut self) -> Result<()> {
        if self.cleaned {
            return Ok(());
        }
        self.client.delete_if_present(DeleteRequest {
            context: operation_context(self.target.id.as_str(), "delete")?,
            target: self.target.clone(),
            mode: DeleteMode::Force,
        })?;
        self.client.close();
        self.stop_owner()?;
        self.reap_log_worker();
        remove_dir_until_absent(&self.bundle_dir)?;
        remove_dir_until_absent(&self.runtime_root)?;
        remove_file_if_exists(&self.runtime_record)?;
        self.cleaned = true;
        Ok(())
    }

    fn stop_owner(&mut self) -> Result<()> {
        super::aster_oci_owner::stop(self.owner_pid, self.owner_pid_start_time)?;
        if let Some(mut owner) = self.owner.take() {
            let _ = owner.try_wait();
            let _ = owner.wait();
        }
        Ok(())
    }

    fn reap_log_worker(&mut self) {
        const LOG_WORKER_EXIT_TIMEOUT: Duration = Duration::from_secs(2);
        if let Some(mut worker) = self.log_worker.take() {
            let deadline = Instant::now() + LOG_WORKER_EXIT_TIMEOUT;
            loop {
                match worker.try_wait() {
                    Ok(Some(_)) => return,
                    Ok(None) if Instant::now() < deadline => {
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    _ => break,
                }
            }
            let _ = worker.kill();
            let _ = worker.wait();
            return;
        }
        let (Some(pid), Some(start_time)) = (self.log_worker_pid, self.log_worker_pid_start_time)
        else {
            return;
        };
        if !crate::process::wait_for_process_exit_with_identity(
            pid,
            start_time,
            LOG_WORKER_EXIT_TIMEOUT,
        ) && crate::process::is_process_alive_with_identity(pid, Some(start_time))
        {
            if let Ok(pid) = i32::try_from(pid) {
                // SAFETY: See the enclosing function's safety documentation.
                unsafe {
                    libc::kill(pid, libc::SIGKILL);
                }
            }
            let _ = crate::process::wait_for_process_exit_with_identity(
                self.log_worker_pid.unwrap_or_default(),
                start_time,
                Duration::from_secs(1),
            );
        }
    }
}

impl VmHandler for AsterOciHandler {
    fn stop(&mut self, signal: i32, timeout_ms: u64) -> Result<()> {
        let mut first_error = None;
        match self.query_state()? {
            Some(record) if *record.state.status() != OciContainerState::Stopped => {
                let signal_error = self.signal_container(signal, "stop").err();
                let wait_result = self.wait_for_exit(timeout_ms);
                match reconcile_signal_with_wait(signal_error, wait_result) {
                    SignalWaitOutcome::Exited => {}
                    SignalWaitOutcome::Running(error) => {
                        first_error = error;
                        if let Err(error) = self.signal_container(SIGKILL_NUMBER, "force-stop") {
                            first_error.get_or_insert(error);
                        }
                        let _ = self.wait_for_exit(2_000);
                    }
                    SignalWaitOutcome::Failed(error) => {
                        first_error = Some(error);
                        let _ = self.signal_container(SIGKILL_NUMBER, "failed-stop-cleanup");
                    }
                }
            }
            Some(_) => {
                if let Err(error) = self.capture_exit_status() {
                    first_error = Some(error);
                }
            }
            None => {}
        }
        if let Err(error) = self.delete_runtime_state() {
            first_error.get_or_insert(error);
        }
        match first_error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    fn metrics(&self) -> VmMetrics {
        if let Ok(stats) = self.client.stats(StatsRequest {
            target: self.target.clone(),
        }) {
            return VmMetrics {
                cpu_percent: None,
                memory_bytes: Some(stats.memory.usage_bytes),
            };
        }
        let pid = Pid::from_u32(self.init_pid);
        let mut system = match self.metrics_sys.lock() {
            Ok(system) => system,
            Err(_) => return VmMetrics::default(),
        };
        system.refresh_process(pid);
        system
            .process(pid)
            .map(|process| VmMetrics {
                cpu_percent: Some(process.cpu_usage()),
                memory_bytes: Some(process.memory()),
            })
            .unwrap_or_default()
    }

    fn is_running(&self) -> bool {
        self.query_state().ok().flatten().is_some_and(|record| {
            matches!(
                *record.state.status(),
                OciContainerState::Created | OciContainerState::Running
            )
        })
    }

    fn has_exited(&self) -> bool {
        !self.is_running()
    }

    fn pid(&self) -> u32 {
        self.init_pid
    }

    fn exit_code(&self) -> Option<i32> {
        self.exit_code
    }

    fn try_wait_exit(&mut self) -> Result<Option<i32>> {
        if self.exit_code.is_none() {
            let Some(status) = self.client.try_wait(WaitRequest {
                target: self.target.clone(),
                timeout_ms: Some(0),
            })?
            else {
                return Ok(None);
            };
            self.exit_code = Some(exit_code(&status));
        }
        // Polling observes completion but does not own teardown. Keeping the
        // terminal generation lets the backend complete its one authoritative
        // destroy path after it has durably projected this exact status.
        Ok(self.exit_code)
    }
}

enum SignalWaitOutcome {
    Exited,
    Running(Option<BoxError>),
    Failed(BoxError),
}

/// Resolve a signal result against the later authoritative lifecycle state.
///
/// A workload can exit naturally after `state` reports it running but before
/// the runtime handles `kill`. If the subsequent wait captures that terminal
/// generation, a "stopped container" signal error is stale and cleanup is
/// already successful. When the workload is still live or wait itself fails,
/// retain the first lifecycle error as before.
fn reconcile_signal_with_wait(
    signal_error: Option<BoxError>,
    wait_result: Result<bool>,
) -> SignalWaitOutcome {
    match wait_result {
        Ok(true) => SignalWaitOutcome::Exited,
        Ok(false) => SignalWaitOutcome::Running(signal_error),
        Err(wait_error) => SignalWaitOutcome::Failed(signal_error.unwrap_or(wait_error)),
    }
}

pub(crate) fn validate_record(
    record: &ContainerRecord,
    target: &ContainerTarget,
    expected_pid: Option<u32>,
) -> Result<()> {
    if record.state.id() != target.id.as_str()
        || record.generation != target.generation.unwrap_or_default()
        || record.driver != DriverKind::NativeLinux
    {
        return Err(BoxError::StateError(
            "Aster OCI runtime returned a different container identity".to_string(),
        ));
    }
    if let Some(expected_pid) = expected_pid {
        let pid = record
            .state
            .pid()
            .and_then(|pid| u32::try_from(pid).ok())
            .ok_or_else(|| {
                BoxError::StateError("Aster OCI runtime returned no valid init PID".to_string())
            })?;
        if pid != expected_pid {
            return Err(BoxError::StateError(
                "Aster OCI runtime PID disagrees with its durable record".to_string(),
            ));
        }
    }
    Ok(())
}

fn state_summary(
    record: &ContainerRecord,
    target: &ContainerTarget,
    expected_pid: Option<u32>,
) -> Result<AsterOciState> {
    validate_record(record, target, expected_pid)?;
    let status = if record.is_paused() {
        "paused".to_string()
    } else {
        record.state.status().to_string()
    };
    let pid = record
        .state
        .pid()
        .and_then(|pid| u32::try_from(pid).ok())
        .unwrap_or(0);
    Ok(AsterOciState { status, pid })
}

fn operation_context(container_id: &str, operation: &str) -> Result<OperationContext> {
    OperationId::new(format!(
        "{container_id}-{operation}-{}",
        uuid::Uuid::new_v4().simple()
    ))
    .map(OperationContext::new)
    .map_err(sdk_argument_error)
}

fn sdk_argument_error(error: aster_oci_sdk::Error) -> BoxError {
    BoxError::ConfigError(error.to_string())
}

fn exit_code(status: &ExitStatus) -> i32 {
    status
        .exit_code
        .or_else(|| status.signal.map(|signal| 128 + signal))
        .unwrap_or(128)
}

fn remove_file_if_exists(path: &Path) -> Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(BoxError::StateError(format!(
            "Failed to remove Aster OCI runtime record {}: {error}",
            path.display()
        ))),
    }
}

fn remove_dir_until_absent(path: &Path) -> Result<()> {
    let deadline = Instant::now() + CLEANUP_RETRY_TIMEOUT;
    loop {
        match std::fs::remove_dir_all(path) {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(_) if Instant::now() < deadline => {
                std::thread::sleep(LIFECYCLE_POLL_INTERVAL);
            }
            Err(error) => {
                return Err(BoxError::StateError(format!(
                    "Failed to remove Aster OCI runtime directory {}: {error}",
                    path.display()
                )))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{reconcile_signal_with_wait, SignalWaitOutcome};
    use aster_box_core::error::BoxError;

    fn state_error(message: &str) -> BoxError {
        BoxError::StateError(message.to_string())
    }

    #[test]
    fn authoritative_exit_suppresses_a_stale_signal_failure() {
        let outcome = reconcile_signal_with_wait(
            Some(state_error("cannot signal stopped container")),
            Ok(true),
        );

        assert!(matches!(outcome, SignalWaitOutcome::Exited));
    }

    #[test]
    fn signal_failure_is_retained_while_the_container_remains_live() {
        let outcome = reconcile_signal_with_wait(Some(state_error("signal failed")), Ok(false));

        let SignalWaitOutcome::Running(Some(error)) = outcome else {
            panic!("expected a live container with its signal failure");
        };
        assert!(error.to_string().contains("signal failed"));
    }

    #[test]
    fn wait_failure_is_retained_when_signaling_succeeded() {
        let outcome = reconcile_signal_with_wait(None, Err(state_error("wait failed")));

        let SignalWaitOutcome::Failed(error) = outcome else {
            panic!("expected the wait failure");
        };
        assert!(error.to_string().contains("wait failed"));
    }
}
