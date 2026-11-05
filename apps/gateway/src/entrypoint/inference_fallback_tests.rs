use super::inference_identity_tests::{
    next_log, raw_header, response_request_id, state_with_access_log, uuid_v4,
};
use super::inference_tests::{
    inference_config, inference_key, read_http_request, spawn_streaming_backend,
    start_test_entrypoint, stop_test_entrypoint,
};
use crate::config::{InferenceTargetConfig, ServerConfig};
use crate::inference::{ATTEMPT_ID_HEADER, REQUEST_ID_HEADER};
use chrono::{Duration as ChronoDuration, Utc};
use futures_util::StreamExt;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use uuid::Uuid;

fn add_fallback_target(
    config: &mut crate::config::GatewayConfig,
    fallback_backend: SocketAddr,
) -> (Uuid, Uuid) {
    let mut fallback_service = config.services["model-service"].clone();
    fallback_service.load_balancer.servers = vec![ServerConfig {
        url: format!("http://{fallback_backend}"),
        weight: 1,
        target: None,
    }];
    config
        .services
        .insert("fallback-service".into(), fallback_service);

    let route = config
        .inference
        .as_mut()
        .unwrap()
        .routes
        .values_mut()
        .next()
        .unwrap();
    let model = route.models.get_mut("allowed-model").unwrap();
    model.targets[0].upstream_model = "primary-upstream".into();
    let primary_target_id = model.targets[0].target_id;
    let fallback_target_id = Uuid::new_v4();
    model.targets.push(InferenceTargetConfig {
        target_id: fallback_target_id,
        service: "fallback-service".into(),
        upstream_model: "fallback-upstream".into(),
        priority: 1,
        weight: 1,
        protocol: Default::default(),
        api_key_env: None,
        translate: false,
        forward_client_identity: false,
    });
    (primary_target_id, fallback_target_id)
}

async fn spawn_response_backend(
    status: u16,
    content_type: &'static str,
    body: &'static str,
) -> (SocketAddr, tokio::sync::oneshot::Receiver<Vec<u8>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (request_tx, request_rx) = tokio::sync::oneshot::channel();

    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let request = read_http_request(&mut stream).await;
        let _ = request_tx.send(request);
        let response = format!(
            "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nContent-Type: {content_type}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = stream.write_all(response.as_bytes()).await;
        let _ = stream.shutdown().await;
    });

    (address, request_rx)
}

async fn spawn_drop_before_response_backend(
) -> (SocketAddr, tokio::sync::oneshot::Receiver<Vec<u8>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (request_tx, request_rx) = tokio::sync::oneshot::channel();

    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let request = read_http_request(&mut stream).await;
        let _ = request_tx.send(request);
        let _ = stream.shutdown().await;
    });

    (address, request_rx)
}

async fn spawn_partial_response_backend() -> (SocketAddr, tokio::sync::oneshot::Receiver<Vec<u8>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (request_tx, request_rx) = tokio::sync::oneshot::channel();

    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let request = read_http_request(&mut stream).await;
        let _ = request_tx.send(request);
        let response = "HTTP/1.1 200 OK\r\nContent-Length: 100\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{}";
        let _ = stream.write_all(response.as_bytes()).await;
        let _ = stream.shutdown().await;
    });

    (address, request_rx)
}

async fn spawn_hanging_backend() -> (SocketAddr, tokio::sync::oneshot::Receiver<Vec<u8>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (request_tx, request_rx) = tokio::sync::oneshot::channel();

    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let request = read_http_request(&mut stream).await;
        let _ = request_tx.send(request);
        tokio::time::sleep(Duration::from_secs(2)).await;
        let _ = stream.shutdown().await;
    });

    (address, request_rx)
}

fn request_model(request: &[u8]) -> String {
    let request = String::from_utf8(request.to_vec()).unwrap();
    let body_offset = request.find("\r\n\r\n").unwrap() + 4;
    serde_json::from_str::<serde_json::Value>(&request[body_offset..]).unwrap()["model"]
        .as_str()
        .unwrap()
        .to_string()
}

fn request_identity(request: &[u8]) -> (Uuid, Uuid) {
    let request = String::from_utf8(request.to_vec()).unwrap();
    (
        uuid_v4(raw_header(&request, REQUEST_ID_HEADER).unwrap()),
        uuid_v4(raw_header(&request, ATTEMPT_ID_HEADER).unwrap()),
    )
}

#[tokio::test]
async fn managed_http_falls_back_after_pre_response_transport_failure() {
    let key = inference_key('a');
    let (primary, primary_request) = spawn_drop_before_response_backend().await;
    let (fallback, fallback_request) =
        spawn_response_backend(200, "application/json", r#"{"source":"fallback"}"#).await;
    let mut config = inference_config(primary, &key, Utc::now() + ChronoDuration::hours(1));
    let (_, fallback_target_id) = add_fallback_target(&mut config, fallback);
    config.validate().unwrap();
    let (state, mut log_rx) = state_with_access_log(&config);
    let (address, shutdown_tx, handle) = start_test_entrypoint(state).await;

    let response = reqwest::Client::new()
        .post(format!("http://{address}/v1/chat/completions"))
        .bearer_auth(&key)
        .header("content-type", "application/json")
        .body(r#"{"model":"allowed-model","messages":[]}"#)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let response_request_id = response_request_id(&response);
    assert_eq!(
        response.json::<serde_json::Value>().await.unwrap()["data"]["source"],
        "fallback"
    );

    let primary_request = primary_request.await.unwrap();
    let fallback_request = fallback_request.await.unwrap();
    assert_eq!(request_model(&primary_request), "primary-upstream");
    assert_eq!(request_model(&fallback_request), "fallback-upstream");
    let (primary_request_id, primary_attempt_id) = request_identity(&primary_request);
    let (fallback_request_id, fallback_attempt_id) = request_identity(&fallback_request);
    assert_eq!(primary_request_id, response_request_id);
    assert_eq!(fallback_request_id, response_request_id);
    assert_ne!(primary_attempt_id, fallback_attempt_id);

    let entry = next_log(&mut log_rx).await;
    let inference = entry.inference.expect("inference access context");
    assert_eq!(inference.request_id, response_request_id);
    assert_eq!(inference.attempt_id, Some(fallback_attempt_id));
    assert_eq!(inference.target_id, Some(fallback_target_id));
    let expected_backend = format!("http://{fallback}");
    assert_eq!(entry.backend.as_deref(), Some(expected_backend.as_str()));

    stop_test_entrypoint(shutdown_tx, handle).await;
}

#[tokio::test]
async fn cross_protocol_fallback_reselects_each_attempt_path_and_body() {
    use crate::config::InferenceProtocol::{Anthropic, Openai};

    const CHAT_JSON: &str = r#"{"id":"c1","model":"fallback-upstream","choices":[{"index":0,"message":{"role":"assistant","content":"ok"},"finish_reason":"stop"}],"usage":{"prompt_tokens":13,"completion_tokens":8,"total_tokens":21}}"#;
    const ANTHROPIC_JSON: &str = r#"{"id":"m1","type":"message","role":"assistant","model":"fallback-upstream","content":[{"type":"text","text":"ok"}],"stop_reason":"end_turn","usage":{"input_tokens":13,"output_tokens":8}}"#;
    const CHAT_SSE: &str = "data: {\"id\":\"c1\",\"model\":\"fallback-upstream\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"ok\"},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":13,\"completion_tokens\":8,\"total_tokens\":21}}\n\ndata: [DONE]\n\n";
    const ANTHROPIC_SSE: &str = "data: {\"type\":\"message_start\",\"message\":{\"id\":\"m1\",\"model\":\"fallback-upstream\",\"usage\":{\"input_tokens\":13,\"output_tokens\":0}}}\n\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"ok\"}}\n\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":8}}\n\ndata: {\"type\":\"message_stop\"}\n\n";
    for stream in [false, true] {
        for (
            primary_protocol,
            fallback_protocol,
            primary_path,
            fallback_path,
            json_body,
            sse_body,
        ) in [
            (
                Openai,
                Anthropic,
                "/v1/chat/completions",
                "/v1/messages",
                ANTHROPIC_JSON,
                ANTHROPIC_SSE,
            ),
            (
                Anthropic,
                Openai,
                "/v1/messages",
                "/v1/chat/completions",
                CHAT_JSON,
                CHAT_SSE,
            ),
        ] {
            let key = inference_key('p');
            let (primary, primary_request) = spawn_drop_before_response_backend().await;
            let body = if stream { sse_body } else { json_body };
            let content_type = if stream {
                "text/event-stream"
            } else {
                "application/json"
            };
            let (fallback, fallback_request) =
                spawn_response_backend(200, content_type, body).await;
            let mut config = inference_config(primary, &key, Utc::now() + ChronoDuration::hours(1));
            add_fallback_target(&mut config, fallback);
            for (service, prefix) in [
                ("model-service", "/primary"),
                ("fallback-service", "/secondary"),
            ] {
                config
                    .services
                    .get_mut(service)
                    .unwrap()
                    .load_balancer
                    .servers[0]
                    .url
                    .push_str(prefix);
            }
            let model = config
                .inference
                .as_mut()
                .unwrap()
                .routes
                .values_mut()
                .next()
                .unwrap()
                .models
                .get_mut("allowed-model")
                .unwrap();
            model.targets[0].protocol = primary_protocol;
            model.targets[0].translate = true;
            model.targets[1].protocol = fallback_protocol;
            model.targets[1].translate = true;
            config.validate().unwrap();
            let (address, shutdown_tx, handle) =
                start_test_entrypoint(super::inference_tests::gateway_state(&config)).await;
            let response = reqwest::Client::new()
                .post(format!("http://{address}/v1/chat/completions?trace=two%20words"))
                .bearer_auth(&key)
                .json(&serde_json::json!({"model":"allowed-model","messages":[{"role":"user","content":"hello"}],"max_tokens":64,"stream":stream}))
                .send().await.unwrap();
            assert_eq!(response.status(), 200);
            if stream {
                let text = response.text().await.unwrap();
                let data = text
                    .lines()
                    .filter_map(|line| line.strip_prefix("data: "))
                    .collect::<Vec<_>>();
                assert_eq!(data.iter().filter(|line| **line == "[DONE]").count(), 1);
                let events = data
                    .iter()
                    .filter(|line| **line != "[DONE]")
                    .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
                    .collect::<Vec<_>>();
                assert_eq!(
                    events
                        .iter()
                        .filter_map(|event| event["choices"][0]["delta"]["content"].as_str())
                        .collect::<String>(),
                    "ok"
                );
                assert_eq!(
                    events.last().unwrap()["choices"][0]["finish_reason"],
                    "stop"
                );
                assert_eq!(events.last().unwrap()["usage"]["prompt_tokens"], 13);
                assert_eq!(events.last().unwrap()["usage"]["completion_tokens"], 8);
            } else {
                let body: serde_json::Value = response.json().await.unwrap();
                assert_eq!(body["data"]["choices"][0]["message"]["content"], "ok");
                assert_eq!(body["data"]["choices"][0]["finish_reason"], "stop");
                assert_eq!(body["data"]["usage"]["prompt_tokens"], 13);
                assert_eq!(body["data"]["usage"]["completion_tokens"], 8);
            }
            let primary_request = primary_request.await.unwrap();
            let fallback_request = fallback_request.await.unwrap();
            assert_eq!(
                String::from_utf8_lossy(&primary_request)
                    .lines()
                    .next()
                    .unwrap(),
                format!("POST /primary{primary_path}?trace=two%20words HTTP/1.1")
            );
            assert_eq!(
                String::from_utf8_lossy(&fallback_request)
                    .lines()
                    .next()
                    .unwrap(),
                format!("POST /secondary{fallback_path}?trace=two%20words HTTP/1.1")
            );
            assert_eq!(request_model(&primary_request), "primary-upstream");
            assert_eq!(request_model(&fallback_request), "fallback-upstream");
            let (request_id, attempt_id) = request_identity(&primary_request);
            let (fallback_request_id, fallback_attempt_id) = request_identity(&fallback_request);
            assert_eq!(request_id, fallback_request_id);
            assert_ne!(attempt_id, fallback_attempt_id);
            stop_test_entrypoint(shutdown_tx, handle).await;
        }
    }
}

#[tokio::test]
async fn managed_http_falls_back_after_first_response_timeout() {
    let key = inference_key('a');
    let (primary, primary_request) = spawn_hanging_backend().await;
    let (fallback, fallback_request) =
        spawn_response_backend(200, "application/json", r#"{"source":"fallback"}"#).await;
    let mut config = inference_config(primary, &key, Utc::now() + ChronoDuration::hours(1));
    add_fallback_target(&mut config, fallback);
    config
        .services
        .get_mut("model-service")
        .unwrap()
        .load_balancer
        .request_timeout = "50ms".into();
    config.validate().unwrap();
    let (address, shutdown_tx, handle) =
        start_test_entrypoint(super::inference_tests::gateway_state(&config)).await;

    let response = reqwest::Client::new()
        .post(format!("http://{address}/v1/chat/completions"))
        .bearer_auth(&key)
        .header("content-type", "application/json")
        .body(r#"{"model":"allowed-model","messages":[]}"#)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let _ = response.bytes().await.unwrap();

    let primary_request = primary_request.await.unwrap();
    let fallback_request = fallback_request.await.unwrap();
    let (_, primary_attempt_id) = request_identity(&primary_request);
    let (_, fallback_attempt_id) = request_identity(&fallback_request);
    assert_ne!(primary_attempt_id, fallback_attempt_id);

    stop_test_entrypoint(shutdown_tx, handle).await;
}

#[tokio::test]
async fn managed_http_does_not_fallback_after_an_upstream_status() {
    let key = inference_key('a');
    let (primary, primary_request) =
        spawn_response_backend(503, "application/json", r#"{"source":"primary"}"#).await;
    let (fallback, fallback_request) =
        spawn_response_backend(200, "application/json", r#"{"source":"fallback"}"#).await;
    let mut config = inference_config(primary, &key, Utc::now() + ChronoDuration::hours(1));
    let (primary_target_id, _) = add_fallback_target(&mut config, fallback);
    config.validate().unwrap();
    let (state, mut log_rx) = state_with_access_log(&config);
    let (address, shutdown_tx, handle) = start_test_entrypoint(state).await;

    let response = reqwest::Client::new()
        .post(format!("http://{address}/v1/chat/completions"))
        .bearer_auth(&key)
        .header("content-type", "application/json")
        .body(r#"{"model":"allowed-model","messages":[]}"#)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 503);
    assert_eq!(
        response.json::<serde_json::Value>().await.unwrap()["source"],
        "primary"
    );
    let _ = primary_request.await.unwrap();
    assert!(
        tokio::time::timeout(Duration::from_millis(150), fallback_request)
            .await
            .is_err()
    );

    let entry = next_log(&mut log_rx).await;
    assert_eq!(entry.inference.unwrap().target_id, Some(primary_target_id));

    stop_test_entrypoint(shutdown_tx, handle).await;
}

#[tokio::test]
async fn managed_http_does_not_fallback_after_upstream_response_start() {
    let key = inference_key('a');
    let (primary, primary_request) = spawn_partial_response_backend().await;
    let (fallback, fallback_request) =
        spawn_response_backend(200, "application/json", r#"{"source":"fallback"}"#).await;
    let mut config = inference_config(primary, &key, Utc::now() + ChronoDuration::hours(1));
    add_fallback_target(&mut config, fallback);
    config.validate().unwrap();
    let (address, shutdown_tx, handle) =
        start_test_entrypoint(super::inference_tests::gateway_state(&config)).await;

    let response = reqwest::Client::new()
        .post(format!("http://{address}/v1/chat/completions"))
        .bearer_auth(&key)
        .header("content-type", "application/json")
        .body(r#"{"model":"allowed-model","messages":[]}"#)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 502);
    assert_eq!(
        response.json::<serde_json::Value>().await.unwrap()["error_code"],
        "upstream.exhausted"
    );
    let _ = primary_request.await.unwrap();
    assert!(
        tokio::time::timeout(Duration::from_millis(150), fallback_request)
            .await
            .is_err()
    );

    stop_test_entrypoint(shutdown_tx, handle).await;
}

#[tokio::test]
async fn managed_sse_falls_back_only_before_the_upstream_response() {
    let key = inference_key('a');
    let (primary, primary_request) = spawn_drop_before_response_backend().await;
    let (fallback, fallback_request) =
        spawn_response_backend(200, "text/event-stream", "data: hello\n\ndata: [DONE]\n\n").await;
    let mut config = inference_config(primary, &key, Utc::now() + ChronoDuration::hours(1));
    add_fallback_target(&mut config, fallback);
    config.validate().unwrap();
    let (address, shutdown_tx, handle) =
        start_test_entrypoint(super::inference_tests::gateway_state(&config)).await;

    let response = reqwest::Client::new()
        .post(format!("http://{address}/v1/chat/completions"))
        .bearer_auth(&key)
        .header("content-type", "application/json")
        .header("accept", "text/event-stream")
        .body(r#"{"model":"allowed-model","messages":[],"stream":true}"#)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let response_request_id = response_request_id(&response);
    assert_eq!(
        response.text().await.unwrap(),
        "data: hello\n\ndata: [DONE]\n\n"
    );

    let primary_request = primary_request.await.unwrap();
    let fallback_request = fallback_request.await.unwrap();
    let (primary_request_id, primary_attempt_id) = request_identity(&primary_request);
    let (fallback_request_id, fallback_attempt_id) = request_identity(&fallback_request);
    assert_eq!(primary_request_id, response_request_id);
    assert_eq!(fallback_request_id, response_request_id);
    assert_ne!(primary_attempt_id, fallback_attempt_id);

    stop_test_entrypoint(shutdown_tx, handle).await;
}

#[tokio::test]
async fn managed_sse_does_not_fallback_after_stream_idle_timeout() {
    let key = inference_key('i');
    let (primary, stream_started, upstream_disconnected) = spawn_streaming_backend().await;
    let (fallback, fallback_request) =
        spawn_response_backend(200, "text/event-stream", "data: fallback\n\n").await;
    let mut config = inference_config(primary, &key, Utc::now() + ChronoDuration::hours(1));
    config
        .services
        .get_mut("model-service")
        .unwrap()
        .load_balancer
        .stream_idle_timeout = "25ms".into();
    config
        .services
        .get_mut("model-service")
        .unwrap()
        .load_balancer
        .stream_total_timeout = "1s".into();
    let (primary_target_id, _) = add_fallback_target(&mut config, fallback);
    config.validate().unwrap();
    let (state, mut log_rx) = state_with_access_log(&config);
    let (address, shutdown_tx, handle) = start_test_entrypoint(state).await;

    let response = reqwest::Client::new()
        .post(format!("http://{address}/v1/chat/completions"))
        .bearer_auth(&key)
        .header("content-type", "application/json")
        .body(r#"{"model":"allowed-model","messages":[],"stream":true}"#)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    stream_started.await.unwrap();
    let mut body = response.bytes_stream();
    assert!(body.next().await.unwrap().unwrap().starts_with(b"data:"));
    assert!(tokio::time::timeout(Duration::from_secs(2), body.next())
        .await
        .unwrap()
        .unwrap()
        .is_err());
    drop(body);

    tokio::time::timeout(Duration::from_secs(2), upstream_disconnected)
        .await
        .unwrap()
        .unwrap();
    assert!(
        tokio::time::timeout(Duration::from_millis(150), fallback_request)
            .await
            .is_err()
    );
    let entry = next_log(&mut log_rx).await;
    assert_eq!(entry.status, 200);
    assert!(entry.response_bytes > 0);
    assert_eq!(entry.inference.unwrap().target_id, Some(primary_target_id));

    stop_test_entrypoint(shutdown_tx, handle).await;
}

#[tokio::test]
async fn managed_fallback_persists_ordered_attempt_boundaries() {
    let key = inference_key('y');
    let (primary, primary_request) = spawn_drop_before_response_backend().await;
    let (fallback, fallback_request) =
        spawn_response_backend(200, "application/json", r#"{"source":"fallback"}"#).await;
    let mut config = inference_config(primary, &key, Utc::now() + ChronoDuration::hours(1));
    add_fallback_target(&mut config, fallback);
    config.validate().unwrap();
    let directory = tempfile::tempdir().unwrap();
    let (state, spool) =
        super::inference_usage_tests::usage_state(&config, directory.path(), 2 * 1024 * 1024).await;
    let (address, shutdown_tx, handle) = start_test_entrypoint(state).await;

    let response = reqwest::Client::new()
        .post(format!("http://{address}/v1/chat/completions"))
        .bearer_auth(&key)
        .header("content-type", "application/json")
        .body(r#"{"model":"allowed-model","messages":[]}"#)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let _ = response.bytes().await.unwrap();
    let _ = primary_request.await.unwrap();
    let _ = fallback_request.await.unwrap();

    let events = super::inference_usage_tests::lifecycle_events(&spool, 6).await;
    assert_eq!(
        events
            .iter()
            .map(|event| event["kind"].as_str().unwrap())
            .collect::<Vec<_>>(),
        [
            "request_started",
            "attempt_started",
            "attempt_terminal",
            "attempt_started",
            "attempt_terminal",
            "request_terminal"
        ]
    );
    assert_eq!(events[2]["outcome"], "fallback");
    assert_eq!(events[4]["outcome"], "succeeded");
    assert_eq!(events[5]["outcome"], "succeeded");
    assert_ne!(
        events[1]["attempt"]["attempt_id"],
        events[3]["attempt"]["attempt_id"]
    );

    stop_test_entrypoint(shutdown_tx, handle).await;
    spool.shutdown().await;
}

#[tokio::test]
async fn target_successor_runtime_routes_only_to_remaining_fallback_without_primary() {
    use super::inference_tests::{gateway_state, gateway_state_with_previous, start_test_runtime};
    use super::GatewayRuntime;

    let key = inference_key('z');
    let (primary, primary_request) =
        spawn_response_backend(200, "application/json", r#"{"source":"primary"}"#).await;
    let (fallback, fallback_request) =
        spawn_response_backend(200, "application/json", r#"{"source":"fallback"}"#).await;
    let mut config = inference_config(primary, &key, Utc::now() + ChronoDuration::hours(1));
    let (primary_target_id, _) = add_fallback_target(&mut config, fallback);
    config.validate().unwrap();
    let runtime = GatewayRuntime::new(gateway_state(&config));
    let (address, shutdown_tx, handle) = start_test_runtime(runtime.clone()).await;

    let old_state = runtime.load();
    let previous = old_state
        .inference_authorizer
        .as_deref()
        .expect("inference authorizer");
    let mut successor = config.clone();
    {
        let route = successor
            .inference
            .as_mut()
            .unwrap()
            .routes
            .values_mut()
            .next()
            .unwrap();
        let model = route.models.get_mut("allowed-model").unwrap();
        model
            .targets
            .retain(|target| target.target_id != primary_target_id);
        assert_eq!(model.targets.len(), 1);
        model.targets[0].priority = 0;
    }
    successor.validate().unwrap();
    runtime.replace(gateway_state_with_previous(&successor, Some(previous)));
    drop(old_state);

    let response = reqwest::Client::new()
        .post(format!("http://{address}/v1/chat/completions"))
        .bearer_auth(&key)
        .header("content-type", "application/json")
        .body(r#"{"model":"allowed-model","messages":[]}"#)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(
        response.json::<serde_json::Value>().await.unwrap()["data"]["source"],
        "fallback"
    );
    let fallback_request = fallback_request.await.unwrap();
    assert_eq!(request_model(&fallback_request), "fallback-upstream");
    assert!(
        tokio::time::timeout(Duration::from_millis(100), primary_request)
            .await
            .is_err(),
        "target successor must never contact the withdrawn primary upstream"
    );

    stop_test_entrypoint(shutdown_tx, handle).await;
}

#[tokio::test]
async fn managed_snapshot_apply_target_successor_routes_only_to_remaining_fallback_without_primary()
{
    use super::inference_tests::{
        gateway_state, gateway_state_with_previous, render_inference_snapshot_acl,
        start_test_runtime,
    };
    use super::GatewayRuntime;
    use crate::managed_snapshot::{
        ManagedSnapshot, ManagedSnapshotReloadCallback, ManagedSnapshotStore,
    };
    use std::sync::{Arc, Mutex};

    let key = inference_key('t');
    let (primary, primary_request) =
        spawn_response_backend(200, "application/json", r#"{"source":"primary"}"#).await;
    let (fallback, fallback_request) =
        spawn_response_backend(200, "application/json", r#"{"source":"fallback"}"#).await;
    let snapshot_expires = Utc::now() + ChronoDuration::hours(1);
    let mut first = inference_config(primary, &key, snapshot_expires);
    first.inference.as_mut().unwrap().expires_at = snapshot_expires;
    let (primary_target_id, _) = add_fallback_target(&mut first, fallback);
    first.validate().unwrap();
    let gateway_id = first.managed.gateway_id.expect("managed gateway id");

    let store = Arc::new(ManagedSnapshotStore::new(Some(gateway_id), None));
    let runtime =
        GatewayRuntime::new(gateway_state(&first)).with_managed_snapshot_store(store.clone());
    let (address, shutdown_tx, handle) = start_test_runtime(runtime.clone()).await;

    let mut successor = first.clone();
    {
        let route = successor
            .inference
            .as_mut()
            .unwrap()
            .routes
            .values_mut()
            .next()
            .unwrap();
        let model = route.models.get_mut("allowed-model").unwrap();
        model
            .targets
            .retain(|target| target.target_id != primary_target_id);
        assert_eq!(model.targets.len(), 1);
        model.targets[0].priority = 0;
    }
    successor.validate().unwrap();

    let previous_config = Arc::new(Mutex::new(first.clone()));
    let callback: ManagedSnapshotReloadCallback = {
        let runtime = runtime.clone();
        let previous_config = previous_config.clone();
        Arc::new(move |config| {
            let runtime = runtime.clone();
            let previous_config = previous_config.clone();
            Box::pin(async move {
                let old = previous_config.lock().unwrap().clone();
                let old_state = runtime.load();
                let previous = old_state.inference_authorizer.as_deref();
                let next_state = gateway_state_with_previous(&config, previous);
                drop(old_state);
                *previous_config.lock().unwrap() = config.clone();
                runtime.replace(next_state);
                Ok(old)
            })
        })
    };

    let first_snapshot = ManagedSnapshot::new(
        gateway_id,
        1,
        None,
        Utc::now(),
        snapshot_expires,
        render_inference_snapshot_acl(&first),
    );
    assert!(
        store
            .apply(first_snapshot, Some(&callback))
            .await
            .status
            .ready
    );

    let successor_expires = Utc::now() + ChronoDuration::hours(1);
    successor.inference.as_mut().unwrap().expires_at = successor_expires;
    let successor_snapshot = ManagedSnapshot::new(
        gateway_id,
        2,
        Some(1),
        Utc::now(),
        successor_expires,
        render_inference_snapshot_acl(&successor),
    );
    assert!(
        store
            .apply(successor_snapshot, Some(&callback))
            .await
            .status
            .ready
    );

    let response = reqwest::Client::new()
        .post(format!("http://{address}/v1/chat/completions"))
        .bearer_auth(&key)
        .header("content-type", "application/json")
        .body(r#"{"model":"allowed-model","messages":[]}"#)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(
        response.json::<serde_json::Value>().await.unwrap()["data"]["source"],
        "fallback"
    );
    let fallback_request = fallback_request.await.unwrap();
    assert_eq!(request_model(&fallback_request), "fallback-upstream");
    assert!(
        tokio::time::timeout(Duration::from_millis(100), primary_request)
            .await
            .is_err(),
        "managed snapshot target successor must never contact the withdrawn primary upstream"
    );

    stop_test_entrypoint(shutdown_tx, handle).await;
}
