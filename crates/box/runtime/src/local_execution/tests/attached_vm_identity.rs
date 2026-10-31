#![cfg(test)]
#![cfg(unix)]

use super::*;
use aster_box_core::{
    BoxConfig, CreateExecutionRequest, ExecutionIsolation, OperationId, VmHandler, VmMetrics,
};
use futures::FutureExt;
use std::cell::Cell;
use std::collections::BTreeMap;
use std::process::{Child, Command};
use std::time::Instant;
use tokio::io::AsyncWriteExt;
use tokio::net::UnixListener;

fn record(home_dir: &Path, isolation: ExecutionIsolation) -> BoxRecord {
    let id = ExecutionId::new(uuid::Uuid::new_v4().to_string()).unwrap();
    super::super::record::build_managed_record(
        home_dir,
        &id,
        OperationId::new("attached-identity-fixture").unwrap(),
        CreateExecutionRequest {
            external_sandbox_id: "attached-identity-fixture".to_string(),
            config: BoxConfig {
                isolation,
                image: "alpine:latest".to_string(),
                ..Default::default()
            },
            labels: BTreeMap::new(),
            policy: Default::default(),
            rootfs_snapshot_id: None,
        },
        chrono::Utc::now(),
    )
    .unwrap()
}

struct SocketDirectory(std::path::PathBuf);

impl SocketDirectory {
    fn create(home_dir: &Path, record: &BoxRecord) -> Self {
        let directory = crate::vm::runtime_socket_dir(home_dir, &record.id);
        std::fs::create_dir_all(directory.parent().unwrap()).unwrap();
        // Only a directory created for this unique test ID may be cleaned up.
        // Unix ignores home_dir for sockets, so a fixed ID is not isolated.
        std::fs::create_dir(&directory).unwrap();
        Self(directory)
    }
}

impl Drop for SocketDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct ControlledChild {
    child: Child,
    start_time: u64,
}

impl ControlledChild {
    fn sleeping() -> Self {
        let child = Command::new("sleep").arg("30").spawn().unwrap();
        let start_time = crate::process::pid_start_time(child.id()).unwrap();
        Self { child, start_time }
    }

    fn bind(&self, record: &mut BoxRecord) -> LocatedProcess {
        record.pid = Some(self.child.id());
        record.pid_start_time = Some(self.start_time);
        record.status = ManagedExecutionState::Running.as_status().to_string();
        LocatedProcess {
            pid: self.child.id(),
            start_time: Some(self.start_time),
        }
    }

    fn terminate_and_observe(&mut self) {
        self.child.kill().unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if self.child.try_wait().unwrap().is_some() {
                return;
            }
            assert!(Instant::now() < deadline, "controlled child did not exit");
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

impl Drop for ControlledChild {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            match self.child.try_wait() {
                Ok(Some(_)) | Err(_) => break,
                Ok(None) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(5));
                }
                Ok(None) => break,
            }
        }
    }
}

#[tokio::test]
async fn recovery_refuses_missing_original_binding() {
    let temporary = tempfile::tempdir().unwrap();
    let backend = VmLocalExecutionBackend::new(temporary.path());
    let mut record = record(temporary.path(), ExecutionIsolation::Microvm);
    for (pid, start_time) in [
        (None, None),
        (Some(std::process::id()), None),
        (None, Some(1)),
        (Some(0), Some(1)),
    ] {
        record.pid = pid;
        record.pid_start_time = start_time;
        assert!(matches!(
            backend.recover_microvm(&record).await,
            Err(ExecutionManagerError::Unavailable(_))
        ));
        assert!(backend.managers.is_empty());
        assert!(!temporary.path().join("boxes").exists());
    }
    record.managed_execution = None;
    assert!(backend.recover_microvm(&record).await.is_err());
    assert!(backend.managers.is_empty());
}

#[tokio::test]
async fn attach_refuses_located_record_mismatch() {
    let temporary = tempfile::tempdir().unwrap();
    let backend = VmLocalExecutionBackend::new(temporary.path());
    let mut record = record(temporary.path(), ExecutionIsolation::Microvm);
    let mut fixture = ControlledChild::sleeping();
    let original = fixture.bind(&mut record);
    let socket_dir = SocketDirectory::create(temporary.path(), &record);
    let listener = UnixListener::bind(socket_dir.0.join("exec.sock")).unwrap();
    for located in [
        LocatedProcess {
            pid: original.pid.wrapping_add(1),
            start_time: original.start_time,
        },
        LocatedProcess {
            pid: original.pid,
            start_time: None,
        },
        LocatedProcess {
            pid: original.pid,
            start_time: Some(fixture.start_time.wrapping_add(1)),
        },
    ] {
        assert!(matches!(
            backend.attach_microvm(&record, located).await,
            Err(ExecutionManagerError::Unavailable(_))
        ));
        assert!(listener.accept().now_or_never().is_none());
        assert!(backend.managers.is_empty());
        assert_eq!(record.pid_start_time, original.start_time);
        assert!(fixture.child.try_wait().unwrap().is_none());
    }
}

#[tokio::test]
async fn attach_rechecks_original_identity_after_heartbeat_await() {
    let temporary = tempfile::tempdir().unwrap();
    let backend = VmLocalExecutionBackend::new(temporary.path());
    let mut record = record(temporary.path(), ExecutionIsolation::Microvm);
    let mut fixture = ControlledChild::sleeping();
    let located = fixture.bind(&mut record);
    let socket_dir = SocketDirectory::create(temporary.path(), &record);
    let listener = UnixListener::bind(socket_dir.0.join("exec.sock")).unwrap();
    let frame_observed = Cell::new(false);
    let stage = Cell::new("waiting_for_connect_probe");
    let observe = |value| {
        stage.set(value);
        eprintln!("attached heartbeat barrier stage={value}");
    };
    let attach = async {
        let result = backend.attach_microvm(&record, located).await;
        assert!(
            frame_observed.get(),
            "attach completed before the heartbeat barrier at stage={}: {}",
            stage.get(),
            match &result {
                Err(error) => format!("{error:?}"),
                Ok(_) => "unexpected attached manager".to_string(),
            }
        );
        result
    };
    let heartbeat = async {
        // Existing connect() performs one probe connection, then heartbeat()
        // sends a real frame on another connection and awaits its response.
        let (probe, _) = listener.accept().await.unwrap();
        observe("connect_probe_accepted");
        drop(probe);
        let (stream, _) = listener.accept().await.unwrap();
        observe("heartbeat_connection_accepted");
        let (read, mut write) = tokio::io::split(stream);
        let mut reader = aster_transport::FrameReader::new(read);
        let frame = reader.read_frame().await.unwrap().unwrap();
        assert_eq!(frame.frame_type, aster_transport::FrameType::Heartbeat);
        frame_observed.set(true);
        observe("heartbeat_frame_received");
        // Only the test fixture owns/reaps this Child. The product attach
        // must refuse after the await, not manufacture attached/owned proof.
        fixture.terminate_and_observe();
        observe("original_fixture_child_reaped");
        write
            .write_all(&aster_transport::Frame::heartbeat().encode().unwrap())
            .await
            .unwrap();
        observe("heartbeat_response_sent");
    };
    let (result, ()) = tokio::time::timeout(Duration::from_secs(3), async {
        tokio::join!(attach, heartbeat)
    })
    .await
    .unwrap_or_else(|error| {
        panic!(
            "heartbeat barrier timed out at stage={}: {error}",
            stage.get()
        )
    });
    assert!(matches!(result, Err(ExecutionManagerError::Unavailable(_))));
    assert!(backend.managers.is_empty());
    assert_eq!(record.pid_start_time, located.start_time);
}

struct CurrentProcessHandler;

impl VmHandler for CurrentProcessHandler {
    fn stop(&mut self, _: i32, _: u64) -> aster_box_core::Result<()> {
        Err(BoxError::StateError("fixture cannot stop".to_string()))
    }

    fn metrics(&self) -> VmMetrics {
        VmMetrics::default()
    }

    fn is_running(&self) -> bool {
        true
    }

    fn pid(&self) -> u32 {
        std::process::id()
    }
}

async fn paused_manager(backend: &VmLocalExecutionBackend, record: &BoxRecord) -> VmManager {
    let mut manager = backend.new_manager(record).unwrap();
    *manager.state.write().await = crate::BoxState::Paused;
    *manager.handler.write().await = Some(Box::new(CurrentProcessHandler));
    manager.exec_socket_path =
        Some(crate::vm::runtime_socket_dir(&backend.home_dir, &record.id).join("exec.sock"));
    manager
}

#[tokio::test]
async fn handle_preserves_record_token_and_refuses_mismatch() {
    let temporary = tempfile::tempdir().unwrap();
    let backend = VmLocalExecutionBackend::new(temporary.path());
    let mut record = record(temporary.path(), ExecutionIsolation::Microvm);
    let pid = std::process::id();
    let start_time = crate::process::pid_start_time(pid).unwrap();
    record.pid = Some(pid);
    record.pid_start_time = Some(start_time);
    let manager = paused_manager(&backend, &record).await;
    let handle = backend
        .handle_from_manager(&record, &manager)
        .await
        .unwrap();
    assert_eq!(handle.pid, record.pid);
    assert_eq!(handle.pid_start_time, Some(start_time));

    for (recorded_pid, recorded_start) in [
        (Some(pid), Some(start_time.wrapping_add(1))),
        (Some(pid.wrapping_add(1)), Some(start_time)),
        (Some(pid), None),
        (None, Some(start_time)),
    ] {
        record.pid = recorded_pid;
        record.pid_start_time = recorded_start;
        assert!(matches!(
            backend.handle_from_manager(&record, &manager).await,
            Err(ExecutionManagerError::Unavailable(_))
        ));
    }

    // The initial first-start observation stays legacy and unqualified.
    record.pid = None;
    record.pid_start_time = None;
    let first = backend
        .handle_from_manager(&record, &manager)
        .await
        .unwrap();
    assert_eq!(first.pid, Some(pid));
    assert_eq!(first.pid_start_time, Some(start_time));

    // Sandbox retains its existing sample/handle contract, not this MicroVM
    // attachment qualification. No Sandbox launch or external adapter runs.
    let mut sandbox = self::record(temporary.path(), ExecutionIsolation::Sandbox);
    sandbox.pid = Some(pid.wrapping_add(1));
    sandbox.pid_start_time = None;
    let sandbox_manager = paused_manager(&backend, &sandbox).await;
    let handle = backend
        .handle_from_manager(&sandbox, &sandbox_manager)
        .await
        .unwrap();
    assert_eq!(handle.pid, Some(pid));
    assert_eq!(handle.pid_start_time, Some(start_time));
}
