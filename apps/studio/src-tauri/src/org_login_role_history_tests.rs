#![cfg(test)]

use std::io::{Read, Write};
use std::net::TcpListener;
use std::time::{Duration, Instant};

const ROLE: &str = "0197e5a2-89f1-7d33-a11c-5f53d9b3af10";

#[test]
fn catalog_agents_are_a_member_read() {
    assert!(super::allowed_member_path(
        "GET",
        "/v1/api/org/catalog-agents"
    ));
    assert!(!super::allowed_member_path(
        "POST",
        "/v1/api/org/catalog-agents"
    ));
    assert!(!super::allowed_member_path(
        "GET",
        "/v1/api/org/catalog-agents/acp-cloud"
    ));
}

#[test]
fn history_gate_accepts_only_scoped_get_with_a_valid_role_id() {
    let root = "/v1/api/org/members/orgs/org-1";
    let path = format!("{root}/roles/{ROLE}/audit");
    assert!(super::allowed_member_path("GET", &path));
    for method in ["POST", "PUT", "DELETE", "PATCH"] {
        assert!(!super::allowed_member_path(method, &path));
    }
    for tail in [
        format!("roles/{ROLE}/audit/extra"),
        format!("positions/{ROLE}/audit"),
        "roles/not-a-uuid/audit".into(),
        "roles/../audit".into(),
        format!("roles/{ROLE}%2fextra/audit"),
    ] {
        assert!(!super::allowed_member_path(
            "GET",
            &format!("{root}/{tail}")
        ));
    }
}

#[test]
fn assignment_history_gate_accepts_only_scoped_get_with_a_valid_member_id() {
    let root = "/v1/api/org/members/orgs/org-1";
    let path = format!("{root}/members/{ROLE}/role-audit");
    assert!(super::allowed_member_path("GET", &path));
    for method in ["POST", "PUT", "DELETE", "PATCH"] {
        assert!(!super::allowed_member_path(method, &path));
    }
    for tail in [
        format!("members/{ROLE}/role-audit/extra"),
        format!("positions/{ROLE}/role-audit"),
        "members/not-a-uuid/role-audit".into(),
        "members/../role-audit".into(),
        format!("members/{ROLE}%2fextra/role-audit"),
    ] {
        assert!(!super::allowed_member_path(
            "GET",
            &format!("{root}/{tail}")
        ));
    }
}

#[test]
fn history_forwarding_preserves_the_member_session_and_read_only_response() {
    assert_history_forwarding(format!("/v1/api/org/members/orgs/org-1/roles/{ROLE}/audit"));
}

#[test]
fn assignment_history_forwarding_preserves_the_member_session_and_read_only_response() {
    assert_history_forwarding(format!(
        "/v1/api/org/members/orgs/org-1/members/{ROLE}/role-audit"
    ));
}

fn assert_history_forwarding(path: String) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("owned upstream listener");
    let origin = format!(
        "http://{}",
        listener.local_addr().expect("upstream address")
    );
    listener.set_nonblocking(true).expect("bounded accept");
    let worker = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(error)
                    if error.kind() == std::io::ErrorKind::WouldBlock
                        && Instant::now() < deadline =>
                {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(error) => panic!("history request did not reach its owned upstream: {error}"),
            }
        };
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("bounded request read");
        let mut bytes = Vec::new();
        let mut chunk = [0u8; 1024];
        while !bytes.windows(4).any(|window| window == b"\r\n\r\n") {
            let length = stream.read(&mut chunk).expect("request bytes");
            assert!(length > 0 && bytes.len() < 8192);
            bytes.extend_from_slice(&chunk[..length]);
        }
        let payload = br#"{"code":0,"data":[]}"#;
        write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", payload.len()).expect("response headers");
        stream.write_all(payload).expect("history bytes");
        String::from_utf8(bytes).expect("request text")
    });
    let response = super::forward_member_api(&origin, "GET", &path, b"", "a3s_ms_history-proof");
    assert_eq!(response.0, 200);
    assert_eq!(response.3, br#"{"code":0,"data":[]}"#);
    let seen = worker.join().expect("owned upstream completion");
    assert!(seen.starts_with(&format!("GET {path} HTTP/1.1\r\n")));
    assert!(seen.contains("Authorization: Bearer a3s_ms_history-proof\r\n"));
    assert_eq!(
        super::forward_member_api(&origin, "POST", &path, b"{}", "a3s_ms_history-proof").0,
        404
    );
}
