use crate::tools::{Tool, ToolContext, ToolExecutor, ToolOutput, MAX_OUTPUT_SIZE};
use async_trait::async_trait;
use std::sync::Arc;

/// A log whose needle sits past the retained head, so only the artifact
/// holds it.
struct LongLogTool;

#[async_trait]
impl Tool for LongLogTool {
    fn name(&self) -> &str {
        "long_log"
    }

    fn description(&self) -> &str {
        "Emit a long log"
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({"type": "object", "additionalProperties": false, "properties": {}})
    }

    async fn execute(
        &self,
        _args: &serde_json::Value,
        _ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        let mut log = String::new();
        let mut line = 0usize;
        while log.len() <= MAX_OUTPUT_SIZE * 2 {
            line += 1;
            log.push_str(&format!("step {line:06} ok padding-padding-padding\n"));
            if line == 4_000 {
                log.push_str("NEEDLE: linker failed on symbol foo\n");
            }
        }
        Ok(ToolOutput::success(log))
    }
}

fn artifact_uri(output: &str) -> String {
    let start = output
        .rfind(crate::tools::FULL_OUTPUT_ARTIFACT_MARKER)
        .expect("artifact marker")
        + crate::tools::FULL_OUTPUT_ARTIFACT_MARKER.len();
    output[start..]
        .split(|c: char| c.is_whitespace() || c == ']')
        .next()
        .expect("artifact uri")
        .to_string()
}

fn retain_log(store: &crate::tools::ArtifactStore, text: &str) -> String {
    let meta = crate::tools::tool_output_artifact("log", text, 0);
    let uri = meta.artifact_uri;
    store
        .put_content_addressed(crate::tools::ToolArtifact {
            artifact_id: meta.artifact_id,
            artifact_uri: uri.clone(),
            tool_name: "log".into(),
            content: text.into(),
            original_bytes: text.len(),
            shown_bytes: 0,
        })
        .unwrap();
    uri
}

#[tokio::test]
async fn the_model_reads_and_greps_the_region_a_truncated_result_omitted() {
    let workspace = tempfile::tempdir().expect("workspace");
    let executor = ToolExecutor::new(workspace.path().display().to_string());
    let registry = executor.registry();
    registry.register(Arc::new(LongLogTool));

    let truncated = registry
        .execute("long_log", &serde_json::json!({}))
        .await
        .expect("long_log");
    assert!(
        !truncated.output.contains("NEEDLE"),
        "needle must be omitted"
    );
    assert!(
        truncated.output.contains("page it with read"),
        "{}",
        &truncated.output[truncated.output.len().saturating_sub(300)..]
    );
    let uri = artifact_uri(&truncated.output);

    let found = registry
        .execute(
            "search",
            &serde_json::json!({"mode": "grep", "query": "needle", "path": uri, "case_sensitive": false}),
        )
        .await
        .expect("search");
    assert_eq!(found.exit_code, 0, "{}", found.output);
    assert!(
        found
            .output
            .contains("4001:NEEDLE: linker failed on symbol foo"),
        "{}",
        found.output
    );

    let page = registry
        .execute(
            "read",
            &serde_json::json!({"file_path": uri, "offset": 4000, "limit": 2}),
        )
        .await
        .expect("read");
    assert_eq!(page.exit_code, 0, "{}", page.output);
    assert!(page.output.contains("  4001\tNEEDLE"), "{}", page.output);
    assert!(
        page.output.contains("continue with offset=4002"),
        "{}",
        page.output
    );

    let wrong_mode = registry
        .execute(
            "search",
            &serde_json::json!({"mode": "glob", "query": "*", "path": uri}),
        )
        .await
        .expect("search");
    assert_ne!(wrong_mode.exit_code, 0);
}

#[tokio::test]
async fn an_unretained_artifact_is_refused_by_name() {
    let workspace = tempfile::tempdir().expect("workspace");
    let executor = ToolExecutor::new(workspace.path().display().to_string());
    let uri = "aster://tool-output/bash/0000";
    let result = executor
        .registry()
        .execute("read", &serde_json::json!({"file_path": uri}))
        .await
        .expect("read");
    assert_ne!(result.exit_code, 0);
    assert!(
        result.output.contains("not retained in this session"),
        "{}",
        result.output
    );
}

#[tokio::test]
async fn artifact_search_honors_output_modes_and_content_cursor() {
    let workspace = tempfile::tempdir().unwrap();
    let executor = ToolExecutor::new(workspace.path().display().to_string());
    let text = "needle needle\nother\nNEEDLE\nneedle\n";
    let uri = retain_log(&executor.artifact_store(), text);
    let registry = executor.registry();
    for (mode, expected) in [
        ("count", format!("{uri}: 3 matching line(s)")),
        ("files_with_matches", uri.clone()),
        ("summary", "3 matching line(s) in 1 file(s)".into()),
    ] {
        let result = registry
            .execute(
                "search",
                &serde_json::json!({
                    "mode":"grep", "query":"needle", "path":uri,
                    "case_sensitive":false, "output_mode":mode
                }),
            )
            .await
            .unwrap();
        assert_eq!(result.exit_code, 0, "{}", result.output);
        assert_eq!(result.output.trim(), expected, "mode={mode}");
    }
    for (cursor, expected, omitted) in [
        ("", "1:needle needle", "3:NEEDLE"),
        ("1", "3:NEEDLE", "1:needle needle"),
    ] {
        let result = registry
            .execute(
                "search",
                &serde_json::json!({
                    "mode":"grep", "query":"needle", "path":uri,
                    "case_sensitive":false, "limit":1, "cursor":cursor
                }),
            )
            .await
            .unwrap();
        assert_eq!(result.exit_code, 0, "{}", result.output);
        assert!(result.output.contains(expected), "{}", result.output);
        assert!(!result.output.contains(omitted), "{}", result.output);
        assert!(result.output.contains(if cursor.is_empty() {
            "cursor=1"
        } else {
            "cursor=2"
        }));
    }
    for args in [
        serde_json::json!({"output_mode":"invalid"}),
        serde_json::json!({"limit":0}),
        serde_json::json!({"cursor":"broken"}),
        serde_json::json!({"cursor":"4"}),
        serde_json::json!({"output_mode":"summary","limit":1}),
    ] {
        let mut args = args.as_object().unwrap().clone();
        args.extend(
            serde_json::json!({"mode":"grep","query":"needle","path":uri})
                .as_object()
                .unwrap()
                .clone(),
        );
        let result = registry.execute("search", &args.into()).await.unwrap();
        assert_ne!(result.exit_code, 0, "{}", result.output);
    }
    for mode in ["content", "count", "files_with_matches", "summary"] {
        let result = registry
            .execute("search", &serde_json::json!({"mode":"grep", "query":"absent", "path":uri,"output_mode":mode}))
            .await.unwrap();
        assert_eq!(result.exit_code, 0);
        assert!(!result.output.contains("needle"));
        assert!(
            result.output.contains(if mode == "summary" {
                "0 matching line(s) in 0 file(s)"
            } else {
                "No matches"
            }),
            "{}",
            result.output
        );
    }
}

#[test]
fn artifact_search_cursor_advances_under_the_output_budget_with_wide_context() {
    let workspace = tempfile::tempdir().unwrap();
    let store = crate::tools::ArtifactStore::new();
    let text = (0..50)
        .map(|index| format!("needle {index} {}\n", "值".repeat(500)))
        .collect::<String>();
    let uri = retain_log(&store, &text);
    let ctx = ToolContext::new(workspace.path().to_path_buf())
        .with_artifact_store(store)
        .with_result_budget_bytes(4096);
    for (budget, context) in [(1024, 0), (1024, 8), (4096, 0), (4096, 8)] {
        let ctx = ctx.clone().with_result_budget_bytes(budget);
        let mut cursor = String::new();
        let mut seen = 0;
        for _ in 0..50 {
            let request =
                super::PageRequest::parse(&serde_json::json!({"cursor":cursor}), 200, 1000)
                    .unwrap();
            let output = super::grep(
                &ctx,
                &uri,
                "needle",
                false,
                context,
                super::GrepOutputMode::Content,
                request,
            );
            assert!(output.success, "{}", output.content);
            assert!(output.content.len() <= ctx.result_budget_bytes());
            let metadata = output.metadata.unwrap();
            let count = metadata["page"]["returned_items"].as_u64().unwrap();
            assert!(count > 0);
            seen += count;
            assert_eq!(metadata["matches"], 50);
            let Some(next) = metadata["page"]["next_cursor"].as_str() else {
                break;
            };
            assert_ne!(next, cursor);
            assert!(output.content.contains(&format!("cursor={next}")));
            cursor = next.into();
        }
        assert_eq!(seen, 50);
    }
}

#[tokio::test]
async fn a_read_page_ends_inside_the_result_budget_and_continues() {
    let workspace = tempfile::tempdir().expect("workspace");
    let body = (1..=2_000)
        .map(|line| format!("line {line:04} {}\n", "z".repeat(40)))
        .collect::<String>();
    std::fs::write(workspace.path().join("big.txt"), &body).expect("write");
    let executor = ToolExecutor::new(workspace.path().display().to_string());
    let registry = executor.registry();
    let budget = crate::tools::ToolResultTransformPolicyV1::default().max_output_bytes;

    let first = registry
        .execute("read", &serde_json::json!({"file_path": "big.txt"}))
        .await
        .expect("read");
    assert_eq!(first.exit_code, 0, "{}", first.output);
    assert!(first.output.len() <= budget, "{}", first.output.len());
    assert!(
        !first.output.contains("omitted"),
        "the page must not be cut mid-page"
    );
    let next = first
        .output
        .rsplit("continue with offset=")
        .next()
        .expect("continuation");
    let next: usize = next
        .trim_end_matches(|c: char| !c.is_ascii_digit())
        .parse()
        .expect("offset");
    assert!(next > 100 && next < 2_000, "{next}");

    let second = registry
        .execute(
            "read",
            &serde_json::json!({"file_path": "big.txt", "offset": next}),
        )
        .await
        .expect("read");
    assert!(
        second
            .output
            .starts_with(&format!("{:>6}\tline {:04}", next + 1, next + 1)),
        "{}",
        &second.output[..80]
    );
}
