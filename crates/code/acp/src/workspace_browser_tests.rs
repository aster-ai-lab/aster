use super::{admit_workspace_browser_url, TOOL_NAME};

#[test]
fn admits_http_https_and_file_urls_like_the_studio_pane() {
    assert_eq!(
        admit_workspace_browser_url("https://example.com/docs").unwrap(),
        "https://example.com/docs"
    );
    assert_eq!(
        admit_workspace_browser_url("www.youtube.com").unwrap(),
        "https://www.youtube.com/"
    );
    assert_eq!(
        admit_workspace_browser_url("file:///workspace/index.html").unwrap(),
        "file:///workspace/index.html"
    );
    assert_eq!(
        admit_workspace_browser_url("127.0.0.1:3000").unwrap(),
        "http://127.0.0.1:3000/"
    );
}

#[test]
fn rejects_blank_credentialed_and_script_urls() {
    for raw in [
        "",
        "   ",
        "about:blank",
        "https://user:secret@example.com/",
        "javascript:alert(1)",
        "data:text/html,hi",
    ] {
        assert!(
            admit_workspace_browser_url(raw).is_err(),
            "expected rejection for {raw:?}"
        );
    }
}

#[test]
fn tool_name_is_stable_for_host_admission() {
    assert_eq!(TOOL_NAME, "open_workspace_browser");
}
