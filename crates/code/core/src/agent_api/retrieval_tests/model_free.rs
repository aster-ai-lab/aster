#![cfg(test)]

use super::*;

#[cfg(feature = "a3s-vec-fts")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn model_free_workspace_index_preparation_keeps_late_attached_projection_current() {
    let _permit = crate::test_support::resource_intensive_test_permit().await;
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source.rs");
    std::fs::write(&source, "pub fn initial_workspace_marker() {}\n").unwrap();
    std::fs::write(root.path().join(".env"), "SECRET=xqzconfidentialsentinel").unwrap();
    let agent = Agent::from_config(super::super::tests::test_config())
        .await
        .unwrap();
    let session = agent
        .session_async(root.path().to_string_lossy(), None)
        .await
        .unwrap();
    let services = &session.tool_context.workspace_services;
    let catalog = services.chunk_catalog().unwrap();
    tokio::time::timeout(Duration::from_secs(15), async {
        while catalog.snapshot().unwrap().source_revision() == 0 {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert!(!root.path().join(".a3s-code/index/CURRENT").exists());
    assert!(session.prepare_workspace_index().await.unwrap());
    let index = services.persistent_index().unwrap();
    for marker in ["initial_workspace_marker", "updated_workspace_marker"] {
        std::fs::write(&source, format!("pub fn {marker}() {{}}\n")).unwrap();
        tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                if index
                    .search(&crate::workspace::LexicalSearchRequest::new(marker))
                    .is_ok_and(|result| {
                        result
                            .hits
                            .iter()
                            .any(|hit| hit.chunk.text.contains(marker))
                    })
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("late-attached a3s-vec must follow the original manifest updates");
        tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                let found = session
                    .tool(
                        "search",
                        serde_json::json!({
                            "mode":"hybrid","query":marker,"limit":1
                        }),
                    )
                    .await
                    .unwrap();
                assert_eq!(found.exit_code, 0, "{}", found.output);
                if found
                    .metadata
                    .as_ref()
                    .is_some_and(|metadata| metadata["index_kind"] == "persistent_a3s_vec_fts")
                {
                    assert!(found.output.contains(marker));
                    break;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("hybrid must reuse the already-published a3s-vec generation");
    }
    assert!(root.path().join(".a3s-code/index/CURRENT").is_file());
    assert!(index
        .search(&crate::workspace::LexicalSearchRequest::new(
            "xqzconfidentialsentinel"
        ))
        .unwrap()
        .hits
        .is_empty());
    session.close().await;
    assert!(matches!(
        session.prepare_workspace_index().await,
        Err(crate::CodeError::SessionClosed { .. })
    ));
}

#[tokio::test]
async fn model_free_session_returns_late_evidence_and_refuses_calls_after_close() {
    let _permit = crate::test_support::resource_intensive_test_permit().await;
    let workspace = tempfile::tempdir().unwrap();
    let mut lines = vec!["// unrelated source"; 79];
    lines[63] = "pub fn release_workspace_vectors() { /* verified-late-evidence */ }";
    std::fs::write(workspace.path().join("cache.rs"), lines.join("\n")).unwrap();
    std::fs::write(
        workspace.path().join(".env"),
        "SECRET=release_workspace_vectors",
    )
    .unwrap();
    let agent = Agent::from_config(super::super::tests::test_config())
        .await
        .unwrap();
    let session = agent
        .session_async(workspace.path().to_string_lossy(), None)
        .await
        .unwrap();
    let result = session
        .tool(
            "search",
            serde_json::json!({
                "mode": "hybrid", "query": "release_workspace_vectors", "limit": 1,
            }),
        )
        .await
        .unwrap();
    assert_eq!(result.exit_code, 0, "{}", result.output);
    assert!(
        result
            .output
            .contains("64 | pub fn release_workspace_vectors()"),
        "{}",
        result.output
    );
    assert!(result.output.contains("verified-late-evidence"));
    assert!(!result.output.contains("SECRET="));
    assert!(result.output.len() <= 24 * 1024);
    let metadata = result.metadata.unwrap();
    assert_eq!(metadata["results"][0]["start_line"], 61);
    assert_eq!(metadata["results"][0]["end_line"], 76);
    assert_eq!(metadata["results"][0]["chunk_start_line"], 1);
    assert_eq!(metadata["results"][0]["chunk_end_line"], 79);
    assert_eq!(metadata["results"][0]["digest_verified"], true);
    assert_eq!(metadata["semantic_status"]["phase"], "disabled");
    assert_eq!(session.workspace_retrieval_status().vector_records, 0);
    let secret = session
        .hybrid_search(
            crate::WorkspaceHybridSearchRequest::new("release_workspace_vectors").with_path(".env"),
        )
        .await
        .unwrap();
    assert!(secret.hits.is_empty());
    session.close().await;
    assert!(matches!(
        session
            .hybrid_search(crate::WorkspaceHybridSearchRequest::new(
                "release_workspace_vectors"
            ))
            .await,
        Err(crate::WorkspaceRetrievalError::Unavailable)
    ));
}

#[tokio::test]
async fn model_free_hybrid_obeys_per_mode_permissions_before_source_admission() {
    let _permit = crate::test_support::resource_intensive_test_permit().await;
    let workspace = tempfile::tempdir().unwrap();
    std::fs::write(
        workspace.path().join("source.rs"),
        "private_hybrid_evidence",
    )
    .unwrap();
    let agent = Agent::from_config(super::super::tests::test_config())
        .await
        .unwrap();
    let policy = crate::permissions::PermissionPolicy::new()
        .allow("Grep(*)")
        .deny("search(hybrid **)");
    let options = SessionOptions::new().with_permission_policy(policy);
    let session = agent
        .session_async(workspace.path().to_string_lossy(), Some(options))
        .await
        .unwrap();
    let result = session
        .governed_tool(
            "search",
            serde_json::json!({
                "mode": "hybrid", "query": "private_hybrid_evidence",
            }),
        )
        .await
        .unwrap();
    assert_ne!(result.exit_code, 0);
    assert!(
        !result.output.contains("private_hybrid_evidence"),
        "{}",
        result.output
    );
    assert!(!session
        .tool_context
        .workspace_services
        .chunk_catalog_started());
    session.close().await;
}

#[tokio::test]
async fn model_free_hybrid_output_budget_keeps_metadata_aligned_with_displayed_results() {
    let _permit = crate::test_support::resource_intensive_test_permit().await;
    let workspace = tempfile::tempdir().unwrap();
    let line = format!("budget_probe {}\n", "x".repeat(600));
    for index in 0..8 {
        std::fs::write(
            workspace.path().join(format!("source-{index}.rs")),
            line.repeat(20),
        )
        .unwrap();
    }
    let agent = Agent::from_config(super::super::tests::test_config())
        .await
        .unwrap();
    let session = agent
        .session_async(workspace.path().to_string_lossy(), None)
        .await
        .unwrap();
    let result = session
        .tool(
            "search",
            serde_json::json!({"mode":"hybrid", "query":"budget_probe", "limit":25}),
        )
        .await
        .unwrap();
    assert_eq!(result.exit_code, 0, "{}", result.output);
    assert!(result.output.len() <= 24 * 1024);
    let metadata = result.metadata.unwrap();
    assert_eq!(metadata["output_budget_reached"], true);
    assert_eq!(metadata["truncated"], true);
    let hits = metadata["results"].as_array().unwrap();
    assert!(!hits.is_empty() && hits.len() < 8);
    assert_eq!(
        metadata["returned_results"].as_u64().unwrap() as usize,
        hits.len()
    );
    assert_eq!(
        metadata["source_anchors"].as_array().unwrap().len(),
        hits.len()
    );
    assert!(result
        .output
        .contains(&format!("{} current-source-verified result(s)", hits.len())));
    session.close().await;
}
