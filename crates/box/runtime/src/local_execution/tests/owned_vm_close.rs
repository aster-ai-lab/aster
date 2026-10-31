#![cfg(test)]

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex as StdMutex;
use std::time::Instant;

use aster_box_core::vmm::OwnedCloseBudget;
use aster_box_core::{
    BoxConfig, CreateExecutionRequest, ExecutionGeneration, ExecutionIsolation, NetworkMode,
    OperationId,
};

use super::*;
use crate::local_execution::record::build_managed_record;
use crate::local_execution::{
    FreshOwnerAdmission, LocalExecutionBackendRouter, LocalExecutionManager, OciMigrationPolicy,
    OwnedLifecycleOutcome, OwnedLifecycleRequest,
};
use crate::ManagedExecutionReservation;

fn record(home: &Path) -> BoxRecord {
    let id = ExecutionId::new("91111111-1111-4111-8111-111111111111").unwrap();
    let mut record = build_managed_record(
        home,
        &id,
        OperationId::new("owned-component-create").unwrap(),
        CreateExecutionRequest {
            external_sandbox_id: "untrusted-display-only".to_string(),
            config: BoxConfig {
                isolation: ExecutionIsolation::Microvm,
                image: "alpine:latest".to_string(),
                network: NetworkMode::None,
                ..Default::default()
            },
            labels: BTreeMap::new(),
            policy: Default::default(),
            rootfs_snapshot_id: None,
        },
        chrono::Utc::now(),
    )
    .unwrap();
    record.managed_execution.as_mut().unwrap().runtime_route = crate::ManagedRuntimeRoute::BoxVm;
    record
}

fn manager(
    home: &Path,
    store: &str,
    backend: Arc<dyn LocalExecutionBackend>,
) -> LocalExecutionManager {
    LocalExecutionManager::new(home.join(store), home, backend)
}

async fn fresh(manager: &LocalExecutionManager, record: &BoxRecord) -> FreshOwnerAdmission {
    let result = manager
        .reserve_for_owner_admission(record.clone())
        .await
        .unwrap();
    assert!(matches!(
        result.reservation,
        ManagedExecutionReservation::Reserved(_)
    ));
    result.admission.unwrap()
}

fn actual_owner(backend: &VmLocalExecutionBackend, id: &str) -> Arc<AdmittedVm> {
    match backend.managers.get(id).unwrap().value() {
        RegisteredVm::Admitted(owner) => Arc::clone(owner),
        RegisteredVm::Legacy(_) => panic!("expected original admitted component owner"),
    }
}

#[derive(Default)]
struct LegacyOnlyBackend {
    calls: AtomicUsize,
}

impl LegacyOnlyBackend {
    fn refuse<T>(&self) -> ExecutionManagerResult<T> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Err(ExecutionManagerError::Unavailable(
            "legacy-body-entered".to_string(),
        ))
    }
}

#[async_trait]
impl LocalExecutionBackend for LegacyOnlyBackend {
    async fn start(&self, _: &BoxRecord) -> ExecutionManagerResult<LocalExecutionHandle> {
        self.refuse()
    }
    async fn inspect(&self, _: &BoxRecord) -> ExecutionManagerResult<LocalExecutionObservation> {
        self.refuse()
    }
    async fn pause(&self, _: &BoxRecord, _: bool) -> ExecutionManagerResult<LocalExecutionHandle> {
        self.refuse()
    }
    async fn resume(&self, _: &BoxRecord) -> ExecutionManagerResult<LocalExecutionHandle> {
        self.refuse()
    }
    async fn kill(&self, _: &BoxRecord) -> ExecutionManagerResult<KillOutcome> {
        self.refuse()
    }
}

#[tokio::test]
async fn genuine_reserved_wrapper_admits_once_without_materializing_and_existing_cannot_mint() {
    let temporary = tempfile::tempdir().unwrap();
    let backend = Arc::new(VmLocalExecutionBackend::new(temporary.path()));
    let manager = manager(temporary.path(), "managed-state", backend.clone());
    let record = record(temporary.path());
    let admission = fresh(&manager, &record).await;
    // Actual lifecycle guard and authoritative read first, not state synthesis.
    let _guard =
        crate::local_execution::acquire_execution_lifecycle_lock(temporary.path(), &record.id)
            .unwrap();
    let committed = manager
        .get(&execution_id(&record).unwrap())
        .await
        .unwrap()
        .unwrap();
    admission
        .binding()
        .validate_record(&committed, temporary.path(), manager.store.path())
        .unwrap();
    backend.admit_fresh_owner(&admission).await.unwrap();
    let original = actual_owner(&backend, &record.id);
    backend.admit_fresh_owner(&admission.clone()).await.unwrap();
    assert!(Arc::ptr_eq(&original, &actual_owner(&backend, &record.id)));
    let facts = backend.owner_facts(&committed).await.unwrap();
    assert!(facts.has_original_owner());
    facts
        .validate_for(&committed, temporary.path(), manager.store.path())
        .unwrap();
    let vm = original.vm.lock().await;
    assert!(vm.handler.read().await.is_none());
    assert_eq!(vm.state().await, crate::BoxState::Created);
    assert!(!record.box_dir.exists());
    assert_eq!(backend.managers.len(), 1);
    drop(vm);
    let existing = manager
        .reserve_for_owner_admission(record.clone())
        .await
        .unwrap();
    assert!(matches!(
        existing.reservation,
        ManagedExecutionReservation::Existing(_)
    ));
    assert!(existing.admission.is_none());
}

#[tokio::test]
async fn public_reservation_and_committed_result_without_wrapper_cannot_mint_lineage() {
    let temporary = tempfile::tempdir().unwrap();
    let backend = Arc::new(VmLocalExecutionBackend::new(temporary.path()));
    let manager = manager(temporary.path(), "managed-state", backend.clone());
    let record = record(temporary.path());
    // A publicly constructible Reserved is data, not an admission conversion.
    let public = ManagedExecutionReservation::Reserved(record.clone());
    assert!(public.is_new());
    assert!(!backend
        .owner_facts(&record)
        .await
        .unwrap()
        .has_original_owner());
    // The actual original reserve commits, but no wrapper capability is observed.
    // This is not a claim that an asynchronous lost-response barrier was tested.
    let actual = manager.reserve(record.clone()).await.unwrap();
    assert!(actual.is_new());
    drop(actual);
    let retry = manager
        .reserve_for_owner_admission(record.clone())
        .await
        .unwrap();
    assert!(matches!(
        retry.reservation,
        ManagedExecutionReservation::Existing(_)
    ));
    assert!(retry.admission.is_none());
    assert!(backend.managers.is_empty());
}

#[tokio::test]
async fn nonselected_actual_reserved_remains_unproved_and_has_no_admission() {
    let temporary = tempfile::tempdir().unwrap();
    let backend = Arc::new(VmLocalExecutionBackend::new(temporary.path()));
    let manager = manager(temporary.path(), "managed-state", backend.clone());
    let mut record = record(temporary.path());
    record.managed_execution.as_mut().unwrap().runtime_route = crate::ManagedRuntimeRoute::OciSdk;
    let result = manager
        .reserve_for_owner_admission(record.clone())
        .await
        .unwrap();
    assert!(matches!(
        result.reservation,
        ManagedExecutionReservation::Reserved(_)
    ));
    assert!(result.admission.is_none());
    assert!(backend.managers.is_empty());
    assert!(!backend
        .owner_facts(&record)
        .await
        .unwrap()
        .has_original_owner());
}

#[tokio::test]
async fn selected_missing_creation_intent_refuses_without_owner_or_rollback_claim() {
    let temporary = tempfile::tempdir().unwrap();
    let backend = Arc::new(VmLocalExecutionBackend::new(temporary.path()));
    let manager = manager(temporary.path(), "managed-state", backend.clone());
    let mut record = record(temporary.path());
    record
        .managed_execution
        .as_mut()
        .unwrap()
        .creation_intent_digest = None;
    assert!(manager
        .reserve_for_owner_admission(record.clone())
        .await
        .is_err());
    assert!(backend.managers.is_empty());
    // The underlying reservation is real; refusal must not imply rollback.
    assert!(manager
        .get(&execution_id(&record).unwrap())
        .await
        .unwrap()
        .is_some());
    assert!(!record.box_dir.exists());
}

#[tokio::test]
async fn failed_first_insertion_stays_consumed_after_slot_becomes_vacant() {
    let temporary = tempfile::tempdir().unwrap();
    let backend = Arc::new(VmLocalExecutionBackend::new(temporary.path()));
    let manager = manager(temporary.path(), "managed-state", backend.clone());
    let record = record(temporary.path());
    let admission = fresh(&manager, &record).await;
    let legacy = Arc::new(Mutex::new(backend.new_manager(&record).unwrap()));
    backend
        .managers
        .insert(record.id.clone(), RegisteredVm::Legacy(Arc::clone(&legacy)));
    assert!(backend.admit_fresh_owner(&admission).await.is_err());
    assert!(Arc::ptr_eq(
        &legacy,
        &backend.manager(&record.id).unwrap().unwrap()
    ));
    backend.remove_manager(&record.id, &legacy).unwrap();
    assert!(backend.admit_fresh_owner(&admission.clone()).await.is_err());
    assert!(backend.managers.is_empty());
    assert!(!record.box_dir.exists());
}

#[tokio::test]
async fn consumed_capability_never_recreates_absent_or_foreign_owner() {
    let temporary = tempfile::tempdir().unwrap();
    let backend = Arc::new(VmLocalExecutionBackend::new(temporary.path()));
    let manager = manager(temporary.path(), "managed-state", backend.clone());
    let record = record(temporary.path());
    let admission = fresh(&manager, &record).await;
    backend.admit_fresh_owner(&admission).await.unwrap();
    let original = actual_owner(&backend, &record.id);
    // Deliberate test-only registry loss, not successful canonical retirement.
    backend.managers.remove(&record.id);
    assert!(backend.admit_fresh_owner(&admission).await.is_err());
    assert!(backend.managers.is_empty());
    let foreign = Arc::new(AdmittedVm {
        binding: original.binding.clone(),
        vm: Arc::new(Mutex::new(
            backend
                .new_manager_from_metadata(&record, backend.metadata(&record).unwrap())
                .unwrap(),
        )),
        progress: None,
    });
    backend.managers.insert(
        record.id.clone(),
        RegisteredVm::Admitted(Arc::clone(&foreign)),
    );
    assert!(backend.admit_fresh_owner(&admission).await.is_err());
    assert!(Arc::ptr_eq(&foreign, &actual_owner(&backend, &record.id)));
}

#[tokio::test]
async fn different_actual_store_origin_cannot_reuse_or_rearm_original_slot() {
    let temporary = tempfile::tempdir().unwrap();
    let backend = Arc::new(VmLocalExecutionBackend::new(temporary.path()));
    let one = manager(temporary.path(), "one-state", backend.clone());
    let two = manager(temporary.path(), "two-state", backend.clone());
    let record = record(temporary.path());
    let admission = fresh(&one, &record).await;
    let other = fresh(&two, &record).await;
    backend.admit_fresh_owner(&admission).await.unwrap();
    let original = actual_owner(&backend, &record.id);
    assert!(backend.admit_fresh_owner(&other).await.is_err());
    let facts = backend.owner_facts(&record).await.unwrap();
    assert!(facts
        .validate_for(&record, temporary.path(), two.store.path())
        .is_err());
    assert!(Arc::ptr_eq(&original, &actual_owner(&backend, &record.id)));
    backend.managers.remove(&record.id); // test-only owner loss
    assert!(backend.admit_fresh_owner(&other).await.is_err());
    assert!(backend.managers.is_empty());
}

#[tokio::test]
async fn original_scope_and_each_creation_identity_field_are_exact() {
    let temporary = tempfile::tempdir().unwrap();
    let backend = Arc::new(VmLocalExecutionBackend::new(temporary.path()));
    let manager = manager(temporary.path(), "managed-state", backend.clone());
    let record = record(temporary.path());
    let admission = fresh(&manager, &record).await;
    assert!(admission
        .validate_scope(&temporary.path().join("."), manager.store.path())
        .is_err());
    assert!(admission
        .validate_scope(temporary.path(), &temporary.path().join("./managed-state"))
        .is_err());
    assert!(
        VmLocalExecutionBackend::new(temporary.path().join("other-home"))
            .admit_fresh_owner(&admission)
            .await
            .is_err()
    );
    backend.admit_fresh_owner(&admission).await.unwrap();
    let facts = backend.owner_facts(&record).await.unwrap();
    let mut variants = Vec::new();
    let mut changed = record.clone();
    changed.id = "92222222-2222-4222-8222-222222222222".into();
    variants.push(changed);
    let mut changed = record.clone();
    changed.managed_execution.as_mut().unwrap().operation_id =
        OperationId::new("other-operation").unwrap();
    variants.push(changed);
    let mut changed = record.clone();
    changed.managed_execution.as_mut().unwrap().generation = ExecutionGeneration::new(9).unwrap();
    variants.push(changed);
    let mut changed = record.clone();
    changed
        .managed_execution
        .as_mut()
        .unwrap()
        .creation_intent_digest = Some(format!("sha256:{}", "0".repeat(64)));
    variants.push(changed);
    let mut changed = record.clone();
    changed.managed_execution.as_mut().unwrap().runtime_route = crate::ManagedRuntimeRoute::OciSdk;
    variants.push(changed);
    let mut changed = record.clone();
    changed.status = ManagedExecutionState::Starting.as_status().into();
    changed
        .managed_execution
        .as_mut()
        .unwrap()
        .pending_operation = Some(ManagedExecutionOperation::Start);
    variants.push(changed);
    for changed in variants {
        assert!(facts
            .validate_for(&changed, temporary.path(), manager.store.path())
            .is_err());
    }
    assert_eq!(backend.managers.len(), 1);
}

#[tokio::test]
async fn legacy_mutators_and_extraction_refuse_actual_admitted_before_effects() {
    let temporary = tempfile::tempdir().unwrap();
    let backend = Arc::new(VmLocalExecutionBackend::new(temporary.path()));
    let manager = manager(temporary.path(), "managed-state", backend.clone());
    let record = record(temporary.path());
    let admission = fresh(&manager, &record).await;
    backend.admit_fresh_owner(&admission).await.unwrap();
    let original = actual_owner(&backend, &record.id);
    std::fs::create_dir_all(&record.box_dir).unwrap();
    let sentinel = record.box_dir.join("must-retain");
    std::fs::write(&sentinel, b"original bytes").unwrap();
    assert!(backend.start(&record).await.is_err());
    assert!(backend.inspect(&record).await.is_err());
    assert!(backend.pause(&record, true).await.is_err());
    assert!(backend.resume(&record).await.is_err());
    assert!(backend.kill(&record).await.is_err());
    assert!(backend.kill_with_status(&record).await.is_err());
    assert!(backend.stop_for_restart(&record, Some(0)).await.is_err());
    assert!(backend.prepare_quiescent_rootfs(&record).await.is_err());
    assert!(backend.cleanup_quiescent_rootfs(&record).await.is_err());
    assert!(backend.manager(&record.id).is_err());
    assert!(backend.remove_manager(&record.id, &original.vm).is_err());
    assert!(backend
        .inspect_registered(&record, Arc::clone(&original.vm))
        .await
        .is_err());
    assert!(backend
        .destroy_registered(&record, Arc::clone(&original.vm), true, false, Some(0))
        .await
        .is_err());
    assert!(Arc::ptr_eq(&original, &actual_owner(&backend, &record.id)));
    assert_eq!(std::fs::read(&sentinel).unwrap(), b"original bytes");
    assert_eq!(std::fs::read_dir(&record.box_dir).unwrap().count(), 1);
    assert!(original.vm.lock().await.handler.read().await.is_none());
}

#[tokio::test]
async fn legacy_and_missing_entries_remain_unproved_without_observation_side_effects() {
    let temporary = tempfile::tempdir().unwrap();
    let backend = VmLocalExecutionBackend::new(temporary.path());
    let record = record(temporary.path());
    assert!(!backend
        .owner_facts(&record)
        .await
        .unwrap()
        .has_original_owner());
    let legacy = Arc::new(Mutex::new(backend.new_manager(&record).unwrap()));
    backend
        .managers
        .insert(record.id.clone(), RegisteredVm::Legacy(Arc::clone(&legacy)));
    assert!(!backend
        .owner_facts(&record)
        .await
        .unwrap()
        .has_original_owner());
    assert!(Arc::ptr_eq(
        &legacy,
        &backend.manager(&record.id).unwrap().unwrap()
    ));
    assert!(!record.box_dir.exists());
}

#[tokio::test]
async fn default_ports_refuse_without_legacy_fallback_or_store_changes() {
    let temporary = tempfile::tempdir().unwrap();
    let backend = Arc::new(LegacyOnlyBackend::default());
    let manager = manager(temporary.path(), "managed-state", backend.clone());
    let record = record(temporary.path());
    let admission = fresh(&manager, &record).await;
    let before = std::fs::read(manager.store.path()).unwrap();
    let physical = OwnedCloseBudget::from_admission(Instant::now(), Duration::ZERO).unwrap();
    assert!(!backend
        .owner_facts(&record)
        .await
        .unwrap()
        .has_original_owner());
    assert!(backend.admit_fresh_owner(&admission).await.is_err());
    assert!(backend
        .kill_with_status_with_budget(&record, physical)
        .await
        .is_err());
    assert!(backend
        .stop_for_restart_with_budget(&record, physical)
        .await
        .is_err());
    let guard = Arc::new(
        crate::local_execution::acquire_execution_lifecycle_lock(temporary.path(), &record.id)
            .unwrap(),
    );
    let request = OwnedLifecycleRequest::start_selected(
        record.clone(),
        manager.store.clone(),
        temporary.path().into(),
        Arc::clone(&guard),
    );
    assert!(backend.continue_owned_lifecycle(request).await.is_err());
    let request = OwnedLifecycleRequest::stop_selected(
        record.clone(),
        manager.store.clone(),
        temporary.path().into(),
        guard,
        physical,
    );
    assert!(backend.continue_owned_lifecycle(request).await.is_err());
    assert_eq!(backend.calls.load(Ordering::SeqCst), 0);
    assert_eq!(std::fs::read(manager.store.path()).unwrap(), before);
    assert!(!record.box_dir.exists());
}

#[derive(Default)]
struct RoutedOwnedProbe {
    legacy: LegacyOnlyBackend,
    budgets: StdMutex<Vec<OwnedCloseBudget>>,
    admissions: AtomicUsize,
    continuations: AtomicUsize,
    observations: AtomicUsize,
    continuation_records: StdMutex<Vec<Option<String>>>,
}

#[async_trait]
impl LocalExecutionBackend for RoutedOwnedProbe {
    async fn start(&self, _: &BoxRecord) -> ExecutionManagerResult<LocalExecutionHandle> {
        self.legacy.refuse()
    }
    async fn inspect(&self, _: &BoxRecord) -> ExecutionManagerResult<LocalExecutionObservation> {
        self.legacy.refuse()
    }
    async fn pause(&self, _: &BoxRecord, _: bool) -> ExecutionManagerResult<LocalExecutionHandle> {
        self.legacy.refuse()
    }
    async fn resume(&self, _: &BoxRecord) -> ExecutionManagerResult<LocalExecutionHandle> {
        self.legacy.refuse()
    }
    async fn kill(&self, _: &BoxRecord) -> ExecutionManagerResult<KillOutcome> {
        self.legacy.refuse()
    }
    async fn owner_facts(&self, _: &BoxRecord) -> ExecutionManagerResult<LocalExecutionOwnerFacts> {
        self.observations.fetch_add(1, Ordering::SeqCst);
        Ok(LocalExecutionOwnerFacts::unproved())
    }
    async fn admit_fresh_owner(&self, _: &FreshOwnerAdmission) -> ExecutionManagerResult<()> {
        self.admissions.fetch_add(1, Ordering::SeqCst);
        Err(ExecutionManagerError::Unavailable("probe-refused".into()))
    }
    async fn continue_owned_lifecycle(
        &self,
        request: OwnedLifecycleRequest,
    ) -> ExecutionManagerResult<OwnedLifecycleOutcome> {
        self.continuation_records
            .lock()
            .unwrap()
            .push(request.record().map(|record| record.id.clone()));
        self.continuations.fetch_add(1, Ordering::SeqCst);
        Err(ExecutionManagerError::Unavailable("probe-refused".into()))
    }
    async fn kill_with_status_with_budget(
        &self,
        _: &BoxRecord,
        budget: OwnedCloseBudget,
    ) -> ExecutionManagerResult<LocalExecutionTermination> {
        self.budgets.lock().unwrap().push(budget);
        Err(ExecutionManagerError::Unavailable("probe-refused".into()))
    }
    async fn stop_for_restart_with_budget(
        &self,
        _: &BoxRecord,
        budget: OwnedCloseBudget,
    ) -> ExecutionManagerResult<KillOutcome> {
        self.budgets.lock().unwrap().push(budget);
        Err(ExecutionManagerError::Unavailable("probe-refused".into()))
    }
}

#[tokio::test]
async fn router_keeps_durable_route_original_budget_and_id_only_box_probe() {
    let temporary = tempfile::tempdir().unwrap();
    let legacy = Arc::new(RoutedOwnedProbe::default());
    let oci = Arc::new(RoutedOwnedProbe::default());
    let router = Arc::new(LocalExecutionBackendRouter::new(
        legacy.clone(),
        oci.clone(),
        OciMigrationPolicy::AllViaOci,
    ));
    let manager = manager(temporary.path(), "managed-state", router.clone());
    let record = record(temporary.path());
    let admission = fresh(&manager, &record).await;
    let budget = OwnedCloseBudget::from_admission(Instant::now(), Duration::from_secs(3)).unwrap();
    assert!(router.admit_fresh_owner(&admission).await.is_err());
    assert!(router
        .kill_with_status_with_budget(&record, budget)
        .await
        .is_err());
    assert!(router
        .stop_for_restart_with_budget(&record, budget)
        .await
        .is_err());
    assert_eq!(*legacy.budgets.lock().unwrap(), vec![budget, budget]);
    assert!(oci.budgets.lock().unwrap().is_empty());
    assert_eq!(legacy.admissions.load(Ordering::SeqCst), 1);
    assert_eq!(oci.admissions.load(Ordering::SeqCst), 0);
    let mut oci_record = record.clone();
    oci_record.managed_execution.as_mut().unwrap().runtime_route =
        crate::ManagedRuntimeRoute::OciSdk;
    assert!(router
        .kill_with_status_with_budget(&oci_record, budget)
        .await
        .is_err());
    assert_eq!(*oci.budgets.lock().unwrap(), vec![budget]);
    assert!(!router
        .owner_facts(&record)
        .await
        .unwrap()
        .has_original_owner());
    assert_eq!(legacy.observations.load(Ordering::SeqCst), 1);
    assert_eq!(oci.observations.load(Ordering::SeqCst), 0);
    let guard = Arc::new(
        crate::local_execution::acquire_execution_lifecycle_lock(temporary.path(), &record.id)
            .unwrap(),
    );
    let start = OwnedLifecycleRequest::start_selected(
        record.clone(),
        manager.store.clone(),
        temporary.path().into(),
        Arc::clone(&guard),
    );
    assert!(router.continue_owned_lifecycle(start).await.is_err());
    let stop = OwnedLifecycleRequest::stop_selected(
        record.clone(),
        manager.store.clone(),
        temporary.path().into(),
        guard,
        budget,
    );
    assert!(router.continue_owned_lifecycle(stop).await.is_err());
    let reconcile = OwnedLifecycleRequest::reconcile_retained_remove(
        execution_id(&record).unwrap(),
        ExecutionGeneration::new(1).unwrap(),
    );
    assert!(router.continue_owned_lifecycle(reconcile).await.is_err());
    assert_eq!(legacy.continuations.load(Ordering::SeqCst), 3);
    assert_eq!(
        *legacy.continuation_records.lock().unwrap(),
        vec![Some(record.id.clone()), Some(record.id.clone()), None]
    );
    assert_eq!(oci.continuations.load(Ordering::SeqCst), 0);
    assert_eq!(legacy.legacy.calls.load(Ordering::SeqCst), 0);
    assert_eq!(oci.legacy.calls.load(Ordering::SeqCst), 0);
}
