#![cfg(test)]

use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[tokio::test]
async fn single_hop_is_explicit_and_the_shared_mcp_builder_preserves_redirects() {
    let clients = {
        let _guard = tests::proxy_env_lock().lock().unwrap();
        tests::clear_proxy_env();
        [
            (
                ReqwestHttpClient::without_redirects().unwrap().client,
                false,
            ),
            (build_reqwest_client(None, None).unwrap(), true),
        ]
    };
    for (client, follows) in clients {
        let destination = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}/final", destination.local_addr().unwrap());
        let calls = Arc::new(AtomicUsize::new(0));
        let observed = calls.clone();
        let target = tokio::spawn(async move {
            let (mut socket, _) = destination.accept().await.unwrap();
            let mut bytes = [0; 4096];
            socket.read(&mut bytes).await.unwrap();
            observed.fetch_add(1, Ordering::SeqCst);
            socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                .await
                .unwrap();
        });
        let source = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let initial = format!("http://{}/initial", source.local_addr().unwrap());
        let redirect = tokio::spawn(async move {
            let (mut socket, _) = source.accept().await.unwrap();
            let mut bytes = [0; 4096];
            socket.read(&mut bytes).await.unwrap();
            socket.write_all(format!("HTTP/1.1 307 Temporary Redirect\r\nLocation: {endpoint}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").as_bytes()).await.unwrap();
        });
        let response = tokio::time::timeout(Duration::from_secs(3), client.get(initial).send())
            .await
            .unwrap()
            .unwrap();
        redirect.await.unwrap();
        target.abort();
        let _ = target.await;
        assert_eq!(response.status().as_u16(), if follows { 200 } else { 307 });
        assert_eq!(calls.load(Ordering::SeqCst), usize::from(follows));
    }
}

#[test]
fn single_hop_construction_does_not_fall_back_after_invalid_proxy_configuration() {
    let result = {
        let _guard = tests::proxy_env_lock().lock().unwrap();
        tests::clear_proxy_env();
        unsafe { env::set_var("HTTP_PROXY", "http://[") };
        let result = ReqwestHttpClient::without_redirects();
        tests::clear_proxy_env();
        result
    };
    assert!(
        result.is_err(),
        "strict endpoint callers must get the construction refusal"
    );
}
