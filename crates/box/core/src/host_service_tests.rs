#![cfg(test)]

use super::*;

fn binding(port: u32, path: &str) -> HostServiceSocket {
    HostServiceSocket {
        guest_port: port,
        socket_path: path.into(),
    }
}

#[test]
fn host_services_are_absent_in_legacy_config_and_instance_specs() {
    let config = serde_json::to_value(crate::BoxConfig::default()).unwrap();
    assert!(config.get("host_services").is_none());
    let restored: crate::BoxConfig = serde_json::from_value(config).unwrap();
    assert!(restored.host_services.is_empty());
    let spec = serde_json::to_value(crate::vmm::InstanceSpec::default()).unwrap();
    assert!(spec.get("host_services").is_none());
    let restored: crate::vmm::InstanceSpec = serde_json::from_value(spec).unwrap();
    assert!(restored.host_services.is_empty());
}

#[cfg(unix)]
#[test]
fn host_services_reject_reserved_and_duplicate_ports_and_paths() {
    let service = binding(61000, "/private/tmp/approved/gateway.sock");
    validate_host_service_sockets(std::slice::from_ref(&service), None).unwrap();
    for port in [
        0,
        1023,
        u32::MAX,
        crate::EXEC_VSOCK_PORT,
        crate::PTY_VSOCK_PORT,
        crate::ATTEST_VSOCK_PORT,
        crate::PORT_FWD_VSOCK_PORT,
        crate::config::SIDECAR_VSOCK_PORT,
    ] {
        assert!(validate_host_service_sockets(
            &[binding(port, "/private/tmp/approved/gateway.sock")],
            None
        )
        .is_err());
    }
    assert!(validate_host_service_sockets(std::slice::from_ref(&service), Some(61000)).is_err());
    assert!(validate_host_service_sockets(
        &[
            service.clone(),
            binding(61000, "/private/tmp/approved/other.sock")
        ],
        None
    )
    .is_err());
    assert!(validate_host_service_sockets(
        &[
            service.clone(),
            binding(61001, "/private/tmp/approved/gateway.sock")
        ],
        None
    )
    .is_err());
    assert!(validate_host_service_sockets(&vec![service; 17], None).is_err());
    for path in [
        "relative.sock",
        "/",
        "/tmp/../gateway.sock",
        "/tmp//gateway.sock",
        "/tmp/./gateway.sock",
        "/tmp/gateway.sock/",
        "/tmp/gateway\0.sock",
    ] {
        assert!(
            validate_host_service_sockets(&[binding(61000, path)], None).is_err(),
            "{path}"
        );
    }
}

#[cfg(unix)]
#[test]
fn host_services_cannot_be_silently_dropped_by_sandbox_or_snapshot_boot() {
    let mut config = crate::BoxConfig {
        host_services: vec![binding(61000, "/private/tmp/approved/gateway.sock")],
        isolation: crate::ExecutionIsolation::Sandbox,
        ..Default::default()
    };
    assert!(crate::resolve_execution(&config)
        .unwrap_err()
        .to_string()
        .contains("Host service"));
    config.isolation = crate::ExecutionIsolation::Microvm;
    config.restore_from = Some("snapshot".into());
    assert!(crate::resolve_execution(&config).is_err());
}

#[cfg(unix)]
#[test]
fn host_service_paths_require_private_owned_socket_and_ancestors() {
    use std::os::unix::fs::{symlink, MetadataExt, PermissionsExt};
    let directory = tempfile::Builder::new()
        .prefix("aster-svc-")
        .tempdir_in("/tmp")
        .unwrap();
    let parent = directory.path().canonicalize().unwrap();
    std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o700)).unwrap();
    let path = parent.join("gateway.sock");
    let service = HostServiceSocket {
        guest_port: 61000,
        socket_path: path.clone(),
    };
    let owner = parent.metadata().unwrap().uid();
    assert!(service.validate_host_path(owner).is_err());
    std::fs::write(&path, "not a socket").unwrap();
    assert!(service.validate_host_path(owner).is_err());
    std::fs::remove_file(&path).unwrap();
    let socket = std::os::unix::net::UnixListener::bind(&path).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    service.validate_host_path(owner).unwrap();
    assert!(service.validate_host_path(owner.wrapping_add(1)).is_err());
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o660)).unwrap();
    assert!(service.validate_host_path(owner).is_err());
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(service.validate_host_path(owner).is_err());
    std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o700)).unwrap();
    let alias = parent.join("alias.sock");
    symlink(&path, &alias).unwrap();
    assert!(HostServiceSocket {
        socket_path: alias,
        ..service.clone()
    }
    .validate_host_path(owner)
    .is_err());
    let alias = parent.join("parent");
    symlink(&parent, &alias).unwrap();
    assert!(HostServiceSocket {
        socket_path: alias.join("gateway.sock"),
        ..service.clone()
    }
    .validate_host_path(owner)
    .is_err());
    drop(socket);
    assert!(
        path.exists(),
        "validation must never unlink an endpoint owned by another lifecycle"
    );
}
