#![cfg(test)]

use super::*;
use crate::config::{InferenceCredentialConfig, InferenceGrantConfig, OperatingMode};
use argon2::password_hash::{PasswordHasher, SaltString};
use argon2::Argon2;

#[cfg(unix)]
#[path = "paperloom_api_job_tests.rs"]
mod api_job;

#[tokio::test]
#[ignore = "requires ASTER_PAPERLOOM_TEST_PYTHON with PyMuPDF and the installed platform OCR runtime"]
async fn paperloom_pdf_pages_use_authenticated_gateway_ocr() {
    let python =
        std::env::var("ASTER_PAPERLOOM_TEST_PYTHON").expect("explicit isolated Python environment");
    ensure_ocr_runtime().await;
    let (chat, _) = spawn_capturing_backend().await;
    let (embeddings, _) = spawn_capturing_backend().await;
    let (config, key, prefix) = workload_ocr_config(chat, embeddings);
    let (address, shutdown, server) = start_test_entrypoint(gateway_state(&config)).await;
    let client = reqwest::Client::new();
    let png = draw_aster_png();
    for credential in [None, Some(format!("{prefix}{}", Uuid::new_v4().simple()))] {
        let mut request = client
            .post(format!("http://{address}/v1/ocr"))
            .header(reqwest::header::CONTENT_TYPE, "image/png")
            .body(png.clone());
        if let Some(credential) = credential {
            request = request.bearer_auth(credential);
        }
        assert_eq!(
            request.send().await.unwrap().status(),
            reqwest::StatusCode::UNAUTHORIZED
        );
    }
    let denied = client
        .post(format!("http://{address}/v1/chat/completions"))
        .bearer_auth(&key)
        .json(&serde_json::json!({
            "model": "qwen",
            "messages": [{ "role": "user", "content": "not an OCR request" }]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(denied.status(), reqwest::StatusCode::FORBIDDEN);
    let root = tempfile::tempdir().unwrap();
    let output = root.path().join("gateway-ocr.json");
    let result_path = output.clone();
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let result = tokio::task::spawn_blocking(move || {
        std::process::Command::new(python)
            .arg(repository.join("apps/use-registry/packages/paperloom/runtime/gateway_ocr.py"))
            .env_clear()
            .env("ASTER_GATEWAY_ORIGIN", format!("http://{address}"))
            .env("ASTER_GATEWAY_KEY", key)
            .env(
                "RETAIN_OCR_SOURCE_PDF",
                repository.join("fixtures/paperloom/two-pages.pdf"),
            )
            .env("RETAIN_OCR_RAW_PAYLOAD_JSON", output)
            .output()
            .expect("adapter process")
    })
    .await
    .unwrap();
    assert!(
        result.status.success(),
        "adapter failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let raw = std::fs::read_to_string(result_path).unwrap();
    save_evidence("paperloom-gateway-ocr.json", &raw);
    let payload: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(payload["source"]["provider"], "aster_gateway");
    let pages = payload["pages"].as_array().unwrap();
    assert_eq!(pages.len(), 2);
    for (index, page) in pages.iter().enumerate() {
        assert_eq!(page["page"], index + 1);
        let blocks = page["blocks"].as_array().unwrap();
        assert!(!blocks.is_empty());
        assert!(blocks
            .iter()
            .any(|block| !block["text"].as_str().unwrap().trim().is_empty()));
        assert!(blocks
            .iter()
            .all(|block| block["metadata"]["ocr_provider"] == "pp-ocr-v6"));
        assert!(blocks.iter().any(|block| block["text"]
            .as_str()
            .unwrap()
            .contains(&format!("PaperLoom regression - Page {}", index + 1))));
    }
    save_evidence(
        "paperloom-workload-authorization.json",
        r#"{"missingCredential":401,"wrongCredential":401,"chatWithoutGrant":403,"ocrPages":2,"principalVerifier":false}"#,
    );
    stop_test_entrypoint(shutdown, server).await;
}

fn workload_ocr_config(
    chat: SocketAddr,
    embeddings: SocketAddr,
) -> (GatewayConfig, String, String) {
    let mut config = platform_config(PlatformRoute {
        verifier: chat,
        chat,
        embeddings,
        ocr_supplier: None,
        decision_supplier: None,
        embedding_supplier: None,
        rerank_supplier: None,
    });
    config.mode = OperatingMode::CloudManaged;
    config.managed.gateway_id = Some(Uuid::new_v4());
    let prefix = format!("aster_inf_{}", &Uuid::new_v4().simple().to_string()[..8]);
    let key = format!("{prefix}{}", Uuid::new_v4().simple());
    let salt = SaltString::encode_b64(Uuid::new_v4().as_bytes()).unwrap();
    let verifier_hash = Argon2::default()
        .hash_password(key.as_bytes(), &salt)
        .unwrap()
        .to_string();
    let credential_id = Uuid::new_v4();
    let inference = config.inference.as_mut().unwrap();
    let route = inference.routes.values_mut().next().unwrap();
    let limits = route.principal.take().unwrap().limits;
    inference.credentials.insert(
        credential_id,
        InferenceCredentialConfig {
            credential_id,
            environment_id: route.environment_id,
            audience: "cloud-inference".into(),
            prefix: prefix.clone(),
            verifier_hash,
            generation: 1,
            expires_at: Utc::now() + ChronoDuration::minutes(10),
            revoked: false,
        },
    );
    route.grants.insert(
        credential_id,
        InferenceGrantConfig {
            credential_generation: 1,
            models: vec!["qwen".into()],
            endpoints: vec![InferenceEndpoint::Ocr],
            limits,
            guardrails: None,
            listings: Vec::new(),
        },
    );
    config.validate().expect("platform route");
    (config, key, prefix)
}

#[cfg(unix)]
#[tokio::test]
#[ignore = "requires the installed platform OCR runtime"]
async fn paperloom_private_unix_gateway_runs_real_ocr_with_endpoint_authorization() {
    use crate::config::EntrypointConfig;
    use std::os::unix::fs::PermissionsExt;
    ensure_ocr_runtime().await;
    let (chat, _) = spawn_capturing_backend().await;
    let (embeddings, _) = spawn_capturing_backend().await;
    let (mut config, key, prefix) = workload_ocr_config(chat, embeddings);
    let directory = tempfile::Builder::new()
        .prefix("aster-ocr-")
        .tempdir_in("/tmp")
        .unwrap();
    let parent = directory.path().canonicalize().unwrap();
    std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o700)).unwrap();
    let path = parent.join("gateway.sock");
    config.entrypoints = HashMap::from([(
        "web".into(),
        EntrypointConfig::new(format!("unix:{}", path.display())),
    )]);
    config.validate().unwrap();
    let (shutdown, receiver) = tokio::sync::watch::channel(false);
    let handles = crate::entrypoint::start_entrypoints(
        &config,
        crate::entrypoint::GatewayRuntime::new(gateway_state(&config)),
        receiver,
    )
    .await
    .unwrap();
    let client = reqwest::Client::builder()
        .no_proxy()
        .unix_socket(path.clone())
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .unwrap();
    let png = draw_aster_png();
    for token in [None, Some(format!("{prefix}{}", Uuid::new_v4().simple()))] {
        let mut request = client
            .post("http://gateway.test/v1/ocr")
            .header(reqwest::header::CONTENT_TYPE, "image/png")
            .body(png.clone());
        if let Some(token) = token {
            request = request.bearer_auth(token);
        }
        assert_eq!(request.send().await.unwrap().status(), 401);
    }
    assert_eq!(client.post("http://gateway.test/v1/chat/completions").bearer_auth(&key)
        .json(&serde_json::json!({"model":"qwen", "messages":[{"role":"user", "content":"denied"}]}))
        .send().await.unwrap().status(), 403);
    let response = client
        .post("http://gateway.test/v1/ocr")
        .bearer_auth(&key)
        .header(reqwest::header::CONTENT_TYPE, "image/png")
        .body(png)
        .send()
        .await
        .unwrap();
    let status = response.status();
    let body = response.text().await.unwrap();
    assert_eq!(status, 200, "{body}");
    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_platform_ocr(&parsed);
    save_evidence("paperloom-private-unix-ocr.json", &body);
    shutdown.send(true).unwrap();
    for (_, handle) in handles {
        tokio::time::timeout(std::time::Duration::from_secs(5), handle.into_task())
            .await
            .unwrap()
            .unwrap();
    }
    assert!(!path.exists());
    assert!(client
        .get("http://gateway.test/v1/models")
        .send()
        .await
        .is_err());
}

#[cfg(unix)]
#[tokio::test]
#[ignore = "requires an owned Box home, digest-pinned PaperLoom image, real HVF/KVM and ASTER_PAPERLOOM_TEST_BOX_RUNTIME_TEST_BIN"]
async fn paperloom_guest_vsock_runs_real_pdf_ocr_without_network_egress() {
    run_box_ocr_test("aster_runtime_driver::host_service_live_tests::paperloom_gateway_vsock_ocr_has_no_ambient_egress").await;
}

#[cfg(unix)]
#[tokio::test]
#[ignore = "requires a runner-owned RAM Secret root, Paperloom image and real HVF/KVM"]
async fn paperloom_service_job_uses_real_gateway_ocr_and_retires_secrets() {
    run_box_ocr_test("aster_runtime_driver::paperloom_processing_live_tests::paperloom_service_completes_real_gateway_ocr_job_and_retires_secrets").await;
}

#[cfg(target_os = "macos")]
#[tokio::test]
#[ignore = "requires runner-owned RAM Secret root and actual macOS MicroVM admission"]
async fn paperloom_unsupported_secret_refuses_before_execution() {
    run_box_ocr_test("aster_runtime_driver::paperloom_processing_live_tests::paperloom_unsupported_secret_refuses_without_execution_or_ram_material").await;
}

#[cfg(unix)]
async fn run_box_ocr_test(test_name: &str) {
    use crate::config::EntrypointConfig;
    use std::os::unix::fs::PermissionsExt;
    let binary = std::path::PathBuf::from(
        std::env::var_os("ASTER_PAPERLOOM_TEST_BOX_RUNTIME_TEST_BIN")
            .expect("explicit Box test binary"),
    );
    assert!(binary.is_file());
    ensure_ocr_runtime().await;
    let (chat, _) = spawn_capturing_backend().await;
    let (embeddings, _) = spawn_capturing_backend().await;
    let (mut config, key, _) = workload_ocr_config(chat, embeddings);
    let directory = tempfile::Builder::new()
        .prefix("aster-ocr-")
        .tempdir_in("/tmp")
        .unwrap();
    let parent = directory.path().canonicalize().unwrap();
    std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o700)).unwrap();
    let path = parent.join("gateway.sock");
    config.entrypoints = HashMap::from([(
        "web".into(),
        EntrypointConfig::new(format!("unix:{}", path.display())),
    )]);
    config.validate().unwrap();
    let (shutdown, receiver) = tokio::sync::watch::channel(false);
    let handles = crate::entrypoint::start_entrypoints(
        &config,
        crate::entrypoint::GatewayRuntime::new(gateway_state(&config)),
        receiver,
    )
    .await
    .unwrap();
    let result = tokio::process::Command::new(binary)
        .args(["--exact", test_name, "--ignored", "--nocapture"])
        .env("ASTER_BOX_TEST_GATEWAY_SOCKET", &path)
        .env("ASTER_BOX_TEST_GATEWAY_KEY", key)
        .kill_on_drop(true)
        .output()
        .await;
    shutdown.send(true).unwrap();
    for (_, handle) in handles {
        tokio::time::timeout(std::time::Duration::from_secs(5), handle.into_task())
            .await
            .unwrap()
            .unwrap();
    }
    assert!(!path.exists());
    let result = result.expect("Box fixture process");
    let log = format!(
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    save_evidence(
        if test_name.contains("processing_live_tests") {
            "paperloom-processing-client.log"
        } else {
            "paperloom-guest-vsock-client.log"
        },
        &log,
    );
    assert!(result.status.success(), "{log}");
    assert!(log.contains("1 passed; 0 failed; 0 ignored"), "{log}");
}
