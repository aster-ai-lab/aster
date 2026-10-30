use super::*;

#[tokio::test]
async fn wiki_attribution_survives_a_core_bounded_preview_as_structured_output() {
    use a3s_code_core::tools::{Tool, ToolContext, ToolOutput, ToolRegistry};
    struct RecordedWiki(serde_json::Value);
    #[async_trait::async_trait]
    impl Tool for RecordedWiki {
        fn name(&self) -> &str {
            "search_wiki"
        }
        fn description(&self) -> &str {
            "Recorded bounded host reply"
        }
        fn parameters(&self) -> serde_json::Value {
            serde_json::json!({"type":"object"})
        }
        async fn execute(
            &self,
            _: &serde_json::Value,
            _: &ToolContext,
        ) -> anyhow::Result<ToolOutput> {
            Ok(ToolOutput::success(self.0.to_string())
                .with_metadata(serde_json::json!({"wiki_result":self.0})))
        }
    }
    let result = serde_json::json!({"phase":"published","scope":"all","complete":true,"searched":1,"pending":0,"sessionId":"session-a","query":"QZ47","nextCursor":null,"hits":(0..5).map(|n|serde_json::json!({
        "ownerKind":"library","ownerId":"library-a","libraryId":"library-a",
        "sourceRoot":"/fixture/materials","outputRoot":"/fixture/materials/wiki/workspace/epoch",
        "name":"Original evidence","path":format!("concepts/source-{n}.md"),"passage":"中文事实 QZ47-683125。\n".repeat(200)
    })).collect::<Vec<_>>()});
    assert!(result.to_string().len() > 16 * 1024);
    assert!(result.to_string().len() < 48 * 1024);
    let root = tempfile::tempdir().unwrap();
    let registry = ToolRegistry::new(root.path().into());
    registry.register(Arc::new(RecordedWiki(result.clone())));
    let output = registry
        .execute_raw("search_wiki", &serde_json::json!({}))
        .await
        .unwrap()
        .unwrap();
    assert!(output.content.contains("tool output bounded"));
    assert!(serde_json::from_str::<serde_json::Value>(&output.content).is_err());
    let raw = tool_publication_output("search_wiki", output.metadata.as_ref(), 0)
        .expect("validated structured Wiki result");
    assert_eq!(raw, result);
    assert!(tool_publication_output("search_wiki", output.metadata.as_ref(), 1).is_none());
    assert!(tool_publication_output("search_wiki", None, 0).is_none());
}

#[test]
fn workspace_wiki_results_keep_complete_structured_hits_past_the_acp_preview_limit() {
    let output = serde_json::json!({"hits": (0..5).map(|index| serde_json::json!({
        "name":format!("Page {index}"), "path":format!("concepts/page-{index}.md"),
        "passage":"Evidence paragraph.\n\n".repeat(60)
    })).collect::<Vec<_>>()})
    .to_string();
    let preview = serde_json::to_value(tool_call_content("search_wiki", None, &output)).unwrap();
    assert!(serde_json::from_str::<serde_json::Value>(
        preview[0]["content"]["text"].as_str().unwrap()
    )
    .is_err());
    for name in ["search_wiki", "search_memory"] {
        let raw = tool_raw_output(name, None, &output, 0).unwrap();
        let complete: serde_json::Value = serde_json::from_str(raw.as_str().unwrap()).unwrap();
        assert_eq!(complete["hits"].as_array().unwrap().len(), 5);
        assert_eq!(complete["hits"][4]["path"], "concepts/page-4.md");
    }
    assert_eq!(
        tool_raw_output("search_wiki", None, &output, 1),
        Some(serde_json::Value::String(output))
    );
}

#[test]
fn validated_publication_survives_an_invalid_truncated_json_preview() {
    let metadata = serde_json::json!({
        "file_path":"/workspace/report.xlsx", "files":["/workspace/report.xlsx"],
        "artifact_relative_path":"report.xlsx", "artifact_requested_path":"report.xlsx",
        "artifact_content_digest":"a".repeat(64), "kind":"xlsx", "bytes":7605,
        "validated":true, "readback":"cell data ".repeat(1000), "readback_truncated":true
    });
    let text = metadata.to_string();
    let contents = serde_json::to_value(tool_call_content("office_file", None, &text)).unwrap();
    let preview = contents[0]["content"]["text"].as_str().unwrap();
    assert!(serde_json::from_str::<serde_json::Value>(preview).is_err());
    let publication = tool_publication_output("office_file", Some(&metadata), 0).unwrap();
    assert_eq!(publication["artifact_relative_path"], "report.xlsx");
    assert_eq!(publication["artifact_content_digest"], "a".repeat(64));
    assert_eq!(publication["validated"], true);
    assert!(publication.get("readback").is_none());
    assert!(publication.to_string().len() < 1000);
}

#[test]
fn failed_reads_and_unverified_metadata_do_not_become_publications() {
    let valid = serde_json::json!({"artifact_relative_path":"report.xlsx","validated":true});
    assert!(tool_publication_output("office_file", Some(&valid), 1).is_none());
    assert!(tool_publication_output("office_read", Some(&valid), 0).is_none());
    assert!(tool_publication_output("office_file", None, 0).is_none());
    for metadata in [
        serde_json::Value::Null,
        serde_json::json!({"file_path":"input.xlsx","validated":true}),
        serde_json::json!({"artifact_relative_path":"report.xlsx","validated":false}),
        serde_json::json!({"artifact_relative_path":"","validated":true}),
        serde_json::json!({"artifact_relative_path":42,"validated":true}),
    ] {
        assert!(tool_publication_output("office_file", Some(&metadata), 0).is_none());
    }
}
