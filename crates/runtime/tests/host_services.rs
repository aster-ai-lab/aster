#![cfg(test)]

mod support;
use aster_runtime::contract::{NetworkMode, RuntimeFeature, RuntimeHostService, RuntimeUnitSpec};
use aster_runtime::RuntimeDriver;
use support::fault_driver::DeterministicFaultDriver;
use support::fixtures::service_spec;

fn service(reference: &str, port: u32) -> RuntimeHostService {
    RuntimeHostService {
        reference: reference.into(),
        vsock_port: port,
    }
}

#[test]
fn empty_host_services_preserve_legacy_wire_and_spec_digest() {
    let spec = service_spec("legacy-wire");
    let digest = spec.digest().unwrap();
    let mut value = serde_json::to_value(&spec).unwrap();
    assert!(value["network"].get("host_services").is_none());
    assert_eq!(
        value["network"],
        serde_json::json!({
            "mode": "none", "ports": []
        })
    );
    value["network"]["host_services"] = serde_json::json!([]);
    let restored: RuntimeUnitSpec = serde_json::from_value(value).unwrap();
    assert_eq!(restored.digest().unwrap(), digest);
    let mut requested = restored;
    requested
        .network
        .host_services
        .push(service("os.gateway", 61000));
    assert_eq!(requested.network.mode, NetworkMode::None);
    assert_ne!(requested.digest().unwrap(), digest);
    let mut value = serde_json::to_value(requested).unwrap();
    value["network"]["host_services"][0]["socket_path"] = "/private/host.sock".into();
    assert!(serde_json::from_value::<RuntimeUnitSpec>(value).is_err());
}

#[test]
fn optional_egress_policy_preserves_locked_wire_and_binds_explicit_restrictions() {
    let mut spec = service_spec("egress-wire");
    let default_digest = spec.digest().unwrap();
    let mut value = serde_json::to_value(&spec).unwrap();
    value["network"]["egress_allowlist"] = serde_json::Value::Null;
    let restored: RuntimeUnitSpec = serde_json::from_value(value).unwrap();
    assert_eq!(restored.digest().unwrap(), default_digest);
    assert!(serde_json::to_value(&restored).unwrap()["network"]
        .get("egress_allowlist")
        .is_none());
    for allowlist in [Vec::new(), vec!["ocr.example".into()]] {
        spec.network.egress_allowlist = Some(allowlist.clone());
        let encoded = serde_json::to_value(&spec).unwrap();
        assert_eq!(
            encoded["network"]["egress_allowlist"],
            serde_json::json!(allowlist)
        );
        assert_ne!(spec.digest().unwrap(), default_digest);
        let restored: RuntimeUnitSpec = serde_json::from_value(encoded).unwrap();
        assert_eq!(restored.digest().unwrap(), spec.digest().unwrap());
    }
}

#[test]
fn host_service_requests_are_bounded_and_cannot_alias_ports_or_references() {
    let mut spec = service_spec("host-policy");
    for port in [0, 1023, u32::MAX] {
        spec.network.host_services = vec![service("os.gateway", port)];
        assert!(spec.validate().is_err());
    }
    for bindings in [
        vec![service("os.gateway", 61000), service("os.gateway", 61001)],
        vec![service("os.gateway", 61000), service("other", 61000)],
        vec![service("bad\nreference", 61000)],
        (0..17)
            .map(|n| service(&format!("service-{n}"), 61000 + n))
            .collect(),
    ] {
        spec.network.host_services = bindings;
        assert!(spec.validate().is_err());
    }
}

#[tokio::test]
async fn explicit_host_services_require_capability_without_enabling_ip_networking() {
    let mut spec = service_spec("capability-gate");
    spec.network
        .host_services
        .push(service("os.gateway", 61000));
    let mut capabilities = DeterministicFaultDriver::new()
        .capabilities()
        .await
        .unwrap();
    assert_eq!(
        capabilities.missing_for(&spec).unwrap(),
        vec!["feature:HostServices"]
    );
    capabilities.features.push(RuntimeFeature::HostServices);
    assert!(capabilities.missing_for(&spec).unwrap().is_empty());
    assert_eq!(spec.network.mode, NetworkMode::None);
}
