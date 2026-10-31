#![cfg(test)]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use aster_box_core::{
    ExecutionGeneration, ExecutionId, ExecutionManagerError, ExecutionPortConnector,
};
use aster_runtime::contract::RuntimeInspection;
use aster_runtime::contract::{
    ArtifactRef, HealthProbe, IsolationLevel, NetworkMode, ResourceLimits, RestartPolicy,
    RuntimeActionRequest, RuntimeApplyRequest, RuntimeHealthCheck, RuntimeNetworkSpec, RuntimePort,
    RuntimeProcessSpec, RuntimeServiceEndpoint, RuntimeServiceLifecycle, RuntimeUnitClass,
    RuntimeUnitSpec, RuntimeUnitState, TransportProtocol,
};
use aster_runtime::{FileRuntimeStateStore, ManagedRuntimeClient, RuntimeClient};
use futures::FutureExt;
use serde_json::json;
use sha2::{Digest, Sha256};

use super::{BoxRuntimeDriver, BoxRuntimeDriverConfig, OCI_IMAGE_MANIFEST};

#[test]
#[ignore = "requires an owned PaperLoom home, digest-pinned image and real HVF/KVM assets"]
fn paperloom_service_uses_private_runtime_endpoints_and_retires_them() {
    run_service_test(false);
}

#[test]
#[ignore = "requires an owned PaperLoom home, digest-pinned image and real HVF/KVM assets"]
fn paperloom_service_activates_through_use_sdk_and_retires() {
    run_service_test(true);
}

fn run_service_test(use_sdk: bool) {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(move || {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .thread_stack_size(8 * 1024 * 1024)
                .enable_all()
                .build()
                .unwrap()
                .block_on(run_private_service(use_sdk));
        })
        .unwrap()
        .join()
        .unwrap();
}

async fn run_private_service(use_sdk: bool) {
    use std::os::unix::fs::PermissionsExt;

    let home = PathBuf::from(std::env::var("ASTER_HOME").expect("explicit private test home"));
    assert_eq!(home.canonicalize().unwrap(), home);
    assert_eq!(home.metadata().unwrap().permissions().mode() & 0o077, 0);
    assert_eq!(
        std::fs::read_to_string(home.join(".paperloom-runtime-e2e")).unwrap(),
        "paperloom-runtime-service-e2e\n"
    );
    let state_root = home.join(format!("private-service-state-{}", uuid::Uuid::new_v4()));
    assert!(!state_root.exists(), "use a fresh Runtime state directory");
    let image = std::env::var("ASTER_BOX_TEST_PAPERLOOM_IMAGE")
        .expect("explicit cached digest-pinned PaperLoom image");
    let (_, digest) = image.rsplit_once('@').expect("digest-pinned OCI reference");
    let artifact = ArtifactRef {
        uri: format!("oci://{image}"),
        digest: digest.into(),
        media_type: OCI_IMAGE_MANIFEST.into(),
    };
    artifact.validate().unwrap();
    let pdf = std::fs::read(std::env::var("ASTER_BOX_TEST_PAPERLOOM_PDF").unwrap()).unwrap();
    assert!(pdf.starts_with(b"%PDF-") && pdf.len() <= 20 * 1024 * 1024);
    let driver = Arc::new(
        BoxRuntimeDriver::new(BoxRuntimeDriverConfig {
            home_dir: home.clone(),
            secret_root: home.join("runtime-secrets"),
            control_timeout: Duration::from_secs(240),
            ..Default::default()
        })
        .unwrap(),
    );
    assert!(driver.manager.managed_records().await.unwrap().is_empty());
    let client = Arc::new(ManagedRuntimeClient::new(
        Arc::new(FileRuntimeStateStore::new(state_root)),
        driver.clone(),
    ));
    let spec = RuntimeUnitSpec {
        schema: RuntimeUnitSpec::SCHEMA.into(),
        unit_id: format!("paperloom-e2e-{}", uuid::Uuid::new_v4().simple()),
        generation: 1,
        class: RuntimeUnitClass::Service,
        artifact,
        process: RuntimeProcessSpec {
            command: Vec::new(),
            args: Vec::new(),
            working_directory: None,
            environment: BTreeMap::from([
                ("RUST_API_BIND_HOST".into(), "127.0.0.1".into()),
                ("RUST_API_KEYS".into(), "paperloom-e2e-only".into()),
            ]),
        },
        mounts: Vec::new(),
        secrets: Vec::new(),
        network: RuntimeNetworkSpec {
            host_services: Vec::new(),
            mode: NetworkMode::Service,
            ports: vec![RuntimePort {
                name: "api".into(),
                container_port: 41000,
                protocol: TransportProtocol::Tcp,
            }],
            egress_allowlist: None,
        },
        resources: ResourceLimits {
            cpu_millis: 2000,
            memory_bytes: 1024 * 1024 * 1024,
            pids: 256,
            ephemeral_storage_bytes: None,
            execution_timeout_ms: None,
        },
        isolation: IsolationLevel::Sandbox,
        health: Some(RuntimeHealthCheck {
            probe: HealthProbe::Http {
                port: "api".into(),
                path: "/ready".into(),
                expected_statuses: vec![200],
            },
            interval_ms: 3000,
            timeout_ms: 2000,
            start_period_ms: 3000,
            success_threshold: 1,
            failure_threshold: 3,
        }),
        service_lifecycle: Some(RuntimeServiceLifecycle {
            liveness: RuntimeHealthCheck {
                probe: HealthProbe::Http {
                    port: "api".into(),
                    path: "/health".into(),
                    expected_statuses: vec![200],
                },
                interval_ms: 3000,
                timeout_ms: 2000,
                start_period_ms: 3000,
                success_threshold: 1,
                failure_threshold: 3,
            },
            shutdown_grace_seconds: 2,
        }),
        restart: RestartPolicy::Never,
        outputs: Vec::new(),
        semantics_profile_digest: None,
        identity_attachment_digest: None,
    };
    let sdk_plan = use_sdk.then(|| use_service_plan(&spec));
    let spec = sdk_plan.as_ref().map_or(spec, |plan| plan.spec().clone());
    let action = |label: &str| RuntimeActionRequest {
        schema: RuntimeActionRequest::SCHEMA.into(),
        request_id: format!("{}-{label}", spec.unit_id),
        unit_id: spec.unit_id.clone(),
        generation: spec.generation,
        deadline_at_ms: None,
    };
    let apply = RuntimeApplyRequest {
        schema: RuntimeApplyRequest::SCHEMA.into(),
        request_id: format!("{}-apply", spec.unit_id),
        deadline_at_ms: None,
        spec: spec.clone(),
        restore_from_snapshot_operation_id: None,
    };
    apply.validate().unwrap();
    let started = if let Some(plan) = &sdk_plan {
        use aster_use::core::{PlanEnforcementProfile, PlannedProviderEvidence};
        use aster_use::plugin_runtime::{runtime_capabilities_digest, PluginRuntimeClient};
        let caps = client.capabilities().await.unwrap();
        let provider = PlannedProviderEvidence {
            surface: plan.surface(),
            provider_id: caps.provider_id.to_string(),
            provider_build_id: caps.provider_build.clone(),
            capability_digest: runtime_capabilities_digest(&caps).unwrap(),
            semantics_profile_digest: plan.spec().semantics_profile_digest.clone().unwrap(),
            enforcement: PlanEnforcementProfile::Sandbox,
        };
        PluginRuntimeClient::new(client.clone())
            .apply_service(plan, &provider, apply.request_id.clone(), None)
            .await
            .map(|activation| activation.observation().clone())
            .map_err(|error| error.to_string())
    } else {
        client
            .apply(&apply)
            .await
            .map_err(|error| error.to_string())
    };
    let probe = std::panic::AssertUnwindSafe(async {
        let observation = started.as_ref().map_err(|error| error.to_string())?;
        if observation.state != RuntimeUnitState::Running {
            if let Some(record) = driver.find_generation(&spec).await.unwrap() {
                for path in [record.console_log.clone(), aster_box_core::log::stderr_console_path(&record.console_log)] {
                    if let Ok(bytes) = std::fs::read(path) {
                        eprintln!("{}", String::from_utf8_lossy(&bytes[bytes.len().saturating_sub(16 * 1024)..]));
                    }
                }
            }
        }
        assert_eq!(observation.state, RuntimeUnitState::Running, "{observation:?}");
        let endpoint = RuntimeServiceEndpoint::from_observation(observation, "api").unwrap();
        assert!(endpoint.address.is_loopback());
        let listening = std::process::Command::new("lsof")
            .args(["-nP", "-a", "-p", &std::process::id().to_string(),
                &format!("-iTCP:{}", endpoint.port), "-sTCP:LISTEN", "-Fn"])
            .output().expect("actual host listener inspection requires lsof");
        assert!(listening.status.success());
        let listeners: Vec<_> = String::from_utf8(listening.stdout).unwrap().lines()
            .filter_map(|line| line.strip_prefix('n').map(str::to_owned)).collect();
        assert_eq!(listeners, vec![endpoint.socket_addr().to_string()]);
        let url = format!("http://{}", endpoint.socket_addr());
        let http = reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap();
        let ready = tokio::time::timeout(Duration::from_secs(20), async {
            loop {
                let response = http.get(format!("{url}/ready")).send().await.unwrap();
                if response.status().is_success() {
                    let body: serde_json::Value = response.json().await.unwrap();
                    assert_eq!(body["data"]["status"], "ready");
                    break body;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await
        .unwrap();
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                if let RuntimeInspection::Found { observation, .. } = client.inspect(&spec.unit_id).await.unwrap() {
                    assert_eq!(RuntimeServiceEndpoint::from_observation(&observation, "api").unwrap(), endpoint);
                    if observation.converges(&spec) { break; }
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }).await.unwrap();
        let replay = client.apply(&apply).await.unwrap();
        assert_eq!(RuntimeServiceEndpoint::from_observation(&replay, "api").unwrap(), endpoint);
        assert_eq!(driver.manager.managed_records().await.unwrap().len(), 1);
        assert_eq!(
            http.get(format!("{url}/api/v1/documents"))
                .send()
                .await
                .unwrap()
                .status(),
            reqwest::StatusCode::UNAUTHORIZED
        );
        let documents = http
            .get(format!("{url}/api/v1/documents"))
            .header("X-API-Key", "paperloom-e2e-only")
            .send()
            .await
            .unwrap();
        assert!(documents.status().is_success());
        let boundary = format!("paperloom-{}", uuid::Uuid::new_v4().simple());
        let mut multipart = format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"runtime-proof.pdf\"\r\nContent-Type: application/pdf\r\n\r\n").into_bytes();
        multipart.extend_from_slice(&pdf);
        multipart.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
        let upload = http.post(format!("{url}/api/v1/uploads"))
            .header("X-API-Key", "paperloom-e2e-only")
            .header("Content-Type", format!("multipart/form-data; boundary={boundary}"))
            .body(multipart).send().await.unwrap();
        assert!(upload.status().is_success());
        let uploaded: serde_json::Value = upload.json().await.unwrap();
        let document = uploaded["data"]["document_id"].as_str().unwrap();
        assert_eq!(document, format!("{:x}", Sha256::digest(&pdf)));
        assert_eq!(uploaded["data"]["page_count"], 2);
        let source = http.get(format!("{url}/api/v1/documents/{document}/source.pdf"))
            .header("X-API-Key", "paperloom-e2e-only").send().await.unwrap();
        assert!(source.status().is_success());
        assert_eq!(source.bytes().await.unwrap().as_ref(), pdf);
        let record = driver.find_generation(&spec).await.unwrap().unwrap();
        assert!(record.port_map.is_empty());
        assert_eq!(record.isolation, aster_box_core::ExecutionIsolation::Microvm);
        let stale = driver.manager.connect_port(
            &ExecutionId::new(record.id.clone()).unwrap(),
            ExecutionGeneration::new(record.managed_execution.as_ref().unwrap().generation.get() + 1).unwrap(),
            std::num::NonZeroU16::new(41000).unwrap(), Duration::from_secs(1),
        ).await;
        assert!(matches!(stale, Err(ExecutionManagerError::Conflict { .. })));
        let pid = record.pid.unwrap();
        let start = record.pid_start_time;
        let stopped = client.stop(&action("stop")).await.unwrap();
        let RuntimeInspection::Found { observation, .. } = stopped else {
            panic!("stopped unit lost its terminal observation");
        };
        assert_eq!(observation.state, RuntimeUnitState::Stopped);
        assert!(!crate::process::is_process_alive_with_identity(pid, start));
        tokio::time::timeout(Duration::from_secs(2), async {
            while tokio::net::TcpStream::connect(endpoint.socket_addr()).await.is_ok() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }).await.unwrap();
        Ok::<_, String>(json!({
            "unitId":spec.unit_id, "endpoint":endpoint,
            "ready":ready, "originalShimPid":pid,
            "originalShimStart":start, "originalShimGone":true,
            "endpointClosedAfterStop":true, "documentId":document,
            "pageCount":2, "originalPdfRoundtrip":true,
            "actualHostListenAddresses":listeners, "runtimeHealthConverged":true,
            "activatedThroughUseSdk":use_sdk,
            "applyReplayKeptSameEndpointAndOneExecution":true,
            "staleExecutionGenerationRefused":true
        }))
    })
    .catch_unwind()
    .await;
    let stopped = client.stop(&action("stop")).await;
    let removed = client.remove(&action("remove")).await;
    assert!(stopped.is_ok(), "stop failed: {stopped:?}");
    assert!(removed.is_ok(), "cleanup failed: {removed:?}");
    assert!(driver.manager.managed_records().await.unwrap().is_empty());
    let proof = probe
        .expect("private service probe panicked")
        .expect("real private Runtime Service proof");
    println!("{proof}");
}

fn use_service_plan(spec: &RuntimeUnitSpec) -> aster_use::plugin_runtime::RuntimeSurfacePlan {
    use aster_use::core::{
        PackageSurfaceKind, PlanScope, PlanScopeKind, PluginSurfaceRef, ToolReleaseDescriptor,
    };
    use aster_use::extension::ToolServiceSurface;
    use aster_use::plugin_runtime::{
        plan_tool_service_release, RuntimeResourcePolicy, RuntimeSurfaceContext,
        RuntimeWorkloadPolicy,
    };

    let mut descriptor: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../../../../use/fixtures/runtime-v4/tool-service.json"
    ))
    .unwrap();
    descriptor["artifact"]["digest"] = json!(spec.artifact.digest);
    descriptor["artifact"]["mediaType"] = json!(spec.artifact.media_type);
    descriptor["workload"]["port"] = json!(41000);
    descriptor["workload"]["portName"] = json!("api");
    descriptor["workload"]["basePath"] = json!("/api/v1");
    descriptor["workload"]["health"]["path"] = json!("/ready");
    descriptor["workload"]["health"]["intervalMs"] = json!(3000);
    descriptor["workload"]["startupTimeoutMs"] = json!(120000);
    descriptor["workload"]["shutdownGraceMs"] = json!(2000);
    let descriptor =
        ToolReleaseDescriptor::from_json(&serde_json::to_vec(&descriptor).unwrap()).unwrap();
    plan_tool_service_release(
        RuntimeSurfaceContext::new(
            "aster/paperloom",
            format!("sha256:{}", "a".repeat(64)),
            PlanScope {
                kind: PlanScopeKind::Workspace,
                id: spec.unit_id.clone(),
            },
            format!("sha256:{}", "b".repeat(64)),
            PluginSurfaceRef {
                kind: PackageSurfaceKind::Tool,
                id: "processor".into(),
            },
            1,
        )
        .unwrap(),
        &ToolServiceSurface {
            release: "releases/fixture.json".into(),
            base_path: "/api/v1".into(),
            contract: None,
        },
        &descriptor,
        spec.artifact.clone(),
        RuntimeWorkloadPolicy {
            isolation: spec.isolation,
            resources: RuntimeResourcePolicy {
                cpu_millis: spec.resources.cpu_millis,
                memory_bytes: spec.resources.memory_bytes,
                pids: spec.resources.pids,
                ephemeral_storage_bytes: spec.resources.ephemeral_storage_bytes,
            },
            mounts: Vec::new(),
            secrets: Vec::new(),
            host_services: Vec::new(),
            service_lifecycle: spec.service_lifecycle.clone(),
            non_secret_environment: spec.process.environment.clone(),
            working_directory: None,
        },
    )
    .unwrap()
}
