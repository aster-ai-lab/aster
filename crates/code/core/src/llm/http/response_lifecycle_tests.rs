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

#[expect(
    clippy::await_holding_lock,
    reason = "Each test owns a separate Tokio runtime; serialize process-global proxy settings"
)]
async fn cancellation_after_headers(streaming: bool, raw: bool) {
    let _guard = super::tests::proxy_env_lock().lock().unwrap();
    let (url, received, server) = stalled_response(if streaming { 503 } else { 200 }).await;
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
#[expect(
    clippy::await_holding_lock,
    reason = "Each test owns a separate Tokio runtime; serialize process-global proxy settings"
)]
async fn stalled_retryable_error_body_has_a_deadline() {
    let _guard = super::tests::proxy_env_lock().lock().unwrap();
    for raw in [false, true] {
        let (url, _received, server) = stalled_response(503).await;
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
        assert_eq!(response.status, 503);
        assert!(
            response.error_body.contains("timed out"),
            "{}",
            response.error_body
        );
    }
}

#[tokio::test]
#[expect(
    clippy::await_holding_lock,
    reason = "Each test owns a separate Tokio runtime; serialize process-global proxy settings"
)]
async fn authorization_headers_return_before_an_outer_model_deadline() {
    let _guard = super::tests::proxy_env_lock().lock().unwrap();
    for status in [401, 403] {
        for mode in 0..3 {
            let (url, _received, server) = stalled_response(status).await;
            let client = ReqwestHttpClient {
                client: reqwest::Client::builder().no_proxy().build().unwrap(),
                request_timeout: None,
            };
            let result = tokio::time::timeout(Duration::from_millis(200), async {
                match mode {
                    0 => client
                        .post(
                            &url,
                            vec![],
                            &serde_json::json!({}),
                            CancellationToken::new(),
                        )
                        .await
                        .map(|response| (response.status, response.body)),
                    1 => client
                        .post_streaming(
                            &url,
                            vec![],
                            &serde_json::json!({}),
                            CancellationToken::new(),
                        )
                        .await
                        .map(|response| (response.status, response.error_body)),
                    _ => client
                        .post_bytes_streaming(&url, vec![], b"{}", CancellationToken::new())
                        .await
                        .map(|response| (response.status, response.error_body)),
                }
            })
            .await;
            server.abort();
            let (returned_status, body) = result
                .expect("known authorization failures must not become model timeouts")
                .unwrap();
            assert_eq!(returned_status, status);
            assert!(body.contains("authorization"), "{body}");
        }
    }
}

#[tokio::test]
#[expect(
    clippy::await_holding_lock,
    reason = "Each test owns a separate Tokio runtime; serialize process-global proxy settings"
)]
async fn complete_authorization_body_keeps_the_provider_diagnostic() {
    let _guard = super::tests::proxy_env_lock().lock().unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = [0; 8192];
        assert!(socket.read(&mut request).await.unwrap() > 0);
        let body = r#"{"error":{"message":"fixture authorization detail"}}"#;
        socket.write_all(format!("HTTP/1.1 401 Unauthorized\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await.unwrap();
    });
    let client = ReqwestHttpClient {
        client: reqwest::Client::builder().no_proxy().build().unwrap(),
        request_timeout: None,
    };
    let response = client
        .post_streaming(
            &format!("http://{address}"),
            vec![],
            &serde_json::json!({}),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(response.status, 401);
    assert!(
        response.error_body.contains("fixture authorization detail"),
        "{}",
        response.error_body
    );
    server.await.unwrap();
}
