//! Production local execution backend backed by [`crate::VmManager`].

#[path = "vm_sandbox.rs"]
mod sandbox;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use aster_box_core::{
    BoxError, EventEmitter, ExecutionBackend, ExecutionId, ExecutionManagerError,
    ExecutionManagerResult, ExecutionState, KillOutcome, DEFAULT_SHUTDOWN_TIMEOUT_MS,
};
use async_trait::async_trait;
use dashmap::mapref::entry::Entry;
use dashmap::DashMap;
use tokio::sync::Mutex;

use super::resources::ExecutionResourceGuard;
use super::vm_process::{locate_microvm_process, LocatedProcess};
#[cfg(any(target_os = "linux", target_os = "macos"))]
use super::TransientRegistryAuthBroker;
use super::{
    LocalExecutionBackend, LocalExecutionHandle, LocalExecutionObservation,
    LocalExecutionOwnerFacts, LocalExecutionResourcePlan, LocalExecutionTermination,
};
use crate::vm::{TERMINAL_EXIT_POLL_INTERVAL, TERMINAL_EXIT_POLL_TIMEOUT};
use crate::{
    BoxRecord, ManagedExecutionMetadata, ManagedExecutionOperation, ManagedExecutionState,
    VmManager,
};

type SharedVm = Arc<Mutex<VmManager>>;

// The original map remains the only registry. Existing insertions are Legacy;
// an Admitted owner is reachable only through the opaque component capability.
enum RegisteredVm {
    Legacy(SharedVm),
    Admitted(Arc<AdmittedVm>),
}

pub(super) struct AdmittedVm {
    binding: super::store::OwnerBinding,
    vm: SharedVm,
    progress: Option<Mutex<OwnedVmProgress>>,
}

impl RegisteredVm {
    fn legacy(&self, execution_id: &str) -> ExecutionManagerResult<SharedVm> {
        match self {
            Self::Legacy(vm) => Ok(Arc::clone(vm)),
            Self::Admitted(_) => Err(legacy_lineage_error(execution_id)),
        }
    }
}

fn legacy_lineage_error(execution_id: &str) -> ExecutionManagerError {
    ExecutionManagerError::Unavailable(format!(
        "owner-binding-mismatch: {execution_id}: admitted owner requires sealed continuation"
    ))
}

/// A SIGSTOP'd shim cannot answer the exec heartbeat, so cross-process attach
/// leaves `Created`. Project `Paused` only when the durable lifecycle is a warm
/// pause or the resume claim that still owns that stopped shim.
///
/// `paused_with_memory` defaults true, so a still-running start must stay
/// `Created` (#424). Cold resume (`paused_with_memory == false`) must not
/// invent a memory pause. An attach that already authenticated stays as-is.
fn project_warm_pause_attach_state(
    paused_with_memory: bool,
    managed: ManagedExecutionState,
    attached: crate::BoxState,
) -> crate::BoxState {
    let warm_stopped_runtime = paused_with_memory
        && matches!(
            managed,
            ManagedExecutionState::Paused | ManagedExecutionState::Resuming
        );
    if warm_stopped_runtime && attached == crate::BoxState::Created {
        crate::BoxState::Paused
    } else {
        attached
    }
}

/// Runtime adapter that owns live [`VmManager`] handles and reconstructs them
/// from durable runtime evidence after a control-plane restart.
#[derive(Clone)]
pub struct VmLocalExecutionBackend {
    home_dir: PathBuf,
    managers: Arc<DashMap<String, RegisteredVm>>,
    pull_progress_fn: Option<crate::PullProgressFn>,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    transient_registry_auth: Option<TransientRegistryAuthBroker>,
}

impl VmLocalExecutionBackend {
    pub fn new(home_dir: impl Into<PathBuf>) -> Self {
        Self {
            home_dir: home_dir.into(),
            managers: Arc::new(DashMap::new()),
            pull_progress_fn: None,
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            transient_registry_auth: None,
        }
    }

    pub fn with_pull_progress_fn(mut self, pull_progress_fn: crate::PullProgressFn) -> Self {
        self.pull_progress_fn = Some(pull_progress_fn);
        self
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    pub(crate) fn with_transient_registry_auth(
        mut self,
        broker: TransientRegistryAuthBroker,
    ) -> Self {
        self.transient_registry_auth = Some(broker);
        self
    }

    pub fn home_dir(&self) -> &Path {
        &self.home_dir
    }

    fn metadata<'a>(
        &self,
        record: &'a BoxRecord,
    ) -> ExecutionManagerResult<&'a ManagedExecutionMetadata> {
        self.metadata_for_route(record, crate::ManagedRuntimeRoute::BoxVm)
    }

    fn metadata_for_route<'a>(
        &self,
        record: &'a BoxRecord,
        expected_route: crate::ManagedRuntimeRoute,
    ) -> ExecutionManagerResult<&'a ManagedExecutionMetadata> {
        uuid::Uuid::parse_str(&record.id).map_err(|error| {
            ExecutionManagerError::Internal(format!(
                "managed execution has an invalid internal ID {}: {error}",
                record.id
            ))
        })?;
        let expected_box_dir = self.home_dir.join("boxes").join(&record.id);
        if record.box_dir != expected_box_dir {
            return Err(ExecutionManagerError::Internal(format!(
                "managed execution {} has an unexpected host directory {}",
                record.id,
                record.box_dir.display()
            )));
        }
        let metadata = record.managed_execution.as_ref().ok_or_else(|| {
            ExecutionManagerError::Internal(format!(
                "execution {} lost managed lifecycle metadata",
                record.id
            ))
        })?;
        metadata
            .validate()
            .map_err(|error| ExecutionManagerError::Internal(error.to_string()))?;
        let resolved_route = super::router::resolved_runtime_route(record)?;
        if resolved_route != expected_route {
            return Err(ExecutionManagerError::Internal(format!(
                "managed execution {} is pinned to {:?}, not {:?}",
                record.id, resolved_route, expected_route
            )));
        }
        if record.isolation != metadata.request.config.isolation {
            return Err(ExecutionManagerError::Internal(format!(
                "managed execution {} has inconsistent isolation metadata",
                record.id
            )));
        }
        Ok(metadata)
    }

    fn new_manager(&self, record: &BoxRecord) -> ExecutionManagerResult<VmManager> {
        self.require_legacy_lineage(&record.id)?;
        let metadata = self.metadata(record)?;
        self.new_manager_from_metadata(record, metadata)
    }

    pub(super) fn new_oci_preparation_manager(
        &self,
        record: &BoxRecord,
    ) -> ExecutionManagerResult<VmManager> {
        self.require_legacy_lineage(&record.id)?;
        let metadata = self.metadata_for_route(record, crate::ManagedRuntimeRoute::OciSdk)?;
        self.new_manager_from_metadata(record, metadata)
    }

    fn new_manager_from_metadata(
        &self,
        record: &BoxRecord,
        metadata: &ManagedExecutionMetadata,
    ) -> ExecutionManagerResult<VmManager> {
        let mut config = metadata.request.config.clone();
        // Network connect/disconnect is intentionally mutable while an
        // execution is inactive. The managed creation request remains the
        // immutable idempotency identity, so apply the current record-level
        // network selection when constructing each runtime generation.
        config.network = record.network_mode.clone();
        if let Some(shm_size) = metadata.request.policy.shm_size {
            let has_shared_memory_mount = config
                .tmpfs
                .iter()
                .any(|entry| entry.split(':').next() == Some("/dev/shm"));
            if !has_shared_memory_mount {
                config.tmpfs.push(format!("/dev/shm:size={shm_size}"));
            }
        }
        let mut manager = VmManager::with_box_id(config, EventEmitter::new(256), record.id.clone());
        manager.home_dir = self.home_dir.clone();
        manager.set_healthcheck_disabled(metadata.request.policy.healthcheck_disabled);
        if let Some(pull_progress_fn) = self.pull_progress_fn.clone() {
            manager.set_pull_progress_fn(pull_progress_fn);
        }
        manager.anonymous_volumes = record.anonymous_volumes.clone();
        manager.set_log_config(record.log_config.clone());
        manager.resolved_execution_plan = Some(metadata.plan.clone());
        manager.managed_secret_root = metadata.request.policy.managed_secret_root.clone();
        Ok(manager)
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn claim_transient_registry_auth_for_boot(&self, manager: &mut VmManager) {
        manager.transient_registry_auth = self
            .transient_registry_auth
            .as_ref()
            .and_then(|broker| broker.take(manager.box_id()));
    }

    fn require_legacy_lineage(&self, execution_id: &str) -> ExecutionManagerResult<()> {
        if self
            .managers
            .get(execution_id)
            .is_some_and(|entry| matches!(entry.value(), RegisteredVm::Admitted(_)))
        {
            return Err(legacy_lineage_error(execution_id));
        }
        Ok(())
    }

    fn manager(&self, execution_id: &str) -> ExecutionManagerResult<Option<SharedVm>> {
        self.managers
            .get(execution_id)
            .map(|entry| entry.value().legacy(execution_id))
            .transpose()
    }

    fn remove_manager(
        &self,
        execution_id: &str,
        expected: &SharedVm,
    ) -> ExecutionManagerResult<()> {
        if let Entry::Occupied(entry) = self.managers.entry(execution_id.to_string()) {
            match entry.get() {
                RegisteredVm::Legacy(vm) if Arc::ptr_eq(vm, expected) => {
                    entry.remove();
                }
                RegisteredVm::Legacy(_) => {}
                RegisteredVm::Admitted(_) => return Err(legacy_lineage_error(execution_id)),
            }
        }
        Ok(())
    }

    async fn handle_from_manager(
        &self,
        record: &BoxRecord,
        manager: &VmManager,
    ) -> ExecutionManagerResult<LocalExecutionHandle> {
        // A start/pause/inspect handle implies operable guest exec — or durable
        // pause bookkeeping. Created / Failed / Stopped managers must not invent
        // a handle from PID + layout path alone (#422 / #415/#416 parity).
        // Paused is allowed so finish_pause can retain a lease handle without
        // inventing Ready while the shim is SIGSTOP'd (#424).
        let state = manager.state().await;
        if !matches!(
            state,
            crate::BoxState::Ready
                | crate::BoxState::Busy
                | crate::BoxState::Compacting
                | crate::BoxState::Paused
        ) {
            return Err(ExecutionManagerError::Unavailable(format!(
                "execution {} has no authenticated Ready runtime for a handle (state={state:?})",
                record.id
            )));
        }
        let execution_id = execution_id(record)?;
        let pid = manager.pid().await.ok_or_else(|| {
            ExecutionManagerError::Internal(format!(
                "runtime returned no host PID for {execution_id}"
            ))
        })?;
        let has_record_binding = record.isolation == aster_box_core::ExecutionIsolation::Microvm
            && (record.pid.is_some() || record.pid_start_time.is_some());
        let pid_start_time = if has_record_binding {
            self.metadata(record)?;
            let (_, original_start_time) = require_attached_record_identity(
                record,
                LocatedProcess {
                    pid,
                    start_time: crate::process::pid_start_time(pid),
                },
            )?;
            Some(original_start_time)
        } else {
            // Both-absent first-start and Sandbox keep their legacy handle
            // observation. This sample is not original-owner qualification.
            let sampled = crate::process::pid_start_time(pid);
            #[cfg(target_os = "linux")]
            if sampled.is_none() {
                return Err(ExecutionManagerError::NotFound(execution_id));
            }
            if !crate::process::is_process_alive_with_identity(pid, sampled) {
                return Err(ExecutionManagerError::NotFound(execution_id));
            }
            sampled
        };
        let exec_socket_path = manager
            .exec_socket_path()
            .map(Path::to_path_buf)
            .ok_or_else(|| {
                ExecutionManagerError::Internal(format!(
                    "runtime returned no exec socket for {}",
                    record.id
                ))
            })?;
        let anonymous_volumes = if manager.anonymous_volumes().is_empty() {
            self.anonymous_volumes_for_record(record).await?
        } else {
            manager.anonymous_volumes().to_vec()
        };
        Ok(LocalExecutionHandle {
            started_at: record.started_at.unwrap_or_else(chrono::Utc::now),
            pid: Some(pid),
            pid_start_time,
            exec_socket_path,
            console_log: record.box_dir.join("logs/console.log"),
            anonymous_volumes,
            oci_runtime: None,
        })
    }

    async fn inspect_registered(
        &self,
        record: &BoxRecord,
        shared: SharedVm,
    ) -> ExecutionManagerResult<LocalExecutionObservation> {
        self.require_legacy_lineage(&record.id)?;
        let mut manager = shared.lock().await;
        let preserve_rootfs = should_force_rootfs_preservation(record)?;
        let exit_code = manager
            .try_wait_exit()
            .await
            .map_err(|error| runtime_error("inspect", record, error))?;
        let mut state = manager.state().await;
        let terminal = exit_code.is_some() || state == crate::BoxState::Stopped;
        if terminal {
            return self
                .finish_registered_terminal(record, &shared, manager, preserve_rootfs, exit_code)
                .await;
        }

        if state == crate::BoxState::Created {
            if manager.has_exited().await {
                return self
                    .finish_registered_terminal(record, &shared, manager, preserve_rootfs, None)
                    .await;
            }
            if !self.promote_if_ready(record, &mut manager).await {
                return Ok(LocalExecutionObservation {
                    state: ExecutionState::Creating,
                    handle: None,
                    exit_code: None,
                });
            }
            state = manager.state().await;
        }

        // SIGSTOP demotes health to false by design. A Paused manager is not
        // terminal — project the durable pause lease instead of inventing an
        // exit-status wait (#424).
        if state == crate::BoxState::Paused {
            let handle = self.handle_from_manager(record, &manager).await?;
            return Ok(LocalExecutionObservation {
                state: ExecutionState::Paused,
                handle: Some(handle),
                exit_code: None,
            });
        }

        if !manager
            .health_check()
            .await
            .map_err(|error| runtime_error("inspect", record, error))?
        {
            return self
                .finish_registered_terminal(record, &shared, manager, preserve_rootfs, None)
                .await;
        }

        if state != crate::BoxState::Ready
            && state != crate::BoxState::Busy
            && state != crate::BoxState::Compacting
        {
            return Err(ExecutionManagerError::Internal(format!(
                "runtime manager for {} is in unexpected state {state:?}",
                record.id
            )));
        }
        if matches!(
            managed_state(record)?,
            ManagedExecutionState::Starting | ManagedExecutionState::RestartStarting
        ) && !exec_endpoint_ready(record.id.as_str(), manager.exec_socket_path()).await
        {
            return Ok(LocalExecutionObservation {
                state: ExecutionState::Creating,
                handle: None,
                exit_code: None,
            });
        }
        let visible_state = visible_active_state(record)?;
        let handle = match self.handle_from_manager(record, &manager).await {
            Ok(handle) => handle,
            Err(ExecutionManagerError::NotFound(_)) => {
                // The runtime state and health probes can still report a live
                // Sandbox after its host process has exited. Treat the missing
                // handle as the same terminal signal as a failed health probe
                // and wait for the exact status before projecting provider
                // loss. This closes the restart/replay race without creating a
                // second lifecycle path.
                return self
                    .finish_registered_terminal(record, &shared, manager, preserve_rootfs, None)
                    .await;
            }
            Err(error) => return Err(error),
        };
        Ok(LocalExecutionObservation {
            state: visible_state,
            handle: Some(handle),
            exit_code: None,
        })
    }

    async fn finish_registered_terminal(
        &self,
        record: &BoxRecord,
        shared: &SharedVm,
        mut manager: tokio::sync::MutexGuard<'_, VmManager>,
        preserve_rootfs: bool,
        exit_code: Option<i32>,
    ) -> ExecutionManagerResult<LocalExecutionObservation> {
        // A workload can exit between the first non-blocking wait and the
        // following health probe. Aster OCI publishes the exact status shortly
        // after the process becomes non-running, so retain its terminal record
        // and poll within a strict bound before any teardown.
        let deadline = tokio::time::Instant::now() + TERMINAL_EXIT_POLL_TIMEOUT;
        let mut exit_code = manager.exit_code().or(exit_code);
        while exit_code.is_none() {
            exit_code = manager
                .try_wait_exit()
                .await
                .map_err(|error| runtime_error("collect exit status", record, error))?;
            if exit_code.is_some() || tokio::time::Instant::now() >= deadline {
                break;
            }
            tokio::time::sleep(TERMINAL_EXIT_POLL_INTERVAL).await;
        }
        // Cached shim/provider zero is not guest success. Authenticate against
        // durable workload status before projecting Stopped (#404/#408 parity).
        #[cfg(not(target_os = "windows"))]
        let authenticated = crate::rootfs::resolve_workload_exit_code_for(
            &record.box_dir,
            exit_code,
            if record.isolation.is_sandbox() {
                crate::rootfs::ProviderExitPolicy::TrustProvider
            } else {
                crate::rootfs::ProviderExitPolicy::RequireGuestProof
            },
        );
        #[cfg(target_os = "windows")]
        let authenticated = authenticate_windows_managed_terminal_exit(
            &record.box_dir,
            &manager.log_config,
            exit_code,
        );
        if authenticated.is_none() {
            // Drop inventable provider-zero cache so a delayed durable status
            // can still complete a later observation poll.
            if manager.shim_exit_code == Some(0) {
                manager.shim_exit_code = None;
            }
        }
        let state = manager.state().await;
        let exit_code = authenticated.ok_or_else(|| {
            ExecutionManagerError::Unavailable(format!(
                "runtime reported execution {} as terminal before its exact exit status became available (state={state:?}, provider_exit={exit_code:?})",
                record.id
            ))
        })?;
        let cleanup = destroy_after_observation(&mut manager, preserve_rootfs).await;
        drop(manager);
        self.remove_manager(&record.id, shared)?;
        cleanup.map_err(|error| runtime_error("clean up", record, error))?;
        Ok(LocalExecutionObservation {
            state: ExecutionState::Stopped,
            handle: None,
            exit_code: Some(exit_code),
        })
    }

    async fn promote_if_ready(&self, record: &BoxRecord, manager: &mut VmManager) -> bool {
        let socket_dir = crate::vm::runtime_socket_dir(&self.home_dir, &record.id);
        let exec_socket = socket_dir.join("exec.sock");
        manager.exec_socket_path = Some(exec_socket);
        manager.pty_socket_path = Some(socket_dir.join("pty.sock"));
        manager.port_forward_socket_path = Some(socket_dir.join("portfwd.sock"));

        // Authenticate and retain proof the same way attach / Sandbox recover do.
        // A dropped one-shot heartbeat must not leave Ready with only PID for
        // later health (#418 / #413/#415 parity).
        #[cfg(unix)]
        {
            manager.promote_ready_if_exec_authenticated().await
        }
        #[cfg(not(unix))]
        {
            // Windows VmManager does not retain ExecClient; still require the
            // named-pipe heartbeat before Ready (no path-presence invent).
            if !exec_endpoint_ready(record.id.as_str(), manager.exec_socket_path()).await {
                return false;
            }
            *manager.state.write().await = crate::BoxState::Ready;
            true
        }
    }

    /// After `boot()` Ok, require authenticated Ready before returning a start
    /// handle that callers durable-promote to Running.
    ///
    /// `#414` may leave `Created` when the restore soft-probe fails while still
    /// returning `Ok(())` from boot. PID + layout socket alone must not invent
    /// Running (#416). In-memory Ready/Busy/Compacting from an earlier one-shot
    /// promote must also re-prove exec health — stale Ready without heartbeat
    /// must not invent a start handle (#421 / #419/#420).
    async fn require_authenticated_ready_for_start(
        &self,
        record: &BoxRecord,
        manager: &mut VmManager,
    ) -> ExecutionManagerResult<()> {
        let state = manager.state().await;
        if matches!(
            state,
            crate::BoxState::Ready | crate::BoxState::Busy | crate::BoxState::Compacting
        ) {
            match manager.health_check().await {
                Ok(true) => return Ok(()),
                Ok(false) | Err(_) => {
                    tracing::debug!(
                        execution_id = %record.id,
                        ?state,
                        "in-memory Ready without exec heartbeat; refusing start until re-auth"
                    );
                }
            }
        }
        if self.promote_if_ready(record, manager).await {
            return Ok(());
        }
        tracing::debug!(
            execution_id = %record.id,
            ?state,
            "boot completed without authenticated exec Ready; refusing start handle"
        );
        Err(ExecutionManagerError::Unavailable(format!(
            "execution {} started but guest exec is not ready yet",
            record.id
        )))
    }

    async fn recover_microvm(&self, record: &BoxRecord) -> ExecutionManagerResult<SharedVm> {
        self.metadata(record)?;
        let pid = record.pid.ok_or_else(|| {
            ExecutionManagerError::Unavailable(format!(
                "execution {} has no original attached process PID",
                record.id
            ))
        })?;
        let (pid, start_time) = require_attached_record_identity(
            record,
            LocatedProcess {
                pid,
                start_time: record.pid_start_time,
            },
        )?;
        let execution_id_label = record.id.clone();
        let recorded = Some((pid, Some(start_time)));
        let located = tokio::task::spawn_blocking(move || {
            locate_microvm_process(&execution_id_label, recorded)
        })
        .await
        .map_err(|error| {
            ExecutionManagerError::Internal(format!(
                "MicroVM process discovery task failed for {}: {error}",
                record.id
            ))
        })?
        .map_err(ExecutionManagerError::Internal)?
        .ok_or_else(|| {
            ExecutionManagerError::Unavailable(format!(
                "execution {} original attached process identity is unproved",
                record.id
            ))
        })?;
        self.attach_microvm(record, located).await
    }

    async fn attach_microvm(
        &self,
        record: &BoxRecord,
        located: LocatedProcess,
    ) -> ExecutionManagerResult<SharedVm> {
        let metadata = self.metadata(record)?;
        let (pid, start_time) = require_attached_record_identity(record, located)?;
        let fixed_handler =
            crate::vmm::ShimHandler::from_recorded_pid(pid, start_time, record.id.clone())
                .map_err(|error| runtime_error("bind attached process", record, error))?;
        let mut manager = self.new_manager(record)?;
        let socket_dir = crate::vm::runtime_socket_dir(&self.home_dir, &record.id);
        manager
            .attach_running_process(
                pid,
                socket_dir.join("exec.sock"),
                Some(socket_dir.join("pty.sock")),
            )
            .await
            .map_err(|error| runtime_error("recover", record, error))?;
        require_attached_record_identity(record, located)?;
        // The original attach helper creates only an unowned local probe. The
        // unpublished manager must retain the fixed record token instead of
        // that probe's fresh sample. No Child is replaced or discarded.
        *manager.handler.write().await = Some(Box::new(fixed_handler));
        require_attached_record_identity(record, located)?;
        // Cross-process pause recovery: a SIGSTOP'd shim cannot answer the exec
        // heartbeat, so attach leaves Created. Project Paused only for a warm
        // pause that is already durable or claimed for resume.
        let attached = project_warm_pause_attach_state(
            metadata.paused_with_memory,
            managed_state(record)?,
            manager.state().await,
        );
        if attached == crate::BoxState::Paused {
            *manager.state.write().await = crate::BoxState::Paused;
        }
        let recovered = Arc::new(Mutex::new(manager));
        // State/handler lock waits above may outlive the checked identity.
        // Even this positive live check is not termination/generation proof.
        require_attached_record_identity(record, located)?;
        match self.managers.entry(record.id.clone()) {
            Entry::Occupied(entry) => entry.get().legacy(&record.id),
            Entry::Vacant(entry) => {
                entry.insert(RegisteredVm::Legacy(Arc::clone(&recovered)));
                Ok(recovered)
            }
        }
    }

    #[cfg(not(windows))]
    async fn require_microvm(&self, record: &BoxRecord) -> ExecutionManagerResult<SharedVm> {
        match self.manager(&record.id)? {
            Some(manager) => Ok(manager),
            None => self.recover_microvm(record).await,
        }
    }

    async fn destroy_registered(
        &self,
        record: &BoxRecord,
        shared: SharedVm,
        remove_anonymous_volumes: bool,
        force_preserve_rootfs: bool,
        timeout_secs: Option<u64>,
    ) -> ExecutionManagerResult<LocalExecutionTermination> {
        self.require_legacy_lineage(&record.id)?;
        let mut manager = shared.lock().await;
        let mut anonymous_volumes = if manager.anonymous_volumes().is_empty() {
            record.anonymous_volumes.clone()
        } else {
            manager.anonymous_volumes().to_vec()
        };
        let result = match (
            graceful_stop_options(record, timeout_secs)?,
            force_preserve_rootfs,
        ) {
            (Some((signal, timeout_ms)), true) => {
                manager
                    .destroy_preserving_rootfs_with_options(signal, timeout_ms)
                    .await
            }
            (Some((signal, timeout_ms)), false) => {
                manager.destroy_with_options(signal, timeout_ms).await
            }
            (None, true) => manager.destroy_preserving_rootfs().await,
            (None, false) => manager.destroy().await,
        };
        let exit_code = manager.exit_code();
        drop(manager);
        result.map_err(|error| runtime_error("kill", record, error))?;
        self.remove_manager(&record.id, &shared)?;
        if remove_anonymous_volumes {
            if anonymous_volumes.is_empty() {
                anonymous_volumes = self.anonymous_volumes_for_record(record).await?;
            }
            self.cleanup_anonymous_volumes(&record.id, anonymous_volumes)
                .await?;
        }
        Ok(LocalExecutionTermination {
            outcome: KillOutcome::Killed,
            exit_code,
        })
    }

    async fn anonymous_volumes_for_record(
        &self,
        record: &BoxRecord,
    ) -> ExecutionManagerResult<Vec<String>> {
        if !record.anonymous_volumes.is_empty() {
            return Ok(record.anonymous_volumes.clone());
        }
        let home_dir = self.home_dir.clone();
        let execution_id = record.id.clone();
        let short_id = record.id.chars().take(8).collect::<String>();
        let result = tokio::task::spawn_blocking(move || -> aster_box_core::Result<Vec<String>> {
            let store =
                crate::VolumeStore::new(home_dir.join("volumes.json"), home_dir.join("volumes"));
            let prefix = format!("anon_{short_id}_");
            let mut names = store
                .load()?
                .into_values()
                .filter(|volume| {
                    volume
                        .labels
                        .get("anonymous")
                        .is_some_and(|value| value == "true")
                        && (volume.in_use_by.iter().any(|id| id == &execution_id)
                            || volume.name.starts_with(&prefix))
                })
                .map(|volume| volume.name)
                .collect::<Vec<_>>();
            names.sort();
            Ok(names)
        })
        .await;
        match result {
            Ok(Ok(names)) => Ok(names),
            Ok(Err(error)) => Err(ExecutionManagerError::Unavailable(format!(
                "failed to load anonymous volumes for {}: {error}",
                record.id
            ))),
            Err(error) => Err(ExecutionManagerError::Internal(format!(
                "anonymous volume recovery task failed for {}: {error}",
                record.id
            ))),
        }
    }

    async fn cleanup_anonymous_volumes(
        &self,
        owner: &str,
        names: Vec<String>,
    ) -> ExecutionManagerResult<()> {
        if names.is_empty() {
            return Ok(());
        }
        let home_dir = self.home_dir.clone();
        let owner = owner.to_string();
        let owner_for_error = owner.clone();
        let task = tokio::task::spawn_blocking(move || -> aster_box_core::Result<()> {
            let store =
                crate::VolumeStore::new(home_dir.join("volumes.json"), home_dir.join("volumes"));
            for name in names {
                // Missing / already-removed anonymous volumes are success (`Ok(false)`).
                store.remove_anonymous(&name, &owner)?;
            }
            Ok(())
        })
        .await;
        match task {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) => Err(ExecutionManagerError::Unavailable(format!(
                "failed to remove anonymous volumes for {owner_for_error}: {error}"
            ))),
            Err(error) => Err(ExecutionManagerError::Internal(format!(
                "anonymous volume cleanup task failed for {owner_for_error}: {error}"
            ))),
        }
    }

    async fn terminate_execution(
        &self,
        record: &BoxRecord,
    ) -> ExecutionManagerResult<LocalExecutionTermination> {
        self.require_legacy_lineage(&record.id)?;
        let metadata = self.metadata(record)?;
        let remove_anonymous_volumes = record.auto_remove;
        let timeout_secs = record.stop_timeout;
        if let Some(manager) = self.manager(&record.id)? {
            return self
                .destroy_registered(
                    record,
                    manager,
                    remove_anonymous_volumes,
                    false,
                    timeout_secs,
                )
                .await;
        }
        // A filesystem-only pause deliberately has no live provider evidence,
        // but a later terminal kill must still apply the configured rootfs and
        // anonymous-volume cleanup policy to the retained generation.
        if !metadata.paused_with_memory {
            let manager = Arc::new(Mutex::new(self.new_manager(record)?));
            return self
                .destroy_registered(
                    record,
                    manager,
                    remove_anonymous_volumes,
                    false,
                    timeout_secs,
                )
                .await;
        }
        match metadata.plan.backend {
            ExecutionBackend::AsterOci => {
                self.destroy_detached_sandbox(record, remove_anonymous_volumes, false, timeout_secs)
                    .await
            }
            ExecutionBackend::Krun => {
                let manager = self.recover_microvm(record).await?;
                self.destroy_registered(
                    record,
                    manager,
                    remove_anonymous_volumes,
                    false,
                    timeout_secs,
                )
                .await
            }
        }
    }
}

fn admit_launch(isolation: aster_box_core::ExecutionIsolation) -> ExecutionManagerResult<()> {
    crate::host_check::admit_requested_isolation(isolation)
        .map_err(|error| ExecutionManagerError::Unavailable(error.to_string()))
}

fn admit_create(isolation: aster_box_core::ExecutionIsolation) -> ExecutionManagerResult<()> {
    crate::host_check::admit_isolation_class(isolation)
        .map_err(|error| ExecutionManagerError::Unavailable(error.to_string()))
}

async fn destroy_after_observation(
    manager: &mut VmManager,
    preserve_rootfs: bool,
) -> aster_box_core::Result<()> {
    if preserve_rootfs {
        manager.destroy_preserving_rootfs().await
    } else {
        manager.destroy().await
    }
}

#[async_trait]
impl LocalExecutionBackend for VmLocalExecutionBackend {
    async fn owner_facts(
        &self,
        record: &BoxRecord,
    ) -> ExecutionManagerResult<LocalExecutionOwnerFacts> {
        let Some(entry) = self.managers.get(&record.id) else {
            return Ok(LocalExecutionOwnerFacts::unproved());
        };
        match entry.value() {
            RegisteredVm::Legacy(_) => Ok(LocalExecutionOwnerFacts::unproved()),
            RegisteredVm::Admitted(owner) => {
                if owner.progress.is_none() {
                    return Ok(LocalExecutionOwnerFacts::unproved());
                }
                owner.binding.validate_record(
                    record,
                    &self.home_dir,
                    owner.binding.store_path(),
                )?;
                Ok(LocalExecutionOwnerFacts::admitted(owner.binding.clone()))
            }
        }
    }

    async fn admit_fresh_owner(
        &self,
        admission: &super::FreshOwnerAdmission,
    ) -> ExecutionManagerResult<()> {
        use super::store::AdmissionState;
        admission.validate_scope(&self.home_dir, admission.binding().store_path())?;
        let record = admission.record();
        let metadata = self.metadata(record)?;
        let mut state = admission.state()?;
        match &*state {
            AdmissionState::Consumed(Some(expected)) => {
                let expected = expected
                    .upgrade()
                    .ok_or_else(|| legacy_lineage_error(&record.id))?;
                match self.managers.get(&record.id) {
                    Some(entry)
                        if matches!(entry.value(), RegisteredVm::Admitted(actual)
                        if Arc::ptr_eq(actual, &expected) && actual.binding == *admission.binding()) =>
                    {
                        Ok(())
                    }
                    _ => Err(legacy_lineage_error(&record.id)),
                }
            }
            AdmissionState::Consumed(None) => Err(legacy_lineage_error(&record.id)),
            AdmissionState::Unused => {
                // No await or effectful body between one-time consumption and
                // the atomic original-map insertion. Any failure stays consumed.
                *state = AdmissionState::Consumed(None);
                let mut manager = self.new_manager_from_metadata(record, metadata)?;
                let scope = crate::vm::OriginalVmScope::from_original(
                    &manager,
                    record,
                    admission.binding().store_path(),
                )
                .map_err(|e| runtime_error("bind original scope", record, e))?;
                manager
                    .bind_original_scope(scope.clone())
                    .map_err(|e| runtime_error("install original scope", record, e))?;
                let vm = Arc::new(Mutex::new(manager));
                let owner = Arc::new(AdmittedVm {
                    binding: admission.binding().clone(),
                    vm,
                    progress: Some(Mutex::new(OwnedVmProgress::new(scope, record.clone()))),
                });
                match self.managers.entry(record.id.clone()) {
                    Entry::Occupied(_) => Err(legacy_lineage_error(&record.id)),
                    Entry::Vacant(entry) => {
                        entry.insert(RegisteredVm::Admitted(Arc::clone(&owner)));
                        *state = AdmissionState::Consumed(Some(Arc::downgrade(&owner)));
                        Ok(())
                    }
                }
            }
        }
    }

    fn route_for_create(
        &self,
        _record: &BoxRecord,
    ) -> ExecutionManagerResult<crate::ManagedRuntimeRoute> {
        Ok(crate::ManagedRuntimeRoute::BoxVm)
    }

    async fn plan_create_resources(
        &self,
        record: &BoxRecord,
    ) -> ExecutionManagerResult<LocalExecutionResourcePlan> {
        self.require_legacy_lineage(&record.id)?;
        let metadata = self.metadata(record)?;
        if !metadata.plan.backend.is_sandbox() {
            return Ok(LocalExecutionResourcePlan::default());
        }

        // Resolve only the immutable image metadata needed to derive stable
        // anonymous-volume identities. The actual VolumeStore claims remain
        // in `start`, after the reservation has durably recorded ownership.
        let mut manager = self.new_manager(record)?;
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            // A Runtime registry credential is staged under the idempotent
            // create operation before this pre-reservation pass. Clone it for
            // the metadata pull, while leaving the broker entry available for
            // the later boot pull. The credential never enters the durable
            // BoxRecord.
            if let Some(broker) = &self.transient_registry_auth {
                manager.transient_registry_auth = broker.clone_auth(metadata.operation_id.as_str());
            }
        }
        let anonymous_volumes = manager
            .plan_image_anonymous_volumes()
            .await
            .map_err(|error| match error {
                BoxError::ConfigError(message) => ExecutionManagerError::InvalidRequest(message),
                error => ExecutionManagerError::Unavailable(format!(
                    "Box image resource planning failed for {}: {error}",
                    record.id
                )),
            })?;
        Ok(LocalExecutionResourcePlan { anonymous_volumes })
    }

    async fn preflight_isolation(
        &self,
        isolation: aster_box_core::ExecutionIsolation,
    ) -> ExecutionManagerResult<()> {
        // CLI and explicit readiness probes open the hypervisor.
        admit_launch(isolation)
    }

    async fn preflight(&self, record: &BoxRecord) -> ExecutionManagerResult<()> {
        self.require_legacy_lineage(&record.id)?;
        if let Some(metadata) = record.managed_execution.as_ref() {
            #[cfg(not(any(target_os = "linux", target_os = "windows")))]
            if record.isolation == aster_box_core::ExecutionIsolation::Microvm {
                VmManager::validate_virtiofs_mount_support(&metadata.request.config)
                    .map_err(|error| ExecutionManagerError::InvalidRequest(error.to_string()))?;
            }
            if !metadata.request.config.host_services.is_empty() {
                aster_box_core::resolve_execution(&metadata.request.config)
                    .map_err(|error| ExecutionManagerError::InvalidRequest(error.to_string()))?;
                #[cfg(unix)]
                for service in &metadata.request.config.host_services {
                    service
                        .validate_host_path(unsafe { libc::geteuid() })
                        .map_err(|error| {
                            ExecutionManagerError::InvalidRequest(error.to_string())
                        })?;
                }
            }
        }
        // Durable create only checks the host class so stub CI and metadata
        // tests can reserve without kvm-group access. `start` still opens it.
        admit_create(record.isolation)
    }

    async fn start(&self, record: &BoxRecord) -> ExecutionManagerResult<LocalExecutionHandle> {
        self.require_legacy_lineage(&record.id)?;
        admit_launch(record.isolation)?;
        super::record::validate_record_health(record)?;
        self.metadata(record)?;
        let box_dir = record.box_dir.clone();
        let execution_id = record.id.clone();
        tokio::task::spawn_blocking(move || {
            crate::rootfs::stage_box_terminal_rootfs_metadata(&box_dir)
        })
        .await
        .map_err(|error| {
            ExecutionManagerError::Internal(format!(
                "rootfs metadata staging task failed for {execution_id}: {error}"
            ))
        })?
        .map_err(|error| {
            ExecutionManagerError::Internal(format!(
                "failed to stage rootfs metadata for {execution_id}: {error}"
            ))
        })?;
        let mut manager = self.new_manager(record)?;
        let requested_persistence = manager.config.persistent;
        if should_reuse_preserved_rootfs(record)?
            && crate::vm::persistent_rootfs_generation_exists(&record.box_dir)
                .map_err(|error| runtime_error("inspect retained rootfs", record, error))?
        {
            manager.config.persistent = true;
        }
        let manager = Arc::new(Mutex::new(manager));
        match self.managers.entry(record.id.clone()) {
            Entry::Occupied(_) => {
                return Err(ExecutionManagerError::Unavailable(format!(
                    "execution {} already has an in-process runtime owner",
                    record.id
                )))
            }
            Entry::Vacant(entry) => {
                entry.insert(RegisteredVm::Legacy(Arc::clone(&manager)));
            }
        }

        let mut guard = manager.lock().await;
        let resource_home = self.home_dir.clone();
        let resource_record = record.clone();
        let resources = match tokio::task::spawn_blocking(move || {
            ExecutionResourceGuard::prepare(&resource_home, &resource_record)
        })
        .await
        {
            Ok(Ok(resources)) => resources,
            Ok(Err(error)) => {
                drop(guard);
                self.remove_manager(&record.id, &manager)?;
                return Err(error);
            }
            Err(error) => {
                drop(guard);
                self.remove_manager(&record.id, &manager)?;
                return Err(ExecutionManagerError::Internal(format!(
                    "managed resource preparation task failed for {}: {error}",
                    record.id
                )));
            }
        };
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        self.claim_transient_registry_auth_for_boot(&mut guard);
        if let Err(error) = guard.boot().await {
            guard.config.persistent = requested_persistence;
            // The teardown that follows deletes the box directory with the
            // only evidence for why the guest exec server never became
            // ready; retain one bounded copy of the boot logs first.
            guard.retain_boot_failure_diagnostics();
            // Sandbox boot cleanup synchronously asks the authoritative OCI
            // handler for its exit status before tearing down transient host
            // artifacts. A very short task can therefore complete while boot
            // is still establishing readiness. Keep the in-process owner so
            // `resolve_start_error` can inspect that authenticated exit and
            // project it into the durable record. Dropping the owner here
            // turns a finished one-shot into ProviderUnavailable.
            if guard.exit_code().is_some() {
                tracing::debug!(
                    execution_id = %record.id,
                    %error,
                    "Runtime completed while startup was establishing readiness"
                );
                // cleanup_boot_failure already stopped the provider. Mark
                // Stopped so the follow-up inspect does not destroy again.
                *guard.state.write().await = crate::BoxState::Stopped;
                resources.disarm();
                drop(guard);
                return Err(ExecutionManagerError::Unavailable(format!(
                    "execution {} completed during startup ({error})",
                    record.id
                )));
            }
            drop(guard);
            self.remove_manager(&record.id, &manager)?;
            let rollback = match tokio::task::spawn_blocking(move || resources.rollback()).await {
                Ok(result) => result,
                Err(rollback_error) => Err(ExecutionManagerError::Internal(format!(
                    "managed resource rollback task failed for {}: {rollback_error}",
                    record.id
                ))),
            };
            let primary = runtime_error("start", record, error);
            return Err(match rollback {
                Ok(()) => primary,
                Err(rollback) => ExecutionManagerError::Internal(format!(
                    "{primary}; managed resource rollback also failed: {rollback}"
                )),
            });
        }
        guard.config.persistent = requested_persistence;
        resources.disarm();
        let exited_during_start = guard
            .try_wait_exit()
            .await
            .map_err(|error| runtime_error("collect startup exit status", record, error))?
            .is_some()
            || guard.has_exited().await;
        if exited_during_start {
            let exit_code = guard.exit_code();
            tracing::debug!(
                execution_id = %record.id,
                ?exit_code,
                "execution completed during startup; inspect will publish the exit"
            );
            drop(guard);
            // Leave the owner registered. resolve_start_error inspects it and
            // finish_registered_terminal drops it after the exit is durable.
            // Removing it first loses the status and blocks OnFailure restart.
            return Err(ExecutionManagerError::Unavailable(format!(
                "execution {} completed during startup (exit_code={exit_code:?})",
                record.id
            )));
        }
        // #414 may leave Created after restore soft-probe failure. Do not invent
        // a start handle (and thus durable Running) from PID + socket alone (#416).
        if let Err(error) = self
            .require_authenticated_ready_for_start(record, &mut guard)
            .await
        {
            // Keep the in-process owner so inspect/promote_if_ready can authenticate.
            drop(guard);
            return Err(error);
        }
        self.handle_from_manager(record, &guard).await
    }

    async fn inspect(
        &self,
        record: &BoxRecord,
    ) -> ExecutionManagerResult<LocalExecutionObservation> {
        self.require_legacy_lineage(&record.id)?;
        let metadata = self.metadata(record)?;
        if metadata.plan.backend.is_sandbox() {
            return self.inspect_sandbox(record).await;
        }
        if let Some(manager) = self.manager(&record.id)? {
            return self.inspect_registered(record, manager).await;
        }
        let manager = self.recover_microvm(record).await?;
        self.inspect_registered(record, manager).await
    }

    async fn pause(
        &self,
        record: &BoxRecord,
        keep_memory: bool,
    ) -> ExecutionManagerResult<LocalExecutionHandle> {
        self.require_legacy_lineage(&record.id)?;
        let metadata = self.metadata(record)?;
        if metadata.plan.backend.is_sandbox() {
            if !keep_memory {
                return Err(unsupported(
                    record,
                    "pause without memory retention",
                    "the Sandbox backend",
                ));
            }
            return self.pause_sandbox(record).await;
        }
        if !keep_memory {
            return Err(unsupported(
                record,
                "pause without memory retention",
                "the local MicroVM backend",
            ));
        }
        #[cfg(windows)]
        {
            Err(unsupported(
                record,
                "pause",
                "the local MicroVM backend on Windows",
            ))
        }
        #[cfg(not(windows))]
        {
            let shared = self.require_microvm(record).await?;
            let manager = shared.lock().await;
            require_recorded_pid(record, &manager).await?;
            manager
                .pause()
                .await
                .map_err(|error| runtime_error("pause", record, error))?;
            self.handle_from_manager(record, &manager).await
        }
    }

    async fn resume(&self, record: &BoxRecord) -> ExecutionManagerResult<LocalExecutionHandle> {
        self.require_legacy_lineage(&record.id)?;
        let metadata = self.metadata(record)?;
        if metadata.plan.backend.is_sandbox() {
            return self.resume_sandbox(record).await;
        }
        #[cfg(windows)]
        {
            Err(unsupported(
                record,
                "resume",
                "the local MicroVM backend on Windows",
            ))
        }
        #[cfg(not(windows))]
        {
            let shared = self.require_microvm(record).await?;
            let manager = shared.lock().await;
            require_recorded_pid(record, &manager).await?;
            manager
                .resume()
                .await
                .map_err(|error| runtime_error("resume", record, error))?;
            self.handle_from_manager(record, &manager).await
        }
    }

    async fn prepare_quiescent_rootfs(&self, record: &BoxRecord) -> ExecutionManagerResult<()> {
        self.require_legacy_lineage(&record.id)?;
        self.new_manager(record)?
            .prepare_preserved_rootfs()
            .map(|_| ())
            .map_err(|error| runtime_error("prepare quiescent rootfs", record, error))
    }

    async fn cleanup_quiescent_rootfs(&self, record: &BoxRecord) -> ExecutionManagerResult<()> {
        self.require_legacy_lineage(&record.id)?;
        self.new_manager(record)?
            .cleanup_preserved_rootfs()
            .map_err(|error| runtime_error("clean up quiescent rootfs", record, error))
    }

    async fn kill(&self, record: &BoxRecord) -> ExecutionManagerResult<KillOutcome> {
        self.require_legacy_lineage(&record.id)?;
        Ok(self.terminate_execution(record).await?.outcome)
    }

    async fn kill_with_status(
        &self,
        record: &BoxRecord,
    ) -> ExecutionManagerResult<LocalExecutionTermination> {
        self.require_legacy_lineage(&record.id)?;
        self.terminate_execution(record).await
    }

    async fn stop_for_restart(
        &self,
        record: &BoxRecord,
        timeout_secs: Option<u64>,
    ) -> ExecutionManagerResult<KillOutcome> {
        self.require_legacy_lineage(&record.id)?;
        let metadata = self.metadata(record)?;
        #[cfg(target_os = "linux")]
        if metadata.plan.backend.is_sandbox() {
            super::snapshot::persist_sandbox_snapshot_mappings(record)?;
        }
        let timeout_secs = timeout_secs.or(record.stop_timeout);
        if let Some(manager) = self.manager(&record.id)? {
            return Ok(self
                .destroy_registered(record, manager, false, true, timeout_secs)
                .await?
                .outcome);
        }
        match metadata.plan.backend {
            ExecutionBackend::AsterOci => Ok(self
                .destroy_detached_sandbox(record, false, true, timeout_secs)
                .await?
                .outcome),
            ExecutionBackend::Krun => {
                let manager = self.recover_microvm(record).await?;
                Ok(self
                    .destroy_registered(record, manager, false, true, timeout_secs)
                    .await?
                    .outcome)
            }
        }
    }
}

pub(super) fn should_force_rootfs_preservation(record: &BoxRecord) -> ExecutionManagerResult<bool> {
    let state = super::support::managed_state(record)?;
    let metadata = record.managed_execution.as_ref().ok_or_else(|| {
        ExecutionManagerError::Internal(format!(
            "execution {} has no managed lifecycle metadata",
            record.id
        ))
    })?;
    Ok(match state {
        // A foreground one-shot can finish before readiness is published. Its
        // exact status is already terminal, but the caller still has to drain
        // stdout/stderr and archive an auto-removed result. Tear down mounts
        // and runtime processes while retaining the box directory until that
        // caller completes its normal terminal cleanup.
        ManagedExecutionState::Starting => true,
        ManagedExecutionState::Pausing => matches!(
            metadata.pending_operation.as_ref(),
            Some(ManagedExecutionOperation::Pause {
                keep_memory: false,
                ..
            })
        ),
        ManagedExecutionState::Resuming => !metadata.paused_with_memory,
        ManagedExecutionState::RestartStopping | ManagedExecutionState::RestartStarting => true,
        _ => false,
    })
}

fn should_reuse_preserved_rootfs(record: &BoxRecord) -> ExecutionManagerResult<bool> {
    Ok(matches!(
        super::support::managed_state(record)?,
        ManagedExecutionState::Resuming | ManagedExecutionState::RestartStarting
    ) && should_force_rootfs_preservation(record)?)
}

fn graceful_stop_options(
    record: &BoxRecord,
    timeout_secs: Option<u64>,
) -> ExecutionManagerResult<Option<(i32, u64)>> {
    if timeout_secs.is_none() && record.stop_signal.is_none() {
        return Ok(None);
    }
    let timeout_ms = timeout_secs
        .unwrap_or(DEFAULT_SHUTDOWN_TIMEOUT_MS / 1_000)
        .checked_mul(1_000)
        .ok_or_else(|| {
            ExecutionManagerError::InvalidRequest(format!(
                "stop timeout is too large for execution {}",
                record.id
            ))
        })?;
    let signal = record
        .stop_signal
        .as_deref()
        .map(aster_box_core::vmm::parse_signal_name)
        .unwrap_or(libc::SIGTERM);
    Ok(Some((signal, timeout_ms)))
}

// Callers keep their existing metadata/route validation. This helper only
// preserves the record's original process identity; it cannot validate the
// current canonical generation or turn a failed query into exit evidence.
fn require_attached_record_identity(
    record: &BoxRecord,
    located: LocatedProcess,
) -> ExecutionManagerResult<(u32, u64)> {
    let unproved = || {
        ExecutionManagerError::Unavailable(format!(
            "execution {} original attached process identity is missing, mismatched or unproved",
            record.id
        ))
    };
    let (pid, start_time) = record.pid.zip(record.pid_start_time).ok_or_else(unproved)?;
    if pid == 0
        || located.pid != pid
        || located.start_time != Some(start_time)
        || crate::process::pid_start_time(pid) != Some(start_time)
        || !crate::process::is_process_alive_with_identity(pid, Some(start_time))
    {
        return Err(unproved());
    }
    Ok((pid, start_time))
}

#[cfg(not(windows))]
async fn require_recorded_pid(
    record: &BoxRecord,
    manager: &VmManager,
) -> ExecutionManagerResult<()> {
    let pid = manager.pid().await.ok_or_else(|| {
        ExecutionManagerError::Unavailable(format!(
            "execution {} has no original attached runtime PID",
            record.id
        ))
    })?;
    require_attached_record_identity(
        record,
        LocatedProcess {
            pid,
            start_time: record.pid_start_time,
        },
    )?;
    Ok(())
}

fn visible_active_state(record: &BoxRecord) -> ExecutionManagerResult<ExecutionState> {
    match managed_state(record)? {
        ManagedExecutionState::Paused => Ok(ExecutionState::Paused),
        ManagedExecutionState::Resuming => {
            let metadata = record.managed_execution.as_ref().ok_or_else(|| {
                ExecutionManagerError::Internal(format!(
                    "execution {} has no managed lifecycle metadata",
                    record.id
                ))
            })?;
            if metadata.paused_with_memory {
                Ok(ExecutionState::Paused)
            } else {
                // A cold-paused execution has no provider process. Any live
                // process observed while its resume is pending is therefore
                // the replacement generation started before record commit.
                Ok(ExecutionState::Running)
            }
        }
        ManagedExecutionState::Starting
        | ManagedExecutionState::RestartStarting
        | ManagedExecutionState::Running
        | ManagedExecutionState::Pausing
        | ManagedExecutionState::Killing => Ok(ExecutionState::Running),
        ManagedExecutionState::Snapshotting => match record
            .managed_execution
            .as_ref()
            .and_then(|metadata| metadata.pending_operation.as_ref())
        {
            Some(ManagedExecutionOperation::Snapshot {
                source_state: ManagedExecutionState::Running,
                ..
            }) => Ok(ExecutionState::Running),
            Some(ManagedExecutionOperation::Snapshot {
                source_state: ManagedExecutionState::Paused,
                ..
            }) => Ok(ExecutionState::Paused),
            _ => Err(ExecutionManagerError::Internal(format!(
                "execution {} has invalid snapshot metadata",
                record.id
            ))),
        },
        ManagedExecutionState::RestartStopping => match record
            .managed_execution
            .as_ref()
            .and_then(|metadata| metadata.pending_operation.as_ref())
        {
            Some(ManagedExecutionOperation::Restart {
                source_state: ManagedExecutionState::Paused,
                ..
            }) => Ok(ExecutionState::Paused),
            Some(ManagedExecutionOperation::Restart {
                source_state: ManagedExecutionState::Running,
                ..
            }) => Ok(ExecutionState::Running),
            _ => Err(ExecutionManagerError::Internal(format!(
                "execution {} has invalid restart teardown metadata",
                record.id
            ))),
        },
        state => Err(ExecutionManagerError::Internal(format!(
            "execution {} has no active runtime in managed state {state}",
            record.id
        ))),
    }
}

fn managed_state(record: &BoxRecord) -> ExecutionManagerResult<ManagedExecutionState> {
    record
        .managed_state()
        .map_err(|error| ExecutionManagerError::Internal(error.to_string()))?
        .ok_or_else(|| {
            ExecutionManagerError::Internal(format!("execution {} is not managed", record.id))
        })
}

fn execution_id(record: &BoxRecord) -> ExecutionManagerResult<ExecutionId> {
    ExecutionId::new(record.id.clone())
        .map_err(|error| ExecutionManagerError::Internal(error.to_string()))
}

/// Authenticate a managed terminal exit on Windows before projecting Stopped.
///
/// Clean provider/shim zero without a durable guest exit file must not invent
/// guest success — same honesty as `resolve_workload_exit_code` / destroy.
#[cfg(target_os = "windows")]
fn authenticate_windows_managed_terminal_exit(
    box_dir: &Path,
    log_config: &aster_box_core::log::LogConfig,
    exit_code: Option<i32>,
) -> Option<i32> {
    let Some(code) = exit_code else {
        return None;
    };
    match crate::vm::collect_windows_guest_result(box_dir, log_config, code) {
        Ok(authenticated) => Some(authenticated),
        // Nonzero provider crash evidence is kept when the guest never wrote
        // its exit file; clean zero is refused.
        Err(_) => (code != 0).then_some(code),
    }
}

fn runtime_error(
    action: &str,
    record: &BoxRecord,
    error: impl std::fmt::Display,
) -> ExecutionManagerError {
    ExecutionManagerError::Internal(format!(
        "failed to {action} execution {}: {error}",
        record.id
    ))
}

fn unsupported(record: &BoxRecord, operation: &str, backend: &str) -> ExecutionManagerError {
    ExecutionManagerError::Unavailable(format!(
        "{operation} is not supported by {backend} for execution {}",
        record.id
    ))
}

#[cfg(unix)]
async fn exec_endpoint_ready(_box_id: &str, path: Option<&Path>) -> bool {
    let Some(path) = path else {
        return false;
    };
    let attempt = async {
        let client = crate::ExecClient::connect(path).await.ok()?;
        client.heartbeat().await.ok().filter(|ready| *ready)
    };
    tokio::time::timeout(Duration::from_millis(500), attempt)
        .await
        .ok()
        .flatten()
        .is_some()
}

/// Windows readiness must authenticate the named-pipe exec channel — layout
/// path presence alone must not invent Ready/Running (#407/#408/#411 parity).
#[cfg(not(unix))]
async fn exec_endpoint_ready(box_id: &str, path: Option<&Path>) -> bool {
    let Some(layout_path) = path else {
        return false;
    };
    let guest_control_ready =
        layout_path.with_file_name(aster_box_core::exec::WINDOWS_GUEST_CONTROL_READY_FILE);
    if !guest_control_ready.is_file() {
        return false;
    }
    let pipe = std::path::PathBuf::from(aster_box_core::exec::windows_exec_pipe_path(box_id));
    let attempt = async {
        let client = crate::ExecClient::connect(&pipe).await.ok()?;
        client.heartbeat().await.ok().filter(|ready| *ready)
    };
    tokio::time::timeout(Duration::from_millis(500), attempt)
        .await
        .ok()
        .flatten()
        .is_some()
}

#[cfg(test)]
#[path = "vm_backend_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/owned_vm_close.rs"]
mod owned_vm_close_tests;

#[cfg(test)]
#[path = "tests/attached_vm_identity.rs"]
mod attached_vm_identity_tests;

// Original-owner component only. No production continuation caller is connected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OwnedVmJobKind {
    StageMetadata,
    PrepareResources,
    RollbackResources,
    FinalizeVm,
    CleanupAnonymousVolumes,
}

impl OwnedVmJobKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::StageMetadata => "stage-metadata",
            Self::PrepareResources => "prepare-resources",
            Self::RollbackResources => "rollback-resources",
            Self::FinalizeVm => "finalize-vm",
            Self::CleanupAnonymousVolumes => "cleanup-anonymous-volumes",
        }
    }
    fn parse(value: &str) -> ExecutionManagerResult<Self> {
        match value {
            "stage-metadata" => Ok(Self::StageMetadata),
            "prepare-resources" => Ok(Self::PrepareResources),
            "rollback-resources" => Ok(Self::RollbackResources),
            "finalize-vm" => Ok(Self::FinalizeVm),
            "cleanup-anonymous-volumes" => Ok(Self::CleanupAnonymousVolumes),
            _ => Err(ExecutionManagerError::InvalidRequest(format!(
                "unknown OwnedVmJobKind: {value}"
            ))),
        }
    }
}

enum OwnedVmJobOutput {
    Metadata(aster_box_core::Result<()>),
    Prepare(ExecutionManagerResult<()>),
    Rollback(ExecutionManagerResult<()>),
    Finalize(aster_box_core::Result<()>),
    Anonymous(aster_box_core::Result<()>),
}

impl OwnedVmJobOutput {
    fn result(&self) -> ExecutionManagerResult<()> {
        match self {
            Self::Metadata(Ok(()))
            | Self::Prepare(Ok(()))
            | Self::Rollback(Ok(()))
            | Self::Finalize(Ok(()))
            | Self::Anonymous(Ok(())) => Ok(()),
            Self::Metadata(Err(e)) | Self::Finalize(Err(e)) | Self::Anonymous(Err(e)) => Err(
                ExecutionManagerError::Unavailable(format!("original owned job failed: {e}")),
            ),
            Self::Prepare(Err(e)) | Self::Rollback(Err(e)) => Err(
                ExecutionManagerError::Unavailable(format!("original owned job failed: {e}")),
            ),
        }
    }
}

enum OwnedVmJobSlot {
    Idle,
    Running {
        kind: OwnedVmJobKind,
        binding: super::store::OwnedStageBinding,
        handle: tokio::task::JoinHandle<OwnedVmJobOutput>,
    },
    Ready {
        kind: OwnedVmJobKind,
        binding: super::store::OwnedStageBinding,
        result: Result<OwnedVmJobOutput, tokio::task::JoinError>,
    },
}

// Completed history contains actual results only, never another handle slot.
struct OwnedVmCompleted {
    kind: OwnedVmJobKind,
    binding: super::store::OwnedStageBinding,
    result: Result<OwnedVmJobOutput, tokio::task::JoinError>,
}

struct OwnedVmProgress {
    scope: crate::vm::OriginalVmScope,
    launch_record: BoxRecord,
    acquisition: crate::vm::OriginalAcquisition,
    resources: Option<Arc<std::sync::Mutex<ExecutionResourceGuard>>>,
    finalization: Option<Arc<std::sync::Mutex<crate::vm::OriginalVmFinalization>>>,
    anonymous: Option<Arc<std::sync::Mutex<OwnedAnonymousCleanup>>>,
    job: OwnedVmJobSlot,
    completed: Vec<OwnedVmCompleted>,
    physical: Option<aster_box_core::Result<aster_box_core::vmm::VmExitConfirmation>>,
    physical_budget: Option<aster_box_core::vmm::OwnedCloseBudget>,
    cleanup_budget: Option<aster_box_core::vmm::OwnedCleanupBudget>,
    #[cfg(test)]
    hooks: Option<OwnedJobHooks>,
}

impl OwnedVmProgress {
    fn new(scope: crate::vm::OriginalVmScope, launch_record: BoxRecord) -> Self {
        Self {
            scope,
            launch_record,
            acquisition: crate::vm::OriginalAcquisition::NeverAcquired,
            resources: None,
            finalization: None,
            anonymous: None,
            job: OwnedVmJobSlot::Idle,
            completed: Vec::new(),
            physical: None,
            physical_budget: None,
            cleanup_budget: None,
            #[cfg(test)]
            hooks: None,
        }
    }

    fn validate_bound_settings(&self, record: &BoxRecord) -> ExecutionManagerResult<()> {
        let actual = serde_json::to_value(record).map_err(|e| {
            ExecutionManagerError::Unavailable(format!("original settings comparison failed: {e}"))
        })?;
        let captured = serde_json::to_value(&self.launch_record).map_err(|e| {
            ExecutionManagerError::Unavailable(format!(
                "original captured settings comparison failed: {e}"
            ))
        })?;
        if actual != captured {
            return Err(ExecutionManagerError::Unavailable(
                "original prepared settings drift refused".into(),
            ));
        }
        Ok(())
    }

    fn refresh_inactive_network(
        &mut self,
        record: &BoxRecord,
        vm: &mut VmManager,
    ) -> ExecutionManagerResult<()> {
        let mut permitted = self.launch_record.clone();
        permitted.network_mode = record.network_mode.clone();
        permitted.network_name = record.network_name.clone();
        let actual = serde_json::to_value(record).map_err(|e| {
            ExecutionManagerError::Unavailable(format!("original settings comparison failed: {e}"))
        })?;
        let permitted = serde_json::to_value(&permitted).map_err(|e| {
            ExecutionManagerError::Unavailable(format!("original settings comparison failed: {e}"))
        })?;
        if actual != permitted {
            return Err(ExecutionManagerError::Unavailable(
                "unknown inactive settings drift refused".into(),
            ));
        }
        let captured = serde_json::to_value(&self.launch_record).map_err(|e| {
            ExecutionManagerError::Unavailable(format!(
                "original captured settings comparison failed: {e}"
            ))
        })?;
        if actual == captured {
            return Ok(());
        }
        if self.acquisition != crate::vm::OriginalAcquisition::NeverAcquired
            || !matches!(self.job, OwnedVmJobSlot::Idle)
            || self.resources.is_some()
        {
            return Err(ExecutionManagerError::Unavailable(
                "inactive network drift after preparation refused".into(),
            ));
        }
        vm.config.network = record.network_mode.clone();
        self.launch_record = record.clone();
        Ok(())
    }

    fn reconcile_ready(
        &self,
        call: &super::store::ValidatedOwnedCall<'_>,
    ) -> ExecutionManagerResult<()> {
        match &self.job {
            OwnedVmJobSlot::Idle => Ok(()),
            OwnedVmJobSlot::Running { binding, .. } => binding.validate_call(call),
            OwnedVmJobSlot::Ready {
                binding, result, ..
            } => {
                binding.validate_call(call)?;
                match result {
                    Ok(output) => output.result(),
                    Err(e) => Err(ExecutionManagerError::Unavailable(format!(
                        "original owned job join is unproved: {e}"
                    ))),
                }
            }
        }
    }

    fn install(
        &mut self,
        call: &super::store::ValidatedOwnedCall<'_>,
        work: OwnedVmWork,
        budget: aster_box_core::vmm::OwnedCleanupBudget,
    ) -> ExecutionManagerResult<()> {
        let binding = call.stage(budget)?;
        self.validate_bound_settings(call.record)?;
        work.validate_original(call, self)?;
        let kind = work.kind();
        match &self.job {
            OwnedVmJobSlot::Running { .. } => {
                return Err(ExecutionManagerError::Unavailable(
                    "original owned job is already running".into(),
                ))
            }
            OwnedVmJobSlot::Ready {
                kind: old,
                binding: prior,
                result,
            } => {
                prior.validate_call(call)?;
                let actual = result.as_ref().map_err(|error| {
                    ExecutionManagerError::Unavailable(format!(
                        "original join remains unproved: {error}"
                    ))
                })?;
                match actual.result() {
                    Ok(()) if *old == kind => return Ok(()),
                    Ok(()) => {}
                    Err(error) => {
                        let rollback =
                            kind == OwnedVmJobKind::RollbackResources && work.remaining_known()?;
                        let retry = *old == kind
                            && budget.deadline() > prior.deadline()
                            && work.remaining_known()?;
                        if !rollback && !retry {
                            return Err(error);
                        }
                    }
                }
            }
            OwnedVmJobSlot::Idle => {}
        }
        for prior in &self.completed {
            if prior.kind == kind {
                prior.binding.validate_call(call)?;
                match &prior.result {
                    Ok(output) if output.result().is_ok() => return Ok(()),
                    Ok(_)
                        if budget.deadline() > prior.binding.deadline()
                            && work.remaining_known()? => {}
                    Ok(output) => return output.result(),
                    Err(error) => {
                        return Err(ExecutionManagerError::Unavailable(format!(
                            "original completed join remains unproved: {error}"
                        )))
                    }
                }
            }
        }
        // Keep the latest actual completed result per closed family (at most5).
        // Only Ready can leave the unique slot; no handle enters this history.
        if let OwnedVmJobSlot::Ready {
            kind,
            binding,
            result,
        } = std::mem::replace(&mut self.job, OwnedVmJobSlot::Idle)
        {
            let prior = OwnedVmCompleted {
                kind,
                binding,
                result,
            };
            if let Some(existing) = self
                .completed
                .iter_mut()
                .find(|existing| existing.kind == kind)
            {
                *existing = prior;
            } else {
                self.completed.push(prior);
            }
        }
        let guard = super::lifecycle_lock::retain_original_guard(call.lifecycle);
        #[cfg(test)]
        let hooks = self.hooks.clone();
        let handle = spawn_original_work(
            work,
            guard,
            budget,
            #[cfg(test)]
            hooks,
        );
        // No await/fallible operation exists between spawn and original storage.
        self.job = OwnedVmJobSlot::Running {
            kind,
            binding,
            handle,
        };
        Ok(())
    }

    async fn poll_job(
        &mut self,
        call: &super::store::ValidatedOwnedCall<'_>,
        budget: aster_box_core::vmm::OwnedCleanupBudget,
    ) -> ExecutionManagerResult<()> {
        use std::future::Future;
        self.reconcile_ready(call)?;
        // A later explicitly admitted caller may spend its own bounded wait;
        // the actual job still owns/uses its unchanged original work budget.
        let wait_deadline = budget.deadline();
        let poll = std::future::poll_fn(|cx| {
            let ready = match &mut self.job {
                OwnedVmJobSlot::Running {
                    kind,
                    binding,
                    handle,
                } => match std::pin::Pin::new(handle).poll(cx) {
                    std::task::Poll::Pending => return std::task::Poll::Pending,
                    std::task::Poll::Ready(result) => (*kind, binding.clone(), result),
                },
                _ => return std::task::Poll::Ready(self.reconcile_ready(call)),
            };
            // The actual result becomes owner storage in the same Ready poll.
            self.job = OwnedVmJobSlot::Ready {
                kind: ready.0,
                binding: ready.1,
                result: ready.2,
            };
            std::task::Poll::Ready(self.reconcile_ready(call))
        });
        tokio::time::timeout_at(tokio::time::Instant::from_std(wait_deadline), poll)
            .await
            .map_err(|_| {
                ExecutionManagerError::Unavailable(
                    "original ancillary wait expired; job retained".into(),
                )
            })??;
        if std::time::Instant::now() >= wait_deadline {
            return Err(ExecutionManagerError::Unavailable(
                "original ancillary attempt expired; actual result retained".into(),
            ));
        }
        Ok(())
    }
}

struct OwnedAnonymousCleanup {
    home: PathBuf,
    owner: String,
    names: Vec<(String, bool)>,
    unproved: bool,
}
impl OwnedAnonymousCleanup {
    fn run(&mut self, budget: aster_box_core::vmm::OwnedCleanupBudget) -> aster_box_core::Result<()> {
        if self.unproved {
            return Err(BoxError::StateError(
                "original anonymous cleanup effect is unproved".into(),
            ));
        }
        let store =
            crate::VolumeStore::new(self.home.join("volumes.json"), self.home.join("volumes"));
        for (name, complete) in &mut self.names {
            if *complete {
                continue;
            }
            require_job_time(budget)?;
            // Mark entered before an error can conceal a committed removal.
            self.unproved = true;
            store.remove_anonymous(name, &self.owner)?;
            *complete = true;
            self.unproved = false;
        }
        require_job_time(budget)
    }
}

enum OwnedVmWork {
    StageMetadata(PathBuf),
    PrepareResources {
        cell: Arc<std::sync::Mutex<ExecutionResourceGuard>>,
        record: BoxRecord,
    },
    RollbackResources(Arc<std::sync::Mutex<ExecutionResourceGuard>>),
    FinalizeVm(Arc<std::sync::Mutex<crate::vm::OriginalVmFinalization>>),
    CleanupAnonymousVolumes(Arc<std::sync::Mutex<OwnedAnonymousCleanup>>),
}
impl OwnedVmWork {
    fn remaining_known(&self) -> ExecutionManagerResult<bool> {
        match self {
            Self::StageMetadata(_) | Self::PrepareResources { .. } => Ok(false),
            Self::RollbackResources(cell) => cell
                .try_lock()
                .map(|guard| guard.remaining_rollback_known())
                .map_err(|_| {
                    ExecutionManagerError::Unavailable(
                        "original rollback cell unavailable/unproved".into(),
                    )
                }),
            Self::FinalizeVm(cell) => cell
                .try_lock()
                .map(|guard| guard.remaining_steps_known())
                .map_err(|_| {
                    ExecutionManagerError::Unavailable(
                        "original finalization cell unavailable/unproved".into(),
                    )
                }),
            Self::CleanupAnonymousVolumes(cell) => {
                cell.try_lock().map(|guard| !guard.unproved).map_err(|_| {
                    ExecutionManagerError::Unavailable(
                        "original anonymous cell unavailable/unproved".into(),
                    )
                })
            }
        }
    }

    fn validate_original(
        &self,
        call: &super::store::ValidatedOwnedCall<'_>,
        progress: &OwnedVmProgress,
    ) -> ExecutionManagerResult<()> {
        let same = match self {
            Self::StageMetadata(path) => path.as_os_str() == call.record.box_dir.as_os_str(),
            Self::PrepareResources { cell, record } => {
                progress.validate_bound_settings(record)?;
                progress
                    .resources
                    .as_ref()
                    .is_some_and(|original| Arc::ptr_eq(original, cell))
            }
            Self::RollbackResources(cell) => progress
                .resources
                .as_ref()
                .is_some_and(|original| Arc::ptr_eq(original, cell)),
            Self::FinalizeVm(cell) => progress
                .finalization
                .as_ref()
                .is_some_and(|original| Arc::ptr_eq(original, cell)),
            Self::CleanupAnonymousVolumes(cell) => progress
                .anonymous
                .as_ref()
                .is_some_and(|original| Arc::ptr_eq(original, cell)),
        };
        if !same {
            return Err(ExecutionManagerError::Unavailable(
                "foreign original job input/cell refused".into(),
            ));
        }
        Ok(())
    }

    fn kind(&self) -> OwnedVmJobKind {
        match self {
            Self::StageMetadata(_) => OwnedVmJobKind::StageMetadata,
            Self::PrepareResources { .. } => OwnedVmJobKind::PrepareResources,
            Self::RollbackResources(_) => OwnedVmJobKind::RollbackResources,
            Self::FinalizeVm(_) => OwnedVmJobKind::FinalizeVm,
            Self::CleanupAnonymousVolumes(_) => OwnedVmJobKind::CleanupAnonymousVolumes,
        }
    }
    fn run(self, budget: aster_box_core::vmm::OwnedCleanupBudget) -> OwnedVmJobOutput {
        match self {
            Self::StageMetadata(path) => OwnedVmJobOutput::Metadata(
                require_job_time(budget)
                    .and_then(|()| crate::rootfs::stage_box_terminal_rootfs_metadata(&path))
                    .and_then(|()| require_job_time(budget)),
            ),
            Self::PrepareResources { cell, record } => OwnedVmJobOutput::Prepare(
                cell.lock()
                    .map_err(|_| {
                        ExecutionManagerError::Unavailable("original resource cell poisoned".into())
                    })
                    .and_then(|mut g| g.prepare_retained(&record, budget)),
            ),
            Self::RollbackResources(cell) => OwnedVmJobOutput::Rollback(
                cell.lock()
                    .map_err(|_| {
                        ExecutionManagerError::Unavailable("original resource cell poisoned".into())
                    })
                    .and_then(|mut g| g.rollback_retained(budget)),
            ),
            Self::FinalizeVm(cell) => OwnedVmJobOutput::Finalize(
                cell.lock()
                    .map_err(|_| BoxError::StateError("original finalization cell poisoned".into()))
                    .and_then(|mut g| g.run(budget)),
            ),
            Self::CleanupAnonymousVolumes(cell) => OwnedVmJobOutput::Anonymous(
                cell.lock()
                    .map_err(|_| BoxError::StateError("original anonymous cell poisoned".into()))
                    .and_then(|mut g| g.run(budget)),
            ),
        }
    }
}
fn require_job_time(budget: aster_box_core::vmm::OwnedCleanupBudget) -> aster_box_core::Result<()> {
    if budget.remaining_at(std::time::Instant::now()).is_zero() {
        Err(BoxError::StateError(
            "original ancillary job deadline expired".into(),
        ))
    } else {
        Ok(())
    }
}

#[cfg(test)]
#[derive(Clone)]
struct OwnedJobHooks {
    before: Option<(Arc<std::sync::Barrier>, Arc<std::sync::Barrier>)>,
    after: Option<(Arc<std::sync::Barrier>, Arc<std::sync::Barrier>)>,
    panic_after: bool,
    error_after: bool,
}

fn spawn_original_work(
    work: OwnedVmWork,
    guard: Arc<super::ExecutionLifecycleLock>,
    budget: aster_box_core::vmm::OwnedCleanupBudget,
    #[cfg(test)] hooks: Option<OwnedJobHooks>,
) -> tokio::task::JoinHandle<OwnedVmJobOutput> {
    // Closed concrete dispatch only, never a generic closure/future host.
    tokio::task::spawn_blocking(move || {
        #[cfg(test)]
        if let Some(h) = &hooks {
            if let Some((entered, release)) = &h.before {
                entered.wait();
                release.wait();
            }
        }
        let output = work.run(budget);
        #[cfg(test)]
        let mut output = output;
        #[cfg(test)]
        if let Some(h) = &hooks {
            if let Some((entered, release)) = &h.after {
                entered.wait();
                release.wait();
            }
            if h.panic_after {
                panic!("controlled owned-job panic after actual work");
            }
            if h.error_after && output.result().is_ok() {
                output = match output {
                    OwnedVmJobOutput::Metadata(_) => OwnedVmJobOutput::Metadata(Err(
                        BoxError::StateError("controlled late metadata error".into()),
                    )),
                    OwnedVmJobOutput::Prepare(_) => {
                        OwnedVmJobOutput::Prepare(Err(ExecutionManagerError::Unavailable(
                            "controlled late preparation error".into(),
                        )))
                    }
                    OwnedVmJobOutput::Rollback(_) => OwnedVmJobOutput::Rollback(Err(
                        ExecutionManagerError::Unavailable("controlled late rollback error".into()),
                    )),
                    OwnedVmJobOutput::Finalize(_) => OwnedVmJobOutput::Finalize(Err(
                        BoxError::StateError("controlled late finalization error".into()),
                    )),
                    OwnedVmJobOutput::Anonymous(_) => OwnedVmJobOutput::Anonymous(Err(
                        BoxError::StateError("controlled late anonymous error".into()),
                    )),
                };
            }
        }
        // Explicitly retain the actual File guard until actual work/late hooks end.
        // Panic unwinding also drops it. No result contains this guard/owner.
        drop(guard);
        output
    })
}

impl VmLocalExecutionBackend {
    fn exact_original(
        &self,
        call: &super::store::ValidatedOwnedCall<'_>,
    ) -> ExecutionManagerResult<Arc<AdmittedVm>> {
        let owner = match self.managers.get(&call.record.id) {
            Some(entry) => match entry.value() {
                RegisteredVm::Admitted(owner) => Arc::clone(owner),
                _ => return Err(legacy_lineage_error(&call.record.id)),
            },
            None => return Err(legacy_lineage_error(&call.record.id)),
        };
        call.validate(&owner.binding)?;
        if owner.progress.is_none() {
            return Err(legacy_lineage_error(&call.record.id));
        }
        Ok(owner)
    }
    fn recheck_original(
        &self,
        call: &super::store::ValidatedOwnedCall<'_>,
        expected: &Arc<AdmittedVm>,
    ) -> ExecutionManagerResult<()> {
        call.validate(&expected.binding)?;
        match self.managers.get(&call.record.id) {
            Some(entry) if matches!(entry.value(),RegisteredVm::Admitted(actual) if Arc::ptr_eq(actual,expected)) => {
                Ok(())
            }
            _ => Err(legacy_lineage_error(&call.record.id)),
        }
    }
    async fn run_original_preparation(
        &self,
        call: &super::store::ValidatedOwnedCall<'_>,
        budget: aster_box_core::vmm::OwnedCleanupBudget,
    ) -> ExecutionManagerResult<()> {
        let owner = self.exact_original(call)?;
        let mut progress = tokio::time::timeout_at(
            tokio::time::Instant::from_std(budget.deadline()),
            owner
                .progress
                .as_ref()
                .ok_or_else(|| legacy_lineage_error(&call.record.id))?
                .lock(),
        )
        .await
        .map_err(|_| {
            ExecutionManagerError::Unavailable(
                "original owner ancillary lock deadline expired".into(),
            )
        })?;
        self.recheck_original(call, &owner)?;
        require_job_time(budget)
            .map_err(|error| runtime_error("original ancillary admission", call.record, error))?;
        {
            let mut vm = tokio::time::timeout_at(
                tokio::time::Instant::from_std(budget.deadline()),
                owner.vm.lock(),
            )
            .await
            .map_err(|_| {
                ExecutionManagerError::Unavailable("original settings lock deadline expired".into())
            })?;
            self.recheck_original(call, &owner)?;
            require_job_time(budget).map_err(|error| {
                runtime_error("original ancillary admission", call.record, error)
            })?;
            progress.refresh_inactive_network(call.record, &mut vm)?;
        }
        progress.install(
            call,
            OwnedVmWork::StageMetadata(call.record.box_dir.clone()),
            budget,
        )?;
        progress.poll_job(call, budget).await?;
        self.recheck_original(call, &owner)?;
        require_job_time(budget)
            .map_err(|error| runtime_error("original ancillary admission", call.record, error))?;
        let cell = Arc::clone(progress.resources.get_or_insert_with(|| {
            Arc::new(std::sync::Mutex::new(
                ExecutionResourceGuard::selected_empty(&self.home_dir, call.record),
            ))
        }));
        progress.install(
            call,
            OwnedVmWork::PrepareResources {
                cell,
                record: call.record.clone(),
            },
            budget,
        )?;
        progress.poll_job(call, budget).await
    }
    async fn rollback_original_resources(
        &self,
        call: &super::store::ValidatedOwnedCall<'_>,
        budget: aster_box_core::vmm::OwnedCleanupBudget,
    ) -> ExecutionManagerResult<()> {
        let owner = self.exact_original(call)?;
        let mut progress = tokio::time::timeout_at(
            tokio::time::Instant::from_std(budget.deadline()),
            owner
                .progress
                .as_ref()
                .ok_or_else(|| legacy_lineage_error(&call.record.id))?
                .lock(),
        )
        .await
        .map_err(|_| {
            ExecutionManagerError::Unavailable(
                "original owner ancillary lock deadline expired".into(),
            )
        })?;
        self.recheck_original(call, &owner)?;
        require_job_time(budget)
            .map_err(|error| runtime_error("original ancillary admission", call.record, error))?;
        if progress.acquisition != crate::vm::OriginalAcquisition::NeverAcquired
            && !matches!(progress.physical, Some(Ok(_)))
        {
            return Err(ExecutionManagerError::Unavailable(
                "resource rollback requires original no-acquire eligibility or physical proof"
                    .into(),
            ));
        }
        let cell = Arc::clone(progress.resources.as_ref().ok_or_else(|| {
            ExecutionManagerError::Unavailable("original resource cell missing".into())
        })?);
        progress.install(call, OwnedVmWork::RollbackResources(cell), budget)?;
        progress.poll_job(call, budget).await
    }
    async fn boot_original_component(
        &self,
        call: &super::store::ValidatedOwnedCall<'_>,
        budget: aster_box_core::vmm::OwnedCleanupBudget,
    ) -> ExecutionManagerResult<()> {
        self.run_original_preparation(call, budget).await?;
        let owner = self.exact_original(call)?;
        let mut progress = tokio::time::timeout_at(
            tokio::time::Instant::from_std(budget.deadline()),
            owner
                .progress
                .as_ref()
                .ok_or_else(|| legacy_lineage_error(&call.record.id))?
                .lock(),
        )
        .await
        .map_err(|_| {
            ExecutionManagerError::Unavailable(
                "original owner ancillary lock deadline expired".into(),
            )
        })?;
        self.recheck_original(call, &owner)?;
        require_job_time(budget)
            .map_err(|error| runtime_error("original ancillary admission", call.record, error))?;
        let mut vm = tokio::time::timeout_at(
            tokio::time::Instant::from_std(budget.deadline()),
            owner.vm.lock(),
        )
        .await
        .map_err(|_| {
            ExecutionManagerError::Unavailable("original boot lock deadline expired".into())
        })?;
        self.recheck_original(call, &owner)?;
        require_job_time(budget)
            .map_err(|error| runtime_error("original ancillary admission", call.record, error))?;
        progress.validate_bound_settings(call.record)?;
        let scope = progress.scope.clone();
        vm.boot_original_owned(&scope, &mut progress.acquisition, call.lifecycle)
            .await
            .map_err(|e| runtime_error("original boot", call.record, e))
    }
    async fn close_original_component(
        &self,
        call: &super::store::ValidatedOwnedCall<'_>,
        signal: i32,
        budget: aster_box_core::vmm::OwnedCloseBudget,
    ) -> ExecutionManagerResult<()> {
        let owner = self.exact_original(call)?;
        let deadline = tokio::time::Instant::from_std(budget.physical_deadline());
        let mut progress = tokio::time::timeout_at(
            deadline,
            owner
                .progress
                .as_ref()
                .ok_or_else(|| legacy_lineage_error(&call.record.id))?
                .lock(),
        )
        .await
        .map_err(|_| {
            ExecutionManagerError::Unavailable("original owner physical deadline expired".into())
        })?;
        self.recheck_original(call, &owner)?;
        progress.validate_bound_settings(call.record)?;
        if budget
            .remaining_physical_at(std::time::Instant::now())
            .is_zero()
        {
            return Err(ExecutionManagerError::Unavailable(
                "original physical attempt expired before reconciliation".into(),
            ));
        }
        if let Some(actual) = &progress.physical {
            match actual {
                Ok(_) => return Ok(()),
                Err(error) if progress.physical_budget == Some(budget) => {
                    return Err(ExecutionManagerError::Unavailable(format!(
                        "original physical attempt remains unproved: {error}"
                    )))
                }
                Err(_) => {} // Only an explicitly different admitted attempt may retry the same handler.
            }
        }
        if matches!(progress.job, OwnedVmJobSlot::Running { .. }) {
            return Err(ExecutionManagerError::Unavailable(
                "original ancillary job is still running".into(),
            ));
        }
        let mut vm = tokio::time::timeout_at(deadline, owner.vm.lock())
            .await
            .map_err(|_| {
                ExecutionManagerError::Unavailable("original VM physical deadline expired".into())
            })?;
        self.recheck_original(call, &owner)?;
        let result = vm
            .close_original_owned(&progress.scope, call.lifecycle, signal, budget)
            .await;
        // Actual result retained before another await; an error is never cleanup permission.
        progress.physical = Some(result);
        progress.physical_budget = Some(budget);
        match progress.physical.as_ref() {
            Some(Ok(_)) => {
                if progress.cleanup_budget.is_none() {
                    progress.cleanup_budget = Some(
                        aster_box_core::vmm::OwnedCleanupBudget::from_validation(
                            std::time::Instant::now(),
                        )
                        .map_err(|e| runtime_error("admit original cleanup", call.record, e))?,
                    );
                }
                Ok(())
            }
            Some(Err(error)) => Err(ExecutionManagerError::Unavailable(format!(
                "original physical close failed: {error}"
            ))),
            None => Err(ExecutionManagerError::Unavailable(
                "original physical result missing".into(),
            )),
        }
    }
    async fn cleanup_original_anonymous(
        &self,
        call: &super::store::ValidatedOwnedCall<'_>,
        budget: aster_box_core::vmm::OwnedCleanupBudget,
    ) -> ExecutionManagerResult<()> {
        let owner = self.exact_original(call)?;
        let deadline = tokio::time::Instant::from_std(budget.deadline());
        let mut progress = tokio::time::timeout_at(
            deadline,
            owner
                .progress
                .as_ref()
                .ok_or_else(|| legacy_lineage_error(&call.record.id))?
                .lock(),
        )
        .await
        .map_err(|_| {
            ExecutionManagerError::Unavailable(
                "original anonymous owner lock deadline expired".into(),
            )
        })?;
        self.recheck_original(call, &owner)?;
        require_job_time(budget)
            .map_err(|error| runtime_error("original ancillary admission", call.record, error))?;
        if progress.acquisition != crate::vm::OriginalAcquisition::NeverAcquired
            && !matches!(progress.physical, Some(Ok(_)))
        {
            return Err(ExecutionManagerError::Unavailable(
                "anonymous cleanup requires original no-acquire eligibility or physical proof"
                    .into(),
            ));
        }
        progress.validate_bound_settings(call.record)?;
        if progress.anonymous.is_none() {
            let vm = tokio::time::timeout_at(deadline, owner.vm.lock())
                .await
                .map_err(|_| {
                    ExecutionManagerError::Unavailable(
                        "original anonymous VM lock deadline expired".into(),
                    )
                })?;
            self.recheck_original(call, &owner)?;
            require_job_time(budget).map_err(|error| {
                runtime_error("original ancillary admission", call.record, error)
            })?;
            let mut names = call.record.anonymous_volumes.clone();
            for name in &vm.created_anonymous_volumes {
                if !names.contains(name) {
                    names.push(name.clone());
                }
            }
            progress.anonymous = Some(Arc::new(std::sync::Mutex::new(OwnedAnonymousCleanup {
                home: self.home_dir.clone(),
                owner: call.record.id.clone(),
                names: names.into_iter().map(|name| (name, false)).collect(),
                unproved: false,
            })));
        }
        let cell = Arc::clone(progress.anonymous.as_ref().ok_or_else(|| {
            ExecutionManagerError::Unavailable("original anonymous cell missing".into())
        })?);
        progress.install(call, OwnedVmWork::CleanupAnonymousVolumes(cell), budget)?;
        progress.poll_job(call, budget).await
    }
    async fn finalize_original_component(
        &self,
        call: &super::store::ValidatedOwnedCall<'_>,
        preserve_rootfs: bool,
        budget: aster_box_core::vmm::OwnedCleanupBudget,
    ) -> ExecutionManagerResult<()> {
        let owner = self.exact_original(call)?;
        let mut progress = tokio::time::timeout_at(
            tokio::time::Instant::from_std(budget.deadline()),
            owner
                .progress
                .as_ref()
                .ok_or_else(|| legacy_lineage_error(&call.record.id))?
                .lock(),
        )
        .await
        .map_err(|_| {
            ExecutionManagerError::Unavailable(
                "original owner ancillary lock deadline expired".into(),
            )
        })?;
        self.recheck_original(call, &owner)?;
        require_job_time(budget)
            .map_err(|error| runtime_error("original ancillary admission", call.record, error))?;
        progress.validate_bound_settings(call.record)?;
        if let Some(cell) = &progress.finalization {
            cell.try_lock()
                .map_err(|_| {
                    ExecutionManagerError::Unavailable(
                        "original finalization policy unavailable/unproved".into(),
                    )
                })?
                .validate_preserve_rootfs(preserve_rootfs)
                .map_err(|error| {
                    runtime_error("original finalization policy", call.record, error)
                })?;
        }
        if progress.cleanup_budget != Some(budget) {
            let previous = progress.cleanup_budget.ok_or_else(|| {
                ExecutionManagerError::Unavailable("original cleanup admission missing".into())
            })?;
            if budget.deadline() <= previous.deadline() || !matches!(progress.physical, Some(Ok(_)))
            {
                return Err(ExecutionManagerError::Unavailable(
                    "original cleanup retry admission mismatch".into(),
                ));
            }
            let cell = progress.finalization.as_ref().ok_or_else(|| {
                ExecutionManagerError::Unavailable(
                    "original finalization retry cell missing".into(),
                )
            })?;
            match &progress.job {
                OwnedVmJobSlot::Ready {
                    kind: OwnedVmJobKind::FinalizeVm,
                    binding,
                    result: Ok(_),
                } => binding.validate_call(call)?,
                _ => {
                    return Err(ExecutionManagerError::Unavailable(
                        "original finalization retry requires actual completed job".into(),
                    ))
                }
            }
            if !OwnedVmWork::FinalizeVm(Arc::clone(cell)).remaining_known()? {
                return Err(ExecutionManagerError::Unavailable(
                    "original finalization remaining effect is unproved".into(),
                ));
            }
            // Supplied explicit attempt only; never construct/reset a clock here.
            progress.cleanup_budget = Some(budget);
        }
        if progress.finalization.is_none() {
            let confirmation = match &progress.physical {
                Some(Ok(proof)) => *proof,
                _ => {
                    return Err(ExecutionManagerError::Unavailable(
                        "original physical proof missing".into(),
                    ))
                }
            };
            let mut vm = tokio::time::timeout_at(
                tokio::time::Instant::from_std(budget.deadline()),
                owner.vm.lock(),
            )
            .await
            .map_err(|_| {
                ExecutionManagerError::Unavailable(
                    "original finalization lock deadline expired".into(),
                )
            })?;
            self.recheck_original(call, &owner)?;
            require_job_time(budget).map_err(|error| {
                runtime_error("original ancillary admission", call.record, error)
            })?;
            let cell = vm
                .take_original_finalization(&progress.scope, confirmation, preserve_rootfs)
                .map_err(|e| runtime_error("capture original finalization", call.record, e))?;
            progress.finalization = Some(Arc::new(std::sync::Mutex::new(cell)));
        }
        let cell = Arc::clone(progress.finalization.as_ref().ok_or_else(|| {
            ExecutionManagerError::Unavailable("original finalization cell missing".into())
        })?);
        progress.install(call, OwnedVmWork::FinalizeVm(cell), budget)?;
        progress.poll_job(call, budget).await
    }
}

#[cfg(test)]
#[path = "tests/owned_vm_jobs.rs"]
mod owned_vm_jobs_tests;
