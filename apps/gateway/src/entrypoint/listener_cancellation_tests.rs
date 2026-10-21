#![cfg(test)]
//! HTTP/1 FIN is cancellation. Connection: close alone receives a complete response.
use super::*;
const POST_BODY: &str = r#"{"model":"fixture","messages":[]}"#;
async fn blocked_post_backend() -> (
    SocketAddr,
    tokio::sync::oneshot::Receiver<()>,
    tokio::sync::oneshot::Sender<()>,
    tokio::sync::oneshot::Receiver<()>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = tokio::sync::oneshot::channel();
    let (closed_tx, closed_rx) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = Vec::new();
        loop {
            let mut bytes = [0u8; 1024];
            let count = socket.read(&mut bytes).await.unwrap();
            assert!(count > 0);
            request.extend_from_slice(&bytes[..count]);
            assert!(request.len() < 16 * 1024);
            if request
                .windows(4)
                .position(|window| window == b"\r\n\r\n")
                .is_some_and(|end| request.len() >= end + 4 + POST_BODY.len())
            {
                break;
            }
        }
        let _ = started_tx.send(());
        let mut bytes = [0u8; 64];
        tokio::select! {
            _ = release_rx => {
                let body = r#"{"ok":true}"#;
                let response = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len());
                let _ = socket.write_all(response.as_bytes()).await;
            }
            result = socket.read(&mut bytes) => { assert!(matches!(result, Ok(0) | Err(_))); }
        }
        let _ = closed_tx.send(());
    });
    (address, started_rx, release_tx, closed_rx)
}
async fn post_socket(address: SocketAddr) -> tokio::net::TcpStream {
    let mut socket = tokio::net::TcpStream::connect(address).await.unwrap();
    let request = format!("POST /v1/chat/completions HTTP/1.1\r\nHost: {address}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{POST_BODY}",POST_BODY.len());
    socket.write_all(request.as_bytes()).await.unwrap();
    socket
}
#[tokio::test]
async fn complete_post_connection_close_receives_delayed_response() {
    let (backend, started, release, _closed) = blocked_post_backend().await;
    let mut config = routed_config(backend);
    config
        .services
        .get_mut("test-service")
        .unwrap()
        .load_balancer
        .request_timeout = "10s".into();
    let (logs, _) = tokio::sync::mpsc::unbounded_channel();
    let (address, shutdown, task) =
        start_test_entrypoint(gateway_state(&config, logs, false)).await;
    let mut socket = post_socket(address).await;
    tokio::time::timeout(Duration::from_secs(2), started)
        .await
        .unwrap()
        .unwrap();
    release.send(()).unwrap();
    let mut response = Vec::new();
    tokio::time::timeout(Duration::from_secs(2), socket.read_to_end(&mut response))
        .await
        .unwrap()
        .unwrap();
    assert!(response.starts_with(b"HTTP/1.1 200 OK\r\n"));
    assert!(response.ends_with(br#"{"ok":true}"#));
    stop_test_entrypoint(shutdown, task).await;
}
#[tokio::test]
async fn write_half_close_cancels_completed_post_without_running_to_deadline() {
    let (backend, started, _release, closed) = blocked_post_backend().await;
    let mut config = routed_config(backend);
    config
        .services
        .get_mut("test-service")
        .unwrap()
        .load_balancer
        .request_timeout = "10s".into();
    let (logs, _) = tokio::sync::mpsc::unbounded_channel();
    let (address, shutdown, task) =
        start_test_entrypoint(gateway_state(&config, logs, false)).await;
    let mut socket = post_socket(address).await;
    tokio::time::timeout(Duration::from_secs(2), started)
        .await
        .unwrap()
        .unwrap();
    socket.shutdown().await.unwrap();
    tokio::time::timeout(Duration::from_secs(2), closed)
        .await
        .expect("FIN must cancel pending upstream before the 10s deadline")
        .unwrap();
    let mut response = Vec::new();
    let _ = tokio::time::timeout(Duration::from_secs(2), socket.read_to_end(&mut response))
        .await
        .unwrap();
    assert!(
        !response.starts_with(b"HTTP/1.1 200"),
        "HTTP/1 write-half-close is cancellation, not a supported response-wait protocol"
    );
    stop_test_entrypoint(shutdown, task).await;
}
async fn sse_close_after_first_frame(reset: bool) {
    let (backend, started, disconnected) =
        super::super::inference_tests::spawn_streaming_backend().await;
    let config = routed_config(backend);
    let (logs, _) = tokio::sync::mpsc::unbounded_channel();
    let state = gateway_state(&config, logs, false);
    let metrics = state.metrics.clone();
    let (address, shutdown, task) = start_test_entrypoint(state).await;
    let mut socket = tokio::net::TcpStream::connect(address).await.unwrap();
    socket.write_all(format!("GET /events HTTP/1.1\r\nHost: {address}\r\nAccept: text/event-stream\r\nConnection: close\r\n\r\n").as_bytes()).await.unwrap();
    tokio::time::timeout(Duration::from_secs(2), started)
        .await
        .unwrap()
        .unwrap();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    let mut response = Vec::new();
    while !response.windows(5).any(|bytes| bytes == b"data:") {
        let mut bytes = [0u8; 1024];
        let count = tokio::time::timeout_at(deadline, socket.read(&mut bytes))
            .await
            .unwrap()
            .unwrap();
        assert!(count > 0);
        response.extend_from_slice(&bytes[..count]);
        assert!(response.len() < 64 * 1024);
    }
    if reset {
        // A zero linger requests an immediate RST; unlike positive linger it cannot block.
        #[allow(deprecated)]
        socket.set_linger(Some(Duration::ZERO)).unwrap();
    }
    drop(socket);
    tokio::time::timeout(Duration::from_secs(2), disconnected)
        .await
        .expect("FIN/RST after first SSE frame must drop upstream")
        .unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if metrics
                .render_prometheus()
                .contains("gateway_service_active_requests{service=\"test-service\"} 0")
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    stop_test_entrypoint(shutdown, task).await;
}
#[tokio::test]
async fn fin_after_first_sse_frame_releases_upstream() {
    sse_close_after_first_frame(false).await;
}
#[tokio::test]
async fn rst_after_first_sse_frame_releases_upstream() {
    sse_close_after_first_frame(true).await;
}
