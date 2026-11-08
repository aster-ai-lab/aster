//! Installer and download proofs against a scripted local HTTP server.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::{install_family, InstallManifest, ManifestFile};
use crate::assets::{bge_status, Family, RECEIPT_FILE};

/// Serialises tests that touch process-global environment variables.
static ENV_LOCK: Mutex<()> = Mutex::new(());

fn hex64(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}

/// Serves one scripted response per connection on a loopback port and
/// returns the URL. Each handler receives the byte offset the client
/// asked to resume from, if any; an exhausted queue closes the
/// connection, which the client treats as a retriable interruption.
async fn scripted_server(responses: Vec<Box<dyn Fn(Option<u64>) -> Vec<u8> + Send>>) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind test listener");
    let address = listener.local_addr().expect("test listener address");
    let queue: Arc<Mutex<VecDeque<Box<dyn Fn(Option<u64>) -> Vec<u8> + Send>>>> =
        Arc::new(Mutex::new(responses.into()));
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            let queue = Arc::clone(&queue);
            tokio::spawn(async move {
                let mut head = Vec::new();
                let mut byte = [0_u8; 1];
                while !head.ends_with(b"\r\n\r\n") {
                    match stream.read(&mut byte).await {
                        Ok(0) | Err(_) => return,
                        Ok(_) => head.push(byte[0]),
                    }
                }
                let head = String::from_utf8_lossy(&head).to_ascii_lowercase();
                let range = head.lines().find_map(|line| {
                    let value = line.strip_prefix("range: bytes=")?;
                    value
                        .split('-')
                        .next()
                        .and_then(|start| start.trim().parse::<u64>().ok())
                });
                let response = {
                    let mut queue = queue.lock().expect("scripted response queue");
                    queue.pop_front().map(|handler| handler(range))
                };
                if let Some(response) = response {
                    let _ = stream.write_all(&response).await;
                }
                let _ = stream.shutdown().await;
            });
        }
    });
    format!("http://{address}/model")
}

fn full_200(body: &[u8]) -> Vec<u8> {
    let mut response =
        format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", body.len()).into_bytes();
    response.extend_from_slice(body);
    response
}

fn partial_206(body: &[u8], start: u64, total: u64) -> Vec<u8> {
    let mut response = format!(
        "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {}-{}/{}\r\nContent-Length: {}\r\n\r\n",
        start,
        total - 1,
        total,
        body.len()
    )
    .into_bytes();
    response.extend_from_slice(body);
    response
}

/// A body declared in full but delivered half-way, then closed.
fn truncated(body: &[u8]) -> Vec<u8> {
    let cut = body.len() / 2;
    let mut response =
        format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", body.len()).into_bytes();
    response.extend_from_slice(&body[..cut]);
    response
}

fn manifest_for(url: &str, body: &[u8], sha256: &str) -> InstallManifest {
    InstallManifest::new(
        Family::BgeM3,
        "00000000-0000-4000-8000-000000000001",
        "http://127.0.0.1:3000",
        "model.onnx",
        "tokenizer.json",
        vec![
            ManifestFile {
                path: "model.onnx".to_string(),
                url: url.to_string(),
                bytes: body.len() as u64,
                sha256: sha256.to_string(),
            },
            ManifestFile {
                path: "tokenizer.json".to_string(),
                url: url.to_string(),
                bytes: body.len() as u64,
                sha256: sha256.to_string(),
            },
        ],
    )
    .expect("valid test manifest")
}

fn with_managed_home(home: &std::path::Path) {
    std::env::set_var("ASTER_USE_BGE_HOME", home);
    std::env::remove_var("ASTER_BGE_M3_MODEL_DIR");
    std::env::remove_var("ASTER_BGE_RERANKER_MODEL_DIR");
}

#[tokio::test]
async fn interrupted_download_resumes_from_the_verified_prefix() {
    let body = b"0123456789abcdefghijklmnopqrstuvwxyz".repeat(64);
    let sha = hex64(&body);
    let url = {
        let body = body.clone();
        let resume = body.clone();
        let tokenizer = body.clone();
        scripted_server(vec![
            // First transfer truncates half-way…
            Box::new(move |range| match range {
                None => truncated(&body),
                Some(start) => partial_206(&body[start as usize..], start, body.len() as u64),
            }),
            // …so the retry must resume from the verified prefix.
            Box::new(move |range| match range {
                None => full_200(&resume),
                Some(start) => partial_206(&resume[start as usize..], start, resume.len() as u64),
            }),
            Box::new(move |_| full_200(&tokenizer)),
        ])
        .await
    };

    let home = tempfile::tempdir().expect("temp home");
    let _guard = ENV_LOCK.lock().unwrap_or_else(|poison| poison.into_inner());
    with_managed_home(home.path());
    let status = install_family(manifest_for(&url, &body, &sha), true)
        .await
        .expect("install");
    assert!(status.available);
    assert!(home.path().join("bge-m3/model.onnx").is_file());
}

#[tokio::test]
async fn substituted_content_range_is_rejected_by_name() {
    let body = b"0123456789".repeat(32);
    let sha = hex64(&body);
    let url = {
        let body = body.clone();
        let resume = body.clone();
        scripted_server(vec![
            Box::new(move |range| match range {
                None => truncated(&body),
                Some(start) => partial_206(&body[start as usize..], start, body.len() as u64),
            }),
            // The resume claims a substituted start offset.
            Box::new(move |range| match range {
                None => full_200(&resume),
                Some(start) => {
                    partial_206(&resume[start as usize..], start + 1, resume.len() as u64)
                }
            }),
        ])
        .await
    };

    let client = super::download::client().expect("client");
    let destination = tempfile::tempdir().expect("temp dir");
    let path = destination.path().join("download.bin");
    let error = super::download::verified(&client, &url, &path, body.len() as u64, &sha)
        .await
        .expect_err("substituted range accepted");
    assert_eq!(error.code, "use.bge.download_failed");
    assert!(error.message.contains("does not continue"));
}

#[tokio::test]
async fn manifest_install_rejects_a_sha256_mismatch_and_keeps_the_previous_install() {
    let good = b"good weights".repeat(64);
    let good_sha = hex64(&good);
    let bad = b"evil weights".repeat(64);

    let url = {
        let good = good.clone();
        let good_tokenizer = good.clone();
        let bad = bad.clone();
        scripted_server(vec![
            Box::new(move |_| full_200(&good)),
            Box::new(move |_| full_200(&good_tokenizer)),
            Box::new(move |_| full_200(&bad)),
        ])
        .await
    };

    let home = tempfile::tempdir().expect("temp home");
    let _guard = ENV_LOCK.lock().unwrap_or_else(|poison| poison.into_inner());
    with_managed_home(home.path());

    let first = install_family(manifest_for(&url, &good, &good_sha), true)
        .await
        .expect("first install");
    assert!(first.available);

    let mismatch = install_family(manifest_for(&url, &bad, &good_sha), true)
        .await
        .expect_err("mismatched sha accepted");
    assert_eq!(mismatch.code, "use.bge.integrity_mismatch");
    assert!(mismatch.message.contains("model.onnx"));

    let status = bge_status(Family::BgeM3);
    assert!(status.available, "the previous install must survive");
    let weights = std::fs::read(home.path().join("bge-m3/model.onnx")).expect("weights");
    assert_eq!(weights, good);
}

#[tokio::test]
async fn receipt_binds_every_file_digest() {
    let body = b"receipt weights".repeat(16);
    let sha = hex64(&body);
    let url = {
        let model = body.clone();
        let tokenizer = body.clone();
        scripted_server(vec![
            Box::new(move |_| full_200(&model)),
            Box::new(move |_| full_200(&tokenizer)),
        ])
        .await
    };

    let home = tempfile::tempdir().expect("temp home");
    let _guard = ENV_LOCK.lock().unwrap_or_else(|poison| poison.into_inner());
    with_managed_home(home.path());
    install_family(manifest_for(&url, &body, &sha), true)
        .await
        .expect("install");

    let receipt_path = home.path().join("bge-m3").join(RECEIPT_FILE);
    let receipt: serde_json::Value =
        serde_json::from_slice(&std::fs::read(receipt_path).expect("receipt"))
            .expect("receipt json");
    let files = receipt["files"].as_array().expect("receipt files");
    assert_eq!(files.len(), 2);
    for file in files {
        let path = file["path"].as_str().expect("path");
        let bound = file["sha256"].as_str().expect("sha256");
        let installed = std::fs::read(home.path().join("bge-m3").join(path)).expect("file");
        assert_eq!(bound, hex64(&installed));
        assert_eq!(
            file["bytes"].as_u64().expect("bytes"),
            installed.len() as u64
        );
    }
}
