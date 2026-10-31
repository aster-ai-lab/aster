#![cfg(test)]

use super::*;
use futures_util::FutureExt;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[tokio::test]
#[ignore = "requires an owned job root, pinned Paperloom API/source/Python and actual platform OCR"]
async fn actual_api_job_publishes_gateway_ocr_bytes() {
    let root = required_path("ASTER_PAPERLOOM_API_JOB_ROOT");
    assert_eq!(
        std::fs::read_to_string(root.join(".owned")).unwrap(),
        "paperloom-real-api-job-20261017\n"
    );
    let binary = required_path("ASTER_PAPERLOOM_API_BIN");
    // Preserve the venv entry point: resolving its symlink loses site-packages.
    let python = PathBuf::from(std::env::var_os("ASTER_PAPERLOOM_PIPELINE_PYTHON").unwrap());
    let source = required_path("ASTER_PAPERLOOM_SOURCE_ROOT");
    assert!(binary.is_file() && python.is_absolute() && python.is_file());
    assert!(source.join("backend/pipeline/retainpdf_pipeline").is_dir());
    let run = root.join(format!("run-{}", Uuid::new_v4().simple()));
    std::fs::create_dir(&run).unwrap();
    std::fs::create_dir(run.join("api")).unwrap();
    std::fs::create_dir(run.join("tmp")).unwrap();
    ensure_ocr_runtime().await;
    let (chat, _) = spawn_capturing_backend().await;
    let (embeddings, _) = spawn_capturing_backend().await;
    let (config, key, _) = workload_ocr_config(chat, embeddings);
    let (gateway, shutdown, server) = start_test_entrypoint(gateway_state(&config)).await;
    let api_listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let simple_listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let api_port = api_listener.local_addr().unwrap().port();
    let simple_port = simple_listener.local_addr().unwrap().port();
    let base = format!("http://127.0.0.1:{api_port}/api/v1");
    let log = run.join("api.log");
    let output = std::fs::File::create(&log).unwrap();
    drop((api_listener, simple_listener));
    let mut child = tokio::process::Command::new(binary)
        .env_clear()
        .env(
            "PATH",
            format!("{}:/usr/bin:/bin", python.parent().unwrap().display()),
        )
        .env("TMPDIR", run.join("tmp"))
        .env("PYTHON_BIN", &python)
        .env("PYTHONPATH", source.join("backend/pipeline"))
        .env(
            "RUST_API_PIPELINE_COMMAND",
            python.parent().unwrap().join("retainpdf-pipeline"),
        )
        .env("RUST_API_PROJECT_ROOT", &source)
        .env("RUST_API_ROOT", run.join("api"))
        .env("RUST_API_DATA_ROOT", run.join("data"))
        .env("RUST_API_BIND_HOST", "127.0.0.1")
        .env("RUST_API_PORT", api_port.to_string())
        .env("RUST_API_SIMPLE_PORT", simple_port.to_string())
        .env("RUST_API_KEYS", "paperloom-api-job-fixture-only")
        .env("RUST_API_MAX_RUNNING_JOBS", "1")
        .env("RUST_API_AI_SUPERVISE", "false")
        .env("RUST_API_JOBS_MODE", "inprocess")
        .env("ASTER_GATEWAY_ORIGIN", format!("http://{gateway}"))
        .env("ASTER_GATEWAY_KEY", &key)
        .env("RUST_LOG", "warn")
        .current_dir(&run)
        .stdout(output.try_clone().unwrap())
        .stderr(output)
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let pid = child.id().unwrap();
    let probe = async {
        let http = reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap();
        tokio::time::timeout(Duration::from_secs(30), async {
            loop {
                assert!(
                    child.try_wait().unwrap().is_none(),
                    "API stopped before readiness; see {}",
                    log.display()
                );
                if http
                    .get(format!("{base}/documents"))
                    .header("X-API-Key", "paperloom-api-job-fixture-only")
                    .send()
                    .await
                    .is_ok_and(|response| response.status().is_success())
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await
        .expect("actual API readiness deadline");
        assert_eq!(
            http.get(format!("{base}/documents"))
                .send()
                .await
                .unwrap()
                .status(),
            401
        );
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let pdf = std::fs::read(repo.join("fixtures/paperloom/two-pages.pdf")).unwrap();
        let digest = sha256(&pdf);
        let boundary = format!("aster-{}", Uuid::new_v4().simple());
        let mut body = format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"paper.pdf\"\r\nContent-Type: application/pdf\r\n\r\n").into_bytes();
        body.extend(&pdf);
        body.extend(format!("\r\n--{boundary}--\r\n").as_bytes());
        let uploaded: Value = http
            .post(format!("{base}/uploads"))
            .header("X-API-Key", "paperloom-api-job-fixture-only")
            .header(
                "Content-Type",
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(body)
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(uploaded["data"]["document_id"], digest);
        let adapter = std::fs::read_to_string(
            repo.join("apps/use-registry/packages/paperloom/runtime/gateway_ocr.py"),
        )
        .unwrap();
        let command = format!("python3 -c '{}'", adapter.replace('\'', "'\"'\"'"));
        let submitted: Value = http.post(format!("{base}/documents/{digest}/ocr")).header("X-API-Key", "paperloom-api-job-fixture-only")
            .json(&json!({"workflow":"ocr", "source":{}, "ocr":{"provider":"local", "options":{"command":command,"raw_provider":"generic_flat_ocr"}}}))
            .send().await.unwrap().error_for_status().unwrap().json().await.unwrap();
        let job = submitted["data"]["job_id"]
            .as_str()
            .expect("actual submitted job");
        let completed = tokio::time::timeout(Duration::from_secs(240), async {
            loop {
                let detail: Value = http
                    .get(format!("{base}/jobs/{job}"))
                    .header("X-API-Key", "paperloom-api-job-fixture-only")
                    .send()
                    .await
                    .unwrap()
                    .error_for_status()
                    .unwrap()
                    .json()
                    .await
                    .unwrap();
                match detail["data"]["status"].as_str() {
                    Some("succeeded" | "failed" | "canceled") => break detail,
                    _ => tokio::time::sleep(Duration::from_millis(250)).await,
                }
            }
        })
        .await
        .expect("actual OCR job deadline");
        std::fs::write(
            run.join("job-final.json"),
            serde_json::to_vec_pretty(&completed).unwrap(),
        )
        .unwrap();
        assert_eq!(
            completed["data"]["status"], "succeeded",
            "see the owned job-final.json"
        );
        assert_eq!(completed["data"]["stages"]["ocr"]["state"], "completed");
        let manifest: Value = http
            .get(format!("{base}/jobs/{job}/artifacts-manifest"))
            .header("X-API-Key", "paperloom-api-job-fixture-only")
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap();
        std::fs::write(
            run.join("manifest.json"),
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .unwrap();
        let mut verified = Vec::new();
        for item in manifest["data"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|item| item["ready"] == true && item["artifact_kind"] == "file")
        {
            let path = item["resource_path"].as_str().unwrap();
            assert!(
                path.starts_with(&format!("/api/v1/ocr/jobs/{job}/"))
                    || path.starts_with(&format!("/api/v1/jobs/{job}/"))
            );
            let bytes = http
                .get(format!("http://127.0.0.1:{api_port}{path}"))
                .header("X-API-Key", "paperloom-api-job-fixture-only")
                .send()
                .await
                .unwrap()
                .error_for_status()
                .unwrap()
                .bytes()
                .await
                .unwrap();
            assert_eq!(item["checksum"], format!("sha256:{}", sha256(&bytes)));
            assert_eq!(item["size_bytes"], bytes.len());
            let studio_bytes = http
                .get(format!(
                    "{base}/jobs/{job}/artifacts/{}",
                    item["artifact_key"].as_str().unwrap()
                ))
                .header("X-API-Key", "paperloom-api-job-fixture-only")
                .send()
                .await
                .unwrap()
                .error_for_status()
                .unwrap()
                .bytes()
                .await
                .unwrap();
            assert_eq!(
                studio_bytes, bytes,
                "Studio download differs from the published resource"
            );
            verified.push(item["artifact_key"].clone());
        }
        assert!(
            !verified.is_empty(),
            "actual OCR produced no verifiable downloadable files"
        );
        let job_root = run.join("data/jobs").join(job).canonicalize().unwrap();
        assert!(job_root.starts_with(run.canonicalize().unwrap()));
        let payload: Value = serde_json::from_slice(
            &std::fs::read(job_root.join("ocr/local_raw/payload.json")).unwrap(),
        )
        .unwrap();
        let pages = payload["pages"].as_array().unwrap();
        assert_eq!(pages.len(), 2);
        for (index, page) in pages.iter().enumerate() {
            let blocks = page["blocks"].as_array().unwrap();
            assert!(blocks
                .iter()
                .all(|block| block["metadata"]["ocr_provider"] == "pp-ocr-v6"));
            assert!(blocks.iter().any(|block| block["text"]
                .as_str()
                .unwrap()
                .contains(&format!("PaperLoom regression - Page {}", index + 1))));
        }
        let normalized: Value = serde_json::from_slice(
            &std::fs::read(job_root.join("ocr/normalized/document.v1.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(normalized["pages"].as_array().unwrap().len(), 2);
        std::fs::write(run.join("result.json"),serde_json::to_vec_pretty(&json!({
            "boundary":"actual host API and pipeline; no Runtime Secret or Studio CUA proof",
            "sourceDigest":digest,"job":completed,"manifest":manifest,"verifiedArtifactKeys":verified,
            "actualGatewayOcrPages":pages.len(),"normalizedPages":2
        })).unwrap()).unwrap();
    };
    let probe = std::panic::AssertUnwindSafe(probe).catch_unwind().await;
    let interrupted = child.try_wait().unwrap().is_none()
        && std::process::Command::new("kill")
            .args(["-INT", &pid.to_string()])
            .status()
            .unwrap()
            .success();
    let retired = tokio::time::timeout(Duration::from_secs(15), child.wait()).await;
    let normal = interrupted && matches!(retired, Ok(Ok(status)) if status.success());
    if !normal {
        let _ = child.kill().await;
    }
    stop_test_entrypoint(shutdown, server).await;
    let mut paths = vec![run.clone()];
    while let Some(path) = paths.pop() {
        for entry in std::fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                paths.push(path);
            } else {
                assert!(
                    !std::fs::read(&path)
                        .unwrap()
                        .windows(key.len())
                        .any(|bytes| bytes == key.as_bytes()),
                    "workload credential persisted in owned job files"
                );
            }
        }
    }
    std::fs::write(run.join("retirement.json"),serde_json::to_vec_pretty(&json!({"pid":pid,"apiNormalExit":normal,"gatewayClosed":true,"credentialAbsentFromOwnedFiles":true})).unwrap()).unwrap();
    probe.expect("actual API OCR job failed; exact child cleanup attempted");
    assert!(normal, "API did not retire through normal shutdown");
    println!("actual API OCR proof: {}", run.display());
}

fn required_path(name: &str) -> PathBuf {
    PathBuf::from(std::env::var_os(name).expect("explicit owned test path"))
        .canonicalize()
        .unwrap()
}

fn sha256(bytes: &[u8]) -> String {
    ring::digest::digest(&ring::digest::SHA256, bytes)
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
