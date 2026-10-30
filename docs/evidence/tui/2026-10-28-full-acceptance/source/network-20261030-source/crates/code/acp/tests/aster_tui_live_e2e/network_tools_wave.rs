#![cfg(test)]

use super::driver::LiveTui;
use super::tool_wave::{finish, one, request};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const ORIGIN: &str = "http://aster-fixture.example.test";

#[test]
#[ignore = "requires the frozen native pair and original kimi/k3.1-pro ACL"]
fn kimi_download_binary_unicode_destination_matches_sha256_and_real_checker() {
    let case = "network-download-binary";
    let server = HttpFixture::new();
    let mut tui = start(case, &server);
    let destination = "artifacts/下载 payload.bin";
    let expected_sha = format!("{:x}", Sha256::digest(&server.binary));
    let check = if cfg!(windows) {
        format!("Get-Item -LiteralPath '{destination}' -ErrorAction Stop")
    } else {
        format!("test -f '{destination}'")
    };
    let facts = request(&mut tui, "download-binary", "download", json!({
        "url":format!("{ORIGIN}/blob"),"file_path":destination,"connections":1,
        "expected_sha256":expected_sha,"max_bytes":131072,"timeout":10
    }), &format!("After success, use bash exactly once with this exact independent verification command: {check}. Report its actual result. Do not call other tools, modify any other file or retry."));
    let output = one(&facts, "download");
    assert_eq!(output["exit_code"], 0);
    assert_eq!(output["metadata"]["file_path"], destination);
    assert_eq!(output["metadata"]["bytes"], server.binary.len());
    assert_eq!(output["metadata"]["sha256"], expected_sha);
    assert_eq!(one(&facts, "bash")["exit_code"], 0);
    assert_eq!(
        std::fs::read(tui.workspace.join(destination)).unwrap(),
        server.binary
    );
    assert_no_temps(&tui.workspace);
    assert!(!server.requests.lock().unwrap().is_empty());
    tui.record("download-binary-exact-bytes", "the actual native downloader writes all unseen binary bytes to a Unicode path containing a space, verifies the supplied SHA-256 and completes a real independent shell check without temporary-file leaks");
    finish(tui, case);
}

#[test]
#[ignore = "requires the frozen native pair and original kimi/k3.1-pro ACL"]
fn kimi_download_existing_destination_refuses_without_overwrite() {
    download_failure(
        "network-download-existing",
        "/blob",
        json!({}),
        true,
        Some("exist"),
        true,
    );
}

#[test]
#[ignore = "requires the frozen native pair and original kimi/k3.1-pro ACL"]
fn kimi_download_sha256_failure_keeps_existing_exact_bytes() {
    download_failure(
        "network-download-hash",
        "/blob",
        json!({"overwrite":true,"expected_sha256":"0000000000000000000000000000000000000000000000000000000000000000"}),
        true,
        Some("SHA-256 mismatch"),
        false,
    );
}

#[test]
#[ignore = "requires the frozen native pair and original kimi/k3.1-pro ACL"]
fn kimi_download_declared_size_limit_leaves_no_destination_or_partial() {
    download_failure(
        "network-download-size",
        "/blob",
        json!({"max_bytes":1024}),
        false,
        Some("max_bytes"),
        false,
    );
}

#[test]
#[ignore = "requires the frozen native pair and original kimi/k3.1-pro ACL"]
fn kimi_download_http_404_leaves_no_destination_or_partial() {
    download_failure(
        "network-download-404",
        "/missing",
        json!({}),
        false,
        Some("404"),
        false,
    );
}

#[test]
#[ignore = "requires the frozen native pair and original kimi/k3.1-pro ACL"]
fn kimi_download_private_literal_refuses_before_proxy_or_file_effect() {
    download_failure(
        "network-download-private",
        "/blob",
        json!({"url":"http://127.0.0.1/private.bin"}),
        false,
        Some("non-public"),
        true,
    );
}

#[test]
#[ignore = "requires the frozen native pair and original kimi/k3.1-pro ACL"]
fn kimi_download_truncated_http_body_never_promotes_partial_file() {
    download_failure(
        "network-download-truncated",
        "/truncated",
        json!({"timeout":3}),
        false,
        None,
        false,
    );
}

#[test]
#[ignore = "requires the frozen native pair and original kimi/k3.1-pro ACL"]
fn kimi_web_fetch_semantic_main_retains_real_markdown_link_and_source() {
    let case = "network-fetch-main";
    let server = HttpFixture::new();
    let mut tui = start(case, &server);
    let url = format!("{ORIGIN}/main.html");
    let facts = request(&mut tui, "fetch-main", "web_fetch", json!({
        "url":url,"format":"markdown","body_only":true,"timeout":5,"max_chars":1000
    }), "Report the actual main content and link, without retrying, inventing content or calling other tools.");
    let output = one(&facts, "web_fetch");
    assert_eq!(output["exit_code"], 0);
    let text = output["text"].as_str().unwrap();
    assert!(text.contains(&server.token));
    assert!(text.contains("[Target](http://aster-fixture.example.test/target)"));
    assert!(!text.contains("OUTSIDE-HEADER") && !text.contains("OUTSIDE-FOOTER"));
    assert!(!text.contains("SHOULD-NOT-EXECUTE"));
    assert_eq!(output["metadata"]["source_anchors"], json!([url]));
    assert_eq!(output["metadata"]["range"]["eof"], true);
    assert_eq!(
        server.requests.lock().unwrap().as_slice(),
        [("GET".into(), "/main.html".into())]
    );
    tui.record("web-fetch-real-main-and-link", "the real HTTP response is converted to Markdown from its semantic main; unseen Unicode content and the actual link survive while header, footer and script do not; the source anchor matches the request");
    finish(tui, case);
}

#[test]
#[ignore = "requires the frozen native pair and original kimi/k3.1-pro ACL"]
fn kimi_web_fetch_unicode_pages_and_retained_artifact_preserve_content() {
    let case = "network-fetch-pages";
    let server = HttpFixture::new();
    let mut tui = start(case, &server);
    let url = format!("{ORIGIN}/text");
    let facts = request(
        &mut tui,
        "fetch-first",
        "web_fetch",
        json!({"url":url,"format":"text","offset":0,"max_chars":17,"timeout":5}),
        "Report the actual page and continuation offset. Do not retry or call other tools.",
    );
    let output = one(&facts, "web_fetch");
    assert_eq!(output["exit_code"], 0);
    let prefix = server.text.chars().take(17).collect::<String>();
    assert!(output["text"].as_str().unwrap().starts_with(&prefix));
    let range = &output["metadata"]["range"];
    assert_eq!(range["offset"], 0);
    assert_eq!(range["returned_chars"], 17);
    assert_eq!(range["next_offset"], 17);
    assert_eq!(range["total_chars"], server.text.chars().count());
    assert_eq!(range["eof"], false);
    let uri = output["metadata"]["artifact_uri"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(uri.starts_with("a3s://tool-output/"));
    tui.record("web-fetch-unicode-first-page", "the first real response uses Unicode character offsets, reports next_offset=17 and retains a content-addressed full-text artifact");
    let before = server.requests.lock().unwrap().len();
    let facts = request(&mut tui, "fetch-artifact", "read", json!({"file_path":uri}), "Report the actual retained content without fetching any URL, retrying or calling other tools.");
    let artifact = one(&facts, "read");
    assert_eq!(artifact["exit_code"], 0);
    assert!(artifact["text"].as_str().unwrap().contains(&server.text));
    assert_eq!(server.requests.lock().unwrap().len(), before);
    let facts = request(
        &mut tui,
        "fetch-second",
        "web_fetch",
        json!({"url":url,"format":"text","offset":17,"max_chars":1000,"timeout":5}),
        "Report the actual remaining page and EOF. Do not retry or call other tools.",
    );
    let output = one(&facts, "web_fetch");
    assert_eq!(output["exit_code"], 0);
    let remaining = server.text.chars().skip(17).collect::<String>();
    assert!(output["text"].as_str().unwrap().starts_with(&remaining));
    assert_eq!(output["metadata"]["range"]["offset"], 17);
    assert_eq!(
        output["metadata"]["range"]["returned_chars"],
        remaining.chars().count()
    );
    assert_eq!(output["metadata"]["range"]["eof"], true);
    assert_eq!(output["metadata"]["artifact_uri"], uri);
    tui.record("web-fetch-artifact-and-eof", "real read retrieves the exact retained full Unicode content without another HTTP request; a second real fetch continues at character 17 to EOF with the same content-addressed artifact");
    finish(tui, case);
}

#[test]
#[ignore = "requires the frozen native pair and original kimi/k3.1-pro ACL"]
fn kimi_web_fetch_redirect_preserves_original_and_final_sources() {
    let case = "network-fetch-redirect";
    let server = HttpFixture::new();
    let mut tui = start(case, &server);
    let url = format!("{ORIGIN}/redirect?fixture=not-a-credential");
    let facts = request(
        &mut tui,
        "fetch-redirect",
        "web_fetch",
        json!({"url":url,"format":"text","timeout":5}),
        "Report the actual fetched content and final source. Do not retry or call other tools.",
    );
    let output = one(&facts, "web_fetch");
    assert_eq!(output["exit_code"], 0);
    assert!(output["text"].as_str().unwrap().contains(&server.token));
    assert_eq!(
        output["metadata"]["source_anchors"],
        json!([format!("{ORIGIN}/redirect"), format!("{ORIGIN}/main.html")])
    );
    let requests = server.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert!(requests[0].1.starts_with("/redirect"));
    assert_eq!(requests[1].1, "/main.html");
    drop(requests);
    tui.record("web-fetch-real-redirect-sources", "the actual proxy records the redirect and destination requests; returned content is from the destination and safe metadata retains original/final source paths without query data");
    finish(tui, case);
}

#[test]
#[ignore = "requires the frozen native pair and original kimi/k3.1-pro ACL"]
fn kimi_web_fetch_http_404_reports_actual_failure_and_recovers() {
    fetch_failure("network-fetch-404", "/missing", 5, "404", false);
}

#[test]
#[ignore = "requires the frozen native pair and original kimi/k3.1-pro ACL"]
fn kimi_web_fetch_http_429_reports_actual_rate_limit_and_recovers() {
    fetch_failure("network-fetch-429", "/limited", 5, "429", false);
}

#[test]
#[ignore = "requires the frozen native pair and original kimi/k3.1-pro ACL"]
fn kimi_web_fetch_timeout_is_real_and_followup_turn_remains_usable() {
    fetch_failure(
        "network-fetch-timeout",
        "/slow",
        1,
        "timed out after 1 seconds",
        false,
    );
}

#[test]
#[ignore = "requires the frozen native pair and original kimi/k3.1-pro ACL"]
fn kimi_web_fetch_private_literal_is_refused_before_proxy_access() {
    fetch_failure(
        "network-fetch-private",
        "http://127.0.0.1/private",
        5,
        "non-public",
        true,
    );
}

fn download_failure(
    case: &str,
    route: &str,
    extra: Value,
    existing: bool,
    needle: Option<&str>,
    no_http: bool,
) {
    let server = HttpFixture::new();
    let mut tui = start(case, &server);
    let path = tui.workspace.join("payload.bin");
    let original = format!("ORIGINAL-{}\r\n", uuid::Uuid::new_v4());
    if existing {
        std::fs::write(&path, &original).unwrap();
    }
    let mut args = json!({"url":format!("{ORIGIN}{route}"),"file_path":"payload.bin","connections":1,"max_bytes":131072,"timeout":5});
    args.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    let facts = request(
        &mut tui,
        "download-error",
        "download",
        args,
        "Report the exact failure. Do not retry, repair, change arguments or call other tools.",
    );
    let output = one(&facts, "download");
    assert_eq!(output["is_error"], true);
    let text = output["text"].as_str().unwrap();
    assert!(!text.is_empty());
    if let Some(needle) = needle {
        assert!(
            text.contains(needle),
            "expected {needle:?}, actual {text:?}"
        );
    }
    if existing {
        assert_eq!(std::fs::read(path).unwrap(), original.as_bytes());
    } else {
        assert!(!path.exists());
    }
    assert_no_temps(&tui.workspace);
    if no_http {
        assert!(server.requests.lock().unwrap().is_empty());
    } else {
        assert!(!server.requests.lock().unwrap().is_empty());
    }
    tui.record("download-refusal-exact-scope", "the actual failed download keeps the original destination bytes or leaves no destination, leaks no adjacent download temporary, and its HTTP dispatch boundary matches the requested refusal");
    finish(tui, case);
}

fn fetch_failure(case: &str, route: &str, timeout: u64, needle: &str, no_http: bool) {
    let server = HttpFixture::new();
    let mut tui = start(case, &server);
    let url = if route.starts_with("http:") {
        route.into()
    } else {
        format!("{ORIGIN}{route}")
    };
    let facts = request(&mut tui, "fetch-error", "web_fetch", json!({"url":url,"format":"text","timeout":timeout}), "Report the exact failure without retrying, switching source, inventing content or calling other tools.");
    let output = one(&facts, "web_fetch");
    assert_eq!(output["is_error"], true);
    assert!(output["text"].as_str().unwrap().contains(needle));
    if no_http {
        assert!(server.requests.lock().unwrap().is_empty());
    } else {
        assert!(!server.requests.lock().unwrap().is_empty());
    }
    tui.record("web-fetch-controlled-error", "the real web_fetch failure preserves its actual status or timeout reason; the controlled endpoint confirms whether network dispatch occurred");
    let facts = tui.turn("Reply only FETCH-ERROR-RECOVERY. Do not call tools.", None);
    assert_eq!(tui.assistant_text(&facts).trim(), "FETCH-ERROR-RECOVERY");
    tui.record(
        "web-fetch-error-recovery",
        "the same session completes a fresh model turn after the real fetch failure",
    );
    finish(tui, case);
}

fn start(case: &str, server: &HttpFixture) -> LiveTui {
    let mut tui = LiveTui::new(case);
    tui.set_env("HTTP_PROXY", &server.proxy);
    tui.launch(&[]);
    tui.wait_for("Aster", 45);
    tui.session_id();
    tui.dismiss();
    tui
}

fn assert_no_temps(path: &Path) {
    for entry in std::fs::read_dir(path).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            assert_no_temps(&entry.path());
        } else {
            assert!(!entry
                .file_name()
                .to_string_lossy()
                .starts_with(".a3s-download-"));
        }
    }
}

struct HttpFixture {
    proxy: String,
    binary: Vec<u8>,
    token: String,
    text: String,
    requests: Arc<Mutex<Vec<(String, String)>>>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl HttpFixture {
    fn new() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let proxy = format!("http://{}", listener.local_addr().unwrap());
        let token = format!("FETCH-中文-{}", uuid::Uuid::new_v4());
        let binary = (0..65536).map(|i| (i % 251) as u8).collect::<Vec<_>>();
        let text = format!("分页-{token}-0123456789abcdef").repeat(4);
        let requests = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let (seen, stopped) = (Arc::clone(&requests), Arc::clone(&stop));
        let (blob, main_token, page) = (binary.clone(), token.clone(), text.clone());
        let worker = thread::spawn(move || {
            while !stopped.load(Ordering::Relaxed) {
                let (mut stream, _) = match listener.accept() {
                    Ok(client) => client,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                        continue;
                    }
                    Err(e) => panic!("fixture accept: {e}"),
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                stream
                    .set_write_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut header = Vec::new();
                let mut buffer = [0; 1024];
                while !header.windows(4).any(|w| w == b"\r\n\r\n") && header.len() < 16384 {
                    let Ok(n) = stream.read(&mut buffer) else {
                        break;
                    };
                    if n == 0 {
                        break;
                    }
                    header.extend_from_slice(&buffer[..n]);
                }
                if header.is_empty() {
                    continue;
                }
                let header = String::from_utf8_lossy(&header);
                let mut parts = header.lines().next().unwrap_or("").split_whitespace();
                let method = parts.next().unwrap_or("");
                let target = parts.next().unwrap_or("");
                let url = url::Url::parse(target).unwrap();
                assert_eq!(
                    url.host_str(),
                    Some("aster-fixture.example.test"),
                    "fixture received a request for an unexpected host"
                );
                let route = url.path();
                let path = url
                    .query()
                    .map_or_else(|| route.to_owned(), |q| format!("{route}?{q}"));
                seen.lock().unwrap().push((method.into(), path));
                let html = format!("<html><body><header>OUTSIDE-HEADER</header><main><h1>Fixture main</h1><p>{main_token}</p><a href='{ORIGIN}/target'>Target</a><script>SHOULD-NOT-EXECUTE</script></main><footer>OUTSIDE-FOOTER</footer></body></html>");
                let (status, content_type, body, extra) = match route {
                    "/blob" => ("200 OK", "application/octet-stream", blob.clone(), ""),
                    "/truncated" => (
                        "200 OK",
                        "application/octet-stream",
                        blob[..32].to_vec(),
                        "",
                    ),
                    "/main.html" => ("200 OK", "text/html; charset=utf-8", html.into_bytes(), ""),
                    "/text" => (
                        "200 OK",
                        "text/plain; charset=utf-8",
                        page.as_bytes().to_vec(),
                        "",
                    ),
                    "/redirect" => (
                        "302 Found",
                        "text/plain",
                        Vec::new(),
                        "Location: http://aster-fixture.example.test/main.html\r\n",
                    ),
                    "/limited" => (
                        "429 Too Many Requests",
                        "text/plain",
                        b"FIXTURE-RATE-LIMIT".to_vec(),
                        "Retry-After: 2\r\n",
                    ),
                    "/slow" => {
                        thread::sleep(Duration::from_secs(3));
                        ("200 OK", "text/plain", b"TOO-LATE-CONTENT".to_vec(), "")
                    }
                    _ => (
                        "404 Not Found",
                        "text/plain",
                        b"FIXTURE-NOT-FOUND".to_vec(),
                        "",
                    ),
                };
                let declared = if route == "/truncated" {
                    1024
                } else {
                    body.len()
                };
                let response = format!("HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {declared}\r\nConnection: close\r\n{extra}\r\n");
                let _ = stream.write_all(response.as_bytes());
                if method != "HEAD" {
                    let _ = stream.write_all(&body);
                }
            }
        });
        Self {
            proxy,
            binary,
            token,
            text,
            requests,
            stop,
            worker: Some(worker),
        }
    }
}

impl Drop for HttpFixture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            if !thread::panicking() {
                worker.join().unwrap();
            } else {
                let _ = worker.join();
            }
        }
    }
}
