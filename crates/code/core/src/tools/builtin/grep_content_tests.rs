use super::*;
use crate::tools::builtin::search::SearchTool;
use serde_json::json;

#[tokio::test]
async fn content_limit_keeps_each_match_and_its_context_and_resumes_with_the_shared_cursor() {
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("资料 folder");
    std::fs::create_dir(&directory).unwrap();
    let original =
        "before first\nneedle-1\nafter first\nseparator\nbefore second\nneedle-2\nafter second\n";
    let path = directory.join("sample.txt");
    std::fs::write(&path, original).unwrap();
    std::fs::write(root.path().join("outside.txt"), "needle-3\n").unwrap();
    let ctx = ToolContext::new(root.path().to_owned());
    let mut args = json!({
        "mode":"grep", "query":"needle-[123]", "path":"资料 folder",
        "context":1, "limit":1,
    });
    let tool = SearchTool::new(true);
    let first = tool.execute(&args, &ctx).await.unwrap();
    assert!(first.success, "{}", first.content);
    for text in ["before first", "needle-1", "after first"] {
        assert!(first.content.contains(text), "{}", first.content);
    }
    assert!(!first.content.contains("needle-2"));
    assert!(!first.content.contains("needle-3"));
    let meta = first.metadata.unwrap();
    assert_eq!(meta["page"]["returned_items"], 1);
    assert_eq!(meta["page"]["total_items"], 2);
    assert_eq!(meta["page"]["next_cursor"], "1");
    assert_eq!(meta["source_anchors"], json!(["资料 folder/sample.txt"]));
    args["cursor"] = meta["page"]["next_cursor"].clone();

    let second = tool.execute(&args, &ctx).await.unwrap();
    assert!(second.success, "{}", second.content);
    for text in ["before second", "needle-2", "after second"] {
        assert!(second.content.contains(text), "{}", second.content);
    }
    assert!(!second.content.contains("needle-1"));
    assert!(!second.content.contains("needle-3"));
    assert_eq!(second.metadata.unwrap()["page"]["next_cursor"], json!(null));
    assert_eq!(std::fs::read(path).unwrap(), original.as_bytes());
}

#[tokio::test]
async fn content_with_a_limit_reports_a_successful_empty_search() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("file.txt"), "unrelated\n").unwrap();
    let ctx = ToolContext::new(root.path().to_owned());
    let output = SearchTool::new(true)
        .execute(
            &json!({"mode":"grep","query":"absent-[12]","limit":1}),
            &ctx,
        )
        .await
        .unwrap();
    assert!(output.success, "{}", output.content);
    assert!(output.content.contains("No matches found"));
    let metadata = output.metadata.unwrap();
    assert_eq!(metadata["page"]["returned_items"], 0);
    assert_eq!(metadata["page"]["next_cursor"], json!(null));
}

#[tokio::test]
async fn content_uses_the_existing_pagination_validation() {
    let root = tempfile::tempdir().unwrap();
    let ctx = ToolContext::new(root.path().to_owned());
    for controls in [
        json!({"limit":0}),
        json!({"limit":-1}),
        json!({"cursor":"invalid"}),
    ] {
        let mut args = json!({"pattern":"needle","output_mode":"content"});
        args.as_object_mut()
            .unwrap()
            .extend(controls.as_object().unwrap().clone());
        let output = GrepTool.execute(&args, &ctx).await.unwrap();
        assert!(!output.success, "{}", output.content);
        assert!(output.content.contains("limit") || output.content.contains("cursor"));
    }
}
