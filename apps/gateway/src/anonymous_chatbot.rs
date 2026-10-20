//! Anonymous Chatbot consumer for the existing Applications delivery API.
//!
//! `POST /v1/chatbot` opens one session and one streaming invocation, then
//! writes ordered Answer frames. Workflow still owns the run. This path is
//! absent from the Cloud OpenAPI document.

use crate::config::AnonymousChatbotUpstream;
use crate::error::json_error_body;
use crate::response_body::ResponseBody;
use bytes::Bytes;
use http_body_util::{BodyExt, Limited};
use hyper::body::{Body, Frame, Incoming};
use serde_json::{json, Value};
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};
use uuid::Uuid;

pub(crate) const PATH: &str = "/v1/chatbot";
const BODY_LIMIT: usize = 64 * 1024;
const POLL_DEADLINE: Duration = Duration::from_secs(15);
const POLL_PAUSE: Duration = Duration::from_millis(40);
const ADMISSION_TIMEOUT: Duration = Duration::from_secs(10);
const UPSTREAM_TIMEOUT: Duration = Duration::from_secs(2);

pub(crate) fn owns_path(path: &str) -> bool {
    path == PATH
}

pub(crate) async fn handle(
    request: hyper::Request<Incoming>,
    upstream: Option<AnonymousChatbotUpstream>,
) -> hyper::Response<ResponseBody> {
    let Some(upstream) = upstream else {
        return json_status(404, "anonymous chatbot is not configured");
    };
    if request.method() != http::Method::POST {
        let mut response = json_status(405, "anonymous chatbot accepts POST");
        response
            .headers_mut()
            .insert(http::header::ALLOW, http::HeaderValue::from_static("POST"));
        return response;
    }
    let input = match collect_input(request.into_body()).await {
        Ok(input) => input,
        Err(error) => return error,
    };
    let client = match reqwest::Client::builder()
        .timeout(ADMISSION_TIMEOUT)
        .build()
    {
        Ok(client) => client,
        Err(_) => return json_status(500, "anonymous chatbot client failed"),
    };
    let session_id = Uuid::new_v4();
    let invocation_id = Uuid::new_v4();
    let admitted = match admit(&client, &upstream, session_id, invocation_id, &input).await {
        Ok(admitted) => admitted,
        Err(error) => return error,
    };
    let (sender, receiver) = tokio::sync::mpsc::channel(16);
    let admission = format!(
        "{}\n",
        json!({
            "kind": "admission",
            "sessionId": session_id,
            "invocationId": invocation_id,
            "workflowRunId": admitted.workflow_run_id,
        })
    );
    tokio::spawn(stream_answers(
        client, upstream, admitted, sender, admission,
    ));
    let mut response = hyper::Response::new(ResponseBody::boxed(NdjsonBody { receiver }));
    *response.status_mut() = http::StatusCode::OK;
    response.headers_mut().insert(
        http::header::CONTENT_TYPE,
        http::HeaderValue::from_static("application/x-ndjson"),
    );
    response
}

struct Admission {
    session_id: Uuid,
    invocation_id: Uuid,
    workflow_run_id: Uuid,
}

async fn admit(
    client: &reqwest::Client,
    upstream: &AnonymousChatbotUpstream,
    session_id: Uuid,
    invocation_id: Uuid,
    input: &Value,
) -> Result<Admission, hyper::Response<ResponseBody>> {
    let session_url = format!(
        "{}/api/v1/anonymous-delivery/organizations/{}/projects/{}/applications/{}/sessions",
        upstream.upstream, upstream.organization_id, upstream.project_id, upstream.application_id
    );
    let session = post_json(
        client,
        &session_url,
        json!({
            "sessionId": session_id,
            "releaseId": upstream.release_id,
            "lookupKey": upstream.lookup_key,
            "initialVariables": {},
        }),
    )
    .await?;
    let aggregate_version = session
        .pointer("/data/session/aggregateVersion")
        .and_then(Value::as_u64)
        .ok_or_else(|| json_status(502, "anonymous chatbot session response is incomplete"))?;
    let invocation_url = format!("{session_url}/{session_id}/invocations");
    let invocation = post_json(
        client,
        &invocation_url,
        json!({
            "invocationId": invocation_id,
            "expectedSessionVersion": aggregate_version,
            "lookupKey": upstream.lookup_key,
            "responseMode": "streaming",
            "input": input,
            "ontologyId": upstream.ontology_id,
            "ontologyRevisionId": upstream.ontology_revision_id,
            "ontologyDigest": upstream.ontology_digest,
        }),
    )
    .await?;
    let workflow_run_id = invocation
        .pointer("/data/workflow/workflowRunId")
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
        .filter(|value| !value.is_nil())
        .ok_or_else(|| json_status(502, "anonymous chatbot invocation response is incomplete"))?;
    let _ = aggregate_version;
    Ok(Admission {
        session_id,
        invocation_id,
        workflow_run_id,
    })
}

async fn post_json(
    client: &reqwest::Client,
    url: &str,
    body: Value,
) -> Result<Value, hyper::Response<ResponseBody>> {
    let response = client
        .post(url)
        .json(&body)
        .send()
        .await
        .map_err(|_| json_status(502, "anonymous chatbot upstream is unreachable"))?;
    read_success(response).await
}

async fn read_success(response: reqwest::Response) -> Result<Value, hyper::Response<ResponseBody>> {
    let status = response.status();
    if status.as_u16() == 401 || status.as_u16() == 403 || status.as_u16() == 404 {
        return Err(json_status(
            status.as_u16(),
            "anonymous chatbot upstream refused the request",
        ));
    }
    if status.as_u16() != 200 && status.as_u16() != 201 {
        return Err(json_status(
            502,
            "anonymous chatbot upstream rejected admission",
        ));
    }
    response
        .json()
        .await
        .map_err(|_| json_status(502, "anonymous chatbot upstream returned invalid JSON"))
}

async fn stream_answers(
    client: reqwest::Client,
    upstream: AnonymousChatbotUpstream,
    admitted: Admission,
    sender: tokio::sync::mpsc::Sender<io::Result<Bytes>>,
    admission: String,
) {
    if sender.send(Ok(Bytes::from(admission))).await.is_err() {
        return;
    }
    let deadline = Instant::now() + POLL_DEADLINE;
    let mut after_sequence = 0_u64;
    let mut last_answer = 0_u64;
    loop {
        if Instant::now() >= deadline {
            let _ = sender
                .send(Ok(Bytes::from(format!(
                    "{}\n",
                    json!({"kind": "error", "message": "anonymous chatbot stream deadline"})
                ))))
                .await;
            return;
        }
        match poll_observation(&client, &upstream, &admitted, after_sequence).await {
            Observation::Page(page) => {
                for frame in &page.frames {
                    if frame.kind != "answer" {
                        continue;
                    }
                    if frame.sequence <= last_answer {
                        continue;
                    }
                    let line = format!(
                        "{}\n",
                        json!({
                            "kind": "answer",
                            "sequence": frame.sequence,
                            "messageId": frame.message_id,
                        })
                    );
                    if sender.send(Ok(Bytes::from(line))).await.is_err() {
                        return;
                    }
                    last_answer = frame.sequence;
                }
                if page.stream_status != "open" {
                    return;
                }
                if page.next_sequence < after_sequence {
                    let _ = sender
                        .send(Ok(Bytes::from(
                            format!(
                                "{}\n",
                                json!({"kind": "error", "message": "anonymous chatbot cursor moved backward"})
                            ),
                        )))
                        .await;
                    return;
                }
                let paused = page.next_sequence == after_sequence || !page.has_more;
                after_sequence = page.next_sequence;
                if paused {
                    tokio::time::sleep(POLL_PAUSE).await;
                }
            }
            Observation::Retry => tokio::time::sleep(POLL_PAUSE).await,
            Observation::Fail(message) => {
                let _ = sender
                    .send(Ok(Bytes::from(format!(
                        "{}\n",
                        json!({"kind": "error", "message": message})
                    ))))
                    .await;
                return;
            }
        }
    }
}

struct ObservationPage {
    stream_status: String,
    next_sequence: u64,
    has_more: bool,
    frames: Vec<ObservationFrame>,
}

struct ObservationFrame {
    sequence: u64,
    kind: String,
    message_id: String,
}

enum Observation {
    Page(ObservationPage),
    Retry,
    Fail(&'static str),
}

async fn poll_observation(
    client: &reqwest::Client,
    upstream: &AnonymousChatbotUpstream,
    admitted: &Admission,
    after_sequence: u64,
) -> Observation {
    let url = format!(
        "{}/api/v1/anonymous-delivery/organizations/{}/projects/{}/applications/{}/sessions/{}/invocations/{}/streaming-observation",
        upstream.upstream,
        upstream.organization_id,
        upstream.project_id,
        upstream.application_id,
        admitted.session_id,
        admitted.invocation_id
    );
    let after_sequence_text = after_sequence.to_string();
    let response = match client
        .get(url)
        .timeout(UPSTREAM_TIMEOUT)
        .query(&[
            ("lookupKey", upstream.lookup_key.as_str()),
            ("afterSequence", after_sequence_text.as_str()),
        ])
        .send()
        .await
    {
        Ok(response) => response,
        Err(_) => return Observation::Retry,
    };
    let status = response.status().as_u16();
    if matches!(status, 400 | 401 | 403 | 404) {
        return Observation::Fail("anonymous chatbot observation was refused");
    }
    if status != 200 {
        let body = response.text().await.unwrap_or_default();
        if body.contains("cannot expose FinalOutput before terminal")
            || body.contains("Succeeded invocation requires Input and FinalOutput")
        {
            return Observation::Retry;
        }
        return Observation::Retry;
    }
    let value: Value = match response.json().await {
        Ok(value) => value,
        Err(_) => return Observation::Retry,
    };
    let data = value.get("data").unwrap_or(&value);
    let stream_status = data
        .get("streamStatus")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    if !matches!(
        stream_status.as_str(),
        "open" | "succeeded" | "failed" | "cancelled"
    ) {
        return Observation::Retry;
    }
    let next_sequence = data
        .get("nextSequence")
        .and_then(Value::as_u64)
        .unwrap_or(after_sequence);
    let has_more = data
        .get("hasMore")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let mut frames = Vec::new();
    if let Some(items) = data.get("frames").and_then(Value::as_array) {
        for item in items {
            let Some(sequence) = item.get("sequence").and_then(Value::as_u64) else {
                continue;
            };
            let Some(kind) = item.get("kind").and_then(Value::as_str) else {
                continue;
            };
            let message_id = item
                .get("messageId")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            frames.push(ObservationFrame {
                sequence,
                kind: kind.to_string(),
                message_id,
            });
        }
    }
    Observation::Page(ObservationPage {
        stream_status,
        next_sequence,
        has_more,
        frames,
    })
}

async fn collect_input(body: Incoming) -> Result<Value, hyper::Response<ResponseBody>> {
    let collected = match Limited::new(body, BODY_LIMIT).collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(_) => return Err(json_status(413, "anonymous chatbot body is too large")),
    };
    if collected.is_empty() {
        return Ok(json!({}));
    }
    match serde_json::from_slice::<Value>(&collected) {
        Ok(Value::Object(object)) => Ok(Value::Object(object)),
        _ => Err(json_status(
            400,
            "anonymous chatbot body must be a JSON object",
        )),
    }
}

fn json_status(status: u16, message: &str) -> hyper::Response<ResponseBody> {
    let mut response = hyper::Response::new(ResponseBody::full(json_error_body(message)));
    *response.status_mut() =
        http::StatusCode::from_u16(status).unwrap_or(http::StatusCode::INTERNAL_SERVER_ERROR);
    response.headers_mut().insert(
        http::header::CONTENT_TYPE,
        http::HeaderValue::from_static("application/json"),
    );
    response
}

struct NdjsonBody {
    receiver: tokio::sync::mpsc::Receiver<io::Result<Bytes>>,
}

impl Body for NdjsonBody {
    type Data = Bytes;
    type Error = io::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Option<io::Result<Frame<Self::Data>>>> {
        match self.receiver.poll_recv(context) {
            Poll::Ready(Some(Ok(bytes))) => Poll::Ready(Some(Ok(Frame::data(bytes)))),
            Poll::Ready(Some(Err(error))) => Poll::Ready(Some(Err(error))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{EntrypointConfig, GatewayConfig};
    use crate::Gateway;
    use http_body_util::Full;
    use hyper::server::conn::http1;
    use hyper::service::service_fn;
    use hyper_util::rt::TokioIo;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;
    use tokio::net::TcpListener;

    fn sample_block(upstream: &str) -> String {
        format!(
            r#"
anonymous_chatbot {{
  upstream = "{upstream}"
  organization_id = "11111111-1111-4111-8111-111111111111"
  project_id = "22222222-2222-4222-8222-222222222222"
  application_id = "33333333-3333-4333-8333-333333333333"
  release_id = "44444444-4444-4444-8444-444444444444"
  lookup_key = "ga2-anonymous-chatbot"
  ontology_id = "55555555-5555-4555-8555-555555555555"
  ontology_revision_id = "66666666-6666-4666-8666-666666666666"
  ontology_digest = "sha256:{digest}"
}}
"#,
            digest = "a".repeat(64),
        )
    }

    #[test]
    fn anonymous_chatbot_block_fails_closed() {
        let upstream = sample_block("http://127.0.0.1:9");
        let parsed = GatewayConfig::from_acl(&upstream).expect("chatbot block");
        let chatbot = parsed.anonymous_chatbot.expect("configured");
        assert_eq!(chatbot.lookup_key, "ga2-anonymous-chatbot");
        assert!(
            GatewayConfig::from_acl("entrypoints \"web\" { address = \"127.0.0.1:1\" }\n")
                .expect("plain config")
                .anonymous_chatbot
                .is_none()
        );
        assert!(GatewayConfig::from_acl(&format!("{upstream}\n{upstream}")).is_err());
        assert!(GatewayConfig::from_acl(
            r#"
anonymous_chatbot {
  upstream = "http://127.0.0.1:9"
  nested { value = "no" }
}
"#
        )
        .is_err());
        assert!(GatewayConfig::from_acl(&sample_block("ftp://127.0.0.1:9")).is_err());
        assert!(GatewayConfig::from_acl(
            r#"
anonymous_chatbot {
  upstream = "http://127.0.0.1:9"
  organization_id = "11111111-1111-4111-8111-111111111111"
  project_id = "22222222-2222-4222-8222-222222222222"
  application_id = "33333333-3333-4333-8333-333333333333"
  release_id = "44444444-4444-4444-8444-444444444444"
  lookup_key = ""
  ontology_id = "55555555-5555-4555-8555-555555555555"
  ontology_revision_id = "66666666-6666-4666-8666-666666666666"
  ontology_digest = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
}
"#
        )
        .is_err());
    }

    async fn start_gateway(config: GatewayConfig) -> (Arc<Gateway>, u16) {
        let port = {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            listener.local_addr().unwrap().port()
        };
        let mut config = config;
        config.entrypoints.clear();
        config.entrypoints.insert(
            "web".to_string(),
            EntrypointConfig::new(format!("127.0.0.1:{port}")),
        );
        let gateway = Arc::new(Gateway::new(config).expect("gateway"));
        gateway.start().await.expect("start");
        for _ in 0..50 {
            if tokio::net::TcpStream::connect(("127.0.0.1", port))
                .await
                .is_ok()
            {
                return (gateway, port);
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        panic!("gateway did not accept");
    }

    #[tokio::test]
    async fn unconfigured_chatbot_does_not_fall_through() {
        let mut config = GatewayConfig::default();
        config.services.insert(
            "api".to_string(),
            crate::config::ServiceConfig {
                load_balancer: crate::config::LoadBalancerConfig {
                    strategy: crate::config::Strategy::RoundRobin,
                    request_timeout: "1s".to_string(),
                    stream_idle_timeout: "5m".to_string(),
                    stream_total_timeout: "60m".to_string(),
                    connect_timeout: "1s".to_string(),
                    servers: vec![crate::config::ServerConfig {
                        url: "http://127.0.0.1:9".to_string(),
                        weight: 1,
                        target: None,
                    }],
                    health_check: None,
                    sticky: None,
                    tls_ca_file: None,
                },
                scaling: None,
                revisions: Vec::new(),
                rollout: None,
                mirror: None,
                failover: None,
            },
        );
        config.routers.insert(
            "all".to_string(),
            crate::config::RouterConfig {
                rule: "PathPrefix(`/`)".to_string(),
                service: "api".to_string(),
                entrypoints: vec!["web".to_string()],
                middlewares: Vec::new(),
                priority: 0,
                parking: None,
            },
        );
        let (_gateway, port) = start_gateway(config).await;
        let response = reqwest::Client::new()
            .post(format!("http://127.0.0.1:{port}/v1/chatbot"))
            .json(&json!({"ticketId": "T-42"}))
            .send()
            .await
            .expect("request");
        assert_eq!(response.status(), 404);
    }

    #[tokio::test]
    async fn configured_chatbot_streams_answers_in_order() {
        let polls = Arc::new(AtomicU64::new(0));
        let polls_for_stub = polls.clone();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let stub_port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else {
                    break;
                };
                let polls = polls_for_stub.clone();
                tokio::spawn(async move {
                    let service = service_fn(move |request: hyper::Request<Incoming>| {
                        let polls = polls.clone();
                        async move { Ok::<_, io::Error>(stub_response(request, polls).await) }
                    });
                    let _ = http1::Builder::new()
                        .serve_connection(TokioIo::new(stream), service)
                        .await;
                });
            }
        });
        let mut config = GatewayConfig::from_acl(&format!(
            "{}\nentrypoints \"web\" {{ address = \"127.0.0.1:1\" }}\n",
            sample_block(&format!("http://127.0.0.1:{stub_port}"))
        ))
        .expect("acl");
        config.anonymous_chatbot.as_mut().unwrap().upstream =
            format!("http://127.0.0.1:{stub_port}");
        let (_gateway, port) = start_gateway(config).await;
        let response = reqwest::Client::new()
            .post(format!("http://127.0.0.1:{port}/v1/chatbot"))
            .json(&json!({"ticketId": "T-42"}))
            .send()
            .await
            .expect("chatbot");
        assert_eq!(response.status(), 200);
        let body = response.text().await.expect("body");
        let lines: Vec<Value> = body
            .lines()
            .filter(|line| !line.is_empty())
            .map(|line| serde_json::from_str(line).expect("ndjson"))
            .collect();
        assert_eq!(lines[0]["kind"], "admission");
        assert!(lines[0]["workflowRunId"].as_str().is_some());
        let answers: Vec<u64> = lines
            .iter()
            .filter(|line| line["kind"] == "answer")
            .map(|line| line["sequence"].as_u64().unwrap())
            .collect();
        assert_eq!(answers, vec![2, 3]);
        assert!(lines.iter().all(|line| line["kind"] != "input"));
        assert!(lines.iter().all(|line| line["kind"] != "error"));
        let method = reqwest::Client::new()
            .get(format!("http://127.0.0.1:{port}/v1/chatbot"))
            .send()
            .await
            .expect("method");
        assert_eq!(method.status(), 405);
        assert!(polls.load(Ordering::SeqCst) >= 2);
    }

    async fn stub_response(
        request: hyper::Request<Incoming>,
        polls: Arc<AtomicU64>,
    ) -> hyper::Response<Full<Bytes>> {
        let path = request.uri().path().to_string();
        let method = request.method().clone();
        let query = request.uri().query().unwrap_or("").to_string();
        let _ = request.into_body().collect().await;
        let workflow_run_id = "77777777-7777-4777-8777-777777777777";
        let body = if method == http::Method::POST && path.ends_with("/sessions") {
            json!({"code": 201, "message": "Success", "data": {"session": {"aggregateVersion": 1}, "replayed": false}}).to_string()
        } else if method == http::Method::POST && path.ends_with("/invocations") {
            json!({"code": 201, "message": "Success", "data": {"workflow": {"workflowRunId": workflow_run_id}, "replayed": false}}).to_string()
        } else if method == http::Method::GET && path.ends_with("/streaming-observation") {
            let after = query
                .split('&')
                .find_map(|item| item.strip_prefix("afterSequence="))
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(0);
            polls.fetch_add(1, Ordering::SeqCst);
            let (frames, status, next) = if after < 1 {
                (
                    json!([{"messageId": "88888888-8888-4888-8888-888888888881", "sequence": 1, "kind": "input"}]),
                    "open",
                    1,
                )
            } else if after < 2 {
                (
                    json!([{"messageId": "88888888-8888-4888-8888-888888888882", "sequence": 2, "kind": "answer"}]),
                    "open",
                    2,
                )
            } else if after < 3 {
                (
                    json!([{"messageId": "88888888-8888-4888-8888-888888888883", "sequence": 3, "kind": "answer"}]),
                    "open",
                    3,
                )
            } else {
                (json!([]), "succeeded", after)
            };
            json!({
                "code": 200,
                "message": "Success",
                "data": {
                    "streamStatus": status,
                    "nextSequence": next,
                    "hasMore": false,
                    "frames": frames
                }
            })
            .to_string()
        } else {
            json!({"code": 404, "message": "missing"}).to_string()
        };
        let mut response = hyper::Response::new(Full::new(Bytes::from(body)));
        *response.status_mut() = if path.ends_with("/sessions") || path.ends_with("/invocations") {
            http::StatusCode::CREATED
        } else if path.ends_with("/streaming-observation") {
            http::StatusCode::OK
        } else {
            http::StatusCode::NOT_FOUND
        };
        response.headers_mut().insert(
            http::header::CONTENT_TYPE,
            http::HeaderValue::from_static("application/json"),
        );
        response
    }
}
