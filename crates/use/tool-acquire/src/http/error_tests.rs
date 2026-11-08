use super::*;
use std::error::Error;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::thread;

fn serve(response: Vec<u8>) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        let mut stream = super::test_server::accept(&listener);
        let mut reader = BufReader::new(&mut stream);
        loop {
            let mut line = String::new();
            assert_ne!(reader.read_line(&mut line).unwrap(), 0);
            if line == "\r\n" {
                break;
            }
        }
        stream.write_all(&response).unwrap();
    });
    (address, server)
}

fn assert_redacted(error: &ToolAcquireError, secret: &str) {
    let mut cause: Option<&(dyn Error + 'static)> = Some(error);
    while let Some(error) = cause {
        assert!(!format!("{error:?} {error}").contains(secret));
        cause = error.source();
    }
}

#[test]
fn http_status_and_transport_errors_keep_typed_sources_without_url_credentials() {
    for (response, status) in [
        (
            b"HTTP/1.1 404 Not Found\r\ncontent-length: 0\r\nconnection: close\r\n\r\n".to_vec(),
            Some(reqwest::StatusCode::NOT_FOUND),
        ),
        (b"invalid HTTP response\r\n\r\n".to_vec(), None),
    ] {
        for metadata in [true, false] {
            let (address, server) = serve(response.clone());
            let source = HttpSource::for_test(&address, &address, &["test-host"]).unwrap();
            let secret = "synthetic-http-credential";
            let url = format!("{address}/{secret}?token={secret}");
            let error = if metadata {
                source.get_json(&url).unwrap_err()
            } else {
                source.download(&url).unwrap_err()
            };
            server.join().unwrap();
            assert_eq!(error.code(), "use.tool_acquire.http");
            let cause = error
                .source()
                .unwrap()
                .downcast_ref::<reqwest::Error>()
                .unwrap();
            assert_eq!(cause.status(), status);
            assert!(cause.url().is_none());
            assert_redacted(&error, secret);
        }
    }
}

#[test]
fn truncated_bodies_keep_the_read_error_without_url_credentials() {
    for metadata in [true, false] {
        let (address, server) =
            serve(b"HTTP/1.1 200 OK\r\ncontent-length: 40\r\nconnection: close\r\n\r\n{}".to_vec());
        let source = HttpSource::for_test(&address, &address, &["test-host"]).unwrap();
        let secret = "synthetic-read-credential";
        let url = format!("{address}/{secret}?token={secret}");
        let error = if metadata {
            source.get_json(&url).unwrap_err()
        } else {
            source.download(&url).unwrap_err()
        };
        server.join().unwrap();
        assert_eq!(error.code(), "use.tool_acquire.http_read");
        assert!(error.source().unwrap().is::<std::io::Error>());
        assert_redacted(&error, secret);
    }
}

#[test]
fn malformed_json_keeps_the_parser_error_without_the_response_payload() {
    let secret = "synthetic-metadata-secret";
    let body = format!("{{\"{secret}\":");
    let response = format!(
        "HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    );
    let (address, server) = serve(response.into_bytes());
    let source = HttpSource::for_test(&address, &address, &["test-host"]).unwrap();
    let error = source.get_json(&address).unwrap_err();
    server.join().unwrap();
    assert_eq!(error.code(), "use.tool_acquire.metadata_json");
    assert!(error.source().unwrap().is::<serde_json::Error>());
    assert_redacted(&error, secret);
}

#[test]
fn metadata_and_asset_size_limits_refuse_before_reading_large_bodies() {
    for metadata in [true, false] {
        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
            MAX_BYTES + 1
        );
        let (address, server) = serve(response.into_bytes());
        let source = HttpSource::for_test(&address, &address, &["test-host"]).unwrap();
        let error = if metadata {
            source.get_json(&address).unwrap_err()
        } else {
            source.download(&address).unwrap_err()
        };
        server.join().unwrap();
        assert!(matches!(error, ToolAcquireError::DownloadRejected));
    }
}
