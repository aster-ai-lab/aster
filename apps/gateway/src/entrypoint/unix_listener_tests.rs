#![cfg(test)]

use super::*;
use crate::config::{InferenceEndpoint, TlsConfig};
use crate::entrypoint::http_listener::PrivateUnixListener;
use crate::entrypoint::inference_tests::{inference_config, inference_key};
use std::os::unix::fs::{symlink, FileTypeExt, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

fn private_directory() -> (tempfile::TempDir, PathBuf) {
    let directory = tempfile::Builder::new()
        .prefix("aster-gw-")
        .tempdir_in("/tmp")
        .unwrap();
    let path = directory.path().canonicalize().unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    (directory, path)
}

fn unix_config(config: &mut GatewayConfig, path: &Path) {
    config.entrypoints.clear();
    config.entrypoints.insert(
        "web".into(),
        EntrypointConfig::new(format!("unix:{}", path.display())),
    );
}

fn client(path: &Path) -> reqwest::Client {
    reqwest::Client::builder()
        .no_proxy()
        .unix_socket(path.to_owned())
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap()
}

async fn start(
    config: &GatewayConfig,
) -> (
    tokio::sync::watch::Sender<bool>,
    crate::entrypoint::listener::EntryPointHandle,
    tokio::sync::mpsc::UnboundedReceiver<AccessLogEntry>,
) {
    let (logs, receiver) = tokio::sync::mpsc::unbounded_channel();
    let runtime = GatewayRuntime::new(gateway_state(config, logs, true));
    let (shutdown, rx) = tokio::sync::watch::channel(false);
    let mut handles = start_entrypoints(config, runtime, rx).await.unwrap();
    (shutdown, handles.remove("web").unwrap(), receiver)
}

#[tokio::test]
async fn unix_http_keeps_auth_and_replaces_spoofed_forwarding_identity() {
    let (_directory, parent) = private_directory();
    let path = parent.join("gateway.sock");
    let (backend, captured) = spawn_capturing_http_backend().await;
    let mut config = routed_config(backend);
    unix_config(&mut config, &path);
    config.middlewares.insert(
        "auth".into(),
        MiddlewareConfig {
            middleware_type: "api-key".into(),
            header: Some("x-api-key".into()),
            keys: vec!["test-only-key".into()],
            ..Default::default()
        },
    );
    config
        .routers
        .get_mut("test-router")
        .unwrap()
        .middlewares
        .push("auth".into());
    config.validate().unwrap();
    let (shutdown, handle, mut logs) = start(&config).await;
    let acceptor = crate::proxy::tls::build_tls_acceptor_from_pem(
        include_str!("../../tests/fixtures/tls/revision-1.crt"),
        include_str!("../../tests/fixtures/tls/revision-1.key"),
        "1.2",
    )
    .unwrap();
    assert!(
        handle.install_http_tls_acceptor(acceptor).is_err(),
        "an asynchronous ACME completion must not enable TLS on a Unix listener"
    );
    let metadata = std::fs::symlink_metadata(&path).unwrap();
    assert!(metadata.file_type().is_socket());
    assert_eq!(metadata.mode() & 0o777, 0o600);
    let client = client(&path);
    assert_eq!(
        client
            .get("http://gateway.test/")
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    let response = client
        .get("http://gateway.test/")
        .header("x-api-key", "test-only-key")
        .header("forwarded", "for=127.0.0.1;proto=https")
        .header("x-forwarded-for", "127.0.0.1")
        .header("x-forwarded-host", "spoofed.test")
        .header("x-forwarded-proto", "https")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.text().await.unwrap(), "{}");
    let headers = captured.await.unwrap().headers.to_ascii_lowercase();
    assert!(
        headers.contains("x-forwarded-for: unknown\r\n"),
        "{headers}"
    );
    assert!(
        headers.contains("x-forwarded-host: gateway.test\r\n"),
        "{headers}"
    );
    assert!(headers.contains("x-forwarded-proto: http\r\n"), "{headers}");
    assert!(!headers.contains("127.0.0.1") && !headers.contains("spoofed.test"));
    assert!(!headers.lines().any(|line| line.starts_with("forwarded:")));
    for _ in 0..2 {
        assert_eq!(next_log(&mut logs).await.client_ip, "unix");
    }
    stop_test_entrypoint(shutdown, handle.into_task()).await;
    assert!(!path.exists());
}

#[tokio::test]
async fn unix_http_does_not_inherit_loopback_ip_permissions() {
    let (_directory, parent) = private_directory();
    let path = parent.join("gateway.sock");
    let mut config = routed_config(free_address().await);
    unix_config(&mut config, &path);
    config.middlewares.insert(
        "loopback".into(),
        MiddlewareConfig {
            middleware_type: "ip-allow".into(),
            allowed_ips: vec!["127.0.0.0/8".into()],
            ..Default::default()
        },
    );
    config
        .routers
        .get_mut("test-router")
        .unwrap()
        .middlewares
        .push("loopback".into());
    let (shutdown, handle, _logs) = start(&config).await;
    let response = client(&path)
        .get("http://gateway.test/")
        .header("x-forwarded-for", "127.0.0.1")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 403);
    stop_test_entrypoint(shutdown, handle.into_task()).await;
    assert!(!path.exists());
}

#[tokio::test]
async fn unix_inference_requires_current_bearer_and_endpoint_grant() {
    let (_directory, parent) = private_directory();
    let path = parent.join("gateway.sock");
    let key = inference_key('a');
    let mut config = inference_config(
        free_address().await,
        &key,
        chrono::Utc::now() + chrono::Duration::minutes(5),
    );
    unix_config(&mut config, &path);
    let route = config
        .inference
        .as_mut()
        .unwrap()
        .routes
        .values_mut()
        .next()
        .unwrap();
    route.grants.values_mut().next().unwrap().endpoints = vec![InferenceEndpoint::Models];
    config.validate().unwrap();
    let (shutdown, handle, _logs) = start(&config).await;
    let client = client(&path);
    for token in [None, Some(inference_key('b'))] {
        let mut request = client.get("http://gateway.test/v1/models");
        if let Some(token) = token {
            request = request.bearer_auth(token);
        }
        assert_eq!(request.send().await.unwrap().status(), 401);
    }
    let response = client
        .get("http://gateway.test/v1/models")
        .bearer_auth(&key)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["data"]["object"], "list");
    assert_eq!(body["data"]["data"][0]["id"], "allowed-model");
    assert!(!body.to_string().contains("hidden-model"));
    assert_eq!(client.post("http://gateway.test/v1/chat/completions").bearer_auth(&key)
        .json(&serde_json::json!({"model":"allowed-model", "messages":[{"role":"user","content":"hello"}]}))
        .send().await.unwrap().status(), 403);
    stop_test_entrypoint(shutdown, handle.into_task()).await;
    assert!(!path.exists());
}

#[tokio::test]
async fn unix_http_drains_inflight_response_before_shutdown() {
    let (_directory, parent) = private_directory();
    let path = parent.join("gateway.sock");
    let upstream = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut config = routed_config(upstream.local_addr().unwrap());
    unix_config(&mut config, &path);
    let (release, released) = tokio::sync::oneshot::channel();
    let backend = tokio::spawn(async move {
        let (mut connection, _) = upstream.accept().await.unwrap();
        crate::entrypoint::inference_tests::read_http_request(&mut connection).await;
        connection
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 11\r\nConnection: close\r\n\r\nfirst")
            .await
            .unwrap();
        released.await.unwrap();
        connection.write_all(b"second").await.unwrap();
    });
    let (shutdown, handle, _logs) = start(&config).await;
    let mut response = client(&path)
        .get("http://gateway.test/")
        .send()
        .await
        .unwrap();
    assert_eq!(response.chunk().await.unwrap().unwrap(), "first");
    shutdown.send(true).unwrap();
    let mut task = handle.into_task();
    assert!(tokio::time::timeout(Duration::from_millis(50), &mut task)
        .await
        .is_err());
    release.send(()).unwrap();
    assert_eq!(response.bytes().await.unwrap(), "second");
    tokio::time::timeout(Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap();
    backend.await.unwrap();
    assert!(!path.exists());
}

#[tokio::test]
async fn unix_socket_refuses_unsafe_parents_and_preserves_occupied_paths() {
    let (_directory, parent) = private_directory();
    let path = parent.join("gateway.sock");
    std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(PrivateUnixListener::bind(&path).is_err());
    assert!(!path.exists());
    std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o700)).unwrap();
    std::fs::write(&path, "keep this file").unwrap();
    assert!(PrivateUnixListener::bind(&path).is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "keep this file");
    let alias = parent.join("alias");
    symlink(&parent, &alias).unwrap();
    assert!(PrivateUnixListener::bind(&alias.join("other.sock")).is_err());
    assert!(!parent.join("other.sock").exists());
    std::fs::remove_file(&path).unwrap();
    let occupied = std::os::unix::net::UnixListener::bind(&path).unwrap();
    let inode = std::fs::symlink_metadata(&path).unwrap().ino();
    assert!(PrivateUnixListener::bind(&path).is_err());
    assert_eq!(std::fs::symlink_metadata(&path).unwrap().ino(), inode);
    drop(occupied);
    assert!(
        PrivateUnixListener::bind(&path).is_err(),
        "stale sockets require operator recovery"
    );
    std::fs::remove_file(&path).unwrap();
    let unsafe_parent = parent.join("unsafe");
    std::fs::create_dir(&unsafe_parent).unwrap();
    std::fs::set_permissions(&unsafe_parent, std::fs::Permissions::from_mode(0o777)).unwrap();
    let private = unsafe_parent.join("private");
    std::fs::create_dir(&private).unwrap();
    std::fs::set_permissions(&private, std::fs::Permissions::from_mode(0o700)).unwrap();
    assert!(PrivateUnixListener::bind(&private.join("gateway.sock")).is_err());
    assert!(!private.join("gateway.sock").exists());
}

#[tokio::test]
async fn unix_socket_abort_removes_only_its_owned_inode() {
    let (_directory, parent) = private_directory();
    let path = parent.join("gateway.sock");
    let mut config = routed_config(free_address().await);
    unix_config(&mut config, &path);
    let (_shutdown, handle, _logs) = start(&config).await;
    let task = handle.into_task();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert!(!path.exists());
    let (_shutdown, handle, _logs) = start(&config).await;
    std::fs::remove_file(&path).unwrap();
    let replacement = std::os::unix::net::UnixListener::bind(&path).unwrap();
    let inode = std::fs::symlink_metadata(&path).unwrap().ino();
    let task = handle.into_task();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert_eq!(std::fs::symlink_metadata(&path).unwrap().ino(), inode);
    drop(replacement);
}

#[test]
fn unix_address_rejects_unsupported_policy_and_path_aliases() {
    let config = EntrypointConfig::new("unix:/tmp/private/gateway.sock");
    let mut bad = config.clone();
    bad.protocol = Protocol::Tcp;
    assert!(bad.validate_listener_policy("private").is_err());
    bad = config.clone();
    bad.protocol = Protocol::Udp;
    assert!(bad.validate_listener_policy("private").is_err());
    bad = config.clone();
    bad.trust_forwarded_headers = true;
    assert!(bad.validate_listener_policy("private").is_err());
    bad = config.clone();
    bad.tls = Some(TlsConfig {
        cert_file: "cert".into(),
        key_file: "key".into(),
        acme: false,
        min_version: "1.2".into(),
        acme_email: None,
        acme_domains: vec![],
        acme_staging: false,
        acme_storage_path: None,
    });
    assert!(bad.validate_listener_policy("private").is_err());
    for path in [
        "unix:",
        "unix:relative.sock",
        "unix:/",
        "unix:/tmp/../gateway.sock",
        "unix:/tmp/./gateway.sock",
        "unix:/tmp//gateway.sock",
        "unix:/tmp/gateway.sock/",
    ] {
        assert!(
            EntrypointConfig::new(path)
                .validate_listener_policy("private")
                .is_err(),
            "{path}"
        );
    }
    let mut duplicate = GatewayConfig::default();
    duplicate.entrypoints = HashMap::from([("one".into(), config.clone()), ("two".into(), config)]);
    assert!(duplicate
        .validate()
        .unwrap_err()
        .to_string()
        .contains("same listen address"));
}

#[tokio::test]
async fn unix_gateway_reload_preserves_socket_and_rejects_unsafe_policy() {
    let (_directory, parent) = private_directory();
    let path = parent.join("gateway.sock");
    let mut config = routed_config(spawn_http_backend("reloaded", "text/plain").await);
    unix_config(&mut config, &path);
    let gateway = crate::Gateway::new(config.clone()).unwrap();
    gateway.start().await.unwrap();
    let inode = std::fs::symlink_metadata(&path).unwrap().ino();
    let client = client(&path);
    assert_eq!(
        client
            .get("http://gateway.test/")
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    let mut unsafe_config = config.clone();
    unsafe_config
        .entrypoints
        .get_mut("web")
        .unwrap()
        .trust_forwarded_headers = true;
    assert!(gateway.reload(unsafe_config).await.is_err());
    assert_eq!(
        client
            .get("http://gateway.test/")
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    config.middlewares.insert(
        "auth".into(),
        MiddlewareConfig {
            middleware_type: "api-key".into(),
            header: Some("x-api-key".into()),
            keys: vec!["new-fixture-key".into()],
            ..Default::default()
        },
    );
    config
        .routers
        .get_mut("test-router")
        .unwrap()
        .middlewares
        .push("auth".into());
    gateway.reload(config).await.unwrap();
    assert_eq!(std::fs::symlink_metadata(&path).unwrap().ino(), inode);
    assert_eq!(
        client
            .get("http://gateway.test/")
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    assert_eq!(
        client
            .get("http://gateway.test/")
            .header("x-api-key", "new-fixture-key")
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap(),
        "reloaded"
    );
    gateway.shutdown().await;
    assert!(!path.exists());
}

#[tokio::test]
async fn unix_startup_rollback_retires_started_socket_without_removing_conflicting_file() {
    let (_directory, parent) = private_directory();
    let mut config = routed_config(free_address().await);
    unix_config(&mut config, &parent.join("one.sock"));
    config.entrypoints.insert(
        "second".into(),
        EntrypointConfig::new(format!("unix:{}", parent.join("two.sock").display())),
    );
    let paths: Vec<_> = config
        .entrypoints
        .values()
        .map(|entrypoint| PathBuf::from(entrypoint.address.strip_prefix("unix:").unwrap()))
        .collect();
    std::fs::write(&paths[1], "occupied path must survive").unwrap();
    let (logs, _receiver) = tokio::sync::mpsc::unbounded_channel();
    let runtime = GatewayRuntime::new(gateway_state(&config, logs, true));
    let (_shutdown, receiver) = tokio::sync::watch::channel(false);
    assert!(start_entrypoints(&config, runtime, receiver).await.is_err());
    assert!(
        !paths[0].exists(),
        "first listener must be retired on later bind failure"
    );
    assert_eq!(
        std::fs::read_to_string(&paths[1]).unwrap(),
        "occupied path must survive"
    );
}
