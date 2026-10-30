use super::*;
use crate::workspace::{WorkspaceReadOverlaySnapshot, WorkspaceServices};
use std::collections::HashMap;
use std::sync::Arc;

#[tokio::test]
async fn actual_file_reads_distinguish_absent_lf_and_crlf_endings() {
    let root = tempfile::tempdir().unwrap();
    let ctx = ToolContext::new(root.path().to_path_buf());
    for (content, ending, notice) in [
        ("same bytes", "none", "no final newline"),
        ("same bytes\n", "LF", "final line ending is LF"),
        ("same bytes\r\n", "CRLF", "final line ending is CRLF"),
        ("first\r\nlast", "none", "no final newline"),
    ] {
        std::fs::write(root.path().join("source.txt"), content).unwrap();
        let output = ReadTool
            .execute(&serde_json::json!({"file_path": "source.txt"}), &ctx)
            .await
            .unwrap();
        assert!(output.success, "{}", output.content);
        assert!(output.content.contains(notice), "{}", output.content);
        assert_eq!(
            output.metadata.unwrap()["range"]["final_line_ending"],
            ending
        );
        assert_eq!(
            std::fs::read(root.path().join("source.txt")).unwrap(),
            content.as_bytes()
        );
    }
}

#[tokio::test]
async fn partial_page_does_not_claim_to_know_the_file_ending() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("source.txt"), "first\nlast").unwrap();
    let ctx = ToolContext::new(root.path().to_path_buf());
    let output = ReadTool
        .execute(
            &serde_json::json!({"file_path": "source.txt", "limit": 1}),
            &ctx,
        )
        .await
        .unwrap();
    assert!(output.success);
    assert!(!output.content.contains("final newline"));
    assert!(output.metadata.unwrap()["range"]["final_line_ending"].is_null());
    let output = ReadTool
        .execute(
            &serde_json::json!({"file_path": "source.txt", "offset": 1}),
            &ctx,
        )
        .await
        .unwrap();
    assert!(
        output.content.contains("no final newline"),
        "{}",
        output.content
    );
}

#[tokio::test]
async fn overlay_ending_is_authoritative_over_saved_disk_bytes() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("source.txt"), "saved\r\n").unwrap();
    let overlay =
        WorkspaceReadOverlaySnapshot::new(HashMap::from([("source.txt".into(), "unsaved".into())]))
            .unwrap();
    let services = WorkspaceServices::local(root.path()).with_read_overlay(Arc::new(overlay));
    let ctx = ToolContext::new(root.path().to_path_buf()).with_workspace_services(services);
    let output = ReadTool
        .execute(&serde_json::json!({"file_path": "source.txt"}), &ctx)
        .await
        .unwrap();
    assert!(output.content.contains("unsaved"));
    assert!(
        output.content.contains("no final newline"),
        "{}",
        output.content
    );
    assert_eq!(
        output.metadata.unwrap()["range"]["final_line_ending"],
        "none"
    );
}

#[tokio::test]
async fn batch_read_keeps_each_files_ending_with_its_own_segment() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("a.txt"), "alpha").unwrap();
    std::fs::write(root.path().join("b.txt"), "bravo\r\n").unwrap();
    let output = ReadTool
        .execute(
            &serde_json::json!({"files": [{"path": "a.txt"}, {"path": "b.txt"}]}),
            &ToolContext::new(root.path().to_path_buf()),
        )
        .await
        .unwrap();
    assert!(output.success);
    let (first, second) = output.content.split_once("=== b.txt ===").unwrap();
    assert!(first.contains("no final newline"), "{first}");
    assert!(second.contains("final line ending is CRLF"), "{second}");
}
