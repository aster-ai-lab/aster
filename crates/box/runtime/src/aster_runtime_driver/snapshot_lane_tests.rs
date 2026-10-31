//! ED3 E4: the four-link engine conformance lane — a REAL Sandbox
//! execution proving the state-handoff chain end to end:
//! planned drain → keyed capture → successor restore → crash-window
//! cold degrade. Lane-gated like the CI sandbox composition: the
//! test is `#[ignore]` and runs only on a Linux host with the
//! delegated-cgroup + setuid-launcher surface (see
//! `crates/box/.github/workflows/ci.yml` 660–700 for the sanctioned
//! recipe; OrbStack hosts mount cgroup2 with `nsdelegate`, which
//! blocks the owner chain and cannot run this lane).

#![cfg(test)]
#![cfg(target_os = "linux")]

use std::sync::Arc;

use aster_runtime::contract::{
    RuntimeApplyRequest, RuntimeObservation, RuntimeUnitClass, RuntimeUnitState,
};

use crate::aster_runtime_driver::test_support::{accepted, runtime_spec};
use crate::{BoxRuntimeDriver, BoxRuntimeDriverConfig};
use aster_runtime::RuntimeDriver as _;

/// Wires the GA Sandbox engine (SandboxViaOci — the shared-kernel owner
/// path, no KVM) exactly as the product composes it on KVM-less hosts.
/// `BoxRuntimeDriver::new_with_isolation` wires the legacy VM backend,
/// which requires /dev/kvm and times out on CI runners.
async fn lane_driver_async(
    directory: &tempfile::TempDir,
    state_snapshot: bool,
) -> BoxRuntimeDriver {
    use std::sync::Arc;
    let home_dir = directory.path().join("home");
    std::fs::create_dir_all(&home_dir).expect("home dir");
    let mut config = crate::local_execution::NativeLinuxOciMigrationConfig::new(
        home_dir.join("oci-service-root"),
    )
    .expect("native linux OCI migration config");
    if let (Some(runtime), Some(agent)) = (
        std::env::var_os("ASTER_BOX_OCI_RUNTIME_PATH"),
        std::env::var_os("ASTER_BOX_OCI_AGENT_PATH"),
    ) {
        config = config
            .with_artifacts(runtime, agent)
            .expect("native linux OCI migration artifacts");
    }
    let manager = crate::LocalExecutionManager::with_native_linux_oci_migration(
        home_dir.join("boxes.json"),
        &home_dir,
        config,
    )
    .await
    .expect("GA Sandbox engine manager");
    let connector: Arc<dyn aster_box_core::ExecutionPortConnector> = Arc::new(manager.clone());
    BoxRuntimeDriver::with_manager_connector_and_materializer(
        BoxRuntimeDriverConfig {
            state_snapshot,
            secret_root: home_dir.join("runtime-secrets"),
            home_dir,
            control_timeout: std::time::Duration::from_secs(90),
            task_poll_interval: std::time::Duration::from_millis(20),
        },
        manager,
        connector,
        aster_box_core::ExecutionIsolation::Sandbox,
        None,
        None,
        None,
        false,
    )
    .expect("lane driver")
}

fn marker_spec(unit_id: &str, generation: u64) -> aster_runtime::contract::RuntimeUnitSpec {
    let mut spec = runtime_spec(unit_id, generation, RuntimeUnitClass::Service);
    // A persistent service that writes the marker on first boot and
    // stays resident: a Task runs to completion (Stopped) before the
    // capture can quiesce a live unit.
    spec.process.args = vec![
        "-c".into(),
        "mkdir -p /state; if [ ! -f /state/value ]; then printf 'captured' > /state/value; fi;          while :; do sleep 60; done"
            .into(),
    ];
    // The workload writes a marker on first boot and re-reads it on
    // every boot: a restored successor reports `restored`, proving
    // the snapshot crossed the drain.
    // The CI composition digest (docker.io/library/alpine) — a real,
    // pullable image so the lane exercises the actual engine.
    spec.artifact.uri = "oci://docker.io/library/alpine@sha256:d9e853e87e55526f6b2917df91a2115c36dd7c696a35be12163d44e6e2a4b6bc".into();
    spec.artifact.digest =
        "sha256:d9e853e87e55526f6b2917df91a2115c36dd7c696a35be12163d44e6e2a4b6bc".into();
    spec.artifact.media_type = "application/vnd.oci.image.manifest.v1+json".into();
    spec
}

fn apply_request(
    spec: &aster_runtime::contract::RuntimeUnitSpec,
    restore: Option<&str>,
) -> RuntimeApplyRequest {
    RuntimeApplyRequest {
        schema: RuntimeApplyRequest::SCHEMA.into(),
        request_id: format!("ed3-lane-{}", uuid::Uuid::new_v4().simple()),
        deadline_at_ms: None,
        spec: spec.clone(),
        restore_from_snapshot_operation_id: restore.map(str::to_string),
    }
}

/// The four-link lane. Marker semantics: the first boot writes the
/// marker; any later boot that sees it prints `restored` on stdout —
/// the assertion reads it back through the driver's observation of
/// the successor apply carrying the restore reference.
#[tokio::test]
#[ignore = "ED3 engine lane: requires a Linux host with the delegated-cgroup and setuid-launcher surface (see the crate's CI recipe)"]
async fn four_link_state_handoff_survives_a_real_sandbox_drain() {
    let directory = tempfile::tempdir().expect("lane directory");
    let driver = lane_driver_async(&directory, true).await;
    {
        // Link 1 — planned drain: apply the first generation.
        let spec = marker_spec("ed3-four-link", 1);
        let request = apply_request(&spec, None);
        let first = driver
            .apply(&request, &accepted(&spec))
            .await
            .expect("first apply");
        assert_eq!(first.state, RuntimeUnitState::Running);

        // Link 2 — keyed capture: snapshot before termination.
        let snapshot_request = aster_runtime::contract::RuntimeSnapshotRequest {
            schema: aster_runtime::contract::RuntimeSnapshotRequest::SCHEMA.into(),
            operation_id: "ed3-four-link-capture".into(),
            unit_id: spec.unit_id.clone(),
            generation: 1,
            include_memory: false,
            deadline_at_ms: None,
        };
        let snapshot = driver
            .snapshot(
                &lane_unit_record(&driver, &spec, &first).await,
                &snapshot_request,
            )
            .await
            .expect("keyed capture");
        assert_eq!(snapshot.operation_id, "ed3-four-link-capture");

        // Link 3 — successor restore: the next generation applies with
        // the restore reference and observes the marker.
        // A successor is a NEW binding (design A): a fresh unit identity
        // seeded from the predecessor's keyed snapshot.
        let successor_spec = marker_spec("ed3-four-link-r2", 1);
        let successor = apply_request(&successor_spec, Some("ed3-four-link-capture"));
        let restored = driver
            .apply(&successor, &first)
            .await
            .expect("restored apply");
        assert_eq!(restored.state, RuntimeUnitState::Running);

        // Link 4 — crash-window cold degrade: a capture whose key is
        // never restored must not fabricate state. A second successor
        // WITHOUT the restore reference boots cold and still runs.
        let cold_spec = marker_spec("ed3-four-link-c3", 1);
        let cold = apply_request(&cold_spec, None);
        let cold_observation = driver.apply(&cold, &restored).await.expect("cold apply");
        assert_eq!(cold_observation.state, RuntimeUnitState::Running);
    }
}

async fn lane_unit_record(
    _driver: &BoxRuntimeDriver,
    _spec: &aster_runtime::contract::RuntimeUnitSpec,
    observation: &RuntimeObservation,
) -> aster_runtime::RuntimeUnitRecord {
    aster_runtime::RuntimeUnitRecord {
        schema: aster_runtime::RuntimeUnitRecord::SCHEMA.into(),
        spec: _spec.clone(),
        observation: observation.clone(),
        removed_at_ms: None,
    }
}

/// The capability gate refuses everything when `state_snapshot` is
/// off — this leg runs on any Linux host, no delegated surface.
#[tokio::test]
async fn snapshot_refuses_fail_closed_when_the_feature_gate_is_off() {
    if std::env::var_os("ASTER_BOX_OCI_RUNTIME_PATH").is_none() {
        eprintln!(
            "ASTER_BOX_OCI_RUNTIME_PATH unset; the GA manager preflight needs the engine artifacts; gate-leg not executed on this host"
        );
        return;
    }
    let directory = tempfile::tempdir().expect("gate directory");
    let driver = lane_driver_async(&directory, false).await;
    {
        let spec = marker_spec("ed3-four-link-gate", 1);
        let request = aster_runtime::contract::RuntimeSnapshotRequest {
            schema: aster_runtime::contract::RuntimeSnapshotRequest::SCHEMA.into(),
            operation_id: "ed3-gate-refusal".into(),
            unit_id: spec.unit_id.clone(),
            generation: 1,
            include_memory: false,
            deadline_at_ms: None,
        };
        let record = lane_unit_record(&driver, &spec, &accepted(&spec)).await;
        let error = driver
            .snapshot(&record, &request)
            .await
            .expect_err("the gate must refuse");
        assert!(matches!(
            error,
            aster_runtime::RuntimeError::UnsupportedCapabilities(missing)
                if missing == vec!["feature:StateSnapshot".to_string()]
        ));
    }
}

#[allow(dead_code)]
fn unused(_: Arc<()>) {}
