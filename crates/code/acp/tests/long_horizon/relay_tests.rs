#![cfg(test)]

use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

const UPSTREAM_KEY: &str = "fixture-upstream-private-value";
const MODEL: &str = "fixture-model";

struct Fake {
    endpoint: String,
    calls: Arc<AtomicUsize>,
    worker: JoinHandle<()>,
}

impl Fake {
    async fn start(status: u16, body: &[u8], sse: bool, retry_after: Option<&str>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}{PATH}", listener.local_addr().unwrap());
        let calls = Arc::new(AtomicUsize::new(0));
        let count = calls.clone();
        let body = body.to_vec();
        let retry_after = retry_after.map(str::to_owned);
        let worker = tokio::spawn(async move {
            loop {
                let (mut stream, _) = listener.accept().await.unwrap();
                let request = read_request(&mut stream, &format!("Bearer {UPSTREAM_KEY}"))
                    .await
                    .unwrap();
                assert_eq!(request["model"], MODEL);
                count.fetch_add(1, Ordering::SeqCst);
                let content_type = if sse {
                    "text/event-stream"
                } else {
                    "application/json"
                };
                let retry = retry_after
                    .as_ref()
                    .map(|value| format!("Retry-After: {value}\r\n"))
                    .unwrap_or_default();
                let head = format!("HTTP/1.1 {status} Response\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n{retry}Connection: close\r\n\r\n", body.len());
                stream.write_all(head.as_bytes()).await.unwrap();
                stream.write_all(&body).await.unwrap();
                stream.shutdown().await.unwrap();
            }
        });
        Self {
            endpoint,
            calls,
            worker,
        }
    }

    fn route(&self) -> Route {
        Route {
            endpoint: self.endpoint.clone(),
            headers: vec![("Authorization".into(), format!("Bearer {UPSTREAM_KEY}"))],
            model: MODEL.into(),
            protected_values: vec![UPSTREAM_KEY.as_bytes().to_vec()],
        }
    }
}

impl Drop for Fake {
    fn drop(&mut self) {
        self.worker.abort();
    }
}

fn body(streaming: bool) -> Value {
    serde_json::json!({"model":MODEL,"messages":[{"role":"user","content":"fixture"}],"max_tokens":512,"stream":streaming})
}

fn frame(key: &str, path: &str, body: &Value) -> Vec<u8> {
    let body = body.to_string();
    format!("POST {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer {key}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", body.len()).into_bytes()
}

async fn request(endpoint: &str, frame: &[u8]) -> Vec<u8> {
    tokio::time::timeout(Duration::from_secs(5), async {
        let url = url::Url::parse(endpoint).unwrap();
        let mut stream = TcpStream::connect(("127.0.0.1", url.port().unwrap()))
            .await
            .unwrap();
        stream.write_all(frame).await.unwrap();
        let mut output = Vec::new();
        stream
            .take((RESPONSE_LIMIT + HEADER_LIMIT + 1) as u64)
            .read_to_end(&mut output)
            .await
            .unwrap();
        output
    })
    .await
    .expect("owned relay request must settle")
}

fn status(response: &[u8]) -> u16 {
    std::str::from_utf8(response)
        .unwrap()
        .lines()
        .next()
        .unwrap()
        .split_whitespace()
        .nth(1)
        .unwrap()
        .parse()
        .unwrap()
}

#[tokio::test]
async fn exact_model_round_trip_and_request_budget_are_enforced() {
    let fake = Fake::start(200, br#"{"content":"pong"}"#, false, None).await;
    let relay = Relay::start(fake.route(), 1, 512).await.unwrap();
    let first = request(relay.endpoint(), &frame(relay.key(), PATH, &body(false))).await;
    assert_eq!(status(&first), 200);
    assert!(String::from_utf8_lossy(&first).contains("pong"));
    let second = request(relay.endpoint(), &frame(relay.key(), PATH, &body(false))).await;
    assert_eq!(status(&second), 429);
    let stats = relay.close().await.unwrap();
    assert_eq!(stats.upstream_calls, 1);
    assert_eq!(stats.rejected, 1);
    assert_eq!(
        stats.rejection_reasons.get(&Reason::RequestBudget),
        Some(&1)
    );
    assert_eq!(stats.requests.len(), 1);
    assert_eq!(stats.requests[0].sequence, 1);
    assert_eq!(stats.requests[0].messages, 1);
    assert!(stats.requests[0].shape_complete);
    assert_eq!(stats.responses.len(), 1);
    assert_eq!(stats.verified_token_total(), None);
    assert_eq!(fake.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn an_upstream_redirect_cannot_escape_the_admitted_endpoint_or_request_budget() {
    for streaming in [false, true] {
        let destination = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let destination_endpoint = format!("http://{}{PATH}", destination.local_addr().unwrap());
        let destination_calls = Arc::new(AtomicUsize::new(0));
        let calls = destination_calls.clone();
        let worker = tokio::spawn(async move {
            loop {
                let (mut socket, _) = destination.accept().await.unwrap();
                calls.fetch_add(1, Ordering::SeqCst);
                let _ = read_request(&mut socket, &format!("Bearer {UPSTREAM_KEY}")).await;
                socket
                    .write_all(
                        b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}",
                    )
                    .await
                    .unwrap();
            }
        });
        let destination = Fake {
            endpoint: destination_endpoint,
            calls: destination_calls,
            worker,
        };
        let source = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}{PATH}", source.local_addr().unwrap());
        let target = destination.endpoint.clone();
        let source_calls = Arc::new(AtomicUsize::new(0));
        let calls = source_calls.clone();
        let worker = tokio::spawn(async move {
            loop {
                let (mut socket, _) = source.accept().await.unwrap();
                read_request(&mut socket, &format!("Bearer {UPSTREAM_KEY}"))
                    .await
                    .unwrap();
                calls.fetch_add(1, Ordering::SeqCst);
                socket.write_all(format!("HTTP/1.1 307 Temporary Redirect\r\nLocation: {target}\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{{}}").as_bytes()).await.unwrap();
            }
        });
        let source = Fake {
            endpoint,
            calls: source_calls,
            worker,
        };
        let relay = Relay::start(source.route(), 1, 512).await.unwrap();
        let response = request(
            relay.endpoint(),
            &frame(relay.key(), PATH, &body(streaming)),
        )
        .await;
        let stats = relay.close().await.unwrap();
        assert_eq!(source.calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            destination.calls.load(Ordering::SeqCst),
            0,
            "a single admitted dispatch must not make a second HTTP request"
        );
        assert_eq!(status(&response), 307);
        assert_eq!(stats.upstream_calls, 1);
        assert_eq!(stats.statuses.get(&307), Some(&1));
        assert_eq!(stats.verified_token_total(), None);
    }
}

#[tokio::test]
async fn raw_json_usage_is_bound_to_the_actual_request_without_response_text() {
    let payload = br#"{"choices":[{"message":{"content":"private-answer","reasoning_content":"private-thought"}}],"usage":{"prompt_tokens":100,"completion_tokens":7,"total_tokens":107,"prompt_cache_hit_tokens":80,"prompt_cache_miss_tokens":20}}"#;
    let fake = Fake::start(200, payload, false, None).await;
    let relay = Relay::start(fake.route(), 1, 512).await.unwrap();
    let response = request(relay.endpoint(), &frame(relay.key(), PATH, &body(false))).await;
    let split = response
        .windows(4)
        .position(|part| part == b"\r\n\r\n")
        .unwrap()
        + 4;
    assert_eq!(&response[split..], payload);
    let stats = relay.close().await.unwrap();
    assert_eq!(stats.verified_token_total(), Some(107));
    let stats = serde_json::to_value(stats).unwrap();
    assert_eq!(stats["responses"]["1"]["state"], "present");
    assert_eq!(stats["responses"]["1"]["usage"]["total_tokens"], 107);
    assert_eq!(stats["responses"]["1"]["usage"]["cache_read_tokens"], 80);
    assert!(!stats.to_string().contains("private-"));
}

#[tokio::test]
async fn raw_sse_usage_is_observed_without_changing_bytes_or_inventing_missing_counts() {
    for payload in [
        b"data: {\"usage\":{\"prompt_tokens\":2,\"completion_tokens\":1,\"total_tokens\":3}}\n\ndata: [DONE]\n\n".as_slice(),
        b"data: {\"choices\":[]}\n\ndata: [DONE]\n\n",
    ] {
        let fake = Fake::start(200, payload, true, None).await;
        let relay = Relay::start(fake.route(), 1, 512).await.unwrap();
        let response = request(relay.endpoint(), &frame(relay.key(), PATH, &body(true))).await;
        let split = response.windows(4).position(|part| part == b"\r\n\r\n").unwrap() + 4;
        assert_eq!(&response[split..], payload);
        let stats = serde_json::to_value(relay.close().await.unwrap()).unwrap();
        if payload.windows(5).any(|part| part == b"usage") {
            assert_eq!(stats["responses"]["1"]["state"], "present");
            assert_eq!(stats["responses"]["1"]["usage"]["total_tokens"], 3);
        } else {
            assert_eq!(stats["responses"]["1"]["state"], "missing");
            assert!(stats["responses"]["1"]["usage"].is_null());
        }
    }
}

#[tokio::test]
async fn wrong_credential_path_and_conflicting_framing_never_reach_upstream() {
    let fake = Fake::start(200, b"{}", false, None).await;
    let relay = Relay::start(fake.route(), 10, 512).await.unwrap();
    let wrong_key = request(relay.endpoint(), &frame("wrong", PATH, &body(false))).await;
    assert_eq!(status(&wrong_key), 401);
    let wrong_path = request(
        relay.endpoint(),
        &frame(relay.key(), "/admin", &body(false)),
    )
    .await;
    assert_eq!(status(&wrong_path), 404);
    let valid = String::from_utf8(frame(relay.key(), PATH, &body(false))).unwrap();
    for insertion in ["Content-Length: 1\r\n", "Transfer-Encoding: chunked\r\n"] {
        let invalid = valid.replacen("Content-Type:", &format!("{insertion}Content-Type:"), 1);
        assert_eq!(
            status(&request(relay.endpoint(), invalid.as_bytes()).await),
            400
        );
    }
    let stats = relay.close().await.unwrap();
    assert_eq!(stats.upstream_calls, 0);
    assert!(stats.requests.is_empty());
    assert!(stats.responses.is_empty());
    assert_eq!(fake.calls.load(Ordering::SeqCst), 0);
    let evidence = serde_json::to_value(stats).unwrap();
    assert_eq!(evidence["rejection_reasons"]["unauthorized"], 1);
    assert_eq!(evidence["rejection_reasons"]["target"], 1);
    assert_eq!(evidence["rejection_reasons"]["framing"], 2);
    assert!(!evidence.to_string().contains(UPSTREAM_KEY));
    assert!(!evidence.to_string().contains("/admin"));
}

#[tokio::test]
async fn unknown_model_and_unbounded_outputs_are_not_forwarded() {
    let fake = Fake::start(200, b"{}", false, None).await;
    let relay = Relay::start(fake.route(), 10, 512).await.unwrap();
    for invalid in [
        serde_json::json!({"model":"other","max_tokens":512}),
        serde_json::json!({"model":MODEL}),
        serde_json::json!({"model":MODEL,"max_tokens":0}),
        serde_json::json!({"model":MODEL,"max_tokens":513}),
        serde_json::json!({"model":MODEL,"max_tokens":1,"max_completion_tokens":1}),
    ] {
        assert_eq!(
            status(&request(relay.endpoint(), &frame(relay.key(), PATH, &invalid)).await),
            400
        );
    }
    let stats = relay.close().await.unwrap();
    assert_eq!(stats.upstream_calls, 0);
    assert_eq!(
        stats.rejection_reasons.get(&Reason::ModelOutputBudget),
        Some(&5)
    );
    assert_eq!(fake.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn rejection_reasons_round_trip_without_accepting_unknown_names() {
    for reason in Reason::ALL {
        assert_eq!(Reason::parse(reason.as_str()), Ok(reason));
        assert_eq!(serde_json::to_value(reason).unwrap(), reason.as_str());
        assert!([400, 401, 404, 411, 413, 429].contains(&reason.http_status()));
    }
    for unknown in ["", "provider_error", "request", "Unauthorized"] {
        assert!(Reason::parse(unknown).is_err());
    }
}

#[tokio::test]
async fn empty_and_incomplete_connections_have_bounded_rejection_evidence() {
    let fake = Fake::start(200, b"{}", false, None).await;
    let relay = Relay::start(fake.route(), 1, 512).await.unwrap();
    let url = url::Url::parse(relay.endpoint()).unwrap();
    for prefix in [b"".as_slice(), b"POST /v1/chat/completions HTTP/1.1\r\n"] {
        let mut socket = TcpStream::connect(("127.0.0.1", url.port().unwrap()))
            .await
            .unwrap();
        socket.write_all(prefix).await.unwrap();
        socket.shutdown().await.unwrap();
        let mut response = Vec::new();
        tokio::time::timeout(Duration::from_secs(3), socket.read_to_end(&mut response))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(status(&response), 400);
    }
    let stats = relay.close().await.unwrap();
    assert_eq!(stats.upstream_calls, 0);
    assert_eq!(fake.calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        stats.rejection_reasons.get(&Reason::EmptyConnection),
        Some(&1)
    );
    assert_eq!(
        stats.rejection_reasons.get(&Reason::IncompleteRequest),
        Some(&1)
    );
    assert_eq!(
        stats.rejected,
        stats.rejection_reasons.values().sum::<usize>()
    );
    assert!(serde_json::to_vec(&stats).unwrap().len() < 1024);
}

#[tokio::test]
async fn upstream_status_and_retry_after_survive_the_relay() {
    let fake = Fake::start(429, br#"{"error":"fixture rate limit"}"#, false, Some("3")).await;
    let relay = Relay::start(fake.route(), 1, 512).await.unwrap();
    let response = request(relay.endpoint(), &frame(relay.key(), PATH, &body(false))).await;
    assert_eq!(status(&response), 429);
    assert!(String::from_utf8_lossy(&response)
        .to_ascii_lowercase()
        .contains("retry-after: 3\r\n"));
    assert_eq!(relay.close().await.unwrap().upstream_calls, 1);
}

#[tokio::test]
async fn actual_transport_credentials_cannot_escape_in_a_provider_response() {
    let echoed = serde_json::json!({"error":UPSTREAM_KEY}).to_string();
    let fake = Fake::start(200, echoed.as_bytes(), false, None).await;
    let relay = Relay::start(fake.route(), 1, 512).await.unwrap();
    let response = request(relay.endpoint(), &frame(relay.key(), PATH, &body(false))).await;
    assert_eq!(status(&response), 502);
    assert!(!String::from_utf8_lossy(&response).contains(UPSTREAM_KEY));
    assert_eq!(relay.close().await.unwrap().protected_response_failures, 1);
}

#[tokio::test]
async fn sse_bytes_are_preserved_and_listener_is_closed_by_its_owner() {
    let sse = b"data: {\"choices\":[{\"delta\":{\"content\":\"pong\"}}]}\n\ndata: [DONE]\n\n";
    let fake = Fake::start(200, sse, true, None).await;
    let relay = Relay::start(fake.route(), 1, 512).await.unwrap();
    let endpoint = relay.endpoint().to_owned();
    let response = request(&endpoint, &frame(relay.key(), PATH, &body(true))).await;
    let split = response
        .windows(4)
        .position(|part| part == b"\r\n\r\n")
        .unwrap()
        + 4;
    assert_eq!(&response[split..], sse);
    let stats = relay.close().await.unwrap();
    assert_eq!(stats.response_bytes, sse.len());
    // Another concurrent fixture can reuse an ephemeral port after close.
    // Check this listener's drop and joined connections, not the new owner.
    assert!(stats.listener_closed);
}

#[test]
fn credential_comparison_checks_every_position_and_length() {
    assert!(credential_matches(b"abcdef", b"abcdef"));
    for changed in [
        b"xbcdef".as_slice(),
        b"abcxef",
        b"abcdex",
        b"abcde",
        b"abcdef0",
    ] {
        assert!(!credential_matches(b"abcdef", changed));
    }
}

#[test]
fn verified_token_total_requires_every_original_call_and_refuses_overflow() {
    let valid = || {
        usage::Observation::json(
            200,
            br#"{"usage":{"prompt_tokens":2,"completion_tokens":1,"total_tokens":3}}"#,
        )
    };
    let mut stats = Stats::default();
    assert_eq!(stats.verified_token_total(), None);
    stats.upstream_calls = 2;
    stats.responses.insert(1, valid());
    assert_eq!(stats.verified_token_total(), None);
    stats.responses.insert(2, Default::default());
    assert_eq!(stats.verified_token_total(), None);
    stats.responses.insert(2, valid());
    assert_eq!(stats.verified_token_total(), Some(6));
    let removed = stats.responses.remove(&1).unwrap();
    stats.responses.insert(3, removed);
    assert_eq!(
        stats.verified_token_total(),
        None,
        "response identity must match actual calls"
    );
    stats.responses.remove(&3);
    let huge = usage::Observation::json(200, br#"{"usage":{"prompt_tokens":18446744073709551615,"completion_tokens":0,"total_tokens":18446744073709551615}}"#);
    assert_eq!(huge.state, usage::State::Present);
    stats.responses.insert(1, huge);
    assert_eq!(stats.verified_token_total(), None);
}

#[tokio::test]
async fn closing_the_owner_cancels_an_admitted_pending_upstream_request() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}{PATH}", listener.local_addr().unwrap());
    let (ready, admitted) = tokio::sync::oneshot::channel();
    let (closed, upstream_eof) = tokio::sync::oneshot::channel();
    let worker = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        read_request(&mut socket, &format!("Bearer {UPSTREAM_KEY}"))
            .await
            .unwrap();
        ready.send(()).unwrap();
        let mut byte = [0];
        let count = tokio::time::timeout(Duration::from_secs(5), socket.read(&mut byte))
            .await
            .unwrap()
            .unwrap();
        closed.send(count).unwrap();
    });
    let fake = Fake {
        endpoint,
        calls: Arc::new(AtomicUsize::new(0)),
        worker,
    };
    let relay = Relay::start(fake.route(), 1, 512).await.unwrap();
    let endpoint = relay.endpoint().to_owned();
    let frame = frame(relay.key(), PATH, &body(false));
    let pending = tokio::spawn(async move { request(&endpoint, &frame).await });
    tokio::time::timeout(Duration::from_secs(5), admitted)
        .await
        .unwrap()
        .unwrap();
    let stats = relay.close().await.unwrap();
    assert_eq!(stats.upstream_calls, 1);
    let response = pending.await.unwrap();
    assert_eq!(stats.responses[&1].state, usage::State::Incomplete);
    assert_eq!(stats.verified_token_total(), None);
    // Cancellation can win before headers, or settle as a transport error.
    assert!(response.is_empty() || status(&response) == 502);
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(5), upstream_eof)
            .await
            .unwrap()
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn a_stream_cannot_echo_actual_upstream_credentials() {
    let sse = format!("data: {{\"content\":\"{UPSTREAM_KEY}\"}}\n\n");
    let fake = Fake::start(200, sse.as_bytes(), true, None).await;
    let relay = Relay::start(fake.route(), 1, 512).await.unwrap();
    let response = request(relay.endpoint(), &frame(relay.key(), PATH, &body(true))).await;
    assert!(!String::from_utf8_lossy(&response).contains(UPSTREAM_KEY));
    assert_eq!(relay.close().await.unwrap().protected_response_failures, 1);
}
