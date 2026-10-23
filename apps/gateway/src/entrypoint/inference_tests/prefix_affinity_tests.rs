use super::*;

async fn affinity_backend(
    label: &'static str,
    observed: tokio::sync::mpsc::UnboundedSender<&'static str>,
) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            let observed = observed.clone();
            tokio::spawn(async move {
                let request = read_http_request(&mut stream).await;
                let body: serde_json::Value =
                    serde_json::from_slice(body_after_headers(&request)).unwrap();
                observed.send(label).unwrap();
                let (mime, body) = if body["stream"] == true {
                    ("text/event-stream", "data: {\"id\":\"fixture\",\"object\":\"chat.completion.chunk\",\"model\":\"fixture\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"OK\"},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n".to_string())
                } else {
                    ("application/json", serde_json::json!({"id":"fixture","object":"chat.completion","model":"fixture",
                        "choices":[{"index":0,"message":{"role":"assistant","content":"OK"},"finish_reason":"stop"}],
                        "usage":{"prompt_tokens":4000,"completion_tokens":1,"total_tokens":4001}}).to_string())
                };
                let response = format!("HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
                stream.write_all(response.as_bytes()).await.unwrap();
            });
        }
    });
    address
}

async fn fixture(
    enabled: bool,
) -> (
    GatewayConfig,
    String,
    tokio::sync::mpsc::UnboundedReceiver<&'static str>,
) {
    let (send, recv) = tokio::sync::mpsc::unbounded_channel();
    let first = affinity_backend("a", send.clone()).await;
    let second = affinity_backend("b", send).await;
    let key = inference_key('p');
    let mut config = inference_config(first, &key, Utc::now() + ChronoDuration::hours(1));
    let mut service = config.services["model-service"].clone();
    service.load_balancer.servers[0].url = format!("http://{second}");
    config.services.insert("second-service".into(), service);
    let route = config
        .inference
        .as_mut()
        .unwrap()
        .routes
        .values_mut()
        .next()
        .unwrap();
    for grant in route.grants.values_mut() {
        grant.limits.tokens_per_minute = 1_000_000;
        grant.limits.request_burst = 60;
    }
    let model = route.models.get_mut("allowed-model").unwrap();
    model.prefix_affinity = enabled;
    let mut second = model.targets[0].clone();
    second.target_id = Uuid::new_v4();
    second.service = "second-service".into();
    model.targets.push(second);
    (config, key, recv)
}

fn body(stream: bool, replay: bool) -> serde_json::Value {
    let mut messages =
        vec![serde_json::json!({"role":"user","content":"synthetic reference ".repeat(700)})];
    if replay {
        messages.extend([
            serde_json::json!({"role":"assistant","content":null,"messageId":"changed-id","usage":{"totalTokens":999999},
                "tool_calls":[{"id":"call-affinity","type":"function","function":{"name":"sum","arguments":"{\"a\":1}"}}]}),
            serde_json::json!({"role":"tool","tool_call_id":"call-affinity","content":"2"}),
            serde_json::json!({"role":"user","content":"continue"}),
        ]);
    }
    serde_json::json!({"model":"allowed-model","messages":messages,"stream":stream})
}

async fn send(address: SocketAddr, key: &str, body: serde_json::Value) {
    let stream = body["stream"] == true;
    let response = reqwest::Client::new()
        .post(format!("http://{address}/v1/chat/completions"))
        .bearer_auth(key)
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let response = response.text().await.unwrap();
    assert!(response.contains("OK"));
    if stream {
        assert!(response.contains("[DONE]"));
    }
}

#[tokio::test]
async fn prefix_affinity_preserves_locality_across_http_and_sse_tool_replay() {
    let (config, key, mut requests) = fixture(true).await;
    let state = gateway_state(&config);
    let (address, shutdown, handle) = start_test_entrypoint(state.clone()).await;
    send(address, &key, body(false, false)).await;
    assert_eq!(requests.recv().await.unwrap(), "a");
    send(address, &key, body(true, true)).await;
    assert_eq!(requests.recv().await.unwrap(), "a");
    // Two outstanding operations exceed the one-operation locality band.
    let busy = state
        .service_registry
        .get("model-service")
        .unwrap()
        .backends()[0]
        .clone();
    busy.inc_connections();
    busy.inc_connections();
    send(address, &key, body(false, true)).await;
    assert_eq!(requests.recv().await.unwrap(), "b");
    busy.dec_connections();
    busy.dec_connections();
    stop_test_entrypoint(shutdown, handle).await;
}

#[tokio::test]
async fn prefix_affinity_disabled_preserves_weighted_target_rotation() {
    let (config, key, mut requests) = fixture(false).await;
    let (address, shutdown, handle) = start_test_entrypoint(gateway_state(&config)).await;
    for expected in ["a", "b", "a", "b"] {
        send(address, &key, body(false, false)).await;
        assert_eq!(requests.recv().await.unwrap(), expected);
    }
    stop_test_entrypoint(shutdown, handle).await;
}

#[tokio::test]
async fn prefix_affinity_never_overrides_priority_or_unhealthy_target_exclusion() {
    let (mut config, key, mut requests) = fixture(true).await;
    config
        .inference
        .as_mut()
        .unwrap()
        .routes
        .values_mut()
        .next()
        .unwrap()
        .models
        .get_mut("allowed-model")
        .unwrap()
        .targets[1]
        .priority = 1;
    let state = gateway_state(&config);
    let first = state
        .service_registry
        .get("model-service")
        .unwrap()
        .backends()[0]
        .clone();
    let (address, shutdown, handle) = start_test_entrypoint(state).await;
    first.set_healthy(false);
    send(address, &key, body(true, false)).await;
    assert_eq!(requests.recv().await.unwrap(), "b");
    first.set_healthy(true);
    send(address, &key, body(false, true)).await;
    assert_eq!(requests.recv().await.unwrap(), "a");
    stop_test_entrypoint(shutdown, handle).await;
}
