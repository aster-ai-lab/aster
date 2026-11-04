use super::SearchTool;
use crate::tools::types::{Tool, ToolContext};
use crate::workspace::{ManifestWorkspaceBackend, WorkspaceServices};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;

/// Manifest-backed workspace, the backend interactive hosts use.
async fn manifest_context(root: &std::path::Path) -> ToolContext {
    let backend = ManifestWorkspaceBackend::new(root);
    let mut ready = backend.manifest().subscribe();
    tokio::time::timeout(Duration::from_secs(5), ready.recv())
        .await
        .unwrap()
        .unwrap();
    let services = WorkspaceServices::local_with_retrieval_backend(Arc::clone(&backend));
    ToolContext::new(root.to_path_buf()).with_workspace_services(services)
}

fn fixture() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    for (name, body) in [
        ("a.txt", "needle one\n"),
        ("b.txt", "hay two\n"),
        ("c.md", "NEEDLE three\n"),
        ("d.rs", "needle four\n"),
    ] {
        std::fs::write(root.path().join(name), body).unwrap();
    }
    root
}

async fn run(ctx: &ToolContext, args: serde_json::Value) -> crate::tools::types::ToolOutput {
    let output = SearchTool::new(true).execute(&args, ctx).await.unwrap();
    assert!(output.success, "{}", output.content);
    output
}

#[tokio::test]
async fn glob_brace_alternatives_match_on_every_local_backend() {
    let root = fixture();
    let local = ToolContext::new(root.path().to_path_buf());
    let manifest = manifest_context(root.path()).await;
    for ctx in [&local, &manifest] {
        let both = run(ctx, json!({"mode": "glob", "query": "*.{txt,md}"})).await;
        for expected in ["a.txt", "b.txt", "c.md"] {
            assert!(both.content.contains(expected), "{}", both.content);
        }
        assert!(!both.content.contains("d.rs"), "{}", both.content);
        assert!(both.content.contains("3 of 3 file(s)"), "{}", both.content);

        let single = run(ctx, json!({"mode": "glob", "query": "*.{md}"})).await;
        assert!(single.content.contains("c.md"), "{}", single.content);
        assert!(!single.content.contains("a.txt"), "{}", single.content);
    }
}

#[tokio::test]
async fn grep_include_braces_behave_like_a_plain_include() {
    let root = fixture();
    let local = ToolContext::new(root.path().to_path_buf());
    let manifest = manifest_context(root.path()).await;
    for ctx in [&local, &manifest] {
        let regex = run(
            ctx,
            json!({"mode": "grep", "query": "needle|hay", "include": "*.{txt,md}"}),
        )
        .await;
        assert!(regex.content.contains("a.txt"), "{}", regex.content);
        assert!(regex.content.contains("b.txt"), "{}", regex.content);
        assert!(!regex.content.contains("d.rs"), "{}", regex.content);

        let exact = run(
            ctx,
            json!({"mode": "grep", "query": "needle", "include": "*.{txt,md}"}),
        )
        .await;
        assert!(exact.content.contains("a.txt"), "{}", exact.content);
        assert!(
            !exact.content.contains("NEEDLE"),
            "case-sensitive exact hit must not widen: {}",
            exact.content
        );
        assert!(exact.metadata.unwrap().get("match").is_none());
    }
}

#[tokio::test]
async fn an_empty_exact_count_is_not_replaced_by_fuzzy_content() {
    let root = fixture();
    let ctx = ToolContext::new(root.path().to_path_buf());
    let output = run(
        &ctx,
        json!({"mode": "grep", "query": "needl", "include": "*.txt", "output_mode": "count"}),
    )
    .await;
    assert!(!output.content.contains('>'), "{}", output.content);
}
