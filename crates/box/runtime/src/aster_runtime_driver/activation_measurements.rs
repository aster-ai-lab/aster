//! ED4 D2: the activation/handoff measurement harness. Rides the same
//! GA Sandbox engine lane as the four-link conformance and emits raw
//! JSON measurements — cold-apply latency, keyed-capture time,
//! restore-apply latency (frozen→ready), and a sustained
//! activation-rate sample. The D1 lane guard is the publication
//! authority: these numbers validate the HARNESS wherever it runs and
//! publish as a baseline only from the dedicated runner. No
//! performance assertions live here — thresholds come only after lane
//! stability on the dedicated runner.

#![cfg(test)]
#![cfg(target_os = "linux")]

use std::sync::Arc;
use std::time::Instant;

use aster_runtime::contract::{
    RuntimeApplyRequest, RuntimeObservation, RuntimeSnapshotRequest, RuntimeUnitClass,
    RuntimeUnitState,
};

use crate::aster_runtime_driver::test_support::{accepted, runtime_spec};
use crate::{BoxRuntimeDriver, BoxRuntimeDriverConfig};
use aster_runtime::RuntimeDriver as _;

async fn lane_driver(directory: &tempfile::TempDir) -> BoxRuntimeDriver {
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
            state_snapshot: true,
            secret_root: home_dir.join("runtime-secrets"),
            home_dir,
            control_timeout: std::time::Duration::from_secs(120),
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
    .expect("measurement lane driver")
}

fn workload_spec(unit_id: &str) -> aster_runtime::contract::RuntimeUnitSpec {
    let mut spec = runtime_spec(unit_id, 1, RuntimeUnitClass::Service);
    spec.artifact.uri = "oci://docker.io/library/alpine@sha256:d9e853e87e55526f6b2917df91a2115c36dd7c696a35be12163d44e6e2a4b6bc".into();
    spec.artifact.digest =
        "sha256:d9e853e87e55526f6b2917df91a2115c36dd7c696a35be12163d44e6e2a4b6bc".into();
    // The glutton shape: allocate and touch a bounded memory block,
    // hold open a bounded set of fds, and persist a bounded disk
    // footprint — then stay resident so the capture quiesces a live,
    // pressured unit.
    spec.process.args = vec![
        "-c".into(),
        "mkdir -p /state /data; dd if=/dev/zero of=/data/blob bs=1M count=16 status=none; \\
         i=0; while [ $i -lt 64 ]; do eval \"exec $i>/dev/null\" 2>/dev/null || true; i=$((i+1)); done; \\
         printf 'glutton-ready' > /state/value; \\
         awk 'BEGIN{size=8*1024*1024; blk=\"\"; for(i=0;i<256;i++) blk=blk \"x\"; \\
           while(1){ for(off=0; off<size; off+=length(blk)) buf=substr(blk,1); }}' &
         while :; do sleep 60; done"
            .into(),
    ];
    spec
}

fn apply_request(
    spec: &aster_runtime::contract::RuntimeUnitSpec,
    restore: Option<&str>,
) -> RuntimeApplyRequest {
    RuntimeApplyRequest {
        schema: RuntimeApplyRequest::SCHEMA.into(),
        request_id: format!("d2-{}", uuid::Uuid::new_v4().simple()),
        deadline_at_ms: None,
        spec: spec.clone(),
        restore_from_snapshot_operation_id: restore.map(str::to_string),
    }
}

fn unit_record(
    spec: &aster_runtime::contract::RuntimeUnitSpec,
    observation: &RuntimeObservation,
) -> aster_runtime::RuntimeUnitRecord {
    aster_runtime::RuntimeUnitRecord {
        schema: aster_runtime::RuntimeUnitRecord::SCHEMA.into(),
        spec: spec.clone(),
        observation: observation.clone(),
        removed_at_ms: None,
    }
}

fn millis(elapsed: std::time::Duration) -> u64 {
    u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
}

/// The harness validation leg: runs the full measured sequence once
/// against the real engine and prints the raw JSON measurement line.
/// Mechanical invariants only (units ran, capture keyed, restore
/// observed Running); every timing is data, never an assertion.
#[tokio::test]
#[ignore = "ED4 D2 measurement harness: requires the engine lane surface (see the CI recipe); numbers validate the harness, baselines publish only from the dedicated runner"]
async fn activation_measurement_harness_emits_raw_json() {
    let directory = tempfile::tempdir().expect("measurement directory");
    let driver = lane_driver(&directory).await;

    // Cold activation (frozen→ready without a snapshot).
    let cold_spec = workload_spec("d2-cold");
    let cold_started = Instant::now();
    let cold = driver
        .apply(&apply_request(&cold_spec, None), &accepted(&cold_spec))
        .await
        .expect("cold apply");
    let cold_ms = millis(cold_started.elapsed());
    assert_eq!(cold.state, RuntimeUnitState::Running);

    // Keyed capture of the live, pressured unit.
    let capture_request = RuntimeSnapshotRequest {
        schema: RuntimeSnapshotRequest::SCHEMA.into(),
        operation_id: "d2-capture".into(),
        unit_id: cold_spec.unit_id.clone(),
        generation: 1,
        include_memory: false,
        deadline_at_ms: None,
    };
    let capture_started = Instant::now();
    let snapshot = driver
        .snapshot(&unit_record(&cold_spec, &cold), &capture_request)
        .await
        .expect("keyed capture");
    let capture_ms = millis(capture_started.elapsed());
    assert_eq!(snapshot.operation_id, "d2-capture");

    // Restore activation (frozen→ready from the snapshot).
    let warm_spec = workload_spec("d2-warm");
    let warm_started = Instant::now();
    let warm = driver
        .apply(&apply_request(&warm_spec, Some("d2-capture")), &cold)
        .await
        .expect("restore apply");
    let warm_ms = millis(warm_started.elapsed());
    assert_eq!(warm.state, RuntimeUnitState::Running);

    // A sustained sample: three more restore activations to bound a
    // rate estimate (small on purpose — the harness validates the
    // measurement path; the dedicated runner sizes the real sample).
    let mut restore_samples_ms = Vec::new();
    for index in 0..3 {
        let spec = workload_spec(&format!("d2-sustained-{index}"));
        let started = Instant::now();
        let observation = driver
            .apply(&apply_request(&spec, Some("d2-capture")), &warm)
            .await
            .expect("sustained restore apply");
        restore_samples_ms.push(millis(started.elapsed()));
        assert_eq!(observation.state, RuntimeUnitState::Running);
    }

    let restore_mean_ms = restore_samples_ms.iter().sum::<u64>()
        / u64::try_from(restore_samples_ms.len()).unwrap_or(1);
    println!(
        "{{\"harness\":\"ed4-d2\",\"isBaseline\":false,\"coldApplyMs\":{cold_ms},\"keyedCaptureMs\":{capture_ms},\"restoreApplyMs\":{warm_ms},\"sustainedRestoreSamplesMs\":{restore_samples_ms:?},\"sustainedRestoreMeanMs\":{restore_mean_ms}}}"
    );
}
