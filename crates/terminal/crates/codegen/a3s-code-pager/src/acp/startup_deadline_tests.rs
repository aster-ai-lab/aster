use super::*;

#[tokio::test]
async fn initialization_failure_cancels_the_agent_token() {
    let home = tempfile::tempdir().unwrap();
    let auth_manager = std::sync::Arc::new(a3s_code_login::AuthManager::new_with_proxy_base_url(
        home.path(),
        Default::default(),
        "http://127.0.0.1:9".into(),
    ));
    let (tx, requests) = tokio::sync::mpsc::unbounded_channel();
    drop(requests);
    let (_notifications, rx) = tokio::sync::mpsc::unbounded_channel();
    let (_status, status_rx) =
        tokio::sync::watch::channel(leader_bridge::ConnectionStatus::Connected { generation: 0 });
    let cancel = CancellationToken::new();
    let endpoint = AgentEndpoint {
        tx,
        rx,
        cancel: cancel.clone(),
        location: AgentLocation::Leader(status_rx),
    };

    let result = initialize_connection(endpoint, &ConnectFlags::default(), auth_manager).await;

    assert!(result.is_err());
    assert!(cancel.is_cancelled(), "failed startup must stop its agent");
}

#[tokio::test(start_paused = true)]
async fn initialize_expires_when_the_live_sidecar_never_responds() {
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let flags = ConnectFlags::default();
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(7200),
        initialize(&tx, &flags, CapabilityTarget::Sidecar),
    )
    .await;
    let error = result
        .expect("initialize must have its own deadline")
        .err()
        .expect("silent sidecar must fail");
    assert!(error.to_string().contains("initialize"), "{error}");
    assert!(error.to_string().contains("timed out"), "{error}");
}

#[tokio::test(start_paused = true)]
async fn tui_authentication_uses_the_same_bounded_request() {
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let methods = [acp::AuthMethod::Agent(acp::AuthMethodAgent::new(
        acp::AuthMethodId::new("cached_token"),
        "Fixture",
    ))];
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(7200),
        authenticate(&tx, &methods, None),
    )
    .await;
    let error = result
        .expect("authenticate must have its own deadline")
        .unwrap_err();
    assert!(error.to_string().contains("authenticate"), "{error}");
    assert!(error.to_string().contains("timed out"), "{error}");
}
