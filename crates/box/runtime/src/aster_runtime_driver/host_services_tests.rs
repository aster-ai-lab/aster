#![cfg(test)]

use super::super::metadata::validate_record_for_spec;
use super::super::test_support::{
    accepted, action, apply_request, fake_driver, runtime_spec, unit,
};
use super::*;
use aster_runtime::contract::{RuntimeFeature, RuntimeUnitClass, RuntimeUnitState};
use aster_runtime::RuntimeDriver;
use std::os::unix::fs::PermissionsExt;
use std::sync::Mutex;

struct Resolver(Mutex<Option<std::path::PathBuf>>);

#[async_trait]
impl BoxHostServiceResolver for Resolver {
    async fn resolve(
        &self,
        _spec: &RuntimeUnitSpec,
        service: &RuntimeHostService,
    ) -> RuntimeResult<HostServiceSocket> {
        if service.reference != "os.gateway" {
            return Err(RuntimeError::InvalidRequest(
                "Host service reference is not authorized".into(),
            ));
        }
        self.0
            .lock()
            .unwrap()
            .clone()
            .map(|socket_path| HostServiceSocket {
                guest_port: service.vsock_port,
                socket_path,
            })
            .ok_or_else(|| RuntimeError::InvalidRequest("Host service grant revoked".into()))
    }
}

fn requested_spec() -> RuntimeUnitSpec {
    let mut spec = runtime_spec("host-services", 1, RuntimeUnitClass::Service);
    spec.network.host_services.push(RuntimeHostService {
        reference: "os.gateway".into(),
        vsock_port: 61000,
    });
    spec
}

fn socket() -> (
    tempfile::TempDir,
    std::os::unix::net::UnixListener,
    std::path::PathBuf,
) {
    let directory = tempfile::Builder::new()
        .prefix("aster-grant-")
        .tempdir_in("/tmp")
        .unwrap();
    let parent = directory.path().canonicalize().unwrap();
    std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o700)).unwrap();
    let path = parent.join("gateway.sock");
    let listener = std::os::unix::net::UnixListener::bind(&path).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    (directory, listener, path)
}

#[tokio::test]
async fn host_services_require_explicit_request_and_configured_resolver() {
    let directory = tempfile::tempdir().unwrap();
    let (driver, _) = fake_driver(&directory);
    assert!(!driver
        .capabilities()
        .await
        .unwrap()
        .supports_feature(RuntimeFeature::HostServices));
    let spec = requested_spec();
    assert!(matches!(
        driver.apply(&apply_request(&spec), &accepted(&spec)).await,
        Err(RuntimeError::UnsupportedCapabilities(_))
    ));
    assert!(driver.manager.managed_records().await.unwrap().is_empty());
    let mut unrequested = spec;
    unrequested.network.host_services.clear();
    unrequested
        .process
        .environment
        .insert("ASTER_GATEWAY_ORIGIN".into(), "vsock://2:61000".into());
    driver
        .apply(&apply_request(&unrequested), &accepted(&unrequested))
        .await
        .unwrap();
    assert!(driver.manager.managed_records().await.unwrap()[0]
        .managed_execution
        .as_ref()
        .unwrap()
        .request
        .config
        .host_services
        .is_empty());
}

#[tokio::test]
async fn unknown_or_reserved_host_service_fails_before_creating_execution() {
    let directory = tempfile::tempdir().unwrap();
    let (_socket_dir, _listener, path) = socket();
    let (driver, _) = fake_driver(&directory);
    let driver = driver
        .with_host_service_resolver(Arc::new(Resolver(Mutex::new(Some(path)))))
        .unwrap();
    let mut spec = requested_spec();
    spec.network.host_services[0].reference = "unknown".into();
    assert!(driver
        .apply(&apply_request(&spec), &accepted(&spec))
        .await
        .is_err());
    spec.network.host_services[0].reference = "os.gateway".into();
    spec.network.host_services[0].vsock_port = aster_box_core::EXEC_VSOCK_PORT;
    assert!(driver
        .apply(&apply_request(&spec), &accepted(&spec))
        .await
        .is_err());
    assert!(driver.manager.managed_records().await.unwrap().is_empty());
}

#[tokio::test]
async fn admitted_host_binding_is_persisted_detects_drift_and_can_retire_after_revocation() {
    let directory = tempfile::tempdir().unwrap();
    let (_socket_dir, _listener, path) = socket();
    let resolver = Arc::new(Resolver(Mutex::new(Some(path.clone()))));
    let (driver, _) = fake_driver(&directory);
    let driver = driver.with_host_service_resolver(resolver.clone()).unwrap();
    assert!(driver
        .capabilities()
        .await
        .unwrap()
        .supports_feature(RuntimeFeature::HostServices));
    let spec = requested_spec();
    let observation = driver
        .apply(&apply_request(&spec), &accepted(&spec))
        .await
        .unwrap();
    assert_eq!(observation.state, RuntimeUnitState::Running);
    let record = driver
        .manager
        .managed_records()
        .await
        .unwrap()
        .pop()
        .unwrap();
    assert_eq!(
        record
            .managed_execution
            .as_ref()
            .unwrap()
            .request
            .config
            .host_services[0]
            .socket_path,
        path
    );
    let mut tampered = record.clone();
    tampered
        .managed_execution
        .as_mut()
        .unwrap()
        .request
        .config
        .host_services[0]
        .socket_path = "/private/changed.sock".into();
    assert!(validate_record_for_spec(
        &tampered,
        &spec,
        driver.execution_isolation,
        None,
        &driver.config.secret_root
    )
    .is_err());
    *resolver.0.lock().unwrap() = None;
    assert!(driver
        .host_services
        .authorize_start(&spec, &record)
        .await
        .is_err());
    let mut unit = unit(spec.clone(), observation);
    unit.observation = driver
        .stop(&unit, &action("stop-after-revocation", &spec))
        .await
        .unwrap();
    driver
        .remove(&unit, &action("remove-after-revocation", &spec))
        .await
        .unwrap();
    assert!(driver.manager.managed_records().await.unwrap().is_empty());
    assert!(path.exists());
}
