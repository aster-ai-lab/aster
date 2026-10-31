#![cfg(test)]

use super::*;
#[cfg(unix)]
use crate::vmm::VmHandler;
use aster_box_core::vmm::{OwnedCleanupBudget, OwnedCloseBudget};
use aster_box_core::{CreateExecutionRequest, ExecutionIsolation, NetworkMode, OperationId};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Barrier, Mutex as StdMutex};
use std::time::Instant;

struct OwnedFixtureSocket {
    path: PathBuf,
    owned: bool,
}
impl OwnedFixtureSocket {
    fn new(home: &Path, id: &str) -> Self {
        let path = crate::host_sockets::runtime_socket_dir(home, id);
        #[cfg(unix)]
        {
            crate::host_sockets::ensure_shared_runtime_socket_root(
                &crate::host_sockets::shared_runtime_socket_root(),
            )
            .unwrap();
            // Exclusive creation must succeed before this fixture owns cleanup.
            std::fs::create_dir(&path).unwrap();
            Self { path, owned: true }
        }
        #[cfg(not(unix))]
        Self { path, owned: false }
    }
}
impl Drop for OwnedFixtureSocket {
    fn drop(&mut self) {
        if self.owned {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
}

fn budget() -> OwnedCleanupBudget {
    OwnedCleanupBudget::from_validation(Instant::now()).unwrap()
}
fn record(home: &Path) -> BoxRecord {
    let id = ExecutionId::new(uuid::Uuid::new_v4().to_string()).unwrap();
    let mut request = CreateExecutionRequest {
        external_sandbox_id: "owned-jobs-component".into(),
        config: aster_box_core::BoxConfig {
            isolation: ExecutionIsolation::Microvm,
            network: NetworkMode::None,
            image: "alpine:latest".into(),
            ..Default::default()
        },
        labels: Default::default(),
        policy: Default::default(),
        rootfs_snapshot_id: None,
    };
    request.policy.volume_names = vec!["workspace".into()];
    let mut record = crate::local_execution::record::build_managed_record(
        home,
        &id,
        OperationId::new("owned-job-create").unwrap(),
        request,
        chrono::Utc::now(),
    )
    .unwrap();
    record.managed_execution.as_mut().unwrap().runtime_route = crate::ManagedRuntimeRoute::BoxVm;
    record
}
async fn admitted(
    home: &Path,
    record: &BoxRecord,
) -> (
    VmLocalExecutionBackend,
    crate::LocalExecutionManager,
    Arc<AdmittedVm>,
    Arc<super::super::ExecutionLifecycleLock>,
    OwnedFixtureSocket,
) {
    let socket = OwnedFixtureSocket::new(home, &record.id);
    let backend = VmLocalExecutionBackend::new(home);
    let manager =
        crate::LocalExecutionManager::new(home.join("state"), home, Arc::new(backend.clone()));
    let reserved = manager
        .reserve_for_owner_admission(record.clone())
        .await
        .unwrap();
    assert!(matches!(
        reserved.reservation,
        crate::ManagedExecutionReservation::Reserved(_)
    ));
    let guard = Arc::new(
        crate::local_execution::acquire_execution_lifecycle_lock(home, &record.id).unwrap(),
    );
    backend
        .admit_fresh_owner(&reserved.admission.unwrap())
        .await
        .unwrap();
    let owner = match backend.managers.get(&record.id).unwrap().value() {
        RegisteredVm::Admitted(owner) => Arc::clone(owner),
        _ => panic!("original admitted owner required"),
    };
    (backend, manager, owner, guard, socket)
}
fn call<'a>(
    record: &'a BoxRecord,
    manager: &'a crate::LocalExecutionManager,
    guard: &'a Arc<super::super::ExecutionLifecycleLock>,
    owner: &AdmittedVm,
) -> super::super::store::ValidatedOwnedCall<'a> {
    super::super::store::ValidatedOwnedCall::new(
        record,
        &manager.store,
        &manager.home_dir,
        guard,
        &owner.binding,
    )
    .unwrap()
}
fn volumes(home: &Path) -> crate::VolumeStore {
    let store = crate::VolumeStore::new(home.join("volumes.json"), home.join("volumes"));
    store
        .create(aster_box_core::volume::VolumeConfig::new("workspace", ""))
        .unwrap();
    store
}
async fn poll_pending<F: std::future::Future>(future: std::pin::Pin<&mut F>) {
    let mut future = future;
    std::future::poll_fn(|cx| {
        assert!(future.as_mut().poll(cx).is_pending());
        std::task::Poll::Ready(())
    })
    .await;
}
fn hooks(after: bool, error_after: bool) -> (OwnedJobHooks, Arc<Barrier>, Arc<Barrier>) {
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let pair = Some((Arc::clone(&entered), Arc::clone(&release)));
    (
        OwnedJobHooks {
            before: if after { None } else { pair.clone() },
            after: if after { pair } else { None },
            panic_after: false,
            error_after,
        },
        entered,
        release,
    )
}
struct CleanupSpy {
    path: PathBuf,
    calls: Arc<AtomicUsize>,
    panic: bool,
    fail: bool,
}
impl crate::rootfs::RootfsProvider for CleanupSpy {
    fn prepare(&self, box_dir: &Path, _: &Path) -> aster_box_core::Result<PathBuf> {
        Ok(box_dir.into())
    }
    fn cleanup(&self, _: &Path, _: bool) -> aster_box_core::Result<()> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        std::fs::write(&self.path, b"actual-finalization-entered")?;
        if self.panic {
            panic!("controlled panic inside original finalization cell");
        }
        if self.fail {
            Err(BoxError::StateError(
                "actual finalization fixture failure".into(),
            ))
        } else {
            Ok(())
        }
    }
    fn name(&self) -> &'static str {
        "owned-job-spy"
    }
}
#[cfg(unix)]
struct ControlledChild(crate::vmm::ShimHandler);
#[cfg(unix)]
impl crate::vmm::VmHandler for ControlledChild {
    fn stop(&mut self, signal: i32, timeout_ms: u64) -> aster_box_core::Result<()> {
        self.0.stop(signal, timeout_ms)
    }
    fn stop_with_budget(
        &mut self,
        signal: i32,
        budget: OwnedCloseBudget,
    ) -> aster_box_core::Result<()> {
        self.0.stop_with_budget(signal, budget)
    }
    fn exit_confirmation(&self) -> Option<aster_box_core::vmm::VmExitConfirmation> {
        self.0.exit_confirmation()
    }
    fn metrics(&self) -> crate::vmm::VmMetrics {
        self.0.metrics()
    }
    fn is_running(&self) -> bool {
        self.0.is_running()
    }
    fn pid(&self) -> u32 {
        self.0.pid()
    }
    fn try_wait_exit(&mut self) -> aster_box_core::Result<Option<i32>> {
        self.0.try_wait_exit()
    }
}
#[cfg(unix)]
impl Drop for ControlledChild {
    fn drop(&mut self) {
        // Teardown addresses only the Child this fixture actually spawned.
        let attempt = OwnedCloseBudget::from_admission(Instant::now(), Duration::ZERO).unwrap();
        let _ = crate::vmm::VmHandler::stop_with_budget(&mut self.0, 9, attempt);
    }
}

#[cfg(unix)]
async fn finalization(
    owner: &AdmittedVm,
    guard: &Arc<super::super::ExecutionLifecycleLock>,
    path: PathBuf,
    panic: bool,
    fail: bool,
) -> (
    Arc<StdMutex<crate::vm::OriginalVmFinalization>>,
    Arc<AtomicUsize>,
) {
    finalization_with_policy(owner, guard, path, panic, fail, false).await
}

#[cfg(unix)]
async fn finalization_with_policy(
    owner: &AdmittedVm,
    guard: &Arc<super::super::ExecutionLifecycleLock>,
    path: PathBuf,
    panic: bool,
    fail: bool,
    preserve_rootfs: bool,
) -> (
    Arc<StdMutex<crate::vm::OriginalVmFinalization>>,
    Arc<AtomicUsize>,
) {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut progress = owner.progress.as_ref().unwrap().lock().await;
    let mut vm = owner.vm.lock().await;
    vm.set_rootfs_provider(Box::new(CleanupSpy {
        path,
        calls: Arc::clone(&calls),
        panic,
        fail,
    }));
    let child = std::process::Command::new("/bin/sh")
        .args(["-c", "exec sleep 3"])
        .spawn()
        .unwrap();
    *vm.handler.write().await = Some(Box::new(ControlledChild(
        crate::vmm::ShimHandler::from_child(child, "controlled-job-owned-child".into()),
    )));
    let proof = vm
        .close_original_owned(
            &progress.scope,
            guard,
            15,
            OwnedCloseBudget::from_admission(Instant::now(), Duration::ZERO).unwrap(),
        )
        .await
        .unwrap();
    progress.acquisition = crate::vm::OriginalAcquisition::Returned;
    progress.physical = Some(Ok(proof));
    let cleanup = budget();
    progress.cleanup_budget = Some(cleanup);
    let cell = Arc::new(StdMutex::new(
        vm.take_original_finalization(&progress.scope, proof, preserve_rootfs)
            .unwrap(),
    ));
    progress.finalization = Some(Arc::clone(&cell));
    (cell, calls)
}

#[cfg(unix)]
#[tokio::test]
async fn all_five_closed_families_keep_actual_handle_before_after_effect_and_late_outcome() {
    for kind in [
        OwnedVmJobKind::StageMetadata,
        OwnedVmJobKind::PrepareResources,
        OwnedVmJobKind::RollbackResources,
        OwnedVmJobKind::FinalizeVm,
        OwnedVmJobKind::CleanupAnonymousVolumes,
    ] {
        assert_eq!(OwnedVmJobKind::parse(kind.as_str()).unwrap(), kind);
        for after in [false, true] {
            for late_error in [false, true] {
                let temp = tempfile::tempdir().unwrap();
                let record = record(temp.path());
                let store = volumes(temp.path());
                let (_backend, manager, owner, guard, _socket) =
                    admitted(temp.path(), &record).await;
                let marker = record.box_dir.join("rootfs/.aster_exit_code");
                std::fs::create_dir_all(marker.parent().unwrap()).unwrap();
                std::fs::write(&marker, b"17\n").unwrap();
                let final_marker = temp.path().join("finalized");
                let mut cleanup_calls = None;
                let cell = Arc::new(StdMutex::new(ExecutionResourceGuard::selected_empty(
                    temp.path(),
                    &record,
                )));
                if kind == OwnedVmJobKind::RollbackResources {
                    cell.lock()
                        .unwrap()
                        .prepare_retained(&record, budget())
                        .unwrap();
                }
                let anonymous = "anon_owned_job";
                store.claim_anonymous(anonymous, &record.id).unwrap();
                let work = match kind {
                    OwnedVmJobKind::StageMetadata => {
                        OwnedVmWork::StageMetadata(record.box_dir.clone())
                    }
                    OwnedVmJobKind::PrepareResources => OwnedVmWork::PrepareResources {
                        cell: Arc::clone(&cell),
                        record: record.clone(),
                    },
                    OwnedVmJobKind::RollbackResources => {
                        OwnedVmWork::RollbackResources(Arc::clone(&cell))
                    }
                    OwnedVmJobKind::FinalizeVm => {
                        let (cell, calls) =
                            finalization(&owner, &guard, final_marker.clone(), false, false).await;
                        cleanup_calls = Some(calls);
                        OwnedVmWork::FinalizeVm(cell)
                    }
                    OwnedVmJobKind::CleanupAnonymousVolumes => {
                        OwnedVmWork::CleanupAnonymousVolumes(Arc::new(StdMutex::new(
                            OwnedAnonymousCleanup {
                                home: temp.path().into(),
                                owner: record.id.clone(),
                                names: vec![(anonymous.into(), false)],
                                unproved: false,
                            },
                        )))
                    }
                };
                let (h, entered, release) = hooks(after, late_error);
                let owned_call = call(&record, &manager, &guard, &owner);
                let attempt = budget();
                let mut progress = owner.progress.as_ref().unwrap().lock().await;
                progress.hooks = Some(h);
                match &work {
                    OwnedVmWork::PrepareResources { cell, .. }
                    | OwnedVmWork::RollbackResources(cell) => {
                        progress.resources = Some(Arc::clone(cell))
                    }
                    OwnedVmWork::CleanupAnonymousVolumes(cell) => {
                        progress.anonymous = Some(Arc::clone(cell))
                    }
                    _ => {}
                }
                progress.install(&owned_call, work, attempt).unwrap();
                {
                    let waiter = progress.poll_job(&owned_call, attempt);
                    tokio::pin!(waiter);
                    poll_pending(waiter.as_mut()).await;
                }
                entered.wait();
                assert!(
                    matches!(progress.job,OwnedVmJobSlot::Running{kind:actual,..} if actual==kind)
                );
                match kind {
                    OwnedVmJobKind::StageMetadata => assert_eq!(!marker.exists(), after),
                    OwnedVmJobKind::PrepareResources => assert_eq!(
                        store
                            .get("workspace")
                            .unwrap()
                            .unwrap()
                            .in_use_by
                            .contains(&record.id),
                        after
                    ),
                    OwnedVmJobKind::RollbackResources => assert_eq!(
                        !store
                            .get("workspace")
                            .unwrap()
                            .unwrap()
                            .in_use_by
                            .contains(&record.id),
                        after
                    ),
                    OwnedVmJobKind::FinalizeVm => assert_eq!(final_marker.exists(), after),
                    OwnedVmJobKind::CleanupAnonymousVolumes => {
                        assert_eq!(store.get(anonymous).unwrap().is_none(), after)
                    }
                }
                assert!(progress
                    .install(
                        &owned_call,
                        OwnedVmWork::StageMetadata(record.box_dir.clone()),
                        attempt
                    )
                    .is_err());
                release.wait();
                let actual = progress.poll_job(&owned_call, attempt).await;
                assert_eq!(actual.is_err(), late_error);
                assert!(matches!(
                    progress.job,
                    OwnedVmJobSlot::Ready { result: Ok(_), .. }
                ));
                assert_eq!(progress.reconcile_ready(&owned_call).is_err(), late_error);
                if let Some(calls) = cleanup_calls {
                    assert_eq!(calls.load(Ordering::SeqCst), 1);
                }
                assert!(OwnedVmJobKind::parse("caller-success").is_err());
            }
        }
    }
}

#[tokio::test]
async fn partial_resource_failure_retains_real_claim_and_selected_drop_never_rolls_back() {
    let temp = tempfile::tempdir().unwrap();
    let mut record = record(temp.path());
    record.volume_names.push("missing".into());
    let store = volumes(temp.path());
    let (backend, manager, owner, guard, _socket) = admitted(temp.path(), &record).await;
    let owned_call = call(&record, &manager, &guard, &owner);
    assert!(backend
        .run_original_preparation(&owned_call, budget())
        .await
        .is_err());
    assert!(store
        .get("workspace")
        .unwrap()
        .unwrap()
        .in_use_by
        .contains(&record.id));
    let retained = {
        let progress = owner.progress.as_ref().unwrap().lock().await;
        assert!(matches!(
            progress.job,
            OwnedVmJobSlot::Ready {
                result: Ok(OwnedVmJobOutput::Prepare(Err(_))),
                ..
            }
        ));
        Arc::clone(progress.resources.as_ref().unwrap())
    };
    let before = std::fs::read(temp.path().join("volumes.json")).unwrap();
    assert!(backend
        .run_original_preparation(&owned_call, budget())
        .await
        .is_err());
    assert_eq!(
        std::fs::read(temp.path().join("volumes.json")).unwrap(),
        before
    );
    backend
        .rollback_original_resources(&owned_call, budget())
        .await
        .unwrap();
    assert!(backend
        .run_original_preparation(&owned_call, budget())
        .await
        .is_err());
    assert!(!store
        .get("workspace")
        .unwrap()
        .unwrap()
        .in_use_by
        .contains(&record.id));
    // Released claim is not detached again; an external reattachment survives.
    store
        .modify("workspace", |volume| volume.attach(&record.id))
        .unwrap();
    retained
        .lock()
        .unwrap()
        .rollback_retained(budget())
        .unwrap();
    assert!(store
        .get("workspace")
        .unwrap()
        .unwrap()
        .in_use_by
        .contains(&record.id));
    drop(retained);
    drop(owned_call);
    drop(manager);
    drop(owner);
    drop(backend);
    assert!(store
        .get("workspace")
        .unwrap()
        .unwrap()
        .in_use_by
        .contains(&record.id));
    // Legacy guard keeps its existing Drop rollback contract.
    store
        .modify("workspace", |volume| volume.detach(&record.id))
        .unwrap();
    record.volume_names.pop();
    {
        let _legacy = ExecutionResourceGuard::prepare(temp.path(), &record).unwrap();
        assert!(store
            .get("workspace")
            .unwrap()
            .unwrap()
            .in_use_by
            .contains(&record.id));
    }
    assert!(!store
        .get("workspace")
        .unwrap()
        .unwrap()
        .in_use_by
        .contains(&record.id));
    let second = tempfile::tempdir().unwrap();
    let record = self::record(second.path());
    let store = volumes(second.path());
    let (backend, manager, owner, guard, _socket) = admitted(second.path(), &record).await;
    let owned_call = call(&record, &manager, &guard, &owner);
    backend
        .run_original_preparation(&owned_call, budget())
        .await
        .unwrap();
    let cell = Arc::clone(
        owner
            .progress
            .as_ref()
            .unwrap()
            .lock()
            .await
            .resources
            .as_ref()
            .unwrap(),
    );
    let metadata = second.path().join("volumes.json");
    let committed = std::fs::read(&metadata).unwrap();
    std::fs::write(&metadata, b"controlled invalid store bytes").unwrap();
    assert!(backend
        .rollback_original_resources(&owned_call, budget())
        .await
        .is_err());
    std::fs::write(&metadata, &committed).unwrap();
    assert!(backend
        .rollback_original_resources(&owned_call, budget())
        .await
        .is_err());
    assert!(Arc::ptr_eq(
        owner
            .progress
            .as_ref()
            .unwrap()
            .lock()
            .await
            .resources
            .as_ref()
            .unwrap(),
        &cell
    ));
    assert!(store
        .get("workspace")
        .unwrap()
        .unwrap()
        .in_use_by
        .contains(&record.id));
    drop(cell);
    drop(owned_call);
    drop(owner);
    drop(manager);
    drop(backend);
    assert_eq!(std::fs::read(&metadata).unwrap(), committed);
}

#[cfg(unix)]
#[tokio::test]
async fn actual_file_guard_outlives_waiter_and_releases_before_unpolled_ready() {
    let temp = tempfile::tempdir().unwrap();
    let record = record(temp.path());
    let (_backend, manager, owner, guard, _socket) = admitted(temp.path(), &record).await;
    let (h, entered, release) = hooks(true, false);
    let attempt = budget();
    {
        let owned_call = call(&record, &manager, &guard, &owner);
        let mut progress = owner.progress.as_ref().unwrap().lock().await;
        progress.hooks = Some(h);
        progress
            .install(
                &owned_call,
                OwnedVmWork::StageMetadata(record.box_dir.clone()),
                attempt,
            )
            .unwrap();
        {
            let waiter = progress.poll_job(&owned_call, attempt);
            tokio::pin!(waiter);
            poll_pending(waiter.as_mut()).await;
        }
    }
    entered.wait();
    drop(guard);
    let (started_tx, started_rx) = std::sync::mpsc::channel();
    let (acquired_tx, acquired_rx) = std::sync::mpsc::channel();
    let (leave_tx, leave_rx) = std::sync::mpsc::channel();
    let home = temp.path().to_path_buf();
    let id = record.id.clone();
    let contender = std::thread::spawn(move || {
        use std::os::fd::AsRawFd;
        let probe = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(home.join("locks").join(format!("{id}.lifecycle.lock")))
            .unwrap();
        // This exact original lock file is held by the real queued/running job.
        let refused = unsafe { libc::flock(probe.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
        assert_eq!(refused, -1);
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(libc::EWOULDBLOCK)
        );
        started_tx.send(()).unwrap();
        let actual = crate::local_execution::acquire_execution_lifecycle_lock(&home, &id).unwrap();
        acquired_tx.send(()).unwrap();
        leave_rx.recv().unwrap();
        drop(actual);
    });
    started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(acquired_rx.try_recv().is_err());
    release.wait();
    acquired_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(matches!(
        owner.progress.as_ref().unwrap().lock().await.job,
        OwnedVmJobSlot::Running { .. }
    ));
    leave_tx.send(()).unwrap();
    contender.join().unwrap();
    let guard = Arc::new(
        crate::local_execution::acquire_execution_lifecycle_lock(temp.path(), &record.id).unwrap(),
    );
    let owned_call = call(&record, &manager, &guard, &owner);
    let mut progress = owner.progress.as_ref().unwrap().lock().await;
    progress.poll_job(&owned_call, attempt).await.unwrap();
    assert!(matches!(
        progress.job,
        OwnedVmJobSlot::Ready { result: Ok(_), .. }
    ));
}

#[cfg(unix)]
#[tokio::test]
async fn panic_inside_actual_finalization_retains_join_error_poison_and_identity() {
    let temp = tempfile::tempdir().unwrap();
    let record = record(temp.path());
    let (backend, manager, owner, guard, _socket) = admitted(temp.path(), &record).await;
    let marker = temp.path().join("entered");
    let (cell, calls) = finalization(&owner, &guard, marker.clone(), true, false).await;
    let owned_call = call(&record, &manager, &guard, &owner);
    let attempt = owner
        .progress
        .as_ref()
        .unwrap()
        .lock()
        .await
        .cleanup_budget
        .unwrap();
    assert!(backend
        .finalize_original_component(&owned_call, false, attempt)
        .await
        .is_err());
    assert_eq!(
        std::fs::read(&marker).unwrap(),
        b"actual-finalization-entered"
    );
    assert!(cell.is_poisoned());
    let progress = owner.progress.as_ref().unwrap().lock().await;
    assert!(
        matches!(&progress.job,OwnedVmJobSlot::Ready{result:Err(error),..} if error.is_panic())
    );
    assert!(matches!(progress.physical, Some(Ok(_))));
    assert!(Arc::ptr_eq(progress.finalization.as_ref().unwrap(), &cell));
    drop(progress);
    assert!(backend
        .finalize_original_component(&owned_call, false, budget())
        .await
        .is_err());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn ready_storage_and_expired_attempt_keep_actual_result_without_duplicate_work() {
    let temp = tempfile::tempdir().unwrap();
    let record = record(temp.path());
    let (_backend, manager, owner, guard, _socket) = admitted(temp.path(), &record).await;
    let owned_call = call(&record, &manager, &guard, &owner);
    let mut progress = owner.progress.as_ref().unwrap().lock().await;
    let expired =
        OwnedCleanupBudget::from_validation(Instant::now() - Duration::from_secs(11)).unwrap();
    assert!(progress
        .install(
            &owned_call,
            OwnedVmWork::StageMetadata(record.box_dir.clone()),
            expired
        )
        .is_err());
    assert!(matches!(progress.job, OwnedVmJobSlot::Idle));
    assert!(progress
        .install(
            &owned_call,
            OwnedVmWork::StageMetadata(record.box_dir.join("foreign-input")),
            budget()
        )
        .is_err());
    assert!(matches!(progress.job, OwnedVmJobSlot::Idle));
    let attempt = budget();
    progress
        .install(
            &owned_call,
            OwnedVmWork::StageMetadata(record.box_dir.clone()),
            attempt,
        )
        .unwrap();
    progress.poll_job(&owned_call, attempt).await.unwrap();
    assert!(matches!(
        progress.job,
        OwnedVmJobSlot::Ready { result: Ok(_), .. }
    ));
    // A caller disappears immediately after Ready; reacquisition sees that same result.
    drop(progress);
    let mut progress = owner.progress.as_ref().unwrap().lock().await;
    assert!(progress.poll_job(&owned_call, expired).await.is_err());
    assert!(matches!(
        progress.job,
        OwnedVmJobSlot::Ready { result: Ok(_), .. }
    ));
    let marker = record.box_dir.join("rootfs/.aster_exit_code");
    std::fs::create_dir_all(marker.parent().unwrap()).unwrap();
    std::fs::write(&marker, b"must-not-replay").unwrap();
    progress
        .install(
            &owned_call,
            OwnedVmWork::StageMetadata(record.box_dir.clone()),
            budget(),
        )
        .unwrap();
    assert_eq!(std::fs::read(&marker).unwrap(), b"must-not-replay");
    assert!(progress.completed.is_empty());
    assert!(matches!(
        progress.job,
        OwnedVmJobSlot::Ready {
            kind: OwnedVmJobKind::StageMetadata,
            ..
        }
    ));
    drop(progress);
    drop(owned_call);
    drop(guard);
    drop(owner);
    drop(manager);
    let second = tempfile::tempdir().unwrap();
    let record = self::record(second.path());
    let (_backend, manager, owner, guard, _socket) = admitted(second.path(), &record).await;
    let owned_call = call(&record, &manager, &guard, &owner);
    let mut progress = owner.progress.as_ref().unwrap().lock().await;
    let (h, entered, release) = hooks(false, false);
    progress.hooks = Some(h);
    let original = budget();
    progress
        .install(
            &owned_call,
            OwnedVmWork::StageMetadata(record.box_dir.clone()),
            original,
        )
        .unwrap();
    entered.wait();
    assert!(progress.poll_job(&owned_call, expired).await.is_err());
    assert!(matches!(progress.job, OwnedVmJobSlot::Running { .. }));
    assert!(progress
        .install(
            &owned_call,
            OwnedVmWork::StageMetadata(record.box_dir.clone()),
            budget()
        )
        .is_err());
    release.wait();
    progress.poll_job(&owned_call, original).await.unwrap();
    assert!(matches!(
        progress.job,
        OwnedVmJobSlot::Ready { result: Ok(_), .. }
    ));
}

#[tokio::test]
async fn stale_original_store_route_registry_and_settings_refuse_before_new_effects() {
    let temp = tempfile::tempdir().unwrap();
    let record = record(temp.path());
    let (backend, manager, owner, guard, _socket) = admitted(temp.path(), &record).await;
    let foreign_store = crate::ManagedExecutionStore::new(temp.path().join("foreign-state"));
    assert!(super::super::store::ValidatedOwnedCall::new(
        &record,
        &foreign_store,
        temp.path(),
        &guard,
        &owner.binding
    )
    .is_err());
    let mut foreign = record.clone();
    foreign.managed_execution.as_mut().unwrap().runtime_route = crate::ManagedRuntimeRoute::OciSdk;
    assert!(super::super::store::ValidatedOwnedCall::new(
        &foreign,
        &manager.store,
        temp.path(),
        &guard,
        &owner.binding
    )
    .is_err());
    let mut changed = record.clone();
    changed.log_config = aster_box_core::log::LogConfig::default();
    changed.name.push_str("-unknown-drift");
    let changed_call = call(&changed, &manager, &guard, &owner);
    assert!(backend
        .run_original_preparation(&changed_call, budget())
        .await
        .is_err());
    assert!(!record.box_dir.exists());
    let mut network = record.clone();
    network.network_mode = NetworkMode::Bridge {
        network: "changed".into(),
    };
    network.network_name = Some("changed".into());
    {
        let mut progress = owner.progress.as_ref().unwrap().lock().await;
        let mut vm = owner.vm.lock().await;
        progress
            .refresh_inactive_network(&network, &mut vm)
            .unwrap();
        assert_eq!(
            serde_json::to_value(&vm.config.network).unwrap(),
            serde_json::to_value(&network.network_mode).unwrap()
        );
        assert!(Arc::ptr_eq(
            &owner,
            &backend
                .exact_original(&call(&network, &manager, &guard, &owner))
                .unwrap()
        ));
        progress
            .install(
                &call(&network, &manager, &guard, &owner),
                OwnedVmWork::StageMetadata(network.box_dir.clone()),
                budget(),
            )
            .unwrap();
        assert!(progress.refresh_inactive_network(&record, &mut vm).is_err());
    }
    let owned_call = call(&network, &manager, &guard, &owner);
    backend.managers.remove(&record.id);
    assert!(backend.recheck_original(&owned_call, &owner).is_err());
    assert!(backend.exact_original(&owned_call).is_err());
    // Finish the retained real job; registry absence still conveys no owner proof.
    owner
        .progress
        .as_ref()
        .unwrap()
        .lock()
        .await
        .poll_job(&owned_call, budget())
        .await
        .unwrap();
    let legacy = Arc::new(Mutex::new(
        backend
            .new_manager_from_metadata(&record, backend.metadata(&record).unwrap())
            .unwrap(),
    ));
    backend
        .managers
        .insert(record.id.clone(), RegisteredVm::Legacy(legacy));
    assert!(backend.exact_original(&owned_call).is_err());
    let foreign = Arc::new(AdmittedVm {
        binding: owner.binding.clone(),
        vm: Arc::clone(&owner.vm),
        progress: None,
    });
    backend
        .managers
        .insert(record.id.clone(), RegisteredVm::Admitted(foreign));
    assert!(backend.exact_original(&owned_call).is_err());
    assert!(!backend
        .owner_facts(&record)
        .await
        .unwrap()
        .has_original_owner());
}

#[cfg(unix)]
#[tokio::test]
async fn separate_cleanup_deadline_and_late_failure_never_erase_original_physical_proof() {
    let temp = tempfile::tempdir().unwrap();
    let mut record = record(temp.path());
    record.volume_names.clear();
    let (backend, manager, owner, guard, _socket) = admitted(temp.path(), &record).await;
    let owned_call = call(&record, &manager, &guard, &owner);
    let marker = temp.path().join("cleanup-effect");
    let (_cell, calls) = finalization(&owner, &guard, marker.clone(), false, true).await;
    let (proof, attempt) = {
        let progress = owner.progress.as_ref().unwrap().lock().await;
        (
            progress.physical.as_ref().unwrap().as_ref().unwrap().pid(),
            progress.cleanup_budget.unwrap(),
        )
    };
    assert_eq!(
        attempt.deadline().duration_since(attempt.admitted_at()),
        Duration::from_secs(10)
    );
    assert!(backend
        .finalize_original_component(&owned_call, false, attempt)
        .await
        .is_err());
    assert!(marker.exists());
    assert!(backend
        .finalize_original_component(&owned_call, false, attempt)
        .await
        .is_err());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    // Repeated close only reconciles actual proof; it cannot renew the cleanup clock.
    backend
        .close_original_component(
            &owned_call,
            15,
            OwnedCloseBudget::from_admission(Instant::now(), Duration::ZERO).unwrap(),
        )
        .await
        .unwrap();
    let progress = owner.progress.as_ref().unwrap().lock().await;
    assert_eq!(
        progress.physical.as_ref().unwrap().as_ref().unwrap().pid(),
        proof
    );
    assert_eq!(progress.cleanup_budget, Some(attempt));
    assert!(Arc::ptr_eq(
        &owner,
        &backend.exact_original(&owned_call).unwrap()
    ));
    drop(progress);
    drop(owned_call);
    drop(guard);
    drop(owner);
    drop(manager);
    let second = tempfile::tempdir().unwrap();
    let mut record = self::record(second.path());
    record.volume_names.clear();
    let (backend, manager, owner, guard, _socket) = admitted(second.path(), &record).await;
    let (_cell, calls) = finalization(
        &owner,
        &guard,
        second.path().join("confirmed-cleanup"),
        false,
        false,
    )
    .await;
    let owned_call = call(&record, &manager, &guard, &owner);
    let first = {
        let mut progress = owner.progress.as_ref().unwrap().lock().await;
        progress.hooks = Some(OwnedJobHooks {
            before: None,
            after: None,
            panic_after: false,
            error_after: true,
        });
        progress.cleanup_budget.unwrap()
    };
    assert!(backend
        .finalize_original_component(&owned_call, false, first)
        .await
        .is_err());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    owner.progress.as_ref().unwrap().lock().await.hooks = None;
    let explicit_retry = budget();
    assert!(explicit_retry.deadline() > first.deadline());
    backend
        .finalize_original_component(&owned_call, false, explicit_retry)
        .await
        .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let progress = owner.progress.as_ref().unwrap().lock().await;
    assert_eq!(progress.cleanup_budget, Some(explicit_retry));
    assert!(matches!(
        progress.job,
        OwnedVmJobSlot::Ready {
            result: Ok(OwnedVmJobOutput::Finalize(Ok(()))),
            ..
        }
    ));
    assert!(progress
        .completed
        .iter()
        .any(|prior| matches!(prior.result, Ok(OwnedVmJobOutput::Finalize(Err(_))))));
}

#[cfg(unix)]
#[tokio::test]
async fn finalization_policy_mismatch_refuses_idle_ready_and_retry_without_replay() {
    for preserve in [false, true] {
        for late_error in [false, true] {
            let temp = tempfile::tempdir().unwrap();
            let record = record(temp.path());
            let (backend, manager, owner, guard, socket) = admitted(temp.path(), &record).await;
            let other = self::record(temp.path());
            let other_socket = OwnedFixtureSocket::new(temp.path(), &other.id);
            assert_ne!(record.id, other.id);
            assert_ne!(socket.path, other_socket.path);
            assert_eq!(socket.path, owner.vm.lock().await.socket_dir());
            std::fs::write(other_socket.path.join("identity"), other.id.as_bytes()).unwrap();
            std::fs::create_dir_all(&record.box_dir).unwrap();
            let (cell, calls) = finalization_with_policy(
                &owner,
                &guard,
                temp.path().join("policy-effect"),
                false,
                false,
                preserve,
            )
            .await;
            let owned_call = call(&record, &manager, &guard, &owner);
            let (first, proof) = {
                let mut p = owner.progress.as_ref().unwrap().lock().await;
                p.hooks = Some(OwnedJobHooks {
                    before: None,
                    after: None,
                    panic_after: false,
                    error_after: late_error,
                });
                (
                    p.cleanup_budget.unwrap(),
                    *p.physical.as_ref().unwrap().as_ref().unwrap(),
                )
            };
            assert!(backend
                .finalize_original_component(&owned_call, !preserve, first)
                .await
                .is_err());
            assert_eq!(calls.load(Ordering::SeqCst), 0);
            {
                let p = owner.progress.as_ref().unwrap().lock().await;
                assert!(matches!(p.job, OwnedVmJobSlot::Idle));
                assert_eq!(p.cleanup_budget, Some(first));
                assert!(Arc::ptr_eq(p.finalization.as_ref().unwrap(), &cell));
                assert_eq!(
                    p.physical.as_ref().unwrap().as_ref().unwrap().pid(),
                    proof.pid()
                );
            }
            assert_eq!(
                backend
                    .finalize_original_component(&owned_call, preserve, first)
                    .await
                    .is_err(),
                late_error
            );
            assert_eq!(calls.load(Ordering::SeqCst), 1);
            assert_eq!(record.box_dir.exists(), preserve);
            assert!(!socket.path.exists());
            assert_eq!(
                std::fs::read(other_socket.path.join("identity")).unwrap(),
                other.id.as_bytes()
            );
            let retry = budget();
            assert!(retry.deadline() > first.deadline());
            for attempt in [first, retry] {
                assert!(backend
                    .finalize_original_component(&owned_call, !preserve, attempt)
                    .await
                    .is_err());
                let p = owner.progress.as_ref().unwrap().lock().await;
                assert_eq!(p.cleanup_budget, Some(first));
                assert!(Arc::ptr_eq(p.finalization.as_ref().unwrap(), &cell));
                assert_eq!(
                    p.physical.as_ref().unwrap().as_ref().unwrap().pid(),
                    proof.pid()
                );
                assert!(matches!(
                    p.job,
                    OwnedVmJobSlot::Ready {
                        kind: OwnedVmJobKind::FinalizeVm,
                        result: Ok(_),
                        ..
                    }
                ));
                assert_eq!(p.reconcile_ready(&owned_call).is_err(), late_error);
                assert!(p.completed.is_empty());
                assert_eq!(calls.load(Ordering::SeqCst), 1);
            }
            owner.progress.as_ref().unwrap().lock().await.hooks = None;
            let matching = if late_error { retry } else { first };
            backend
                .finalize_original_component(&owned_call, preserve, matching)
                .await
                .unwrap();
            assert_eq!(calls.load(Ordering::SeqCst), 1);
            let p = owner.progress.as_ref().unwrap().lock().await;
            assert_eq!(p.cleanup_budget, Some(matching));
            assert!(Arc::ptr_eq(p.finalization.as_ref().unwrap(), &cell));
            assert_eq!(
                p.physical.as_ref().unwrap().as_ref().unwrap().pid(),
                proof.pid()
            );
            assert!(matches!(
                p.job,
                OwnedVmJobSlot::Ready {
                    result: Ok(OwnedVmJobOutput::Finalize(Ok(()))),
                    ..
                }
            ));
            assert_eq!(record.box_dir.exists(), preserve);
            assert_eq!(
                std::fs::read(other_socket.path.join("identity")).unwrap(),
                other.id.as_bytes()
            );
        }
    }
}
