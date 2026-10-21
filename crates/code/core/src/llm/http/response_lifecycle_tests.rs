use super::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

async fn stalled_response(
    status: u16,
) -> (
    String,
    tokio::sync::oneshot::Receiver<()>,
    tokio::task::JoinHandle<()>,
) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (sent, received) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut bytes = [0; 8192];
        assert!(socket.read(&mut bytes).await.unwrap() > 0);
        socket.write_all(format!("HTTP/1.1 {status} Fixture\r\nContent-Length: 1024\r\nConnection: close\r\n\r\n{{").as_bytes()).await.unwrap();
        sent.send(()).unwrap();
        while socket.read(&mut bytes).await.unwrap_or(0) > 0 {}
    });
    (format!("http://{address}"), received, server)
}

async fn cancellation_after_headers(streaming: bool, raw: bool) {
    let _guard = super::tests::proxy_env_lock().lock().unwrap();
    let (url, received, server) = stalled_response(if streaming { 401 } else { 200 }).await;
    let client = ReqwestHttpClient {
        client: reqwest::Client::builder().no_proxy().build().unwrap(),
        request_timeout: None,
    };
    let cancel = CancellationToken::new();
    let request_cancel = cancel.clone();
    let request = async {
        if raw {
            client
                .post_bytes_streaming(&url, vec![], b"{}", request_cancel)
                .await
                .map(|_| ())
        } else if streaming {
            client
                .post_streaming(&url, vec![], &serde_json::json!({}), request_cancel)
                .await
                .map(|_| ())
        } else {
            client
                .post(&url, vec![], &serde_json::json!({}), request_cancel)
                .await
                .map(|_| ())
        }
    };
    let cancelled = async {
        received.await.unwrap();
        // Let the client receive the headers and enter the unfinished body read.
        tokio::time::sleep(Duration::from_millis(100)).await;
        cancel.cancel();
    };
    let (result, ()) = tokio::join!(
        tokio::time::timeout(Duration::from_secs(1), request),
        cancelled
    );
    server.abort();
    let error = result
        .expect("cancellation must interrupt an unfinished response body")
        .unwrap_err();
    assert!(matches!(
        error.downcast_ref::<HttpClientError>(),
        Some(HttpClientError::Cancelled { .. })
    ));
    assert!(!is_retryable_http_failure(&error));
}

#[tokio::test]
async fn cancellation_interrupts_non_streaming_body() {
    cancellation_after_headers(false, false).await;
}

#[tokio::test]
async fn cancellation_interrupts_streaming_error_body() {
    cancellation_after_headers(true, false).await;
}

#[tokio::test]
async fn cancellation_interrupts_raw_streaming_error_body() {
    cancellation_after_headers(true, true).await;
}

#[tokio::test]
async fn stalled_error_body_preserves_non_retryable_http_status() {
    let _guard = super::tests::proxy_env_lock().lock().unwrap();
    for raw in [false, true] {
        let (url, _received, server) = stalled_response(401).await;
        let client = ReqwestHttpClient {
            client: reqwest::Client::builder().no_proxy().build().unwrap(),
            request_timeout: Some(Duration::from_millis(50)),
        };
        let result = tokio::time::timeout(Duration::from_secs(1), async {
            if raw {
                client
                    .post_bytes_streaming(&url, vec![], b"{}", CancellationToken::new())
                    .await
            } else {
                client
                    .post_streaming(
                        &url,
                        vec![],
                        &serde_json::json!({}),
                        CancellationToken::new(),
                    )
                    .await
            }
        })
        .await;
        server.abort();
        let response = result.expect("error body must have a deadline").unwrap();
        assert_eq!(response.status, 401);
        assert!(
            response.error_body.contains("timed out"),
            "{}",
            response.error_body
        );
    }
}
