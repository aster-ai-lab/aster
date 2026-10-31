//! Injectable process/runtime boundary for local execution orchestration.

use super::store::{
    FreshOwnerAdmission, OwnedLifecycleOutcome, OwnedLifecycleRequest, OwnerBinding,
};
use aster_box_core::vmm::OwnedCloseBudget;
use std::path::{Path, PathBuf};

use aster_box_core::{
    pty::PtyRequest, ExecOutput, ExecRequest, ExecutionEventBatch, ExecutionEventsRequest,
    ExecutionId, ExecutionIsolation, ExecutionManagerError, ExecutionManagerResult,
    ExecutionProcess, ExecutionProcessInventory, ExecutionResourceUpdate, ExecutionState,
    ExecutionStats, FileRequest, FileResponse, FilesystemRequest, FilesystemResponse, KillOutcome,
    OperationId,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::collections::HashSet;

use crate::local_execution::OciRuntimeBinding;
use crate::{BoxRecord, ManagedRuntimeRoute};

/// Product resources derived by a backend before a new execution is reserved.
///
/// Planning may resolve immutable image metadata, but it must not create any
/// execution-owned resource. The manager validates and persists this plan in
/// the [`BoxRecord`] before `start` may materialize it, so restart recovery and
/// removal never have to infer ownership from host artifacts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LocalExecutionResourcePlan {
    /// Exact Box-owned anonymous volume identities required by the image.
    pub anonymous_volumes: Vec<String>,
}

impl LocalExecutionResourcePlan {
    pub(crate) fn apply_to(self, record: &mut BoxRecord) -> ExecutionManagerResult<()> {
        if !record.anonymous_volumes.is_empty() {
            return Err(ExecutionManagerError::Internal(format!(
                "new execution {} already contains anonymous-volume ownership",
                record.id
            )));
        }

        let mut seen = HashSet::with_capacity(self.anonymous_volumes.len());
        for name in &self.anonymous_volumes {
            if !valid_planned_volume_name(name) {
                return Err(ExecutionManagerError::Internal(format!(
                    "backend planned an invalid anonymous-volume identity for {}: {name:?}",
                    record.id
                )));
            }
            if !seen.insert(name) {
                return Err(ExecutionManagerError::Internal(format!(
                    "backend planned duplicate anonymous-volume identity {name:?} for {}",
                    record.id
                )));
            }
        }
        record.anonymous_volumes = self.anonymous_volumes;
        Ok(())
    }
}

fn valid_planned_volume_name(name: &str) -> bool {
    name.starts_with("anon_")
        && name.len() <= 128
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

/// Runtime evidence persisted after an execution becomes ready.
#[derive(Debug, Clone)]
pub struct LocalExecutionHandle {
    pub started_at: DateTime<Utc>,
    pub pid: Option<u32>,
    pub pid_start_time: Option<u64>,
    pub exec_socket_path: PathBuf,
    pub console_log: PathBuf,
    pub anonymous_volumes: Vec<String>,
    /// Exact Aster OCI identity when lifecycle ownership is delegated through
    /// the public SDK rather than a Box-owned VM process.
    pub oci_runtime: Option<OciRuntimeBinding>,
}

impl LocalExecutionHandle {
    pub(crate) fn validate(&self, execution_id: &ExecutionId) -> ExecutionManagerResult<()> {
        if self.pid.is_none() && self.pid_start_time.is_some() {
            return Err(ExecutionManagerError::Internal(format!(
                "backend returned a PID start time without a PID for {execution_id}"
            )));
        }
        if let Some(binding) = &self.oci_runtime {
            binding.validate_for(execution_id)?;
        }
        if self.exec_socket_path.as_os_str().is_empty() && self.oci_runtime.is_none() {
            return Err(ExecutionManagerError::Internal(format!(
                "backend returned an empty exec socket path for {execution_id}"
            )));
        }
        if self.console_log.as_os_str().is_empty() {
            return Err(ExecutionManagerError::Internal(format!(
                "backend returned an empty console log path for {execution_id}"
            )));
        }
        Ok(())
    }
}

/// One backend observation used during inspection and restart recovery.
#[derive(Debug, Clone)]
pub struct LocalExecutionObservation {
    pub state: ExecutionState,
    pub handle: Option<LocalExecutionHandle>,
    pub exit_code: Option<i32>,
}

/// Result of a terminal backend operation, including authoritative status when
/// the runtime was able to observe it before teardown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalExecutionTermination {
    pub outcome: KillOutcome,
    pub exit_code: Option<i32>,
}

impl LocalExecutionObservation {
    pub(crate) fn validate(&self, execution_id: &ExecutionId) -> ExecutionManagerResult<()> {
        match self.state {
            ExecutionState::Running | ExecutionState::Paused => {
                self.handle.as_ref().ok_or_else(|| {
                    ExecutionManagerError::Internal(format!(
                        "backend returned {:?} without runtime evidence for {execution_id}",
                        self.state
                    ))
                })?;
            }
            ExecutionState::Created
            | ExecutionState::Creating
            | ExecutionState::Stopped
            | ExecutionState::Failed => {}
        }
        if let Some(handle) = &self.handle {
            handle.validate(execution_id)?;
        }
        Ok(())
    }
}

/// Side-effect-free original-lineage observation. This is not physical exit,
/// cleanup or durable phase authority; only the current original owner can bind
/// a positive observation. A missing/legacy owner remains unproved.
#[derive(Debug, Clone)]
pub struct LocalExecutionOwnerFacts {
    binding: Option<OwnerBinding>,
}

impl LocalExecutionOwnerFacts {
    pub(super) fn unproved() -> Self {
        Self { binding: None }
    }
    pub(super) fn admitted(binding: OwnerBinding) -> Self {
        Self {
            binding: Some(binding),
        }
    }
    pub fn has_original_owner(&self) -> bool {
        self.binding.is_some()
    }
    pub(super) fn validate_for(
        &self,
        record: &BoxRecord,
        home: &Path,
        store: &Path,
    ) -> ExecutionManagerResult<()> {
        self.binding
            .as_ref()
            .ok_or_else(|| {
                ExecutionManagerError::Unavailable(format!("owner-exit-unproved: {}", record.id))
            })?
            .validate_record(record, home, store)
    }
}

/// Backend operations invoked outside the durable state lock.
///
/// Implementations must key all host/runtime paths by [`BoxRecord::id`]. The
/// external sandbox ID in managed metadata is an untrusted diagnostic label.
#[async_trait]
pub trait LocalExecutionBackend: Send + Sync {
    /// Observe lineage without starting, recovering, cleaning up or registering.
    async fn owner_facts(
        &self,
        _record: &BoxRecord,
    ) -> ExecutionManagerResult<LocalExecutionOwnerFacts> {
        Ok(LocalExecutionOwnerFacts::unproved())
    }

    /// Register only an opaque capability from the trusted observed-Reserved
    /// wrapper. Default refusal never falls back to legacy start/inspect.
    async fn admit_fresh_owner(
        &self,
        _admission: &FreshOwnerAdmission,
    ) -> ExecutionManagerResult<()> {
        Err(ExecutionManagerError::Unavailable(
            "owner-admission-unsupported".to_string(),
        ))
    }

    /// The sole sealed continuation port. Until the retained physical/job
    /// implementation exists, unsupported must fail before all effects.
    async fn continue_owned_lifecycle(
        &self,
        _request: OwnedLifecycleRequest,
    ) -> ExecutionManagerResult<OwnedLifecycleOutcome> {
        Err(ExecutionManagerError::Unavailable(
            "owner-continuation-unsupported".to_string(),
        ))
    }

    async fn kill_with_status_with_budget(
        &self,
        _record: &BoxRecord,
        _budget: OwnedCloseBudget,
    ) -> ExecutionManagerResult<LocalExecutionTermination> {
        Err(ExecutionManagerError::Unavailable(
            "owner-close-budget-unsupported".to_string(),
        ))
    }

    async fn stop_for_restart_with_budget(
        &self,
        _record: &BoxRecord,
        _budget: OwnedCloseBudget,
    ) -> ExecutionManagerResult<KillOutcome> {
        Err(ExecutionManagerError::Unavailable(
            "owner-close-budget-unsupported".to_string(),
        ))
    }

    /// Perform any backend-only launch-readiness probe available before callers
    /// execute external image operations. Backends without a context-free probe
    /// may defer to record preflight. Mutable checks must be repeated there.
    async fn preflight_isolation(
        &self,
        _isolation: ExecutionIsolation,
    ) -> ExecutionManagerResult<()> {
        Ok(())
    }

    /// Select the durable route for a new record before capability preflight.
    ///
    /// Custom test or embedding backends may leave the route unspecified. Box
    /// production backends must return an exact route so a later policy change
    /// cannot reinterpret a claimed or stopped generation.
    fn route_for_create(&self, _record: &BoxRecord) -> ExecutionManagerResult<ManagedRuntimeRoute> {
        Ok(ManagedRuntimeRoute::Unspecified)
    }

    /// Reject an unsupported execution before its durable reservation is
    /// published. Backends must repeat mutable capability checks at launch.
    async fn preflight(&self, _record: &BoxRecord) -> ExecutionManagerResult<()> {
        Ok(())
    }

    /// Derive execution-owned resource identities without materializing them.
    ///
    /// The manager invokes this only after backend preflight and publishes the
    /// returned identities as part of the initial durable reservation.
    async fn plan_create_resources(
        &self,
        _record: &BoxRecord,
    ) -> ExecutionManagerResult<LocalExecutionResourcePlan> {
        Ok(LocalExecutionResourcePlan::default())
    }

    async fn start(&self, record: &BoxRecord) -> ExecutionManagerResult<LocalExecutionHandle>;

    async fn inspect(
        &self,
        record: &BoxRecord,
    ) -> ExecutionManagerResult<LocalExecutionObservation>;

    async fn pause(
        &self,
        record: &BoxRecord,
        keep_memory: bool,
    ) -> ExecutionManagerResult<LocalExecutionHandle>;

    async fn resume(&self, record: &BoxRecord) -> ExecutionManagerResult<LocalExecutionHandle>;

    /// Validate a complete live resource contract before a durable mutation claim.
    async fn preflight_resource_update(
        &self,
        _record: &BoxRecord,
        _update: &ExecutionResourceUpdate,
    ) -> ExecutionManagerResult<()> {
        Err(ExecutionManagerError::Unavailable(
            "this execution backend does not support live resource updates".to_string(),
        ))
    }

    /// Apply the exact persisted resource update operation.
    async fn update_resources(
        &self,
        _record: &BoxRecord,
        _operation_id: &OperationId,
        _update: &ExecutionResourceUpdate,
    ) -> ExecutionManagerResult<()> {
        Err(ExecutionManagerError::Unavailable(
            "this execution backend does not support live resource updates".to_string(),
        ))
    }

    /// Return live init and exec processes for the record's exact runtime target.
    async fn list_processes(
        &self,
        _record: &BoxRecord,
    ) -> ExecutionManagerResult<ExecutionProcessInventory> {
        Err(ExecutionManagerError::Unavailable(
            "this execution backend does not expose process inventory".to_string(),
        ))
    }

    /// Return normalized counters for the record's exact runtime target.
    async fn stats(&self, _record: &BoxRecord) -> ExecutionManagerResult<ExecutionStats> {
        Err(ExecutionManagerError::Unavailable(
            "this execution backend does not expose runtime stats".to_string(),
        ))
    }

    /// Poll ordered events for the record's exact runtime target.
    async fn events(
        &self,
        _record: &BoxRecord,
        _request: ExecutionEventsRequest,
    ) -> ExecutionManagerResult<ExecutionEventBatch> {
        Err(ExecutionManagerError::Unavailable(
            "this execution backend does not expose runtime events".to_string(),
        ))
    }

    /// Execute one captured process through a backend-owned, generation-fenced
    /// session boundary. The legacy VM backend retains its socket transport;
    /// SDK-backed implementations override this method.
    async fn execute(
        &self,
        _record: &BoxRecord,
        _request: ExecRequest,
    ) -> ExecutionManagerResult<ExecOutput> {
        Err(ExecutionManagerError::Unavailable(
            "this execution backend does not expose captured process sessions".to_string(),
        ))
    }

    /// Start one streaming non-terminal process through the backend boundary.
    async fn start_process(
        &self,
        _record: &BoxRecord,
        _request: ExecRequest,
    ) -> ExecutionManagerResult<ExecutionProcess> {
        Err(ExecutionManagerError::Unavailable(
            "this execution backend does not expose streaming process sessions".to_string(),
        ))
    }

    /// Start one interactive terminal process through the backend boundary.
    async fn start_pty(
        &self,
        _record: &BoxRecord,
        _request: PtyRequest,
    ) -> ExecutionManagerResult<ExecutionProcess> {
        Err(ExecutionManagerError::Unavailable(
            "this execution backend does not expose PTY sessions".to_string(),
        ))
    }

    /// Transfer one file through the backend's exact-generation session.
    async fn transfer_file(
        &self,
        _record: &BoxRecord,
        _request: FileRequest,
    ) -> ExecutionManagerResult<FileResponse> {
        Err(ExecutionManagerError::Unavailable(
            "this execution backend does not expose file transfer sessions".to_string(),
        ))
    }

    /// Inspect or mutate the exact generation's workload filesystem.
    async fn filesystem(
        &self,
        _record: &BoxRecord,
        _request: FilesystemRequest,
    ) -> ExecutionManagerResult<FilesystemResponse> {
        Err(ExecutionManagerError::Unavailable(
            "this execution backend does not expose filesystem sessions".to_string(),
        ))
    }

    /// Make a stopped, storage-retained rootfs available for a filesystem
    /// snapshot without starting the execution runtime.
    async fn prepare_quiescent_rootfs(&self, _record: &BoxRecord) -> ExecutionManagerResult<()> {
        Ok(())
    }

    /// Release any transient rootfs mount created by
    /// [`Self::prepare_quiescent_rootfs`] while retaining guest data.
    async fn cleanup_quiescent_rootfs(&self, _record: &BoxRecord) -> ExecutionManagerResult<()> {
        Ok(())
    }

    /// Stop the current runtime while preserving execution-owned storage for
    /// the replacement generation.
    async fn stop_for_restart(
        &self,
        record: &BoxRecord,
        _timeout_secs: Option<u64>,
    ) -> ExecutionManagerResult<KillOutcome> {
        self.kill(record).await
    }

    async fn kill(&self, record: &BoxRecord) -> ExecutionManagerResult<KillOutcome>;

    /// Kill the current execution and return its exact terminal status when the
    /// backend provides one. Implementations that cannot observe status retain
    /// the legacy `kill` behavior through this default.
    async fn kill_with_status(
        &self,
        record: &BoxRecord,
    ) -> ExecutionManagerResult<LocalExecutionTermination> {
        Ok(LocalExecutionTermination {
            outcome: self.kill(record).await?,
            exit_code: None,
        })
    }
}
