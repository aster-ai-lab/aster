#![cfg(test)]

use super::super::workbench_proof::{Finding, FindingStatus, Policy};
use super::*;

fn proof() -> Proof {
    let mut proof = Proof::new(
        "s",
        "m",
        "r",
        "model",
        "检查结果",
        Policy {
            review_mode: true,
            artifact_trace: false,
        },
    );
    proof.source_text = "The final report exists on disk.".into();
    proof
}

#[test]
fn independent_output_requires_exact_evidence_not_keyword_heuristics() {
    let proof = proof();
    let valid = r#"{"findings":[{"severity":"high","category":"change","claim":"缺少写入证据","evidenceQuote":"report exists on disk","guidance":"核实文件"}],"resolutions":[]}"#;
    assert_eq!(
        validate_result(valid, &proof, None).unwrap().findings.len(),
        1
    );
    assert!(
        validate_result(
            &valid.replace("report exists on disk", "invented quote"),
            &proof,
            None
        )
        .is_err()
    );
    assert!(validate_result("not JSON", &proof, None).is_err());
    assert!(
        validate_result(
            r#"{"findings":[],"resolutions":[],"unexpected":true}"#,
            &proof,
            None
        )
        .is_err()
    );
    assert!(
        validate_result(
            &valid.replace("\"high\"", "\"critical-unknown\""),
            &proof,
            None
        )
        .is_err()
    );
}

#[test]
fn repair_success_does_not_optimistically_resolve_findings() {
    let mut original = proof();
    original.review.findings.push(Finding {
        id: "finding-1".into(),
        severity: Severity::High,
        category: Category::Change,
        claim: "claim".into(),
        evidence_quote: "report exists".into(),
        guidance: "verify".into(),
        status: FindingStatus::Pending,
        reply: None,
        receipt_digest: None,
    });
    let repair = proof();
    assert!(
        validate_result(
            r#"{"findings":[],"resolutions":[]}"#,
            &original,
            Some(&repair)
        )
        .is_err()
    );
    let checked = r#"{"findings":[],"resolutions":[{"findingId":"finding-1","resolved":true,"reason":"已对照实际内容"}]}"#;
    assert!(validate_result(checked, &original, Some(&repair)).is_ok());
    assert!(
        validate_result(
            &checked.replace("finding-1", "other-task"),
            &original,
            Some(&repair)
        )
        .is_err()
    );
    assert!(validate_result(checked, &original, None).is_err());
}

#[tokio::test]
async fn gateway_call_is_independent_and_has_no_tool_capability() {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let thread = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut request = Vec::new();
        let mut chunk = [0u8; 4096];
        loop {
            let n = socket.read(&mut chunk).unwrap();
            if n == 0 {
                break;
            }
            request.extend_from_slice(&chunk[..n]);
            let text = String::from_utf8_lossy(&request);
            if let Some(end) = text.find("\r\n\r\n") {
                let length: usize = text[..end]
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .and_then(|length| length.trim().parse().ok())
                    })
                    .unwrap();
                if request.len() >= end + 4 + length {
                    break;
                }
            }
        }
        let text = String::from_utf8(request).unwrap();
        assert!(text.starts_with("POST /v1/chat/completions "));
        let body: serde_json::Value =
            serde_json::from_str(text.split_once("\r\n\r\n").unwrap().1).unwrap();
        assert!(body.get("tools").is_none());
        assert_eq!(body["messages"][0]["role"], "system");
        assert!(!body["messages"].to_string().contains("secret-test-token"));
        assert_eq!(body["temperature"], 0);
        assert_eq!(body["response_format"]["type"], "json_object");
        assert_eq!(body["thinking"]["type"], "disabled");
        let evidence: serde_json::Value =
            serde_json::from_str(body["messages"][1]["content"][0]["text"].as_str().unwrap())
                .unwrap();
        assert!(evidence["original"]["review"].get("status").is_none());
        assert!(evidence["original"].get("policy").is_none());
        assert_eq!(evidence["original"]["sourceText"], proof().source_text);
        let result=json!({"choices":[{"finish_reason":"stop","message":{"content":"{\"findings\":[],\"resolutions\":[]}"}}]}).to_string();
        write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",result.len(),result).unwrap();
    });
    let result = review(
        ProviderCredentials {
            base_url: format!("http://{address}/v1"),
            api_key: "secret-test-token".into(),
        },
        "model",
        &proof(),
        None,
        &[],
    )
    .await
    .unwrap();
    assert!(result.findings.is_empty());
    thread.join().unwrap();
}

async fn protocol_correction(second_valid: bool) -> Result<ReviewResult, String> {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let mut original_messages = None;
        for attempt in 0..2 {
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            let mut socket = loop {
                match listener.accept() {
                    Ok((socket, _)) => break socket,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(
                            std::time::Instant::now() < deadline,
                            "missing bounded correction request"
                        );
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Err(error) => panic!("{error}"),
                }
            };
            socket.set_nonblocking(false).unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = Vec::new();
            let mut chunk = [0; 4096];
            loop {
                let n = socket.read(&mut chunk).unwrap();
                assert_ne!(n, 0);
                request.extend_from_slice(&chunk[..n]);
                let text = String::from_utf8_lossy(&request);
                if let Some(end) = text.find("\r\n\r\n") {
                    let length: usize = text[..end]
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .and_then(|length| length.trim().parse().ok())
                        })
                        .unwrap();
                    if request.len() >= end + 4 + length {
                        break;
                    }
                }
            }
            let text = String::from_utf8(request).unwrap();
            let body: serde_json::Value =
                serde_json::from_str(text.split_once("\r\n\r\n").unwrap().1).unwrap();
            assert!(body.get("tools").is_none());
            assert!(!body["messages"].to_string().contains("secret-test-token"));
            let messages = body["messages"].as_array().unwrap();
            if attempt == 0 {
                assert_eq!(messages.len(), 2);
                original_messages = Some(messages.clone());
            } else {
                assert_eq!(&messages[..2], original_messages.as_ref().unwrap());
                assert_eq!(messages.len(), 3);
                assert!(
                    messages[2]["content"]
                        .as_str()
                        .unwrap()
                        .contains("exact substring")
                );
            }
            let quote = if attempt == 1 && second_valid {
                "report exists on disk"
            } else {
                "unseen sentence"
            };
            let result = json!({"findings":[{"severity":"high","category":"change","claim":"Need actual evidence","evidenceQuote":quote,"guidance":"Verify the report"}],"resolutions":[]}).to_string();
            let response =
                json!({"choices":[{"finish_reason":"stop","message":{"content":result}}]})
                    .to_string();
            write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",response.len(),response).unwrap();
        }
    });
    let result = review(
        ProviderCredentials {
            base_url: format!("http://{address}/v1"),
            api_key: "secret-test-token".into(),
        },
        "model",
        &proof(),
        None,
        &[],
    )
    .await;
    server.join().unwrap();
    result
}

#[tokio::test]
async fn a_bounded_protocol_correction_keeps_the_finding_and_the_original_evidence() {
    let result = protocol_correction(true).await.unwrap();
    assert_eq!(result.findings.len(), 1);
    assert_eq!(result.findings[0].evidence_quote, "report exists on disk");
}

#[tokio::test]
async fn two_invalid_protocol_results_fail_without_a_clean_fallback() {
    assert!(
        protocol_correction(false)
            .await
            .unwrap_err()
            .contains("exact substring")
    );
}

#[tokio::test]
async fn a_denied_gateway_request_is_not_retried() {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut bytes = Vec::new();
        let mut chunk = [0; 4096];
        loop {
            let n = socket.read(&mut chunk).unwrap();
            assert!(n > 0);
            bytes.extend_from_slice(&chunk[..n]);
            let request = String::from_utf8_lossy(&bytes);
            if let Some(end) = request.find("\r\n\r\n") {
                let length: usize = request[..end]
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .and_then(|value| value.trim().parse().ok())
                    })
                    .unwrap();
                if bytes.len() >= end + 4 + length {
                    break;
                }
            }
        }
        socket
            .write_all(
                b"HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            )
            .unwrap();
        listener
    });
    let result = review(
        ProviderCredentials {
            base_url: format!("http://{address}/v1"),
            api_key: "secret-test-token".into(),
        },
        "model",
        &proof(),
        None,
        &[],
    )
    .await;
    assert_eq!(
        result.unwrap_err(),
        "independent reviewer failed (HTTP 401)"
    );
    let listener = server.join().unwrap();
    listener.set_nonblocking(true).unwrap();
    assert!(
        matches!(listener.accept(), Err(error) if error.kind()==std::io::ErrorKind::WouldBlock)
    );
}
