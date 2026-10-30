#![cfg(test)]

use super::*;

async fn run(args: Value) -> ToolOutput {
    OutputCheckTool
        .execute(&args, &ToolContext::new(".".into()))
        .await
        .unwrap()
}

#[tokio::test]
async fn date_spans_require_explicit_counting_and_validate_calendar_dates() {
    for (counting, days) in [("elapsed", 2), ("inclusive", 3)] {
        let output = run(json!({"action":"date_span","start":"2024-02-28","end":"2024-03-01","endpoint_counting":counting})).await;
        assert!(output.success, "{}", output.content);
        let result: Value = serde_json::from_str(&output.content).unwrap();
        assert_eq!(result["days"], days);
        assert_eq!(result["endpoint_counting"], counting);
        assert_eq!(
            metrics::EndpointCounting::parse(counting).unwrap().as_str(),
            counting
        );
    }
    for (start, end, counting) in [
        ("2025-02-29", "2025-03-01", "elapsed"),
        ("2026-01-02", "2026-01-01", "elapsed"),
        ("2026-1-01", "2026-01-02", "elapsed"),
        ("0000-01-01", "2026-01-02", "elapsed"),
        ("2026-01-01", "2026-01-02", "guess"),
    ] {
        assert!(
            !run(
                json!({"action":"date_span","start":start,"end":end,"endpoint_counting":counting})
            )
            .await
            .success
        );
    }
    assert!(
        !run(json!({"action":"date_span","start":"2026-01-01","end":"2026-01-02"}))
            .await
            .success
    );
    for (counting, days) in [("elapsed", 0), ("inclusive", 1)] {
        let result: Value = serde_json::from_str(&run(json!({"action":"date_span","start":"2026-01-01","end":"2026-01-01","endpoint_counting":counting})).await.content).unwrap();
        assert_eq!(result["days"], days);
    }
}

#[tokio::test]
async fn sequences_report_every_decrease_and_keep_missing_and_zero_distinct() {
    let output = run(json!({"action":"sequence_changes","values":[100,92,88,95,0,0]})).await;
    assert!(output.success, "{}", output.content);
    let result: Value = serde_json::from_str(&output.content).unwrap();
    assert_eq!(
        result["decreases"],
        json!([
            {"from_index":0,"to_index":1},{"from_index":1,"to_index":2},{"from_index":3,"to_index":4}
        ])
    );
    assert_eq!(result["non_decreasing"], false);
    assert_eq!(result["steps"][4]["difference"], 0.0);
    assert!(result["steps"][4]["percentage_change"].is_null());
    assert!(result["steps"][4]["reason"]
        .as_str()
        .unwrap()
        .contains("zero baseline"));
    let missing: Value = serde_json::from_str(
        &run(json!({"action":"sequence_changes","values":[0,null,2,1]}))
            .await
            .content,
    )
    .unwrap();
    assert_eq!(missing["all_steps_observed"], false);
    assert!(missing["non_decreasing"].is_null());
    assert_eq!(missing["decreases"], json!([{"from_index":2,"to_index":3}]));
    assert!(missing["steps"][0]["difference"].is_null());
    for args in [
        json!({"action":"sequence_changes","values":[1]}),
        json!({"action":"sequence_changes","values":vec![0;513]}),
        json!({"action":"sequence_changes","values":[1,"missing"]}),
        json!({"action":"sequence_changes","values":[1,2],"text":"extra"}),
    ] {
        assert!(!run(args).await.success);
    }
}

#[tokio::test]
async fn projected_schema_and_governed_execution_accept_the_same_action_fields() {
    let root = tempfile::tempdir().unwrap();
    let store = tempfile::tempdir().unwrap();
    let agent = a3s_code_core::Agent::from_config(
        crate::agent::test_launch(Some(store.path().to_path_buf())).config,
    )
    .await
    .unwrap();
    let session = agent
        .session_async(
            root.path().to_string_lossy().into_owned(),
            Some(a3s_code_core::SessionOptions::new().with_permission_policy(
                a3s_code_core::permissions::PermissionPolicy::new().allow("output_check"),
            )),
        )
        .await
        .unwrap();
    session
        .register_dynamic_tool(std::sync::Arc::new(OutputCheckTool))
        .unwrap();
    for args in [
        json!({"action":"text_length","text":"核对所有来源。"}),
        json!({"action":"percentage_change","pairs":[{"previous":10,"current":8}]}),
        json!({"action":"date_span","start":"2026-01-01","end":"2026-01-03","endpoint_counting":"inclusive"}),
        json!({"action":"sequence_changes","values":[10,8,9]}),
        json!({"action":"record_provenance","key":"id","source":[{"id":"A","value":1}],"output":[{"id":"A","value":1},{"id":"TOTAL","value":1}],"compare_fields":["value"],"allow_derived_keys":["TOTAL"]}),
    ] {
        let result = session.governed_tool("output_check", args).await.unwrap();
        assert_eq!(result.exit_code, 0, "{}", result.output);
    }
    for args in [
        json!({"action":"text_length","text":"abc","pairs":[]}),
        json!({"action":"percentage_change","pairs":[],"text":"extra"}),
        json!({"action":"date_span","start":"2026-01-01","end":"2026-01-03","endpoint_counting":"inclusive","values":[1,2]}),
        json!({"action":"sequence_changes","values":[1,2],"start":"2026-01-01"}),
    ] {
        let result = session.governed_tool("output_check", args).await.unwrap();
        assert_ne!(result.exit_code, 0);
        assert!(
            result
                .output
                .contains("Invalid arguments for tool 'output_check'"),
            "{}",
            result.output
        );
    }
    session.close().await;
}
