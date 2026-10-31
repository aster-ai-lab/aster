#![cfg(test)]

use super::{BoxHostServiceResolver, BoxRuntimeDriver, BoxRuntimeDriverConfig};
use aster_box_core::{ExecRequest, ExecutionManager, ExecutionSessionManager, HostServiceSocket};
use aster_runtime::contract::{
    RuntimeFeature, RuntimeHostService, RuntimeUnitClass, RuntimeUnitState,
};
use aster_runtime::{
    FileRuntimeStateStore, ManagedRuntimeClient, RuntimeClient, RuntimeError, RuntimeResult,
};
use async_trait::async_trait;
use base64::Engine;
use futures::FutureExt;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

const GUEST_PROBE: &str = r#"
import base64, http.client, json, os, socket, sys, tempfile
from pathlib import Path
payload = json.load(sys.stdin)
assert os.getuid() == 10001 and os.getgid() == 10001
status = dict(line.split(':', 1) for line in Path('/proc/self/status').read_text().splitlines() if ':' in line)
assert all(int(status[name].strip(), 16) == 0 for name in ['CapInh', 'CapPrm', 'CapEff', 'CapBnd', 'CapAmb'])
assert status['NoNewPrivs'].strip() == '1'
def refused(family, target):
    with socket.socket(family, socket.SOCK_STREAM) as stream:
        stream.settimeout(2)
        try:
            stream.connect(target)
        except OSError:
            return True
        return False
assert refused(socket.AF_VSOCK, (2, payload['port'] + 1)), 'unmapped VSOCK port accepted'
assert refused(socket.AF_INET, ('127.0.0.1', payload['sentinelPort'])), 'host loopback exposed'
assert not any(row.split()[1] == '00000000' for row in Path('/proc/net/route').read_text().splitlines()[1:])
assert all(row.split()[-1] == 'lo' for row in Path('/proc/net/if_inet6').read_text().splitlines())
namespace = {'__name__': 'aster_gateway_ocr'}
exec(compile(payload['adapter'], '<shipped-gateway-ocr>', 'exec'), namespace)
with tempfile.TemporaryDirectory(prefix='aster-vsock-ocr-') as directory:
    source = Path(directory) / 'source.pdf'
    source.write_bytes(base64.b64decode(payload['sourcePdf']))
    output = Path(directory) / 'result.json'
    environment = {'ASTER_GATEWAY_ORIGIN': 'vsock://2:' + str(payload['port']),
                   'ASTER_GATEWAY_KEY': payload['key'],
                   'RETAIN_OCR_SOURCE_PDF': str(source), 'RETAIN_OCR_RAW_PAYLOAD_JSON': str(output)}
    namespace['run'](environment)
    result = json.loads(output.read_text())
    print('ASTER_VSOCK_OCR_RESULT=' + json.dumps({'uid': os.getuid(), 'gid': os.getgid(),
          'noCapabilities': True, 'noNewPrivileges': True, 'unmappedVsockDenied': True,
          'hostLoopbackDenied': True, 'noDefaultIpv4Route': True, 'noExternalIpv6Interface': True,
          'ocr': result}), flush=True)
"#;

pub(super) struct GatewayGrant {
    pub(super) path: PathBuf,
    pub(super) active: AtomicBool,
}

#[async_trait]
impl BoxHostServiceResolver for GatewayGrant {
    async fn resolve(
        &self,
        _spec: &aster_runtime::contract::RuntimeUnitSpec,
        service: &RuntimeHostService,
    ) -> RuntimeResult<HostServiceSocket> {
        if service.reference != "os.gateway" || !self.active.load(Ordering::SeqCst) {
            return Err(RuntimeError::InvalidRequest(
                "Host service grant unavailable".into(),
            ));
        }
        Ok(HostServiceSocket {
            guest_port: service.vsock_port,
            socket_path: self.path.clone(),
        })
    }
}

#[test]
#[ignore = "requires owned PaperLoom home, real HVF/KVM assets and an authenticated Gateway Unix socket"]
fn paperloom_gateway_vsock_ocr_has_no_ambient_egress() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .thread_stack_size(8 * 1024 * 1024)
                .enable_all()
                .build()
                .unwrap()
                .block_on(run());
        })
        .unwrap()
        .join()
        .unwrap();
}

async fn run() {
    use std::os::unix::fs::PermissionsExt;
    let home = PathBuf::from(std::env::var("ASTER_HOME").expect("owned test home"));
    assert_eq!(home.canonicalize().unwrap(), home);
    assert_eq!(home.metadata().unwrap().permissions().mode() & 0o777, 0o700);
    assert_eq!(
        std::fs::read_to_string(home.join(".paperloom-host-service-e2e")).unwrap(),
        "paperloom-host-service-e2e\n"
    );
    let image = std::env::var("ASTER_BOX_TEST_PAPERLOOM_IMAGE").unwrap();
    assert!(image.rsplit_once('@').unwrap().1.starts_with("sha256:"));
    let socket = PathBuf::from(std::env::var("ASTER_BOX_TEST_GATEWAY_SOCKET").unwrap());
    let key = std::env::var("ASTER_BOX_TEST_GATEWAY_KEY").unwrap();
    let port = 61000;
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../..")
        .canonicalize()
        .unwrap();
    let adapter = std::fs::read_to_string(
        repository.join("apps/use-registry/packages/paperloom/runtime/gateway_ocr.py"),
    )
    .unwrap();
    let pdf = std::fs::read(repository.join("fixtures/paperloom/two-pages.pdf")).unwrap();
    let digest = format!("{:x}", Sha256::digest(&pdf));
    let grant = Arc::new(GatewayGrant {
        path: socket.clone(),
        active: AtomicBool::new(true),
    });
    let driver = Arc::new(
        BoxRuntimeDriver::new(BoxRuntimeDriverConfig {
            home_dir: home.clone(),
            secret_root: home.join("runtime-secrets"),
            control_timeout: Duration::from_secs(240),
            ..Default::default()
        })
        .unwrap()
        .with_host_service_resolver(grant.clone())
        .unwrap(),
    );
    let manager = &driver.manager;
    assert!(manager.managed_records().await.unwrap().is_empty());
    let state_root = home.join(format!(
        "host-service-state-{}",
        uuid::Uuid::new_v4().simple()
    ));
    let client = ManagedRuntimeClient::new(
        Arc::new(FileRuntimeStateStore::new(&state_root)),
        driver.clone(),
    );
    assert!(client
        .capabilities()
        .await
        .unwrap()
        .supports_feature(RuntimeFeature::HostServices));
    let sentinel = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut spec = super::test_support::runtime_spec(
        &format!("paperloom-vsock-{}", uuid::Uuid::new_v4().simple()),
        1,
        RuntimeUnitClass::Service,
    );
    spec.artifact.uri = format!("oci://{image}");
    spec.artifact.digest = image.rsplit_once('@').unwrap().1.into();
    spec.process.command = vec!["/bin/sh".into(), "-c".into()];
    spec.process.args = vec!["while :; do sleep 1; done".into()];
    spec.process.working_directory = None;
    spec.restart = aster_runtime::contract::RestartPolicy::Never;
    spec.resources.cpu_millis = 2000;
    spec.resources.memory_bytes = 1024 * 1024 * 1024;
    spec.resources.pids = 256;
    spec.network.host_services = vec![RuntimeHostService {
        reference: "os.gateway".into(),
        vsock_port: port,
    }];
    let started = client
        .apply(&super::test_support::apply_request(&spec))
        .await;
    let result = std::panic::AssertUnwindSafe(async {
        assert_eq!(started.as_ref().unwrap().state, RuntimeUnitState::Running);
        let record = driver.find_generation(&spec).await.unwrap().unwrap();
        let (execution_id, generation, _) = super::metadata::local_identity(&record).unwrap();
        let payload = json!({"port":port,"key":key,"adapter":adapter,
            "sourcePdf":base64::engine::general_purpose::STANDARD.encode(&pdf),
            "sentinelPort":sentinel.local_addr().unwrap().port()});
        let output = manager
            .execute(
                &execution_id,
                generation,
                ExecRequest {
                    request_id: Some(format!("ocr-{}", uuid::Uuid::new_v4().simple())),
                    cmd: vec![
                        "/usr/local/bin/python3".into(),
                        "-c".into(),
                        GUEST_PROBE.into(),
                    ],
                    timeout_ns: 240_000_000_000,
                    env: Vec::new(),
                    working_dir: None,
                    rootfs: None,
                    stdin: Some(serde_json::to_vec(&payload).unwrap()),
                    stdin_streaming: false,
                    user: Some("10001:10001".into()),
                    streaming: false,
                },
            )
            .await
            .unwrap();
        assert_eq!(
            output.exit_code,
            0,
            "guest stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!output.truncated);
        let stdout = String::from_utf8(output.stdout).unwrap();
        let raw = stdout
            .lines()
            .find_map(|line| line.strip_prefix("ASTER_VSOCK_OCR_RESULT="))
            .expect("OCR receipt");
        let receipt: serde_json::Value = serde_json::from_str(raw).unwrap();
        let ocr = &receipt["ocr"];
        assert_eq!(ocr["source"]["provider"], "aster_gateway");
        assert_eq!(ocr["source"]["sha256"], digest);
        let pages = ocr["pages"].as_array().unwrap();
        assert_eq!(pages.len(), 2);
        for (index, page) in pages.iter().enumerate() {
            let blocks = page["blocks"].as_array().unwrap();
            assert!(!blocks.is_empty());
            assert!(blocks
                .iter()
                .all(|block| block["metadata"]["ocr_provider"] == "pp-ocr-v6"));
            assert!(blocks.iter().any(|block| block["text"]
                .as_str()
                .unwrap()
                .contains(&format!("PaperLoom regression - Page {}", index + 1))));
        }
        assert!(
            tokio::time::timeout(Duration::from_millis(100), sentinel.accept())
                .await
                .is_err()
        );
        let record = manager
            .managed_record(&execution_id)
            .await
            .unwrap()
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
            socket
        );
        let state = std::fs::read_to_string(home.join("boxes.json")).unwrap();
        assert!(
            !state.contains(&key),
            "gateway bearer must not be written into Box state"
        );
        assert_eq!(
            record.labels.get(super::metadata::SPEC_DIGEST_LABEL),
            Some(&spec.digest().unwrap())
        );
        assert!(record
            .managed_execution
            .as_ref()
            .unwrap()
            .request
            .labels
            .contains_key("aster.runtime.host-services-digest"));
        let mut pending = vec![state_root.clone()];
        while let Some(directory) = pending.pop() {
            for entry in std::fs::read_dir(directory).unwrap() {
                let entry = entry.unwrap();
                if entry.file_type().unwrap().is_dir() {
                    pending.push(entry.path());
                } else {
                    let bytes = std::fs::read(entry.path()).unwrap();
                    assert!(
                        !bytes.windows(key.len()).any(|part| part == key.as_bytes()),
                        "gateway bearer must not enter Runtime state or receipts"
                    );
                }
            }
        }
        if let Some(path) = std::env::var_os("ASTER_BOX_TEST_EVIDENCE_DIR") {
            std::fs::write(
                PathBuf::from(path).join("guest-vsock-ocr.json"),
                serde_json::to_vec_pretty(&receipt).unwrap(),
            )
            .unwrap();
        }
    })
    .catch_unwind()
    .await;
    grant.active.store(false, Ordering::SeqCst);
    let stopped = client
        .stop(&super::test_support::action("stop-revoked", &spec))
        .await;
    let removed = if stopped.is_ok() {
        client
            .remove(&super::test_support::action("remove-revoked", &spec))
            .await
            .map(|_| true)
    } else {
        Ok(false)
    };
    assert!(stopped.is_ok(), "owned guest stop failed: {stopped:?}");
    assert_eq!(removed.unwrap(), true);
    assert!(manager.managed_records().await.unwrap().is_empty());
    assert!(
        socket.exists(),
        "Box cleanup must not remove the Gateway-owned endpoint"
    );
    if let Err(panic) = result {
        std::panic::resume_unwind(panic);
    }
    let mut ungranted = spec.clone();
    ungranted.unit_id = format!("ungranted-vsock-{}", uuid::Uuid::new_v4().simple());
    ungranted.network.host_services.clear();
    let started = client
        .apply(&super::test_support::apply_request(&ungranted))
        .await;
    let result = std::panic::AssertUnwindSafe(async {
        assert_eq!(started.as_ref().unwrap().state, RuntimeUnitState::Running);
        let record = driver.find_generation(&ungranted).await.unwrap().unwrap();
        let (id, generation, _) = super::metadata::local_identity(&record).unwrap();
        let output = manager
            .execute(
                &id,
                generation,
                ExecRequest {
                    request_id: Some(format!("ungranted-{}", uuid::Uuid::new_v4().simple())),
                    cmd: vec![
                        "/usr/local/bin/python3".into(),
                        "-c".into(),
                        r#"
import socket
with socket.socket(socket.AF_VSOCK, socket.SOCK_STREAM) as connection:
    connection.settimeout(2)
    try:
        connection.connect((2, 61000))
    except OSError:
        print('ungranted-binding-denied')
    else:
        raise AssertionError('another Box inherited the service binding')
"#
                        .into(),
                    ],
                    timeout_ns: 10_000_000_000,
                    env: Vec::new(),
                    working_dir: None,
                    rootfs: None,
                    stdin: None,
                    stdin_streaming: false,
                    user: Some("10001:10001".into()),
                    streaming: false,
                },
            )
            .await
            .unwrap();
        assert_eq!(
            output.exit_code,
            0,
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8(output.stdout)
            .unwrap()
            .contains("ungranted-binding-denied"));
    })
    .catch_unwind()
    .await;
    let stopped = client
        .stop(&super::test_support::action("stop-ungranted", &ungranted))
        .await;
    let removed = if stopped.is_ok() {
        client
            .remove(&super::test_support::action("remove-ungranted", &ungranted))
            .await
            .map(|_| true)
    } else {
        Ok(false)
    };
    assert!(stopped.is_ok(), "ungranted guest stop failed: {stopped:?}");
    assert_eq!(removed.unwrap(), true);
    assert!(manager.managed_records().await.unwrap().is_empty());
    assert!(socket.exists());
    if let Err(panic) = result {
        std::panic::resume_unwind(panic);
    }
    if let Some(path) = std::env::var_os("ASTER_BOX_TEST_EVIDENCE_DIR") {
        std::fs::write(
            PathBuf::from(path).join("host-service-retirement.json"),
            serde_json::to_vec_pretty(&json!({"ungrantedBoxDenied":true,"recordsRemaining":0,
                "bothGuestsStoppedAndRemoved":true,"gatewayOwnedSocketPreserved":true,
                "runtimeClientLifecycle":true,"revokedGrantRetirement":true}))
            .unwrap(),
        )
        .unwrap();
    }
}
