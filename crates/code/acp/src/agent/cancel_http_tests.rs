use super::*;
use acp::Agent as _;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stop_and_send_now_cancel_a_stalled_provider() {
    tokio::task::LocalSet::new().run_until(async {
        for send_now in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let base = format!("http://{}", listener.local_addr().unwrap());
            let requested = Arc::new(tokio::sync::Notify::new());
            let incoming = requested.clone();
            let closed = Arc::new(tokio::sync::Notify::new());
            let peer_closed = closed.clone();
            let (stop, mut stopped) = tokio::sync::oneshot::channel();
            let provider = tokio::spawn(async move {
                let mut count = 0;
                loop {
                    let (socket, _) = tokio::select! {
                        _ = &mut stopped => break,
                        incoming = listener.accept() => incoming.unwrap(),
                    };
                    let mut socket = tokio::io::BufReader::new(socket);
                    let mut length = 0;
                    loop {
                        let mut line = String::new();
                        assert_ne!(socket.read_line(&mut line).await.unwrap(), 0);
                        if line == "\r\n" { break; }
                        if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                            length = value.trim().parse().unwrap();
                        }
                    }
                    let mut body = vec![0; length];
                    socket.read_exact(&mut body).await.unwrap();
                    count += 1;
                    if count == 1 {
                        incoming.notify_one();
                        let mut byte = [0];
                        assert_eq!(socket.read(&mut byte).await.unwrap(), 0, "cancellation must close the stalled HTTP request");
                        peer_closed.notify_one();
                        continue;
                    }
                    let body = [
                        serde_json::json!({"type":"message_start","message":{"id":"fixture","usage":{"input_tokens":1,"output_tokens":0}}}),
                        serde_json::json!({"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}),
                        serde_json::json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"READY"}}),
                        serde_json::json!({"type":"content_block_stop","index":0}),
                        serde_json::json!({"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":1}}),
                        serde_json::json!({"type":"message_stop"}),
                    ].into_iter().map(|event| format!("data: {event}\n\n")).collect::<String>();
                    let response = format!("HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
                    socket.get_mut().write_all(response.as_bytes()).await.unwrap();
                }
                count
            });
            let mut launch = tests::test_launch(Some(root.path().join("sessions")));
            launch.workspace = root.path().into();
            launch.config.providers[0].base_url = Some(base);
            launch.config.memory = Some(aster_core::memory::MemoryConfig { llm_extraction: false, ..Default::default() });
            let host = AsterCodeAgent::new(launch);
            let id = host.new_session(acp::NewSessionRequest::new(root.path())).await.unwrap().session_id;
            let prompt = |text: &str| acp::PromptRequest::new(id.clone(), vec![acp::ContentBlock::Text(acp::TextContent::new(text))]);
            let original = host.prompt(prompt("first stalled request"));
            tokio::pin!(original);
            tokio::time::timeout(Duration::from_secs(5), async {
                tokio::select! {
                    _ = requested.notified() => {},
                    result = &mut original => panic!("provider must still be stalled: {result:?}"),
                }
            }).await.unwrap();
            if send_now {
                let next = host.prompt(prompt("next prompt").meta(serde_json::json!({"sendNow":true}).as_object().cloned()));
                let (old, next) = tokio::time::timeout(Duration::from_secs(5), async { tokio::join!(original, next) }).await.expect("send now must retire the stalled request and admit its replacement");
                assert_eq!(old.unwrap().stop_reason, acp::StopReason::Cancelled);
                assert_eq!(next.unwrap().stop_reason, acp::StopReason::EndTurn);
            } else {
                let registry = host.sessions.lock().await;
                let cancel = host.cancel(acp::CancelNotification::new(id.clone()));
                tokio::pin!(cancel);
                tokio::time::timeout(Duration::from_secs(3), async {
                    tokio::select! {
                        _ = closed.notified() => {},
                        result = &mut cancel => panic!("registry cleanup must still wait: {result:?}"),
                        result = &mut original => panic!("public cleanup must still wait: {result:?}"),
                    }
                }).await.expect("the exact run must cancel its provider without waiting for the session registry");
                drop(registry);
                let (old, stopped) = tokio::time::timeout(Duration::from_secs(3), async {
                    tokio::join!(original, cancel)
                }).await.expect("stop must return while the provider is stalled");
                stopped.unwrap();
                assert_eq!(old.unwrap().stop_reason, acp::StopReason::Cancelled);
                assert_eq!(tokio::time::timeout(Duration::from_secs(5), host.prompt(prompt("next prompt"))).await.unwrap().unwrap().stop_reason, acp::StopReason::EndTurn);
            }
            host.sessions.lock().await[id.0.as_ref()].session.close().await;
            stop.send(()).unwrap();
            assert!(provider.await.unwrap() >= 2);
        }
    }).await;
}
