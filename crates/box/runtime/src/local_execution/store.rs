use std::path::{Path, PathBuf};
#[cfg(feature = "vm")]
use std::sync::MutexGuard;
#[cfg(feature = "vm")]
use std::sync::Weak;
use std::sync::{Arc, Mutex};

use aster_box_core::{
    ExecutionGeneration, ExecutionId, ExecutionIsolation, ExecutionManagerError,
    ExecutionManagerResult, ExecutionResourceUpdate, ExecutionSnapshotId, KillExecutionOptions,
    OperationId, RestartExecutionOptions,
};

use super::record::{
    apply_handle, apply_restart_handle, apply_start_handle, clear_live_runtime,
    clear_live_runtime_for_cold_pause, clear_live_runtime_for_restart, execution_id,
};
use super::support::{generation, managed_state};
use super::{LocalExecutionHandle, LocalExecutionManager};
use crate::{
    BoxRecord, ManagedExecutionOperation, ManagedExecutionReservation, ManagedExecutionState,
    ManagedExecutionStoreError, ManagedRuntimeRoute, SnapshotStore,
};

/// An observed successful reservation capability. No public constructor or
/// serialized representation exists; the public reservation enum cannot mint it.
#[derive(Clone)]
pub struct FreshOwnerAdmission {
    record: BoxRecord,
    binding: OwnerBinding,
    state: Arc<Mutex<AdmissionState>>,
}

impl std::fmt::Debug for FreshOwnerAdmission {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FreshOwnerAdmission")
            .field("execution_id", &self.binding.execution_id)
            .field("generation", &self.binding.generation)
            .finish_non_exhaustive()
    }
}

/// Internal identity copied only from the actual observed Reserved result.
/// Exact OS strings bind the original scope; path aliases are not equivalent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct OwnerBinding {
    execution_id: String,
    creation_operation: OperationId,
    generation: ExecutionGeneration,
    creation_intent: String,
    home_dir: PathBuf,
    store_path: PathBuf,
}

pub(super) enum AdmissionState {
    Unused,
    #[cfg(feature = "vm")]
    Consumed(Option<Weak<super::vm_backend::AdmittedVm>>),
}

/// The trusted wrapper preserves the actual enum outcome, including Existing.
/// Its sole caller in T002 is the bounded component suite, not api.create.
pub(super) struct OwnerReservation {
    pub(super) reservation: ManagedExecutionReservation,
    pub(super) admission: Option<FreshOwnerAdmission>,
}

/// Sealed requests for the already selected original lifecycle. Production
/// constructors/dispatch are deliberately not connected until T006.
pub struct OwnedLifecycleRequest {
    stage: OwnedLifecycleStage,
}

enum OwnedLifecycleStage {
    StartSelected {
        record: BoxRecord,
        store: crate::ManagedExecutionStore,
        home_dir: PathBuf,
        lifecycle: Arc<super::ExecutionLifecycleLock>,
    },
    StopSelected {
        record: BoxRecord,
        store: crate::ManagedExecutionStore,
        home_dir: PathBuf,
        lifecycle: Arc<super::ExecutionLifecycleLock>,
        physical_budget: aster_box_core::vmm::OwnedCloseBudget,
    },
    ReconcileRetainedRemove {
        execution_id: ExecutionId,
        generation: ExecutionGeneration,
    },
}

/// No completion can be constructed at this component-only stage. T005/T006
/// supply actual same-owner results; a default refusal cannot fabricate one.
pub struct OwnedLifecycleOutcome {
    completion: OwnedLifecycleCompletion,
}

enum OwnedLifecycleCompletion {}

impl OwnedLifecycleRequest {
    pub(super) fn record(&self) -> Option<&BoxRecord> {
        match &self.stage {
            OwnedLifecycleStage::StartSelected { record, .. }
            | OwnedLifecycleStage::StopSelected { record, .. } => Some(record),
            OwnedLifecycleStage::ReconcileRetainedRemove { .. } => None,
        }
    }

    // These factories accept only the existing canonical objects. They remain
    // private and have no production callers in T002; there is no generic job.
    pub(super) fn start_selected(
        record: BoxRecord,
        store: crate::ManagedExecutionStore,
        home_dir: PathBuf,
        lifecycle: Arc<super::ExecutionLifecycleLock>,
    ) -> Self {
        Self {
            stage: OwnedLifecycleStage::StartSelected {
                record,
                store,
                home_dir,
                lifecycle,
            },
        }
    }

    pub(super) fn stop_selected(
        record: BoxRecord,
        store: crate::ManagedExecutionStore,
        home_dir: PathBuf,
        lifecycle: Arc<super::ExecutionLifecycleLock>,
        physical_budget: aster_box_core::vmm::OwnedCloseBudget,
    ) -> Self {
        Self {
            stage: OwnedLifecycleStage::StopSelected {
                record,
                store,
                home_dir,
                lifecycle,
                physical_budget,
            },
        }
    }

    pub(super) fn reconcile_retained_remove(
        execution_id: ExecutionId,
        generation: ExecutionGeneration,
    ) -> Self {
        Self {
            stage: OwnedLifecycleStage::ReconcileRetainedRemove {
                execution_id,
                generation,
            },
        }
    }
}

impl OwnerBinding {
    fn from_reserved(
        record: &BoxRecord,
        home: &Path,
        store: &Path,
    ) -> ExecutionManagerResult<Self> {
        let metadata = record
            .managed_execution
            .as_ref()
            .ok_or_else(|| owner_binding_error(record))?;
        metadata
            .validate()
            .map_err(|_| owner_binding_error(record))?;
        if metadata.runtime_route != ManagedRuntimeRoute::BoxVm
            || record.isolation != ExecutionIsolation::Microvm
            || metadata.request.config.isolation != ExecutionIsolation::Microvm
            || metadata.pending_operation.is_some()
            || managed_state(record)? != ManagedExecutionState::Created
            || record.box_dir.as_os_str() != home.join("boxes").join(&record.id).as_os_str()
        {
            return Err(owner_binding_error(record));
        }
        let creation_intent = metadata
            .creation_intent_digest
            .clone()
            .ok_or_else(|| owner_binding_error(record))?;
        Ok(Self {
            execution_id: record.id.clone(),
            creation_operation: metadata.operation_id.clone(),
            generation: metadata.generation,
            creation_intent,
            home_dir: home.to_path_buf(),
            store_path: store.to_path_buf(),
        })
    }

    pub(super) fn store_path(&self) -> &Path {
        &self.store_path
    }

    pub(super) fn validate_record(
        &self,
        record: &BoxRecord,
        home: &Path,
        store: &Path,
    ) -> ExecutionManagerResult<()> {
        let observed = Self::from_reserved(record, home, store)?;
        if self != &observed
            || self.home_dir.as_os_str() != home.as_os_str()
            || self.store_path.as_os_str() != store.as_os_str()
        {
            return Err(owner_binding_error(record));
        }
        Ok(())
    }
}

impl FreshOwnerAdmission {
    fn from_observed_reserved(
        record: BoxRecord,
        home: &Path,
        store: &Path,
    ) -> ExecutionManagerResult<Self> {
        let binding = OwnerBinding::from_reserved(&record, home, store)?;
        Ok(Self {
            record,
            binding,
            state: Arc::new(Mutex::new(AdmissionState::Unused)),
        })
    }

    pub(super) fn record(&self) -> &BoxRecord {
        &self.record
    }
    pub(super) fn binding(&self) -> &OwnerBinding {
        &self.binding
    }
    pub(super) fn validate_scope(&self, home: &Path, store: &Path) -> ExecutionManagerResult<()> {
        self.binding.validate_record(&self.record, home, store)
    }
    #[cfg(feature = "vm")]
    pub(super) fn state(&self) -> ExecutionManagerResult<MutexGuard<'_, AdmissionState>> {
        self.state.lock().map_err(|_| {
            ExecutionManagerError::Internal(format!(
                "owner-admission-state-poisoned: {}",
                self.record.id
            ))
        })
    }
}

fn owner_binding_error(record: &BoxRecord) -> ExecutionManagerError {
    ExecutionManagerError::Unavailable(format!("owner-binding-mismatch: {}", record.id))
}

impl LocalExecutionManager {
    // Observe after the original async reserve returns. A dropped/lost await can
    // commit in the store, but cannot execute this capability constructor.
    pub(super) async fn reserve_for_owner_admission(
        &self,
        record: BoxRecord,
    ) -> ExecutionManagerResult<OwnerReservation> {
        let reservation = self.reserve(record).await?;
        let admission = match &reservation {
            ManagedExecutionReservation::Reserved(record)
                if record.isolation == ExecutionIsolation::Microvm
                    && record.managed_execution.as_ref().is_some_and(|metadata| {
                        metadata.runtime_route == ManagedRuntimeRoute::BoxVm
                    }) =>
            {
                Some(FreshOwnerAdmission::from_observed_reserved(
                    record.clone(),
                    &self.home_dir,
                    self.store.path(),
                )?)
            }
            ManagedExecutionReservation::Reserved(_) | ManagedExecutionReservation::Existing(_) => {
                None
            }
        };
        Ok(OwnerReservation {
            reservation,
            admission,
        })
    }

    pub(super) async fn reserve(
        &self,
        mut record: BoxRecord,
    ) -> ExecutionManagerResult<ManagedExecutionReservation> {
        let store = self.store.clone();
        let Some(snapshot_id) = record
            .managed_execution
            .as_ref()
            .and_then(|metadata| metadata.request.rootfs_snapshot_id.clone())
        else {
            return run_store(move || store.reserve(record)).await;
        };
        let home_dir = self.home_dir.clone();
        tokio::task::spawn_blocking(move || {
            let snapshots = SnapshotStore::new(&home_dir.join("snapshots")).map_err(|error| {
                ExecutionManagerError::Unavailable(format!(
                    "failed to open filesystem snapshot store: {error}"
                ))
            })?;
            let _snapshot_lock = snapshots.acquire_exclusive_lock().map_err(|error| {
                ExecutionManagerError::Unavailable(format!(
                    "failed to lock filesystem snapshot store: {error}"
                ))
            })?;
            let metadata = snapshots
                .get(snapshot_id.as_str())
                .map_err(|error| {
                    ExecutionManagerError::Unavailable(format!(
                        "failed to inspect filesystem snapshot {snapshot_id}: {error}"
                    ))
                })?
                .ok_or_else(|| {
                    ExecutionManagerError::Unavailable(format!(
                        "filesystem snapshot {snapshot_id} is unavailable"
                    ))
                })?;
            if metadata.id != snapshot_id.as_str()
                || !snapshots.rootfs_path(snapshot_id.as_str()).is_dir()
            {
                return Err(ExecutionManagerError::Unavailable(format!(
                    "filesystem snapshot {snapshot_id} is not a valid published snapshot"
                )));
            }
            metadata.require_image_config().map_err(|error| {
                ExecutionManagerError::Unavailable(format!(
                    "filesystem snapshot {snapshot_id} cannot be restored: {error}"
                ))
            })?;

            let health_check = record
                .health_check
                .clone()
                .or_else(|| metadata.health_check.clone());
            let healthcheck_disabled =
                record.healthcheck_disabled || metadata.healthcheck_disabled;
            record.health_check = health_check.clone();
            record.healthcheck_disabled = healthcheck_disabled;
            if let Some(managed) = record.managed_execution.as_mut() {
                managed.request.policy.health_check = health_check;
                managed.request.policy.healthcheck_disabled = healthcheck_disabled;
            }
            super::record::validate_record_health(&record)?;

            #[cfg(windows)]
            if !healthcheck_disabled
                && metadata
                    .image_config
                    .as_ref()
                    .and_then(|config| config.health_check.as_ref())
                    .is_some_and(aster_box_core::SnapshotImageHealthCheck::is_enabled)
            {
                return Err(ExecutionManagerError::InvalidRequest(
                    "container health checks are not supported on Windows; the snapshot image defines an effective health check"
                        .to_string(),
                ));
            }
            store.reserve(record).map_err(map_store_error)
        })
        .await
        .map_err(|error| {
            ExecutionManagerError::Internal(format!("managed state task failed: {error}"))
        })?
    }

    pub(super) async fn get(
        &self,
        execution_id: &ExecutionId,
    ) -> ExecutionManagerResult<Option<BoxRecord>> {
        let store = self.store.clone();
        let execution_id = execution_id.clone();
        run_store(move || store.get(&execution_id)).await
    }

    pub(super) async fn get_by_operation(
        &self,
        operation_id: &OperationId,
    ) -> ExecutionManagerResult<Option<BoxRecord>> {
        let store = self.store.clone();
        let operation_id = operation_id.clone();
        run_store(move || store.get_by_operation_id(&operation_id)).await
    }

    pub(super) async fn transition(
        &self,
        record: &BoxRecord,
        from: ManagedExecutionState,
        to: ManagedExecutionState,
        update: RuntimeUpdate,
    ) -> ExecutionManagerResult<BoxRecord> {
        let store = self.store.clone();
        let execution_id = execution_id(record)?;
        let generation = generation(record, &execution_id)?;
        run_store(move || {
            store.transition_with(&execution_id, generation, from, to, |record| match update {
                RuntimeUpdate::None => {}
                RuntimeUpdate::Handle(handle) => apply_handle(record, &handle),
                RuntimeUpdate::StartHandle(handle) => apply_start_handle(record, &handle),
                RuntimeUpdate::Terminal(exit_code) => clear_live_runtime(record, exit_code),
                RuntimeUpdate::KillTerminal(exit_code) => {
                    clear_live_runtime(record, exit_code);
                    record.stopped_by_user = true;
                }
                RuntimeUpdate::ColdPause => clear_live_runtime_for_cold_pause(record),
                RuntimeUpdate::PauseClaim {
                    keep_memory,
                    operation_id,
                } => {
                    if let Some(metadata) = record.managed_execution.as_mut() {
                        metadata.pending_operation = Some(ManagedExecutionOperation::Pause {
                            keep_memory,
                            operation_id: Some(operation_id),
                        });
                    }
                }
                RuntimeUpdate::ResumeClaim(operation_id) => {
                    if let Some(metadata) = record.managed_execution.as_mut() {
                        metadata.pending_operation = Some(ManagedExecutionOperation::Resume {
                            operation_id: Some(operation_id),
                        });
                    }
                }
                RuntimeUpdate::ResourceUpdateClaim {
                    operation_id,
                    update,
                } => {
                    if let Some(metadata) = record.managed_execution.as_mut() {
                        metadata.pending_operation =
                            Some(ManagedExecutionOperation::UpdateResources {
                                operation_id,
                                update,
                            });
                    }
                }
                RuntimeUpdate::KillClaim(options) => {
                    if let Some(metadata) = record.managed_execution.as_mut() {
                        metadata.pending_operation = Some(ManagedExecutionOperation::Kill {
                            signal: options.signal,
                            timeout_secs: options.timeout_secs,
                        });
                    }
                }
                RuntimeUpdate::SnapshotClaim {
                    snapshot_id,
                    source_state,
                    operation_id,
                } => {
                    if let Some(metadata) = record.managed_execution.as_mut() {
                        metadata.pending_operation = Some(ManagedExecutionOperation::Snapshot {
                            snapshot_id,
                            source_state,
                            operation_id: Some(operation_id),
                            freezer_applied: false,
                        });
                    }
                }
                RuntimeUpdate::RestartClaim {
                    operation_id,
                    options,
                } => {
                    if let Some(metadata) = record.managed_execution.as_mut() {
                        metadata.pending_operation = Some(ManagedExecutionOperation::Restart {
                            operation_id,
                            source_generation: metadata.generation,
                            source_state: from,
                            stop_timeout_secs: options.stop_timeout_secs,
                        });
                    }
                }
                RuntimeUpdate::RestartAdvance => clear_live_runtime_for_restart(record),
                RuntimeUpdate::RestartHandle(handle) => apply_restart_handle(record, &handle),
                RuntimeUpdate::RestartTerminal(exit_code) => clear_live_runtime(record, exit_code),
            })
        })
        .await
    }

    pub(super) async fn complete_with_handle(
        &self,
        record: &BoxRecord,
        from: ManagedExecutionState,
        to: ManagedExecutionState,
        handle: LocalExecutionHandle,
    ) -> ExecutionManagerResult<BoxRecord> {
        let update =
            if from == ManagedExecutionState::Starting && to == ManagedExecutionState::Running {
                RuntimeUpdate::StartHandle(handle)
            } else {
                RuntimeUpdate::Handle(handle)
            };
        self.complete_transition(record, from, to, update).await
    }

    pub(super) async fn complete_transition(
        &self,
        record: &BoxRecord,
        from: ManagedExecutionState,
        to: ManagedExecutionState,
        update: RuntimeUpdate,
    ) -> ExecutionManagerResult<BoxRecord> {
        let execution_id = execution_id(record)?;
        let current_generation = generation(record, &execution_id)?;
        let expected_generation = if matches!(
            (from, to),
            (
                ManagedExecutionState::Pausing,
                ManagedExecutionState::Paused
            ) | (
                ManagedExecutionState::Resuming,
                ManagedExecutionState::Running
            )
        ) {
            ExecutionGeneration::new(current_generation.get().checked_add(1).ok_or_else(|| {
                ExecutionManagerError::Internal(format!(
                    "execution {execution_id} generation is exhausted"
                ))
            })?)?
        } else {
            current_generation
        };
        match self.transition(record, from, to, update).await {
            Ok(record) => Ok(record),
            Err(error @ ExecutionManagerError::Conflict { .. }) => {
                let Some(current) = self.get(&execution_id).await? else {
                    return Err(ExecutionManagerError::NotFound(execution_id));
                };
                if managed_state(&current)? == to
                    && generation(&current, &execution_id)? == expected_generation
                {
                    Ok(current)
                } else {
                    Err(error)
                }
            }
            Err(error) => Err(error),
        }
    }

    pub(super) async fn complete_resource_update(
        &self,
        record: &BoxRecord,
    ) -> ExecutionManagerResult<BoxRecord> {
        let store = self.store.clone();
        let execution_id = execution_id(record)?;
        let generation = generation(record, &execution_id)?;
        run_store(move || store.finish_resource_update(&execution_id, generation)).await
    }

    pub(super) async fn mark_snapshot_freezer_applied(
        &self,
        record: &BoxRecord,
    ) -> ExecutionManagerResult<BoxRecord> {
        let store = self.store.clone();
        let execution_id = execution_id(record)?;
        let generation = generation(record, &execution_id)?;
        let (snapshot_id, operation_id) = match record
            .managed_execution
            .as_ref()
            .and_then(|metadata| metadata.pending_operation.as_ref())
        {
            Some(ManagedExecutionOperation::Snapshot {
                snapshot_id,
                source_state: ManagedExecutionState::Running,
                operation_id,
                ..
            }) => (snapshot_id.clone(), operation_id.clone()),
            _ => {
                return Err(ExecutionManagerError::Internal(format!(
                    "execution {execution_id} has no running snapshot freezer claim"
                )))
            }
        };
        run_store(move || {
            store.mark_snapshot_freezer_applied(
                &execution_id,
                generation,
                &snapshot_id,
                operation_id.as_ref(),
            )
        })
        .await
    }
}

pub(super) enum RuntimeUpdate {
    None,
    Handle(LocalExecutionHandle),
    StartHandle(LocalExecutionHandle),
    Terminal(Option<i32>),
    KillTerminal(Option<i32>),
    ColdPause,
    PauseClaim {
        keep_memory: bool,
        operation_id: OperationId,
    },
    ResumeClaim(OperationId),
    ResourceUpdateClaim {
        operation_id: OperationId,
        update: ExecutionResourceUpdate,
    },
    KillClaim(KillExecutionOptions),
    SnapshotClaim {
        snapshot_id: ExecutionSnapshotId,
        source_state: ManagedExecutionState,
        operation_id: OperationId,
    },
    RestartClaim {
        operation_id: OperationId,
        options: RestartExecutionOptions,
    },
    RestartAdvance,
    RestartHandle(LocalExecutionHandle),
    RestartTerminal(Option<i32>),
}

pub(super) async fn run_store<T>(
    operation: impl FnOnce() -> Result<T, ManagedExecutionStoreError> + Send + 'static,
) -> ExecutionManagerResult<T>
where
    T: Send + 'static,
{
    tokio::task::spawn_blocking(operation)
        .await
        .map_err(|error| {
            ExecutionManagerError::Internal(format!("managed state task failed: {error}"))
        })?
        .map_err(map_store_error)
}

pub(super) fn map_store_error(error: ManagedExecutionStoreError) -> ExecutionManagerError {
    match error {
        ManagedExecutionStoreError::Io(error) => {
            ExecutionManagerError::Unavailable(error.to_string())
        }
        ManagedExecutionStoreError::NotFound(execution_id) => {
            ExecutionManagerError::NotFound(execution_id)
        }
        ManagedExecutionStoreError::Conflict {
            execution_id,
            message,
        } => ExecutionManagerError::Conflict {
            execution_id,
            message,
        },
        ManagedExecutionStoreError::Unmanaged(execution_id) => ExecutionManagerError::Internal(
            format!("execution record is not managed: {execution_id}"),
        ),
        ManagedExecutionStoreError::InvalidRecord(message) => {
            ExecutionManagerError::Internal(message)
        }
        ManagedExecutionStoreError::InvalidTransition {
            execution_id,
            from,
            to,
        } => ExecutionManagerError::Internal(format!(
            "invalid managed transition for {execution_id}: {from} -> {to}"
        )),
    }
}

/// Ephemeral context after exact original-owner acceptance, never owner storage.
#[cfg(feature = "vm")]
pub(super) struct ValidatedOwnedCall<'a> {
    pub(super) record: &'a BoxRecord,
    pub(super) store: &'a crate::ManagedExecutionStore,
    pub(super) lifecycle: &'a Arc<super::ExecutionLifecycleLock>,
    binding: OwnerBinding,
}

#[cfg(feature = "vm")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct OwnedStageBinding {
    owner: OwnerBinding,
    phase: String,
    operation: Option<crate::ManagedExecutionOperation>,
    current_operation: OperationId,
    deadline: std::time::Instant,
}

#[cfg(feature = "vm")]
impl<'a> ValidatedOwnedCall<'a> {
    pub(super) fn new(
        record: &'a BoxRecord,
        store: &'a crate::ManagedExecutionStore,
        home: &Path,
        lifecycle: &'a Arc<super::ExecutionLifecycleLock>,
        binding: &OwnerBinding,
    ) -> ExecutionManagerResult<Self> {
        // This component has no authority to advance/reconstruct lineage.
        binding.validate_record(record, home, store.path())?;
        Ok(Self {
            record,
            store,
            lifecycle,
            binding: binding.clone(),
        })
    }

    pub(super) fn validate(&self, binding: &OwnerBinding) -> ExecutionManagerResult<()> {
        if binding != &self.binding {
            return Err(owner_binding_error(self.record));
        }
        binding.validate_record(self.record, &binding.home_dir, self.store.path())
    }

    pub(super) fn stage(
        &self,
        budget: aster_box_core::vmm::OwnedCleanupBudget,
    ) -> ExecutionManagerResult<OwnedStageBinding> {
        self.validate(&self.binding)?;
        if budget.remaining_at(std::time::Instant::now()).is_zero() {
            return Err(ExecutionManagerError::Unavailable(
                "original ancillary attempt expired before spawn".into(),
            ));
        }
        let metadata = self
            .record
            .managed_execution
            .as_ref()
            .ok_or_else(|| owner_binding_error(self.record))?;
        Ok(OwnedStageBinding {
            owner: self.binding.clone(),
            phase: self.record.status.clone(),
            operation: metadata.pending_operation.clone(),
            current_operation: metadata.operation_id.clone(),
            deadline: budget.deadline(),
        })
    }
}

#[cfg(feature = "vm")]
impl OwnedStageBinding {
    pub(super) fn deadline(&self) -> std::time::Instant {
        self.deadline
    }

    pub(super) fn validate_call(
        &self,
        call: &ValidatedOwnedCall<'_>,
    ) -> ExecutionManagerResult<()> {
        call.validate(&self.owner)?;
        let metadata = call
            .record
            .managed_execution
            .as_ref()
            .ok_or_else(|| owner_binding_error(call.record))?;
        if self.phase != call.record.status
            || self.operation != metadata.pending_operation
            || self.current_operation != metadata.operation_id
        {
            return Err(owner_binding_error(call.record));
        }
        Ok(())
    }
}
