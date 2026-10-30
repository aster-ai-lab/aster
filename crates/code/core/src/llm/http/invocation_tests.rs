#![cfg(test)]

use super::*;
use crate::llm::{create_client_with_config, LlmConfig, Message, RetryConfig, StreamEvent};
use serde_json::json;
use std::sync::atomic::{AtomicUsize, Ordering};
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

#[tokio::test]
async fn provider_headers_keep_one_request_identity_across_retry_and_new_identity_for_the_next_call(
) {
    for (provider, streaming) in [
        ("openai", false),
        ("openai", true),
        ("anthropic", false),
        ("anthropic", true),
    ] {
        let server = MockServer::start().await;
        let calls = Arc::new(AtomicUsize::new(0));
        Mock::given(method("POST")).respond_with(move |_request: &Request| {
            if calls.fetch_add(1, Ordering::SeqCst) == 0 { return ResponseTemplate::new(503).set_body_json(json!({"error":{"message":"retry"}})); }
            if streaming {
                ResponseTemplate::new(200).insert_header("content-type","text/event-stream").set_body_string(if provider == "openai" {
                    "data: {\"choices\":[{\"delta\":{\"content\":\"ok\"},\"finish_reason\":null}]}\n\ndata: {\"choices\":[],\"usage\":{\"prompt_tokens\":1,\"completion_tokens\":1,\"total_tokens\":2}}\n\ndata: [DONE]\n\n"
                } else {
                    "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"message-a\",\"model\":\"model-a\",\"usage\":{\"input_tokens\":1,\"output_tokens\":0}}}\n\nevent: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\nevent: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"ok\"}}\n\nevent: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":1}}\n\nevent: message_stop\ndata: {\"type\":\"message_stop\"}\n\n"
                })
            } else if provider == "openai" {
                ResponseTemplate::new(200).set_body_json(json!({"model":"model-a","choices":[{"message":{"role":"assistant","content":"ok"},"finish_reason":"stop"}],"usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}}))
            } else {
                ResponseTemplate::new(200).set_body_json(json!({"id":"message-a","model":"model-a","type":"message","role":"assistant","content":[{"type":"text","text":"ok"}],"stop_reason":"end_turn","usage":{"input_tokens":1,"output_tokens":1}}))
            }
        }).mount(&server).await;
        let mut config = LlmConfig::new(provider, "model-a", "synthetic-host-key")
            .with_base_url(server.uri())
            .with_headers(
                [
                    ("authorization".into(), "Bearer synthetic-host-key".into()),
                    ("x-aster-source".into(), "aster-chat".into()),
                    ("x-aster-session".into(), "session-a".into()),
                    ("x-aster-request-id".into(), "stale-id".into()),
                ]
                .into_iter()
                .collect(),
            )
            .with_retry_config(RetryConfig {
                max_retries: 1,
                base_delay_ms: 0,
                max_delay_ms: 0,
                ..Default::default()
            });
        config.http_override = Some(Arc::new(ReqwestHttpClient {
            client: reqwest::Client::builder().no_proxy().build().unwrap(),
            request_timeout: Some(Duration::from_secs(5)),
        }));
        let client = create_client_with_config(config);
        for _ in 0..2 {
            if streaming {
                let mut receiver = client
                    .complete_streaming(
                        &[Message::user("synthetic prompt")],
                        None,
                        &[],
                        CancellationToken::new(),
                    )
                    .await
                    .unwrap();
                let mut done = false;
                while let Some(event) = receiver.recv().await {
                    if let StreamEvent::Done(response) = event {
                        assert_eq!(response.message.text(), "ok");
                        done = true;
                    }
                }
                assert!(done);
            } else {
                assert_eq!(
                    client
                        .complete(&[Message::user("synthetic prompt")], None, &[])
                        .await
                        .unwrap()
                        .message
                        .text(),
                    "ok"
                );
            }
        }
        let received = server.received_requests().await.unwrap();
        assert_eq!(received.len(), 3);
        let identity = |index: usize| {
            received[index]
                .headers
                .get("x-aster-request-id")
                .unwrap()
                .to_str()
                .unwrap()
        };
        assert_ne!(identity(0), "stale-id");
        assert_eq!(identity(0), identity(1));
        assert_ne!(identity(1), identity(2));
        for request in &received {
            assert_eq!(
                request.headers["authorization"],
                "Bearer synthetic-host-key"
            );
            assert_eq!(request.headers["x-aster-source"], "aster-chat");
            assert_eq!(request.headers["x-aster-session"], "session-a");
        }
    }
}
