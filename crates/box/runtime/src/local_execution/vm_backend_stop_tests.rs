#![cfg(test)]

use super::*;

struct RefusingStop;

impl VmHandler for RefusingStop {
    fn stop(&mut self, _signal: i32, _timeout_ms: u64) -> aster_box_core::Result<()> {
        Err(BoxError::StateError("stop identity unavailable".into()))
    }

    fn is_running(&self) -> bool {
        true
    }
    fn pid(&self) -> u32 {
        4242
    }
    fn metrics(&self) -> VmMetrics {
        VmMetrics::default()
    }
}

#[tokio::test]
async fn stop_failure_retains_registered_vm_owner() {
    let directory = tempfile::tempdir().unwrap();
    let backend = VmLocalExecutionBackend::new(directory.path());
    let record = record(directory.path(), ExecutionIsolation::Microvm);
    let manager = backend.new_manager(&record).unwrap();
    *manager.state.write().await = crate::BoxState::Ready;
    *manager.handler.write().await = Some(Box::new(RefusingStop));
    let shared = Arc::new(Mutex::new(manager));
    backend
        .managers
        .insert(record.id.clone(), RegisteredVm::Legacy(Arc::clone(&shared)));

    assert!(backend
        .destroy_registered(&record, Arc::clone(&shared), false, false, Some(0))
        .await
        .is_err());
    assert!(Arc::ptr_eq(
        &backend.manager(&record.id).unwrap().unwrap(),
        &shared
    ));
    assert!(shared.lock().await.handler.read().await.is_some());
}
