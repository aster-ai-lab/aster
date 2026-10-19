#![cfg(test)]

use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread::{self, JoinHandle};
use std::time::Duration;

fn upstream(payload: &[u8]) -> (String, JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let payload = payload.to_vec();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut raw = Vec::new();
        let mut buf = [0_u8; 2048];
        loop {
            let n = stream.read(&mut buf).unwrap();
            assert_ne!(n, 0, "request must contain its declared body");
            raw.extend_from_slice(&buf[..n]);
            if let Some(split) = raw.windows(4).position(|part| part == b"\r\n\r\n") {
                let head = String::from_utf8_lossy(&raw[..split]);
                let length = head
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().unwrap())
                    })
                    .unwrap();
                if raw.len() >= split + 4 + length {
                    break;
                }
            }
        }
        write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", payload.len()).unwrap();
        stream.write_all(&payload).unwrap();
        String::from_utf8(raw).unwrap()
    });
    (origin, server)
}

#[test]
fn challenge_issue_reaches_the_published_admin_captcha_route() {
    let (origin, server) = upstream(br#"{"data":{"challengeId":"challenge"}}"#);
    let response = super::route(
        "POST",
        &["v1", "api", "captcha", "challenges"],
        b"{}",
        &origin,
        "computer",
    )
    .unwrap();
    assert_eq!(response.0, 200);
    assert!(
        server
            .join()
            .unwrap()
            .starts_with("POST /v1/api/admin/captcha/challenges HTTP/1.1\r\n")
    );
}

#[test]
fn challenge_verification_preserves_the_answer_on_the_published_route() {
    let id = "018f3b2a-7c31-7a3e-9b2a-6c1d0e5f4a11";
    let body = br#"{"x":120,"durationMs":800,"points":12}"#;
    let (origin, server) = upstream(br#"{"data":{"verified":true,"token":"example"}}"#);
    let response = super::route(
        "POST",
        &["v1", "api", "captcha", "challenges", id, "verifications"],
        body,
        &origin,
        "computer",
    )
    .unwrap();
    assert_eq!(response.0, 200);
    let raw = server.join().unwrap();
    assert!(raw.starts_with(&format!(
        "POST /v1/api/admin/captcha/challenges/{id}/verifications HTTP/1.1\r\n"
    )));
    assert!(raw.ends_with(std::str::from_utf8(body).unwrap()));
}

#[test]
fn consumed_token_is_checked_on_the_published_route_and_removed_from_credentials() {
    let (origin, server) = upstream(br#"{"data":{"valid":true}}"#);
    let body = br#"{"identifier":"13800138000","password":"example-password","captchaToken":"captcha_v1.example"}"#;
    let credentials = super::consume_captcha(&origin, body).unwrap();
    let raw = server.join().unwrap();
    assert!(raw.starts_with("POST /v1/api/admin/captcha/token-verifications HTTP/1.1\r\n"));
    let value: serde_json::Value = serde_json::from_slice(&credentials).unwrap();
    assert_eq!(value["identifier"], "13800138000");
    assert_eq!(value["password"], "example-password");
    assert!(value.get("captchaToken").is_none());
}

#[test]
fn invalid_upstream_token_still_refuses_login() {
    let (origin, server) = upstream(br#"{"data":{"valid":false}}"#);
    let response = super::route(
        "POST",
        &["v1", "api", "org", "members", "sessions"],
        br#"{"captchaToken":"captcha_v1.invalid"}"#,
        &origin,
        "computer",
    )
    .unwrap();
    assert_eq!(response.0, 400);
    assert!(
        String::from_utf8(response.3)
            .unwrap()
            .contains("captcha required")
    );
    assert!(
        server
            .join()
            .unwrap()
            .starts_with("POST /v1/api/admin/captcha/token-verifications ")
    );
}

#[test]
fn malformed_challenge_addresses_do_not_reach_the_upstream() {
    for id in [
        "../config",
        "not-a-uuid",
        "018f3b2a-7c31-7a3e-9b2a-6c1d0e5f4a11/extra",
    ] {
        assert!(
            super::route(
                "POST",
                &["v1", "api", "captcha", "challenges", id, "verifications"],
                b"{}",
                "http://127.0.0.1:1",
                "computer"
            )
            .is_none()
        );
    }
}
