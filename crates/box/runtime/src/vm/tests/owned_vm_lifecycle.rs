#![cfg(test)]

use super::*;
use aster_box_core::vmm::{InstanceSpec, OwnedCleanupBudget, OwnedCloseBudget, VmExitEvidenceKind};
use aster_box_core::{
    CreateExecutionRequest, ExecutionGeneration, ExecutionIsolation, NetworkMode, OperationId,
};
use async_trait::async_trait;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex as StdMutex;
use std::time::{Duration, Instant};

struct OriginalFixtureHome {
    home: tempfile::TempDir,
    _socket: OwnedFixtureSocket,
}
impl OriginalFixtureHome {
    fn path(&self) -> &Path {
        self.home.path()
    }
}

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

fn original() -> (
    OriginalFixtureHome,
    VmManager,
    OriginalVmScope,
    Arc<crate::local_execution::ExecutionLifecycleLock>,
) {
    let temp = tempfile::tempdir().unwrap();
    let id = uuid::Uuid::new_v4().to_string();
    let config = BoxConfig {
        isolation: ExecutionIsolation::Microvm,
        network: NetworkMode::None,
        ..Default::default()
    };
    let mut vm = VmManager::with_box_id(config.clone(), EventEmitter::new(32), id.clone());
    vm.home_dir = temp.path().into();
    let mut record: crate::BoxRecord = serde_json::from_value(serde_json::json!({
        "id":id.clone(),"short_id":&id[..8],"name":"owned-leaf","image":"alpine:latest","status":"created",
        "pid":null,"cpus":1,"memory_mb":128,"volumes":[],"env":{},"cmd":[],
        "box_dir":temp.path().join("boxes").join(id.clone()),"console_log":temp.path().join("console.log"),
        "created_at":"2026-10-10T00:00:00Z","started_at":null,"auto_remove":false
    }))
    .unwrap();
    record.managed_execution = Some(
        crate::ManagedExecutionMetadata::new(
            OperationId::new("owned-leaf-create").unwrap(),
            ExecutionGeneration::INITIAL,
            CreateExecutionRequest {
                external_sandbox_id: "leaf".into(),
                config,
                labels: Default::default(),
                policy: Default::default(),
                rootfs_snapshot_id: None,
            },
        )
        .unwrap(),
    );
    let scope = OriginalVmScope::from_original(&vm, &record, &temp.path().join("state")).unwrap();
    vm.bind_original_scope(scope.clone()).unwrap();
    let guard = Arc::new(
        crate::local_execution::acquire_execution_lifecycle_lock(temp.path(), &id).unwrap(),
    );
    let socket = OwnedFixtureSocket::new(temp.path(), &id);
    (
        OriginalFixtureHome {
            home: temp,
            _socket: socket,
        },
        vm,
        scope,
        guard,
    )
}

struct ChildHandler {
    inner: crate::vmm::ShimHandler,
    budgets: Arc<StdMutex<Vec<OwnedCloseBudget>>>,
    fail: bool,
}
impl VmHandler for ChildHandler {
    fn stop(&mut self, signal: i32, timeout_ms: u64) -> Result<()> {
        self.inner.stop(signal, timeout_ms)
    }
    fn stop_with_budget(&mut self, signal: i32, budget: OwnedCloseBudget) -> Result<()> {
        self.budgets.lock().unwrap().push(budget);
        if self.fail {
            self.fail = false;
            return Err(BoxError::StateError(
                "controlled original stop failure".into(),
            ));
        }
        self.inner.stop_with_budget(signal, budget)
    }
    fn exit_confirmation(&self) -> Option<aster_box_core::vmm::VmExitConfirmation> {
        self.inner.exit_confirmation()
    }
    fn metrics(&self) -> crate::vmm::VmMetrics {
        self.inner.metrics()
    }
    fn is_running(&self) -> bool {
        self.inner.is_running()
    }
    fn pid(&self) -> u32 {
        self.inner.pid()
    }
    fn exit_code(&self) -> Option<i32> {
        self.inner.exit_code()
    }
    fn try_wait_exit(&mut self) -> Result<Option<i32>> {
        self.inner.try_wait_exit()
    }
}
impl Drop for ChildHandler {
    fn drop(&mut self) {
        // Test fixture owns this bounded child; never address an external PID.
        let budget = OwnedCloseBudget::from_admission(Instant::now(), Duration::ZERO).unwrap();
        let _ = self.inner.stop_with_budget(9, budget);
    }
}
fn child(fail: bool) -> (Box<dyn VmHandler>, Arc<StdMutex<Vec<OwnedCloseBudget>>>) {
    let process = std::process::Command::new("/bin/sh")
        .args(["-c", "exec sleep 3"])
        .spawn()
        .unwrap();
    let budgets = Arc::new(StdMutex::new(Vec::new()));
    (
        Box::new(ChildHandler {
            inner: crate::vmm::ShimHandler::from_child(process, "controlled-owned-leaf".into()),
            budgets: Arc::clone(&budgets),
            fail,
        }),
        budgets,
    )
}
struct ReturnProvider {
    handler: StdMutex<Option<Box<dyn VmHandler>>>,
    calls: Arc<AtomicUsize>,
}
#[async_trait]
impl VmmProvider for ReturnProvider {
    async fn start(&self, _: &InstanceSpec) -> Result<Box<dyn VmHandler>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.handler
            .lock()
            .unwrap()
            .take()
            .ok_or_else(|| BoxError::StateError("provider repeated".into()))
    }
}
struct PendingProvider {
    entered: Arc<AtomicUsize>,
    error: bool,
}
#[async_trait]
impl VmmProvider for PendingProvider {
    async fn start(&self, _: &InstanceSpec) -> Result<Box<dyn VmHandler>> {
        self.entered.fetch_add(1, Ordering::SeqCst);
        if self.error {
            Err(BoxError::StateError(
                "controlled provider error after entry".into(),
            ))
        } else {
            std::future::pending().await
        }
    }
}
async fn poll_pending<F: std::future::Future>(future: std::pin::Pin<&mut F>) {
    let mut future = future;
    std::future::poll_fn(|cx| {
        assert!(future.as_mut().poll(cx).is_pending());
        std::task::Poll::Ready(())
    })
    .await;
}

#[cfg(unix)]
#[tokio::test]
async fn returned_owned_child_survives_install_waiter_drop_and_conflicting_slot() {
    for conflict in [false, true] {
        let (_temp, mut vm, scope, guard) = original();
        let (returned, _) = child(false);
        let returned_pid = returned.pid();
        let calls = Arc::new(AtomicUsize::new(0));
        vm.provider = Some(Box::new(ReturnProvider {
            handler: StdMutex::new(Some(returned)),
            calls: Arc::clone(&calls),
        }));
        let slot = Arc::clone(&vm.handler);
        let mut locked = slot.write().await;
        if conflict {
            let (other, _) = child(false);
            *locked = Some(other);
        }
        let mut acquisition = OriginalAcquisition::NeverAcquired;
        {
            let spec = InstanceSpec::default();
            let future = vm.start_original_handler(&spec, &scope, &mut acquisition);
            tokio::pin!(future);
            poll_pending(future.as_mut()).await;
        }
        assert_eq!(acquisition, OriginalAcquisition::Returned);
        assert_eq!(vm.pending_handler.as_ref().unwrap().pid(), returned_pid);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        drop(locked);
        assert!(vm
            .start_original_handler(&InstanceSpec::default(), &scope, &mut acquisition)
            .await
            .is_err());
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        if conflict {
            assert!(vm
                .close_original_owned(
                    &scope,
                    &guard,
                    15,
                    OwnedCloseBudget::from_admission(Instant::now(), Duration::ZERO).unwrap()
                )
                .await
                .is_err());
            assert!(vm.handler.read().await.is_some());
            assert!(vm.pending_handler.is_some());
        } else {
            let proof = vm
                .close_original_owned(
                    &scope,
                    &guard,
                    15,
                    OwnedCloseBudget::from_admission(Instant::now(), Duration::ZERO).unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(proof.pid(), returned_pid);
            assert_eq!(proof.kind(), VmExitEvidenceKind::OwnedReaped);
            assert!(vm.pending_handler.is_some());
            assert!(vm.handler.read().await.is_none());
        }
        assert_eq!(vm.state().await, BoxState::Created);
    }
}

#[tokio::test]
async fn provider_entry_is_attempted_before_yield_error_and_reentry_never_reacquires() {
    for error in [false, true] {
        let (_temp, mut vm, scope, _guard) = original();
        let entered = Arc::new(AtomicUsize::new(0));
        vm.provider = Some(Box::new(PendingProvider {
            entered: Arc::clone(&entered),
            error,
        }));
        let mut acquisition = OriginalAcquisition::NeverAcquired;
        if error {
            assert!(vm
                .start_original_handler(&InstanceSpec::default(), &scope, &mut acquisition)
                .await
                .is_err());
        } else {
            let spec = InstanceSpec::default();
            let future = vm.start_original_handler(&spec, &scope, &mut acquisition);
            tokio::pin!(future);
            poll_pending(future.as_mut()).await;
        }
        assert_eq!(acquisition, OriginalAcquisition::Attempted);
        assert_eq!(entered.load(Ordering::SeqCst), 1);
        assert!(vm
            .start_original_handler(&InstanceSpec::default(), &scope, &mut acquisition)
            .await
            .is_err());
        assert_eq!(entered.load(Ordering::SeqCst), 1);
        assert!(vm.pending_handler.is_none());
    }
    let (_temp, mut vm, mut scope, _guard) = original();
    let entered = Arc::new(AtomicUsize::new(0));
    vm.provider = Some(Box::new(PendingProvider {
        entered: Arc::clone(&entered),
        error: true,
    }));
    scope.store.push("foreign");
    let mut acquisition = OriginalAcquisition::NeverAcquired;
    assert!(vm
        .start_original_handler(&InstanceSpec::default(), &scope, &mut acquisition)
        .await
        .is_err());
    assert_eq!(acquisition, OriginalAcquisition::NeverAcquired);
    assert_eq!(entered.load(Ordering::SeqCst), 0);
}

#[cfg(unix)]
#[tokio::test]
async fn original_close_forwards_exact_budget_and_retains_handler_after_expiry_or_error() {
    let (_temp, mut vm, scope, guard) = original();
    let (handler, budgets) = child(true);
    let pid = handler.pid();
    vm.pending_handler = Some(handler);
    let expired =
        OwnedCloseBudget::from_admission(Instant::now() - Duration::from_secs(6), Duration::ZERO)
            .unwrap();
    assert!(vm
        .close_original_owned(&scope, &guard, 15, expired)
        .await
        .is_err());
    assert!(budgets.lock().unwrap().is_empty());
    let admitted = OwnedCloseBudget::from_admission(Instant::now(), Duration::ZERO).unwrap();
    assert!(vm
        .close_original_owned(&scope, &guard, 15, admitted)
        .await
        .is_err());
    assert_eq!(budgets.lock().unwrap().as_slice(), &[admitted]);
    assert_eq!(vm.pending_handler.as_ref().unwrap().pid(), pid);
    assert!(vm
        .pending_handler
        .as_ref()
        .unwrap()
        .exit_confirmation()
        .is_none());
    assert_eq!(vm.state().await, BoxState::Created);
    let retry = OwnedCloseBudget::from_admission(Instant::now(), Duration::ZERO).unwrap();
    let proof = vm
        .close_original_owned(&scope, &guard, 15, retry)
        .await
        .unwrap();
    assert_eq!(proof.pid(), pid);
    assert_eq!(proof.kind(), VmExitEvidenceKind::OwnedReaped);
    assert!(proof.status().is_some());
    assert!(proof.status().unwrap().code().is_none());
    assert_eq!(vm.pending_handler.as_ref().unwrap().exit_code(), None);
    assert_eq!(budgets.lock().unwrap().as_slice(), &[admitted, retry]);
    let repeat = vm
        .close_original_owned(&scope, &guard, 15, retry)
        .await
        .unwrap();
    assert_eq!(repeat.pid(), proof.pid());
    assert_eq!(vm.state().await, BoxState::Created);
}

struct RootfsSpy {
    cleanups: Arc<AtomicUsize>,
    fail: bool,
}
impl crate::rootfs::RootfsProvider for RootfsSpy {
    fn prepare(&self, box_dir: &Path, _: &Path) -> Result<PathBuf> {
        Ok(box_dir.into())
    }
    fn cleanup(&self, _: &Path, _: bool) -> Result<()> {
        self.cleanups.fetch_add(1, Ordering::SeqCst);
        if self.fail {
            Err(BoxError::StateError(
                "controlled rootfs cleanup failure".into(),
            ))
        } else {
            Ok(())
        }
    }
    fn name(&self) -> &'static str {
        "owned-test-rootfs"
    }
}
struct CompanionSpy {
    stops: Arc<AtomicUsize>,
    socket: PathBuf,
}
impl crate::network::NetworkBackend for CompanionSpy {
    fn socket_path(&self) -> &Path {
        &self.socket
    }
    fn stop(&mut self) {
        self.stops.fetch_add(1, Ordering::SeqCst);
    }
}

#[cfg(unix)]
#[tokio::test]
async fn finalization_requires_actual_bound_child_and_config_none_plus_actual_none() {
    for (required, actual) in [(false, false), (true, false), (false, true)] {
        let (temp, mut vm, scope, guard) = original();
        let (handler, _) = child(false);
        vm.pending_handler = Some(handler);
        let counter = Arc::new(AtomicUsize::new(0));
        let stops = Arc::new(AtomicUsize::new(0));
        vm.rootfs_provider = Arc::new(RootfsSpy {
            cleanups: Arc::clone(&counter),
            fail: false,
        });
        if required {
            vm.config.network = NetworkMode::Bridge {
                network: "required".into(),
            };
        }
        if actual {
            vm.net_manager = Some(Box::new(CompanionSpy {
                stops: Arc::clone(&stops),
                socket: temp.path().join("companion"),
            }));
        }
        std::fs::create_dir_all(temp.path().join("boxes").join(&vm.box_id)).unwrap();
        let proof = vm
            .close_original_owned(
                &scope,
                &guard,
                15,
                OwnedCloseBudget::from_admission(Instant::now(), Duration::ZERO).unwrap(),
            )
            .await
            .unwrap();
        let foreign = aster_box_core::vmm::VmExitConfirmation::owned_reaped(
            proof.pid() + 1,
            proof.pid_start_time(),
            proof.status().unwrap(),
        )
        .unwrap();
        assert!(vm
            .take_original_finalization(&scope, foreign, false)
            .is_err());
        assert_eq!(counter.load(Ordering::SeqCst), 0);
        let mut cell = vm.take_original_finalization(&scope, proof, false).unwrap();
        let result = cell.run(OwnedCleanupBudget::from_validation(Instant::now()).unwrap());
        assert_eq!(result.is_ok(), !required && !actual);
        assert_eq!(
            counter.load(Ordering::SeqCst),
            usize::from(!required && !actual)
        );
        assert_eq!(stops.load(Ordering::SeqCst), 0);
        assert_eq!(
            temp.path().join("boxes").join(&vm.box_id).exists(),
            required || actual
        );
    }
}

#[cfg(unix)]
#[tokio::test]
async fn finalization_failure_keeps_exact_provider_paths_and_direct_managers_unqualified() {
    let (temp, mut vm, scope, guard) = original();
    let (handler, _) = child(false);
    vm.pending_handler = Some(handler);
    let counter = Arc::new(AtomicUsize::new(0));
    vm.set_rootfs_provider(Box::new(RootfsSpy {
        cleanups: Arc::clone(&counter),
        fail: true,
    }));
    let box_dir = temp.path().join("boxes").join(&vm.box_id);
    std::fs::create_dir_all(&box_dir).unwrap();
    std::fs::write(box_dir.join("identity"), b"same-owner").unwrap();
    let proof = vm
        .close_original_owned(
            &scope,
            &guard,
            15,
            OwnedCloseBudget::from_admission(Instant::now(), Duration::ZERO).unwrap(),
        )
        .await
        .unwrap();
    let mut cell = vm.take_original_finalization(&scope, proof, false).unwrap();
    let budget = OwnedCleanupBudget::from_validation(Instant::now()).unwrap();
    assert!(cell.run(budget).is_err());
    assert!(cell
        .run(OwnedCleanupBudget::from_validation(Instant::now()).unwrap())
        .is_err());
    assert_eq!(counter.load(Ordering::SeqCst), 1);
    assert_eq!(
        std::fs::read(box_dir.join("identity")).unwrap(),
        b"same-owner"
    );
    assert_eq!(
        vm.pending_handler
            .as_ref()
            .unwrap()
            .exit_confirmation()
            .unwrap()
            .pid(),
        proof.pid()
    );
    for isolation in [ExecutionIsolation::Microvm, ExecutionIsolation::Sandbox] {
        let mut direct = VmManager::new(
            BoxConfig {
                isolation,
                ..Default::default()
            },
            EventEmitter::new(8),
        );
        assert!(direct.original_scope.is_none());
        assert!(direct.pending_handler.is_none());
        assert!(direct
            .close_original_owned(
                &scope,
                &guard,
                15,
                OwnedCloseBudget::from_admission(Instant::now(), Duration::ZERO).unwrap()
            )
            .await
            .is_err());
    }
}

#[cfg(unix)]
#[test]
fn original_fixture_uuid_socket_paths_and_owned_drop_are_isolated() {
    let (first, first_vm, _, _) = original();
    let (second, second_vm, _, _) = original();
    assert_ne!(first_vm.box_id, second_vm.box_id);
    let first_socket = first_vm.socket_dir();
    let second_socket = second_vm.socket_dir();
    assert_ne!(first_socket, second_socket);
    assert_eq!(
        first_socket,
        crate::host_sockets::runtime_socket_dir(first.path(), &first_vm.box_id)
    );
    assert_eq!(
        second_socket,
        crate::host_sockets::runtime_socket_dir(second.path(), &second_vm.box_id)
    );
    std::fs::write(first_socket.join("identity"), first_vm.box_id.as_bytes()).unwrap();
    std::fs::write(second_socket.join("identity"), second_vm.box_id.as_bytes()).unwrap();
    drop(first);
    assert!(!first_socket.exists());
    assert_eq!(
        std::fs::read(second_socket.join("identity")).unwrap(),
        second_vm.box_id.as_bytes()
    );
    drop(second);
    assert!(!second_socket.exists());
}
