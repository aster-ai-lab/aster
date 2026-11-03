use super::*;
use serde_json::json;
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn transport() -> (MockServer, Arc<HttpSseTransport>) {
    let server = MockServer::start().await;
    Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/messages"))
        .and(wiremock::matchers::query_param(
            "sessionId",
            "synthetic-secret",
        ))
        .respond_with(ResponseTemplate::new(202))
        .mount(&server)
        .await;
    let (_, notification_rx) = NotificationReceiver::channel(16);
    let transport = Arc::new(HttpSseTransport {
        client: Client::new(),
        session: Arc::new(Mutex::new(Session {
            endpoint: Some(
                Url::parse(&format!(
                    "{}/messages?sessionId=synthetic-secret",
                    server.uri()
                ))
                .unwrap(),
            ),
            pending: HashMap::new(),
        })),
        notification_rx,
        request_timeout: Duration::from_secs(1),
        closed: CancellationToken::new(),
        reader: Mutex::new(None),
    });
    (server, transport)
}

async fn wait_for_posts(server: &MockServer, count: usize) {
    tokio::time::timeout(Duration::from_secs(3), async {
        while server.received_requests().await.unwrap().len() < count {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn advertised_endpoint_preserves_query_and_notification_data() {
    let session = Mutex::new(Session::default());
    let (notifications, receiver) = NotificationReceiver::channel(16);
    let mut fifo = receiver.take();
    let url = Url::parse("https://example.test/configured/sse?token=synthetic-secret").unwrap();
    let (ready, result) = oneshot::channel();
    let mut ready = Some(ready);
    HttpSseTransport::receive_event(
        "event: endpoint\r\ndata: /messages?sessionId=owned\r\n",
        &url,
        &session,
        &notifications,
        &mut ready,
    )
    .await
    .unwrap();
    result.await.unwrap().unwrap();
    assert_eq!(
        session.lock().unwrap().endpoint.as_ref().unwrap().as_str(),
        "https://example.test/messages?sessionId=owned"
    );
    HttpSseTransport::receive_event("event: message\r\ndata: {\"jsonrpc\":\"2.0\",\r\ndata: \"method\":\"notifications/tools/list_changed\"}\r\n", &url, &session, &notifications, &mut ready).await.unwrap();
    assert!(matches!(
        fifo.recv().await,
        Some(McpNotification::ToolsListChanged)
    ));
    let error = HttpSseTransport::receive_event(
        "event: endpoint\ndata: https://other.test/messages",
        &url,
        &session,
        &notifications,
        &mut ready,
    )
    .await
    .unwrap_err();
    assert!(!error.to_string().contains("synthetic-secret"));
    assert_eq!(
        session
            .lock()
            .unwrap()
            .endpoint
            .as_ref()
            .unwrap()
            .host_str(),
        Some("example.test")
    );
}

#[tokio::test]
async fn concurrent_requests_receive_their_own_out_of_order_sse_reply() {
    let (server, transport) = transport().await;
    let one = Arc::clone(&transport);
    let first = tokio::spawn(async move {
        one.request(JsonRpcRequest::new(1, "tools/list", None))
            .await
    });
    let two = Arc::clone(&transport);
    let second = tokio::spawn(async move {
        two.request(JsonRpcRequest::new(2, "tools/list", None))
            .await
    });
    wait_for_posts(&server, 2).await;
    let (notifications, _) = NotificationReceiver::channel(16);
    for id in [2, 1] {
        let event = format!(
            "data: {}\n",
            json!({"jsonrpc":"2.0","id":id,"result":{"text":format!("回复{id} 🐦")}})
        );
        HttpSseTransport::receive_event(
            &event,
            &Url::parse(&server.uri()).unwrap(),
            &transport.session,
            &notifications,
            &mut None,
        )
        .await
        .unwrap();
    }
    assert_eq!(
        first.await.unwrap().unwrap().result.unwrap()["text"],
        "回复1 🐦"
    );
    assert_eq!(
        second.await.unwrap().unwrap().result.unwrap()["text"],
        "回复2 🐦"
    );
    assert!(transport.session.lock().unwrap().pending.is_empty());
}

#[tokio::test]
async fn timeout_and_dropped_request_remove_pending_responses_without_replay() {
    let (server, transport) = transport().await;
    assert!(transport
        .request(JsonRpcRequest::new(1, "tools/call", None))
        .await
        .is_err());
    assert!(transport.session.lock().unwrap().pending.is_empty());
    let caller = Arc::clone(&transport);
    let request = tokio::spawn(async move {
        caller
            .request(JsonRpcRequest::new(2, "tools/call", None))
            .await
    });
    wait_for_posts(&server, 2).await;
    request.abort();
    assert!(request.await.unwrap_err().is_cancelled());
    assert!(transport.session.lock().unwrap().pending.is_empty());
    assert_eq!(server.received_requests().await.unwrap().len(), 2);
}

#[tokio::test]
async fn close_releases_pending_requests_and_refuses_later_work() {
    let (server, transport) = transport().await;
    let caller = Arc::clone(&transport);
    let request = tokio::spawn(async move {
        caller
            .request(JsonRpcRequest::new(1, "tools/call", None))
            .await
    });
    wait_for_posts(&server, 1).await;
    transport.close().await.unwrap();
    tokio::time::timeout(Duration::from_millis(200), request)
        .await
        .unwrap()
        .unwrap()
        .unwrap_err();
    transport.close().await.unwrap();
    assert!(!transport.is_connected());
    assert!(transport.session.lock().unwrap().pending.is_empty());
    assert!(transport
        .request(JsonRpcRequest::new(2, "tools/list", None))
        .await
        .is_err());
    assert!(transport
        .notify(JsonRpcNotification::new("notifications/initialized", None))
        .await
        .is_err());
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
    assert!(!format!("{transport:?}").contains("synthetic-secret"));
}

#[tokio::test]
async fn http_failures_are_reported_and_secret_headers_are_validated() {
    let server = MockServer::start().await;
    Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/exact/sse"))
        .respond_with(ResponseTemplate::new(401))
        .expect(1)
        .mount(&server)
        .await;
    let url = format!("{}/exact/sse?token=synthetic-secret", server.uri());
    let error = HttpSseTransport::connect(&url, HashMap::new())
        .await
        .unwrap_err();
    assert!(!format!("{error:#}").contains("synthetic-secret"));
    let headers = HashMap::from([("Authorization".into(), "synthetic-secret\ninvalid".into())]);
    let error = HttpSseTransport::connect(url, headers).await.unwrap_err();
    assert!(!format!("{error:#}").contains("synthetic-secret"));
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}

#[tokio::test]
async fn notification_post_failure_is_not_reported_as_success() {
    let (_, transport) = transport().await;
    let server = MockServer::start().await;
    Mock::given(wiremock::matchers::method("POST"))
        .respond_with(ResponseTemplate::new(403))
        .expect(1)
        .mount(&server)
        .await;
    transport.session.lock().unwrap().endpoint = Some(Url::parse(&server.uri()).unwrap());
    assert!(transport
        .notify(JsonRpcNotification::new("notifications/initialized", None))
        .await
        .is_err());
}
