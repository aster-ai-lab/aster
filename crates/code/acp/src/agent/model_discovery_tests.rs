use super::*;
use agent_client_protocol::Agent as _;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn selected_catalog_updates_the_menu_and_execution() {
    let root = tempfile::tempdir().unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let requested = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let incoming = requested.clone();
    let released = release.clone();
    let (stop, mut stopped) = tokio::sync::oneshot::channel();
    let provider = tokio::spawn(async move {
        let mut captured = Vec::new();
        loop {
            let (socket, _) = tokio::select! {
                _ = &mut stopped => break,
                incoming = listener.accept() => incoming.unwrap(),
            };
            let mut socket = tokio::io::BufReader::new(socket);
            let mut first = String::new();
            socket.read_line(&mut first).await.unwrap();
            let mut length = 0;
            loop {
                let mut line = String::new();
                socket.read_line(&mut line).await.unwrap();
                if line == "\r\n" {
                    break;
                }
                if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = value.trim().parse().unwrap();
                }
            }
            let catalog = first.starts_with("GET ");
            let body = if catalog {
                assert!(
                    first.starts_with("GET /v1/models/claude-sonnet-4-20250514 "),
                    "{first}"
                );
                incoming.notify_one();
                released.notified().await;
                serde_json::json!({"id":"claude-sonnet-4-20250514","max_input_tokens":131072,"max_tokens":32768,"capabilities":{"effort":{"supported":true,"low":{"supported":true},"high":{"supported":true}},"thinking":{"types":{"adaptive":{"supported":true}}}}}).to_string()
            } else {
                let mut bytes = vec![0; length];
                socket.read_exact(&mut bytes).await.unwrap();
                let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
                captured.push(value);
                [
                    serde_json::json!({"type":"message_start","message":{"id":"fixture","usage":{"input_tokens":1,"output_tokens":0}}}),
                    serde_json::json!({"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}),
                    serde_json::json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"READY"}}),
                    serde_json::json!({"type":"content_block_stop","index":0}),
                    serde_json::json!({"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":1}}),
                    serde_json::json!({"type":"message_stop"}),
                ].into_iter().map(|event| format!("data: {event}\n\n")).collect::<String>()
            };
            let mime = if catalog {
                "application/json"
            } else {
                "text/event-stream"
            };
            let response = format!("HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
            socket
                .get_mut()
                .write_all(response.as_bytes())
                .await
                .unwrap();
        }
        captured
    });
    let mut launch = tests::test_launch(Some(root.path().join("store")));
    launch.workspace = root.path().into();
    launch.config.providers[0].base_url = Some(base);
    launch.config.memory = Some(aster_core::memory::MemoryConfig {
        llm_extraction: false,
        prune_policy: None,
        ..Default::default()
    });
    let host = AsterCodeAgent::new(launch);
    host.initialize(
        acp::InitializeRequest::new(acp::ProtocolVersion::V1).meta(Some(
            [("aster/modelDiscovery".into(), true.into())]
                .into_iter()
                .collect(),
        )),
    )
    .await
    .unwrap();
    let opened = host.new_session(acp::NewSessionRequest::new(root.path()));
    tokio::pin!(opened);
    tokio::select! {
        _ = requested.notified() => {},
        _ = &mut opened => panic!("session admission must wait for its selected catalog"),
        _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => panic!("catalog was not requested asynchronously"),
    }
    let pending = host.available_model_infos();
    assert_eq!(
        pending[0].meta.as_ref().unwrap()["capabilitiesStatus"],
        "loading"
    );
    tokio::task::yield_now().await;
    release.notify_one();
    let opened = opened.await.unwrap();
    let id = opened.session_id;
    let model = opened.models.unwrap().available_models.remove(0);
    let meta = model.meta.unwrap();
    assert_eq!(meta["reasoningSource"], "provider");
    assert_eq!(meta["contextWindowSource"], "provider");
    assert_eq!(meta["totalContextTokens"], 131072);
    assert_eq!(meta["reasoningEfforts"].as_array().unwrap().len(), 2);
    let session = host.sessions.lock().await[&id.0.to_string()]
        .session
        .clone();
    assert_eq!(session.context_window_tokens(), 131072);
    session.send("Reply READY", None).await.unwrap();
    host.set_session_model(
        acp::SetSessionModelRequest::new(id.clone(), model.model_id.clone()).meta(Some(
            [("reasoningEffort".into(), "default".into())]
                .into_iter()
                .collect(),
        )),
    )
    .await
    .unwrap();
    let session = host.sessions.lock().await[&id.0.to_string()]
        .session
        .clone();
    assert_eq!(session.context_window_tokens(), 131072);
    session.send("Reply READY again", None).await.unwrap();
    session.close().await;
    stop.send(()).unwrap();
    let requests = provider.await.unwrap();
    let first = requests
        .iter()
        .find(|body| {
            body.to_string().contains("Reply READY")
                && !body.to_string().contains("Reply READY again")
        })
        .unwrap();
    let second = requests
        .iter()
        .find(|body| body.to_string().contains("Reply READY again"))
        .unwrap();
    assert_eq!(first["output_config"]["effort"], "high");
    assert_eq!(first["max_tokens"], 32768);
    assert!(second.get("output_config").is_none());
}

#[tokio::test]
async fn switches_rebuild_native_controls() {
    use aster_core::config::{ReasoningCapabilities, ReasoningEffort, ReasoningMode};

    let root = tempfile::tempdir().unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let captured = Arc::new(tokio::sync::Mutex::new(Vec::<serde_json::Value>::new()));
    let requests = Arc::clone(&captured);
    let (stop, mut stopped) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        loop {
            let (socket, _) = tokio::select! {
                _ = &mut stopped => break,
                incoming = listener.accept() => incoming.unwrap(),
            };
            let mut socket = tokio::io::BufReader::new(socket);
            let mut first = String::new();
            socket.read_line(&mut first).await.unwrap();
            let mut length = 0;
            loop {
                let mut line = String::new();
                socket.read_line(&mut line).await.unwrap();
                if line == "\r\n" {
                    break;
                }
                if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = value.trim().parse().unwrap();
                }
            }
            if first.starts_with("GET /v1/models ") {
                let body = serde_json::json!({"data":[{"id":"deepseek-flash","context_window":1_048_576,"max_output_tokens":393_216}]}).to_string();
                let response = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
                socket.write_all(response.as_bytes()).await.unwrap();
                continue;
            }
            assert!(first.starts_with("POST /v1/chat/completions "), "{first}");
            let mut bytes = vec![0; length];
            socket.read_exact(&mut bytes).await.unwrap();
            let request: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            let streaming = request["stream"] == true;
            requests.lock().await.push(request);
            let usage =
                serde_json::json!({"prompt_tokens":1,"completion_tokens":1,"total_tokens":2});
            let (mime, body) = if streaming {
                let text = serde_json::json!({"choices":[{"delta":{"content":"READY"}}]});
                let done = serde_json::json!({"choices":[{"delta":{},"finish_reason":"stop"}],"usage":usage});
                (
                    "text/event-stream",
                    format!("data: {text}\n\ndata: {done}\n\ndata: [DONE]\n\n"),
                )
            } else {
                ("application/json", serde_json::json!({"id":"fixture","choices":[{"index":0,"message":{"role":"assistant","content":"READY"},"finish_reason":"stop"}],"usage":usage}).to_string())
            };
            let response = format!("HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
            socket.write_all(response.as_bytes()).await.unwrap();
        }
    });

    let mut launch = tests::test_launch(Some(root.path().join("store")));
    launch.workspace = root.path().into();
    launch.model_id = "deepseek/deepseek-flash".into();
    launch.config.default_model = Some(launch.model_id.clone());
    launch.config.reasoning_effort = Some(ReasoningEffort::Max);
    launch.config.providers.remove(0);
    let provider = &mut launch.config.providers[0];
    provider.name = "deepseek".into();
    provider.protocol = Some(aster_core::config::ProviderProtocol::Openai);
    provider.base_url = Some(format!("{base}/v1"));
    let model = &mut provider.models[0];
    model.id = "deepseek-flash".into();
    model.reasoning_control = Some(
        ReasoningCapabilities::new(
            ReasoningMode::Effort,
            vec![
                ReasoningEffort::None,
                ReasoningEffort::Low,
                ReasoningEffort::High,
                ReasoningEffort::Max,
            ],
        )
        .unwrap()
        .with_default_level(ReasoningEffort::High)
        .unwrap(),
    );
    model.reasoning = true;
    model.limit.context = 0;
    model.limit.output = 500_000;
    launch.config.memory = Some(aster_core::memory::MemoryConfig {
        llm_extraction: false,
        prune_policy: None,
        ..Default::default()
    });
    let host = AsterCodeAgent::new(launch);
    host.initialize(
        acp::InitializeRequest::new(acp::ProtocolVersion::V1).meta(Some(
            [("aster/modelDiscovery".into(), true.into())]
                .into_iter()
                .collect(),
        )),
    )
    .await
    .unwrap();
    let id = host
        .new_session(acp::NewSessionRequest::new(root.path()))
        .await
        .unwrap()
        .session_id;
    for (index, level) in ["max", "default", "none", "low", "high"]
        .into_iter()
        .enumerate()
    {
        if index > 0 {
            let response = host
                .set_session_model(
                    acp::SetSessionModelRequest::new(
                        id.clone(),
                        acp::ModelId::new("deepseek/deepseek-flash"),
                    )
                    .meta(Some(
                        [("reasoningEffort".into(), level.into())]
                            .into_iter()
                            .collect(),
                    )),
                )
                .await
                .unwrap();
            assert_eq!(response.meta.unwrap()["reasoningEffort"], level);
        }
        let session = host.sessions.lock().await[&id.0.to_string()]
            .session
            .clone();
        assert_eq!(session.context_window_tokens(), 1_048_576);
        let before = captured.lock().await.len();
        session
            .send(&format!("Reply READY for {level}"), None)
            .await
            .unwrap();
        let sent = captured.lock().await;
        assert!(sent.len() > before);
        for request in &sent[before..] {
            assert_eq!(request["model"], "deepseek-flash");
            assert_eq!(request["max_tokens"], 393_216);
            if level == "default" {
                assert!(
                    request.get("reasoning_effort").is_none(),
                    "default retained an effort override"
                );
                assert!(
                    request.get("thinking").is_none(),
                    "default retained a thinking override"
                );
            } else {
                assert_eq!(request["reasoning_effort"], level);
                assert_eq!(
                    request["thinking"]["type"],
                    if level == "none" {
                        "disabled"
                    } else {
                        "enabled"
                    }
                );
            }
        }
    }
    let session = host.sessions.lock().await[&id.0.to_string()]
        .session
        .clone();
    session.close().await;
    stop.send(()).unwrap();
    server.await.unwrap();
}
