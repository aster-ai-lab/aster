use super::*;
use agent_client_protocol::Agent as _;

fn meta(value: serde_json::Value) -> Option<acp::Meta> {
    Some([("reasoningEffort".into(), value)].into_iter().collect())
}

#[tokio::test]
async fn switching_revalidates_and_returns_the_effective_selection() {
    let root = tempfile::tempdir().unwrap();
    let mut launch = tests::test_launch(Some(root.path().join("store")));
    launch.workspace = root.path().into();
    launch.config.providers[1].models[0].reasoning_control = None;
    let host = AsterCodeAgent::new(launch);
    let id = host
        .new_session(acp::NewSessionRequest::new(root.path()))
        .await
        .unwrap()
        .session_id;
    let response = host
        .set_session_model(acp::SetSessionModelRequest::new(
            id.clone(),
            acp::ModelId::new("openai/gpt-4o"),
        ))
        .await
        .unwrap();
    assert_eq!(response.meta.unwrap()["reasoningEffort"], "default");
    assert!(host.sessions.lock().await[&id.0.to_string()]
        .effort
        .is_none());
    let session = host.sessions.lock().await[&id.0.to_string()]
        .session
        .clone();
    assert!(host
        .set_session_model(
            acp::SetSessionModelRequest::new(id.clone(), acp::ModelId::new("openai/gpt-4o"))
                .meta(meta(serde_json::json!("high")))
        )
        .await
        .is_err());
    assert!(Arc::ptr_eq(
        &session,
        &host.sessions.lock().await[&id.0.to_string()].session
    ));
}

#[tokio::test]
async fn selections_are_owned_by_their_session() {
    let root = tempfile::tempdir().unwrap();
    let mut launch = tests::test_launch(Some(root.path().join("store")));
    launch.workspace = root.path().into();
    let host = AsterCodeAgent::new(launch);
    let first = host
        .new_session(acp::NewSessionRequest::new(root.path()))
        .await
        .unwrap()
        .session_id;
    let second = host
        .new_session(acp::NewSessionRequest::new(root.path()))
        .await
        .unwrap()
        .session_id;
    for (id, token) in [(&first, "low"), (&second, "max")] {
        host.set_session_model(
            acp::SetSessionModelRequest::new(id.clone(), acp::ModelId::new("openai/gpt-4o"))
                .meta(meta(serde_json::json!(token))),
        )
        .await
        .unwrap();
    }
    let result = host
        .set_session_model(acp::SetSessionModelRequest::new(
            first.clone(),
            acp::ModelId::new("openai/gpt-4o"),
        ))
        .await
        .unwrap();
    assert_eq!(result.meta.unwrap()["reasoningEffort"], "low");
    let result = host
        .set_session_model(
            acp::SetSessionModelRequest::new(first.clone(), acp::ModelId::new("openai/gpt-4o"))
                .meta(meta(serde_json::json!("default"))),
        )
        .await
        .unwrap();
    assert_eq!(result.meta.unwrap()["reasoningEffort"], "default");
    assert_eq!(
        host.sessions.lock().await[&second.0.to_string()]
            .effort
            .unwrap()
            .token,
        "max"
    );
}

#[tokio::test]
async fn new_agent_restores_acl_effort_and_explicit_launch_override_wins() {
    let root = tempfile::tempdir().unwrap();
    for (override_token, expected) in [(None, "max"), (Some("low"), "low")] {
        let mut launch = tests::test_launch(Some(root.path().join("store")));
        launch.workspace = root.path().into();
        launch.config.reasoning_effort = Some(aster_core::config::ReasoningEffort::Max);
        let mut agent = AsterCodeAgent::new(launch);
        if let Some(token) = override_token {
            agent = agent.with_default_reasoning_effort(token).unwrap();
        }
        let created = agent
            .new_session(acp::NewSessionRequest::new(root.path()))
            .await
            .unwrap();
        let models = created.models.unwrap();
        let active = models
            .available_models
            .iter()
            .find(|model| model.model_id == models.current_model_id)
            .unwrap();
        assert_eq!(active.meta.as_ref().unwrap()["reasoningEffort"], expected);
    }
    let mut launch = tests::test_launch(Some(root.path().join("default-store")));
    launch.workspace = root.path().into();
    launch.config.reasoning_effort = Some(aster_core::config::ReasoningEffort::Default);
    let agent = AsterCodeAgent::new(launch);
    assert!(
        agent.effort.lock().await.is_none(),
        "an explicitly saved default removes effort overrides"
    );
}

#[tokio::test]
async fn terminal_initial_effort_reaches_new_session_and_allows_later_selection() {
    let root = tempfile::tempdir().unwrap();
    let mut launch = tests::test_launch(Some(root.path().join("store")));
    launch.workspace = root.path().into();
    let host = AsterCodeAgent::new(launch)
        .with_default_reasoning_effort("medium")
        .unwrap();
    let created = host
        .new_session(acp::NewSessionRequest::new(root.path()))
        .await
        .unwrap();
    let models = created.models.unwrap();
    let active = models
        .available_models
        .iter()
        .find(|model| model.model_id == models.current_model_id)
        .unwrap();
    assert_eq!(active.meta.as_ref().unwrap()["reasoningEffort"], "medium");
    assert_eq!(host.effort.lock().await.unwrap().thinking_budget, 8192);
    host.set_session_model(
        acp::SetSessionModelRequest::new(created.session_id, models.current_model_id)
            .meta(meta(serde_json::json!("low"))),
    )
    .await
    .unwrap();
    assert_eq!(host.effort.lock().await.unwrap().thinking_budget, 2048);
    assert!(AsterCodeAgent::new(tests::test_launch(None))
        .with_default_reasoning_effort("unknown")
        .is_err());
}

#[tokio::test]
async fn explicit_effort_default_clears_override_and_absence_preserves_selection() {
    let root = tempfile::tempdir().unwrap();
    let mut launch = tests::test_launch(Some(root.path().join("store")));
    launch.workspace = root.path().into();
    let host = AsterCodeAgent::new(launch);
    let created = host
        .new_session(acp::NewSessionRequest::new(root.path()))
        .await
        .unwrap();
    for model in created.models.unwrap().available_models {
        let meta = model.meta.unwrap();
        assert_eq!(meta["reasoningEffortReset"], true);
        assert!(!meta["reasoningEfforts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry["value"] == "default"));
    }
    let id = created.session_id;
    assert_eq!(
        host.effort.lock().await.map(|effort| effort.token),
        Some("high")
    );
    for (selection, expected) in [
        (meta(serde_json::json!("low")), Some("low")),
        (None, Some("low")),
        (meta(serde_json::Value::Null), Some("low")),
        (meta(serde_json::json!("none")), Some("none")),
        (meta(serde_json::json!("default")), None),
        (None, None),
    ] {
        host.set_session_model(
            acp::SetSessionModelRequest::new(id.clone(), acp::ModelId::new("openai/gpt-4o"))
                .meta(selection),
        )
        .await
        .unwrap();
        assert_eq!(
            host.effort.lock().await.map(|effort| effort.token),
            expected
        );
    }
}

#[tokio::test]
async fn unsupported_effort_is_refused_without_replacing_the_session() {
    let root = tempfile::tempdir().unwrap();
    let mut launch = tests::test_launch(Some(root.path().join("store")));
    launch.workspace = root.path().into();
    let host = AsterCodeAgent::new(launch);
    let id = host
        .new_session(acp::NewSessionRequest::new(root.path()))
        .await
        .unwrap()
        .session_id;
    let previous = host
        .sessions
        .lock()
        .await
        .get(id.0.as_ref())
        .unwrap()
        .session
        .clone();
    for value in [
        serde_json::json!("auto"),
        serde_json::json!("ultra"),
        serde_json::json!("wrong"),
        serde_json::json!(""),
        serde_json::json!(42),
    ] {
        let error = host
            .set_session_model(
                acp::SetSessionModelRequest::new(id.clone(), acp::ModelId::new("openai/gpt-4o"))
                    .meta(meta(value)),
            )
            .await
            .expect_err("unsupported effort");
        assert!(format!("{error:?}").contains("reasoning effort"));
        let current = host
            .sessions
            .lock()
            .await
            .get(id.0.as_ref())
            .unwrap()
            .session
            .clone();
        assert!(Arc::ptr_eq(&previous, &current));
        assert_eq!(
            host.effort.lock().await.map(|effort| effort.token),
            Some("high")
        );
    }
}

/// Only the loopback provider is scripted. Real ACP session replacement and
/// Core restoration build the subsequent requests and retain the transcript.
#[tokio::test]
async fn switching_preserves_history_and_anthropic_parameters() {
    let phrasings = [
        "核对材料中的数值后交付报告。",
        "请逐条检查来源，说明不能同时满足的要求。",
        "先确认日期口径，再逐段检查变化。",
        "完成文件后，复核回复中的每项结论。",
    ];
    use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/v1", listener.local_addr().unwrap());
    let (ready, mut turns) = tokio::sync::mpsc::channel::<()>(1);
    let provider = async move {
        let mut requests = Vec::<serde_json::Value>::new();
        let mut awaiting_turn = true;
        for _ in 0..16 {
            if awaiting_turn {
                // Session rebuilds are outside the provider's request deadline.
                turns
                    .recv()
                    .await
                    .expect("the next session is ready to send");
            }
            let (socket, _) =
                tokio::time::timeout(std::time::Duration::from_secs(5), listener.accept())
                    .await
                    .unwrap()
                    .unwrap();
            let mut socket = BufReader::new(socket);
            let mut line = String::new();
            socket.read_line(&mut line).await.unwrap();
            assert!(line.starts_with("POST /v1/messages "), "{line}");
            let mut length = None;
            loop {
                line.clear();
                assert!(socket.read_line(&mut line).await.unwrap() > 0);
                if line == "\r\n" {
                    break;
                }
                if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = Some(value.trim().parse::<usize>().unwrap());
                }
            }
            let length = length.unwrap();
            assert!(length <= 1024 * 1024);
            let mut bytes = vec![0; length];
            socket.read_exact(&mut bytes).await.unwrap();
            let request: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            let streaming = request["stream"] == true;
            awaiting_turn = streaming;
            let index = requests.len();
            if streaming {
                requests.push(request);
            }
            let body = if streaming {
                [
                serde_json::json!({"type":"message_start","message":{"id":"fixture","model":"claude-sonnet-4-20250514","type":"message","usage":{"input_tokens":10,"output_tokens":0}}}),
                serde_json::json!({"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}),
                serde_json::json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":format!("reply-{index}")}}),
                serde_json::json!({"type":"content_block_stop","index":0}),
                serde_json::json!({"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":1}}),
                serde_json::json!({"type":"message_stop"}),
            ].into_iter().map(|event| format!("data: {event}\n\n")).collect::<String>()
            } else {
                serde_json::json!({"content":[{"type":"text","text":"[]"}],"stop_reason":"end_turn","usage":{"input_tokens":1,"output_tokens":1}}).to_string()
            };
            let content_type = if streaming {
                "text/event-stream"
            } else {
                "application/json"
            };
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            socket
                .get_mut()
                .write_all(response.as_bytes())
                .await
                .unwrap();
            socket.get_mut().shutdown().await.unwrap();
            if requests.len() == 5 {
                break;
            }
        }
        assert_eq!(requests.len(), 5);
        requests
    };
    let root = tempfile::tempdir().unwrap();
    let mut launch = tests::test_launch(Some(root.path().join("store")));
    launch.workspace = root.path().into();
    launch.config.providers[0].base_url = Some(base);
    launch.config.memory = Some(aster_core::memory::MemoryConfig {
        llm_extraction: false,
        prune_policy: None,
        ..Default::default()
    });
    let model = launch.model_id.clone();
    let host = AsterCodeAgent::new(launch.clone());
    let initialize = || {
        acp::InitializeRequest::new(acp::ProtocolVersion::V1).meta(Some(
            [
                (
                    "rules".into(),
                    serde_json::json!("Initialize rules marker: BLUE_WHALE_9090"),
                ),
                ("aster/terminalMarkdown".into(), serde_json::json!(true)),
            ]
            .into_iter()
            .collect(),
        ))
    };
    host.initialize(initialize()).await.unwrap();
    let id = host
        .new_session(acp::NewSessionRequest::new(root.path()))
        .await
        .unwrap()
        .session_id;
    let provider = tokio::spawn(provider);
    for (index, selected) in [None, Some("low"), Some("max"), Some("default")]
        .into_iter()
        .enumerate()
    {
        let previous = host
            .sessions
            .lock()
            .await
            .get(id.0.as_ref())
            .unwrap()
            .session
            .clone();
        if let Some(selected) = selected {
            host.set_session_model(
                acp::SetSessionModelRequest::new(id.clone(), model.clone())
                    .meta(meta(serde_json::json!(selected))),
            )
            .await
            .unwrap();
            assert!(previous.is_closed());
        }
        host.set_session_mode(acp::SetSessionModeRequest::new(id.clone(), "auto"))
            .await
            .unwrap();
        let session = {
            let sessions = host.sessions.lock().await;
            let live = sessions.get(id.0.as_ref()).unwrap();
            assert_eq!(live.permissions.lock().unwrap().posture.as_str(), "auto");
            assert_eq!(live.session.session_id(), id.0.as_ref());
            live.session.clone()
        };
        let history = serde_json::to_string(&session.durable_transcript()).unwrap();
        for earlier in 0..index {
            assert!(history.contains(&format!("marker-{earlier}")));
            assert!(history.contains(&format!("reply-{earlier}")));
        }
        ready.send(()).await.unwrap();
        tokio::time::timeout(
            std::time::Duration::from_secs(5),
            session.send(&format!("marker-{index}\n{}", phrasings[index]), None),
        )
        .await
        .unwrap()
        .unwrap();
        assert!(!session.is_closed());
    }
    let session = host
        .sessions
        .lock()
        .await
        .get(id.0.as_ref())
        .unwrap()
        .session
        .clone();
    session.close().await;
    drop(session);
    drop(host);
    let reopened = AsterCodeAgent::new(launch);
    reopened.initialize(initialize()).await.unwrap();
    reopened
        .load_session(acp::LoadSessionRequest::new(id.clone(), root.path()))
        .await
        .unwrap();
    let session = reopened
        .sessions
        .lock()
        .await
        .get(id.0.as_ref())
        .unwrap()
        .session
        .clone();
    ready.send(()).await.unwrap();
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        session.send("marker-4", None),
    )
    .await
    .unwrap()
    .unwrap();
    session.close().await;
    let requests = provider.await.unwrap();
    for (index, request) in requests.iter().enumerate() {
        let system = request["system"].to_string();
        assert_eq!(
            system.matches("BLUE_WHALE_9090").count(),
            1,
            "initialize rules must reach request {index} exactly once"
        );
        assert!(
            system.contains(SESSION_OUTCOME_BOUNDARY),
            "core boundary must remain in request {index}"
        );
        assert!(system.contains(TERMINAL_MARKDOWN_PRESENTATION),
            "terminal artifact guidance must reach the actual provider request after model switches");
        let messages = request["messages"].to_string();
        if index < phrasings.len() {
            assert!(messages.contains(phrasings[index]));
        }
        for earlier in 0..index {
            assert!(
                messages.contains(&format!("marker-{earlier}")),
                "{messages}"
            );
            assert!(messages.contains(&format!("reply-{earlier}")), "{messages}");
        }
    }
    assert_eq!(requests[0]["output_config"]["effort"], "high");
    assert_eq!(requests[1]["output_config"]["effort"], "low");
    assert_eq!(requests[2]["output_config"]["effort"], "max");
    for request in &requests[1..3] {
        assert_eq!(request["thinking"]["type"], "adaptive");
        assert!(request["thinking"].get("budget_tokens").is_none());
    }
    assert!(requests[3].get("output_config").is_none());
    assert!(requests[3].get("thinking").is_none());
}
