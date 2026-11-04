use super::{admit_workspace_browser_url, OpenWorkspaceBrowserTool, TOOL_NAME};
use aster_core::tools::{Tool, ToolContext};
use serde_json::json;

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

#[tokio::test]
async fn a_missing_local_file_is_rejected_before_the_host_is_called() {
    let root = tempfile::tempdir().unwrap();
    let missing = root.path().join("definitely-missing.html");
    let url = url::Url::from_file_path(&missing).unwrap();
    // The bridge has no worker, so reaching the host would time out instead.
    let tool = OpenWorkspaceBrowserTool::new(crate::document_read::DocumentBridge::new(), "s");
    let ctx = ToolContext::new(root.path().to_path_buf());
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        tool.execute(&json!({"url": url.as_str()}), &ctx),
    )
    .await
    .expect("rejected without a host round trip")
    .unwrap();
    assert!(!output.success, "{}", output.content);
    assert!(
        output.content.contains("does not exist"),
        "{}",
        output.content
    );
    assert_eq!(output.metadata.unwrap()["reason"], "missing_file");
}

#[tokio::test]
async fn an_existing_local_file_passes_the_existence_check() {
    let root = tempfile::tempdir().unwrap();
    let page = root.path().join("index.html");
    std::fs::write(&page, "<p>hi</p>").unwrap();
    let url = url::Url::from_file_path(&page).unwrap();
    assert_eq!(super::missing_local_file(url.as_str()).await, None);
    assert_eq!(
        super::missing_local_file("https://example.com/").await,
        None
    );
}
