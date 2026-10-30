use super::*;
use crate::inference::decision::{DecisionContentPart, DecisionLevel};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn request() -> DecisionRequest {
    DecisionRequest {
        decision: DecisionDefinition::Ordinal {
            instructions: "Assess difficulty".into(),
            levels: vec![
                DecisionLevel {
                    id: "simple".into(),
                    criterion: "Routine turn".into(),
                },
                DecisionLevel {
                    id: "complex".into(),
                    criterion: "Multi-stage reasoning".into(),
                },
            ],
        },
        latest_user: vec![DecisionContentPart::Text {
            text: "中文🦀".repeat(2000),
        }],
        visible_conversation: vec![],
        history_partial: false,
        assessment_target: None,
    }
}
fn answer() -> serde_json::Value {
    serde_json::json!({"decision":{"kind":"ordinal","probabilities":{"simple":0.95,"complex":0.05}}})
}
fn chat(content: serde_json::Value) -> serde_json::Value {
    serde_json::json!({"choices":[{"finish_reason":"stop","message":{"role":"assistant","content":content.to_string()}}]})
}

#[test]
fn decision_protocol_is_closed_and_round_trips() {
    use crate::config::InferenceDecisionProtocol as Protocol;
    for p in [Protocol::Decisions, Protocol::OpenaiChat, Protocol::Apofasi] {
        assert_eq!(Protocol::parse(p.as_str()).unwrap(), p);
    }
    assert!(Protocol::parse("guessed").is_err());
    assert!(serde_json::from_str::<Protocol>("\"guessed\"").is_err());
}
#[test]
fn unicode_chat_uses_bounded_typed_document_without_truncation() {
    let service = HttpDecisionTransport::new_chat("http://unused".into(), None, "internal".into());
    let req = request();
    let body: serde_json::Value =
        serde_json::from_slice(&service.wire_body(&req).unwrap()).unwrap();
    let document: serde_json::Value =
        serde_json::from_str(body["messages"][1]["content"].as_str().unwrap()).unwrap();
    assert_eq!(document["latest_user"][0]["text"], "中文🦀".repeat(2000));
    assert_eq!(body["model"], "internal");
    assert_eq!(body["stream"], false);
    assert!(body.get("tools").is_none());
}
#[test]
fn oversized_turn_is_refused_before_send() {
    let mut req = request();
    req.latest_user = vec![DecisionContentPart::Text {
        text: "x".repeat(65536),
    }];
    let service = HttpDecisionTransport::new_chat("http://unused".into(), None, "internal".into());
    assert!(service.wire_body(&req).is_err());
}
#[test]
fn decision_chat_strictly_validates_model_output() {
    let service = HttpDecisionTransport::new_chat("http://unused".into(), None, "internal".into());
    let req = request();
    assert!(service
        .decode(&serde_json::to_vec(&chat(answer())).unwrap(), &req)
        .is_ok());
    let mut variants = Vec::new();
    let valid = chat(answer());
    for field in ["length", "tool_calls", "content_filter"] {
        let mut v = valid.clone();
        v["choices"][0]["finish_reason"] = field.into();
        variants.push(v);
    }
    let mut v = valid.clone();
    v["choices"][0]["message"]["tool_calls"] = serde_json::json!([]);
    variants.push(v);
    let mut v = valid.clone();
    v["choices"][0]["message"]["content"] = "```json\n{}\n```".into();
    variants.push(v);
    let mut v = valid.clone();
    v["choices"]
        .as_array_mut()
        .unwrap()
        .push(valid["choices"][0].clone());
    variants.push(v);
    variants.push(chat(
        serde_json::json!({"decision":{"kind":"ordinal","probabilities":{"other":1.0}}}),
    ));
    variants.push(chat(serde_json::json!({"decision":{"kind":"ordinal","probabilities":{"simple":0.8,"complex":0.8}}})));
    variants.push(chat(serde_json::json!({"decision":{"kind":"ordinal","probabilities":{"simple":0.95,"complex":0.05}},"model":"injected"})));
    for v in variants {
        assert!(
            service
                .decode(&serde_json::to_vec(&v).unwrap(), &req)
                .is_err(),
            "{v}"
        );
    }
}
async fn server(response: Vec<u8>) -> (String, tokio::sync::oneshot::Receiver<Vec<u8>>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let (tx, rx) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut received = Vec::new();
        loop {
            let mut chunk = [0u8; 4096];
            let count = socket.read(&mut chunk).await.unwrap();
            if count == 0 {
                break;
            }
            received.extend_from_slice(&chunk[..count]);
            assert!(received.len() < 128 * 1024);
            if let Some(end) = received.windows(4).position(|w| w == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&received[..end]).to_ascii_lowercase();
                let length = headers
                    .lines()
                    .find_map(|line| line.strip_prefix("content-length:"))
                    .unwrap()
                    .trim()
                    .parse::<usize>()
                    .unwrap();
                if received.len() >= end + 4 + length {
                    break;
                }
            }
        }
        let _ = tx.send(received);
        let _ = socket.write_all(&response).await;
    });
    (url, rx)
}
fn response(body: &[u8]) -> Vec<u8> {
    let mut bytes=format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len()).into_bytes();
    bytes.extend_from_slice(body);
    bytes
}
#[tokio::test]
async fn native_contract_keeps_full_authorization_header() {
    let (url, rx) = server(response(&serde_json::to_vec(&answer()).unwrap())).await;
    let service = HttpDecisionTransport::new(url, Some("Bearer upstream-fixture".into()));
    assert!(service.decide(&request()).await.is_ok());
    let sent = String::from_utf8(rx.await.unwrap()).unwrap();
    assert!(sent.starts_with("POST /v1/decisions "));
    assert!(sent
        .to_ascii_lowercase()
        .contains("authorization: bearer upstream-fixture\r\n"));
    assert!(!sent.contains("Bearer Bearer"));
}
#[tokio::test]
async fn neural_chat_calls_configured_backend_without_gateway_recursion() {
    let (url, rx) = server(response(&serde_json::to_vec(&chat(answer())).unwrap())).await;
    let service = HttpDecisionTransport::new_chat(
        format!("{url}/v1"),
        Some("Bearer fixture".into()),
        "prism-internal".into(),
    );
    assert!(service.decide(&request()).await.is_ok());
    let sent = String::from_utf8(rx.await.unwrap()).unwrap();
    assert!(sent.starts_with("POST /v1/chat/completions "));
    let body: serde_json::Value =
        serde_json::from_str(sent.split_once("\r\n\r\n").unwrap().1).unwrap();
    assert_eq!(body["model"], "prism-internal");
}
#[tokio::test]
async fn redirects_are_refused_instead_of_forwarding_secrets() {
    let (url,_)=server(b"HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:1/\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec()).await;
    let service = HttpDecisionTransport::new(url, Some("Bearer fixture".into()));
    assert!(matches!(
        service.decide(&request()).await,
        Err(DecisionError::Unreachable(_))
    ));
}
#[tokio::test]
async fn declared_oversized_response_is_refused() {
    let (url, _) =
        server(b"HTTP/1.1 200 OK\r\nContent-Length: 65537\r\nConnection: close\r\n\r\n".to_vec())
            .await;
    let service = HttpDecisionTransport::new(url, None);
    assert!(matches!(
        service.decide(&request()).await,
        Err(DecisionError::Invalid(_))
    ));
}
#[tokio::test]
async fn chunked_oversized_response_is_refused() {
    let mut bytes =
        b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n10001\r\n"
            .to_vec();
    bytes.extend_from_slice(&vec![b'x'; 65537]);
    bytes.extend_from_slice(b"\r\n0\r\n\r\n");
    let (url, _) = server(bytes).await;
    let service = HttpDecisionTransport::new(url, None);
    assert!(matches!(
        service.decide(&request()).await,
        Err(DecisionError::Invalid(_))
    ));
}
