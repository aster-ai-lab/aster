#![cfg(all(test, unix))]

use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn private_http_cli_authenticates_without_tcp_listener_and_cleans_up_on_sigterm() {
    let directory = tempfile::Builder::new()
        .prefix("aster-gw-")
        .tempdir_in("/tmp")
        .unwrap();
    let directory = directory.path().canonicalize().unwrap();
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700)).unwrap();
    let path = directory.join("gateway.sock");
    let config = directory.join("gateway.acl");
    let upstream = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = upstream.local_addr().unwrap();
    let backend = tokio::spawn(async move {
        let (mut stream, _) = upstream.accept().await.unwrap();
        let mut request = [0; 8192];
        let read = stream.read(&mut request).await.unwrap();
        assert!(String::from_utf8_lossy(&request[..read]).contains("GET /proof HTTP/1.1"));
        stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 15\r\nConnection: close\r\n\r\nprivate-gateway").await.unwrap();
    });
    std::fs::write(
        &config,
        format!(
            r#"
entrypoints "private" {{ address = "unix:{}" }}
routers "private" {{
  rule = "PathPrefix(`/`)"
  service = "upstream"
  entrypoints = ["private"]
  middlewares = ["auth"]
}}
services "upstream" {{ load_balancer {{ servers = [{{ url = "http://{address}" }}] }} }}
middlewares "auth" {{ type = "api-key" header = "x-api-key" keys = ["fixture-only-key"] }}
providers {{ file {{ watch = false }} }}
observability {{ metrics_enabled = false access_log_enabled = false }}
shutdown_timeout_secs {{ shutdown_timeout_secs = 2 }}
"#,
            path.display()
        ),
    )
    .unwrap();
    let binary = env!("CARGO_BIN_EXE_aster-gateway");
    let validation = tokio::process::Command::new(binary)
        .arg("validate")
        .arg("--config")
        .arg(&config)
        .output()
        .await
        .unwrap();
    assert!(
        validation.status.success(),
        "{}",
        String::from_utf8_lossy(&validation.stderr)
    );
    let log = std::fs::File::create(directory.join("gateway.log")).unwrap();
    let mut child = tokio::process::Command::new(binary)
        .arg("--config")
        .arg(&config)
        .stdin(Stdio::null())
        .stdout(log.try_clone().unwrap())
        .stderr(log)
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let pid = child.id().unwrap();
    let client = reqwest::Client::builder()
        .no_proxy()
        .unix_socket(path.clone())
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap();
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            assert!(
                child.try_wait().unwrap().is_none(),
                "gateway exited: {}",
                std::fs::read_to_string(directory.join("gateway.log")).unwrap()
            );
            if let Ok(response) = client.get("http://gateway.test/proof").send().await {
                assert_eq!(response.status(), 401);
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("private entrypoint ready");
    assert_eq!(
        std::fs::symlink_metadata(&path).unwrap().mode() & 0o777,
        0o600
    );
    assert_eq!(
        client
            .get("http://gateway.test/proof")
            .header("x-api-key", "fixture-only-key")
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap(),
        "private-gateway"
    );
    backend.await.unwrap();
    let listeners = tokio::process::Command::new("lsof")
        .args(["-a", "-p", &pid.to_string(), "-iTCP", "-sTCP:LISTEN", "-Fn"])
        .output()
        .await
        .expect("lsof is required for socket ownership evidence");
    assert!(matches!(listeners.status.code(), Some(0 | 1)));
    assert!(
        !String::from_utf8_lossy(&listeners.stdout)
            .lines()
            .any(|line| line.starts_with('n')),
        "gateway must not open a TCP listener: {}",
        String::from_utf8_lossy(&listeners.stdout)
    );
    assert!(tokio::process::Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .status()
        .await
        .unwrap()
        .success());
    let exit = tokio::time::timeout(Duration::from_secs(5), child.wait())
        .await
        .unwrap()
        .unwrap();
    assert!(exit.success(), "gateway exit: {exit}");
    assert!(
        !path.exists(),
        "graceful SIGTERM must retire the owned Unix socket"
    );
    assert!(client
        .get("http://gateway.test/proof")
        .send()
        .await
        .is_err());
}
