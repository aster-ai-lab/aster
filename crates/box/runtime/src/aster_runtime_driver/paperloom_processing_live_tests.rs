#![cfg(test)]

use super::{
    BoxRegistryCredential, BoxRuntimeDriver, BoxRuntimeDriverConfig, BoxSecretMaterial,
    BoxSecretMaterializationError, BoxSecretMaterializer, OCI_IMAGE_MANIFEST,
};
use aster_runtime::contract::{
    ArtifactRef, HealthProbe, IsolationLevel, NetworkMode, ResourceLimits, RestartPolicy,
    RuntimeActionRequest, RuntimeApplyRequest, RuntimeHealthCheck, RuntimeHostService,
    RuntimeNetworkSpec, RuntimePort, RuntimeProcessSpec, RuntimeServiceEndpoint, RuntimeUnitClass,
    RuntimeUnitSpec, SecretReference, SecretTarget, TransportProtocol,
};
use aster_runtime::{FileRuntimeStateStore, ManagedRuntimeClient, RuntimeClient};
use async_trait::async_trait;
use futures::FutureExt;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

struct OcrSecret {
    bytes: zeroize::Zeroizing<Vec<u8>>,
    resolutions: AtomicUsize,
}

#[async_trait]
impl BoxSecretMaterializer for OcrSecret {
    async fn materialize(
        &self,
        reference: &SecretReference,
    ) -> Result<BoxSecretMaterial, BoxSecretMaterializationError> {
        if reference.reference != "secret://paperloom-e2e/ocr/1" || reference.name != "gateway-ocr"
        {
            return Err(BoxSecretMaterializationError::Rejected(
                "unbound test reference".into(),
            ));
        }
        self.resolutions.fetch_add(1, Ordering::SeqCst);
        BoxSecretMaterial::new(self.bytes.to_vec())
    }
    async fn materialize_registry_credential(
        &self,
        _: &SecretReference,
        _: &str,
    ) -> Result<BoxRegistryCredential, BoxSecretMaterializationError> {
        Err(BoxSecretMaterializationError::Rejected(
            "no registry grant".into(),
        ))
    }
}

#[test]
#[ignore = "requires a runner-owned memory Secret root, Gateway Unix socket and real Paperloom MicroVM image"]
fn paperloom_service_completes_real_gateway_ocr_job_and_retires_secrets() {
    run_test(false);
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "requires runner-owned RAM Secret root and actual macOS MicroVM admission"]
fn paperloom_unsupported_secret_refuses_without_execution_or_ram_material() {
    run_test(true);
}

fn run_test(expect_readonly_refusal: bool) {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(move || {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .thread_stack_size(8 * 1024 * 1024)
                .enable_all()
                .build()
                .unwrap()
                .block_on(run(expect_readonly_refusal));
        })
        .unwrap()
        .join()
        .unwrap();
}

async fn run(expect_readonly_refusal: bool) {
    use std::os::unix::fs::PermissionsExt;
    let home = PathBuf::from(std::env::var("ASTER_HOME").unwrap());
    assert_eq!(home.canonicalize().unwrap(), home);
    assert_eq!(
        std::fs::read_to_string(home.join(".paperloom-host-service-e2e")).unwrap(),
        "paperloom-host-service-e2e\n"
    );
    let secret_root = PathBuf::from(std::env::var("ASTER_BOX_TEST_SECRET_ROOT").unwrap());
    assert_eq!(secret_root.canonicalize().unwrap(), secret_root);
    assert_eq!(
        secret_root.metadata().unwrap().permissions().mode() & 0o777,
        0o700
    );
    let key = std::env::var("ASTER_BOX_TEST_GATEWAY_KEY").unwrap();
    let socket = PathBuf::from(std::env::var("ASTER_BOX_TEST_GATEWAY_SOCKET").unwrap());
    let image = std::env::var("ASTER_BOX_TEST_PAPERLOOM_IMAGE").unwrap();
    let (_, digest) = image.rsplit_once('@').unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../..")
        .canonicalize()
        .unwrap();
    let pdf = std::fs::read(root.join("fixtures/paperloom/two-pages.pdf")).unwrap();
    let source_digest = format!("{:x}", Sha256::digest(&pdf));
    let adapter = std::fs::read_to_string(
        root.join("apps/use-registry/packages/paperloom/runtime/gateway_ocr.py"),
    )
    .unwrap();
    let command = format!("python3 -c '{}'", adapter.replace('\'', "'\"'\"'"));
    let materializer = Arc::new(OcrSecret {
        bytes: zeroize::Zeroizing::new(key.as_bytes().to_vec()),
        resolutions: AtomicUsize::new(0),
    });
    let driver = Arc::new(
        BoxRuntimeDriver::new(BoxRuntimeDriverConfig {
            home_dir: home.clone(),
            secret_root: secret_root.clone(),
            control_timeout: Duration::from_secs(240),
            ..Default::default()
        })
        .unwrap()
        .with_host_service_resolver(Arc::new(super::host_service_live_tests::GatewayGrant {
            path: socket,
            active: AtomicBool::new(true),
        }))
        .unwrap()
        .with_secret_materializer(materializer.clone()),
    );
    assert!(driver.manager.managed_records().await.unwrap().is_empty());
    let state_root = home.join(format!(
        "processing-state-{}",
        uuid::Uuid::new_v4().simple()
    ));
    let client = ManagedRuntimeClient::new(
        Arc::new(FileRuntimeStateStore::new(&state_root)),
        driver.clone(),
    );
    let spec = RuntimeUnitSpec {
        schema: RuntimeUnitSpec::SCHEMA.into(),
        unit_id: format!("paperloom-ocr-job-{}", uuid::Uuid::new_v4().simple()),
        generation: 1,
        class: RuntimeUnitClass::Service,
        artifact: ArtifactRef {
            uri: format!("oci://{image}"),
            digest: digest.into(),
            media_type: OCI_IMAGE_MANIFEST.into(),
        },
        process: RuntimeProcessSpec {
            command: Vec::new(),
            args: Vec::new(),
            working_directory: None,
            environment: BTreeMap::from([
                ("RUST_API_BIND_HOST".into(), "127.0.0.1".into()),
                ("RUST_API_KEYS".into(), "paperloom-e2e-only".into()),
                ("ASTER_GATEWAY_ORIGIN".into(), "vsock://2:61000".into()),
            ]),
        },
        mounts: Vec::new(),
        secrets: vec![SecretReference {
            name: "gateway-ocr".into(),
            reference: "secret://paperloom-e2e/ocr/1".into(),
            target: SecretTarget::Environment {
                variable: "ASTER_GATEWAY_KEY".into(),
            },
        }],
        network: RuntimeNetworkSpec {
            mode: NetworkMode::Service,
            egress_allowlist: None,
            host_services: vec![RuntimeHostService {
                reference: "os.gateway".into(),
                vsock_port: 61000,
            }],
            ports: vec![RuntimePort {
                name: "api".into(),
                container_port: 41000,
                protocol: TransportProtocol::Tcp,
            }],
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
            start_period_ms: 0,
            success_threshold: 1,
            failure_threshold: 20,
        }),
        service_lifecycle: None,
        restart: RestartPolicy::Never,
        outputs: Vec::new(),
        semantics_profile_digest: None,
        identity_attachment_digest: None,
    };
    spec.validate()
        .expect("the live fixture must satisfy Runtime admission");
    let action = |name: &str| RuntimeActionRequest {
        schema: RuntimeActionRequest::SCHEMA.into(),
        request_id: format!("{}-{name}", spec.unit_id),
        unit_id: spec.unit_id.clone(),
        generation: spec.generation,
        deadline_at_ms: None,
    };
    let before_boxes = box_paths(&home);
    let started = client
        .apply(&RuntimeApplyRequest {
            schema: RuntimeApplyRequest::SCHEMA.into(),
            request_id: format!("{}-apply", spec.unit_id),
            deadline_at_ms: None,
            spec: spec.clone(),
            restore_from_snapshot_operation_id: None,
        })
        .await;
    if expect_readonly_refusal {
        let error = started.expect_err("unsupported Secret mount must refuse before boot");
        assert!(
            error
                .to_string()
                .contains("host-enforced virtio-fs write denial"),
            "{error}"
        );
        assert!(driver.manager.managed_records().await.unwrap().is_empty());
        assert_eq!(materializer.resolutions.load(Ordering::SeqCst), 0);
        assert!(!super::secret::secret_directory(&secret_root, &spec)
            .unwrap()
            .exists());
        assert_eq!(box_paths(&home), before_boxes);
        let ram_entries: Vec<_> = std::fs::read_dir(&secret_root)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        println!(
            "{}",
            json!({"unsupportedSecretRefusedBeforeExecution":true,"remainingExecutions":0,"secretResolutionCalls":0,"ownedSecretDirectoryPresent":false,"ramMetadataEntries":ram_entries,"processingVerified":false})
        );
        return;
    }
    let probe = std::panic::AssertUnwindSafe(async {
        let observation = started.as_ref().unwrap();
        assert!(observation.converges(&spec));
        let endpoint = RuntimeServiceEndpoint::from_observation(observation,"api").unwrap();
        let base = format!("http://{}/api/v1",endpoint.socket_addr());
        let http = reqwest::Client::builder().no_proxy().timeout(Duration::from_secs(10)).build().unwrap();
        let boundary = format!("paperloom-{}",uuid::Uuid::new_v4().simple());
        let mut body = format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"source.pdf\"\r\nContent-Type: application/pdf\r\n\r\n").into_bytes();
        body.extend(&pdf); body.extend(format!("\r\n--{boundary}--\r\n").as_bytes());
        let uploaded: Value = http.post(format!("{base}/uploads")).header("X-API-Key","paperloom-e2e-only")
            .header("Content-Type",format!("multipart/form-data; boundary={boundary}")).body(body).send().await.unwrap().error_for_status().unwrap().json().await.unwrap();
        assert_eq!(uploaded["data"]["document_id"], source_digest);
        let submitted: Value = http.post(format!("{base}/documents/{source_digest}/ocr")).header("X-API-Key","paperloom-e2e-only")
            .json(&json!({"workflow":"ocr","source":{},"ocr":{"provider":"local","options":{"command":command,"raw_provider":"generic_flat_ocr"}}}))
            .send().await.unwrap().error_for_status().unwrap().json().await.unwrap();
        let job = submitted["data"]["job_id"].as_str().expect("actual submitted job id");
        let completed = tokio::time::timeout(Duration::from_secs(240), async {
            loop {
                let detail: Value = http.get(format!("{base}/jobs/{job}")).header("X-API-Key","paperloom-e2e-only")
                    .send().await.unwrap().error_for_status().unwrap().json().await.unwrap();
                match detail["data"]["status"].as_str() {
                    Some("succeeded") => break detail,
                    Some("failed"|"canceled") => panic!("actual OCR job did not complete: {detail}"),
                    _ => tokio::time::sleep(Duration::from_millis(250)).await,
                }
            }
        }).await.expect("actual OCR job deadline");
        assert_eq!(completed["data"]["stages"]["ocr"]["state"],"completed");
        let manifest: Value = http.get(format!("{base}/jobs/{job}/artifacts-manifest")).header("X-API-Key","paperloom-e2e-only")
            .send().await.unwrap().error_for_status().unwrap().json().await.unwrap();
        assert!(!manifest.to_string().contains(&key));
        if let Some(dir) = std::env::var_os("ASTER_BOX_TEST_EVIDENCE_DIR") {
            std::fs::write(Path::new(&dir).join("paperloom-actual-ocr-job.json"),serde_json::to_vec_pretty(&json!({"documentId":source_digest,"job":completed,"artifacts":manifest})).unwrap()).unwrap();
        }
        let mut paths = vec![state_root.clone()];
        while let Some(path) = paths.pop() {
            for entry in std::fs::read_dir(path).unwrap() {
                let path=entry.unwrap().path();
                if path.is_dir() { paths.push(path); } else {
                    assert!(!std::fs::read(&path).unwrap().windows(key.len()).any(|bytes|bytes==key.as_bytes()),"credential persisted in Runtime state");
                }
            }
        }
        json!({"realPaperloomJobSucceeded":true,"sourceDigest":source_digest,"jobId":job,"secretAbsentFromRuntimeState":true})
    }).catch_unwind().await;
    let stopped = client.stop(&action("stop")).await;
    let removed = client.remove(&action("remove")).await;
    if probe.is_ok() {
        assert!(stopped.is_ok(), "exact stop failed: {stopped:?}");
        assert!(removed.is_ok(), "exact remove failed: {removed:?}");
    }
    assert!(driver.manager.managed_records().await.unwrap().is_empty());
    assert!(
        !super::secret::secret_directory(&secret_root, &spec)
            .unwrap()
            .exists(),
        "RAM Secret material remains after retirement"
    );
    let result = probe.expect("actual OCR job failed; exact Runtime cleanup was attempted");
    println!("{result}");
}

fn box_paths(home: &Path) -> BTreeSet<PathBuf> {
    match std::fs::read_dir(home.join("boxes")) {
        Ok(entries) => entries.map(|entry| entry.unwrap().path()).collect(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => BTreeSet::new(),
        Err(error) => panic!("owned Box directory could not be inspected: {error}"),
    }
}
